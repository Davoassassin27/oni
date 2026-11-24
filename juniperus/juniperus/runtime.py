"""Backends de inferencia y fallback determinístico.

* ``llama``: GGUF con llama-cpp-python (el camino de producción).
* ``transformers``: base + adapter LoRA (evaluación y desarrollo).
* ``mock``: reglas determinísticas, sin modelo. Es el fallback que Oni usa si
  el sidecar no está disponible.
* ``oracle``: sólo evaluación; devuelve el label dorado para validar el
  harness.
"""

from __future__ import annotations

import json
import re
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

from schema.intent_schema import (
    AgentTarget,
    Intent,
    IntentParseError,
    LoopStatus,
    RouterContext,
    RouterIntent,
    WorktreeStrategy,
    parse_decision,
)

from .prompts import build_messages

TASK_STATES_DONE = {"merged", "discarded", "failed", "stopped"}


class RouteBackendError(RuntimeError):
    """El backend no pudo ejecutarse (no es un JSON inválido del modelo)."""


@dataclass
class RouteMeta:
    backend: str
    model: str = ""
    latency_ms: float = 0.0
    retries: int = 0
    degraded: bool = False
    error: str | None = None
    prompt_tokens: int | None = None
    completion_tokens: int | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "backend": self.backend,
            "model": self.model,
            "latency_ms": round(self.latency_ms, 2),
            "retries": self.retries,
            "degraded": self.degraded,
            "error": self.error,
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
        }


@dataclass
class RouteResult:
    decision: RouterIntent
    meta: RouteMeta
    raw: str | None = None


class Backend(Protocol):
    name: str
    model: str

    def route(
        self,
        text: str,
        context: RouterContext | dict[str, Any] | None = None,
        gold: RouterIntent | dict[str, Any] | None = None,
    ) -> RouteResult: ...


def _ctx(context: RouterContext | dict[str, Any] | None) -> RouterContext:
    if isinstance(context, RouterContext):
        return context
    return RouterContext.model_validate(context or {})


def _project_for(ctx: RouterContext, text: str, selected: str | None = None) -> Any:
    lowered = text.lower()
    for project in ctx.projects:
        if re.search(rf"\b{re.escape(project.name.lower())}\b", lowered):
            return project
    return ctx.short_project(selected)


def _task_for(ctx: RouterContext, text: str) -> Any:
    lowered = text.lower()
    for task in ctx.tasks:
        if task.id.lower() in lowered:
            return task
        title = task.title.lower().strip('"')
        if title and title in lowered:
            return task
    for task in ctx.tasks:
        if task.state not in TASK_STATES_DONE and task.id.lower()[:8] in lowered:
            return task
    return ctx.short_task(ctx.selected_task_id)


def _mentions_main(text: str) -> bool:
    return bool(re.search(r"\b(main|master|rama principal|base branch)\b", text, re.I))


class MockBackend:
    """Router determinístico. Sin descargas, sin GPU, sin red.

    No pretende ser tan bueno como el SLM: es el fallback duro de Oni y el
    baseline del bench.
    """

    name = "mock"
    model = "rules-v1"

    def route(
        self,
        text: str,
        context: RouterContext | dict[str, Any] | None = None,
        gold: RouterIntent | dict[str, Any] | None = None,
    ) -> RouteResult:
        started = time.perf_counter()
        ctx = _ctx(context)
        lowered = text.lower().strip()
        task = _task_for(ctx, lowered)
        project = _project_for(ctx, lowered)
        selected_project = project or ctx.short_project(ctx.selected_project_id)

        def decision(**kwargs: Any) -> RouterIntent:
            return RouterIntent.model_validate(kwargs)

        def task_action(intent: Intent, loop: LoopStatus, iterations: int = 1) -> RouterIntent:
            ref = None
            if task is not None:
                ref = task.title if task.title.lower() in lowered else task.id[:8]
            return decision(
                intent=intent,
                project_ref=task.project if task is not None else None,
                task_ref=ref,
                loop_status=loop if ref else LoopStatus.NEEDS_HUMAN,
                max_iterations=iterations,
                reasoning=(
                    {
                        Intent.MERGE_WORKTREE: "Se pidió integrar una tarea.",
                        Intent.RETRY_TASK: "Se pidió reintentar una tarea.",
                        Intent.CANCEL_TASK: "Se pidió detener una tarea.",
                        Intent.DISCARD_TASK: "Se pidió descartar una tarea.",
                    }[intent]
                    if ref
                    else "Falta identificar la tarea."
                ),
            )

        spawn_pattern = (
            r"\b(add|agreg[aá]|cre[aá]|implement[aá]|build|arregl[aá]|fix|refactor|"
            r"actualiz[aá]|update|migr[aá]|document[aá]|escrib[ií]|write|test|hac[eé]|"
            r"bumpe[aá]|optimiz[aá]|limpi[aá]|clean|separ[aá]|split|proteg[eé]|protect|"
            r"valid[aá]|validate|revis[aá]|elimin[aá]|remove|renombr[aá]|rename|"
            r"correg[ií]|correct|dockeriz[aá]|dockerize|export[aá]?|spike|diff)\b"
        )

        if re.search(r"\b(merge|mergea|mergeá|integr[aá]|fusion[aá]|ship)\b", lowered):
            result = task_action(Intent.MERGE_WORKTREE, LoopStatus.COMPLETED)
        elif re.search(
            r"\b(retry|reintent[aá]|otra oportunidad|volv[eé] a correr|run .* again)\b", lowered
        ):
            result = task_action(Intent.RETRY_TASK, LoopStatus.CONTINUE_LOOP, 3)
        elif re.search(
            r"\b(cancel[aá]?|stop|fren[aá]|deten[eé]|mat[aá]|kill|pará|para la tarea)\b",
            lowered,
        ):
            result = task_action(Intent.CANCEL_TASK, LoopStatus.COMPLETED)
        elif re.search(
            r"\b(discard|descart[aá]|tir[aá] la tarea|borr[aá] la tarea|delete|throw away)\b",
            lowered,
        ):
            result = task_action(Intent.DISCARD_TASK, LoopStatus.COMPLETED)
        elif re.search(
            r"\b(help|ayuda|qu[eé] pod[eé]s|para qu[eé] serv[ií]s|what can you do|c[oó]mo funciona)\b",
            lowered,
        ):
            result = decision(
                intent=Intent.HELP,
                loop_status=LoopStatus.COMPLETED,
                max_iterations=1,
                reasoning="El usuario pide ayuda.",
            )
        elif re.search(r"\b(projects|proyectos|repos|repositorios)\b", lowered):
            result = decision(
                intent=Intent.LIST_PROJECTS,
                loop_status=LoopStatus.COMPLETED,
                max_iterations=1,
                reasoning="Consulta de proyectos.",
            )
        elif re.search(spawn_pattern, lowered):
            if selected_project is None:
                result = decision(
                    intent=Intent.SPAWN_AGENT,
                    task_prompt=text,
                    loop_status=LoopStatus.NEEDS_HUMAN,
                    max_iterations=3,
                    reasoning="Falta elegir el proyecto; no se inventa uno.",
                )
            else:
                result = decision(
                    intent=Intent.SPAWN_AGENT,
                    agent_target=selected_project.agent
                    if selected_project.agent in {"opencode", "claude", "custom"}
                    else AgentTarget.AUTO,
                    worktree_strategy=WorktreeStrategy.ISOLATED_BRANCH,
                    branch_name=_branch_for(text),
                    project_ref=selected_project.name,
                    task_prompt=text,
                    loop_status=LoopStatus.NEEDS_HUMAN
                    if _mentions_main(text)
                    else LoopStatus.SINGLE_SHOT,
                    max_iterations=3,
                    success_criterion=selected_project.verify_command or None,
                    reasoning=(
                        "La rama base está prohibida; se usará un worktree aislado."
                        if _mentions_main(text)
                        else "Pedido de implementación delegado a un agente."
                    ),
                )
        elif re.search(
            r"\b(status|estado|c[oó]mo va|que pas[oó]|qu[eé] pas[oó]|log|review|tareas|"
            r"tasks|stuck|trabad|lista|listo|ready|terminada|done)\b",
            lowered,
        ):
            task_ref = None
            if task is not None:
                task_ref = task.title if task.title.lower() in lowered else task.id[:8]
            result = decision(
                intent=Intent.QUERY_STATUS,
                project_ref=task.project if task is not None else None,
                task_ref=task_ref,
                loop_status=LoopStatus.COMPLETED,
                max_iterations=1,
                reasoning="Consulta de estado.",
            )
        else:
            result = decision(
                intent=Intent.UNKNOWN,
                loop_status=LoopStatus.NEEDS_HUMAN,
                max_iterations=1,
                reasoning="No se pudo interpretar el pedido.",
            )

        return RouteResult(
            decision=result,
            meta=RouteMeta(
                backend=self.name,
                model=self.model,
                latency_ms=(time.perf_counter() - started) * 1000,
            ),
        )


class OracleBackend:
    """Devuelve el label dorado. Sólo para validar el bench."""

    name = "oracle"
    model = "gold"

    def route(
        self,
        text: str,
        context: RouterContext | dict[str, Any] | None = None,
        gold: RouterIntent | dict[str, Any] | None = None,
    ) -> RouteResult:
        if gold is None:
            raise RouteBackendError("oracle backend needs a gold label")
        decision = gold if isinstance(gold, RouterIntent) else RouterIntent.model_validate(gold)
        return RouteResult(
            decision=decision,
            meta=RouteMeta(backend=self.name, model=self.model),
            raw=decision.to_json(),
        )


class LlamaBackend:
    name = "llama"
    model = ""

    def __init__(
        self,
        model_path: str | Path,
        *,
        n_ctx: int = 2048,
        n_threads: int | None = None,
        n_gpu_layers: int = 0,
        verbose: bool = False,
    ) -> None:
        path = Path(model_path)
        if not path.exists():
            raise RouteBackendError(f"GGUF model not found: {path}")
        try:
            from llama_cpp import Llama
        except ImportError as error:
            raise RouteBackendError(
                "llama-cpp-python is not installed; run `uv sync --extra llm`"
            ) from error

        kwargs: dict[str, Any] = {
            "model_path": str(path),
            "n_ctx": n_ctx,
            "n_gpu_layers": n_gpu_layers,
            "verbose": verbose,
        }
        if n_threads:
            kwargs["n_threads"] = n_threads
        self._llm = Llama(**kwargs)
        self.model = str(path)

    def route(
        self,
        text: str,
        context: RouterContext | dict[str, Any] | None = None,
        gold: RouterIntent | dict[str, Any] | None = None,
    ) -> RouteResult:
        started = time.perf_counter()
        messages = build_messages(text, context)
        raw = ""
        error: str | None = None
        tokens: dict[str, Any] = {}
        for attempt in range(2):
            request: dict[str, Any] = {
                "messages": messages,
                "temperature": 0.0,
                "max_tokens": 256,
                "stop": ["\n\nUSER", "\n\nPEDIDO"],
            }
            try:
                response = self._llm.create_chat_completion(
                    **request, response_format={"type": "json_object"}
                )
            except TypeError:
                response = self._llm.create_chat_completion(**request)
            raw = response["choices"][0]["message"]["content"] or ""
            tokens = response.get("usage") or {}
            try:
                decision = parse_decision(raw)
                return RouteResult(
                    decision=decision,
                    meta=RouteMeta(
                        backend=self.name,
                        model=self.model,
                        latency_ms=(time.perf_counter() - started) * 1000,
                        retries=attempt,
                        prompt_tokens=tokens.get("prompt_tokens"),
                        completion_tokens=tokens.get("completion_tokens"),
                    ),
                    raw=raw,
                )
            except IntentParseError as parse_error:
                error = str(parse_error)
                messages = [
                    *messages,
                    {"role": "assistant", "content": raw[:1200]},
                    {
                        "role": "user",
                        "content": "Tu salida anterior no era un objeto JSON válido. "
                        "Respondé SOLO el objeto JSON del contrato, sin markdown.",
                    },
                ]

        return RouteResult(
            decision=RouterIntent(
                intent=Intent.UNKNOWN,
                loop_status=LoopStatus.NEEDS_HUMAN,
                max_iterations=1,
                reasoning="El modelo no produjo una decisión válida.",
            ),
            meta=RouteMeta(
                backend=self.name,
                model=self.model,
                latency_ms=(time.perf_counter() - started) * 1000,
                retries=1,
                degraded=True,
                error=error,
            ),
            raw=raw,
        )


class TransformersBackend:
    name = "transformers"
    model = ""

    def __init__(
        self,
        base_model: str,
        *,
        adapter: str | Path | None = None,
        device: str | None = None,
        max_new_tokens: int = 256,
    ) -> None:
        try:
            import torch
            from transformers import AutoModelForCausalLM, AutoTokenizer
        except ImportError as error:
            raise RouteBackendError("torch/transformers are not installed") from error

        self._torch = torch
        self._max_new_tokens = max_new_tokens
        self._tokenizer = AutoTokenizer.from_pretrained(base_model, trust_remote_code=True)
        dtype = torch.bfloat16 if torch.cuda.is_available() else torch.float32
        try:
            self._model = AutoModelForCausalLM.from_pretrained(
                base_model, dtype=dtype, trust_remote_code=True, low_cpu_mem_usage=True
            )
        except TypeError:
            self._model = AutoModelForCausalLM.from_pretrained(
                base_model, torch_dtype=dtype, trust_remote_code=True, low_cpu_mem_usage=True
            )

        if adapter:
            adapter_path = Path(adapter)
            if not adapter_path.exists():
                raise RouteBackendError(f"adapter not found: {adapter_path}")
            try:
                from peft import PeftModel
            except ImportError as error:
                raise RouteBackendError("peft is not installed") from error
            self._model = PeftModel.from_pretrained(self._model, str(adapter_path))

        self._device = device or ("cuda" if torch.cuda.is_available() else "cpu")
        self._model.to(self._device)
        self._model.eval()
        self.model = f"{base_model}+{adapter}" if adapter else base_model

    def route(
        self,
        text: str,
        context: RouterContext | dict[str, Any] | None = None,
        gold: RouterIntent | dict[str, Any] | None = None,
    ) -> RouteResult:
        torch = self._torch
        started = time.perf_counter()
        messages = build_messages(text, context)
        prompt = self._tokenizer.apply_chat_template(
            messages, tokenize=False, add_generation_prompt=True
        )
        inputs = self._tokenizer(prompt, return_tensors="pt").to(self._device)
        with torch.no_grad():
            output = self._model.generate(
                **inputs,
                max_new_tokens=self._max_new_tokens,
                do_sample=False,
                pad_token_id=self._tokenizer.eos_token_id,
            )
        generated = self._tokenizer.decode(
            output[0, inputs["input_ids"].shape[1] :], skip_special_tokens=True
        )
        try:
            decision = parse_decision(generated)
            error = None
            degraded = False
        except IntentParseError as parse_error:
            decision = RouterIntent(
                intent=Intent.UNKNOWN,
                loop_status=LoopStatus.NEEDS_HUMAN,
                max_iterations=1,
                reasoning="El modelo no produjo una decisión válida.",
            )
            error = str(parse_error)
            degraded = True
        return RouteResult(
            decision=decision,
            meta=RouteMeta(
                backend=self.name,
                model=self.model,
                latency_ms=(time.perf_counter() - started) * 1000,
                degraded=degraded,
                error=error,
            ),
            raw=generated,
        )


def _branch_for(prompt: str, max_len: int = 60) -> str:
    ascii_text = re.sub(r"[^a-z0-9]+", "-", prompt.lower()).strip("-")
    return f"oni/{ascii_text[:max_len].strip('-') or 'task'}"


def build_backend(
    kind: str = "auto",
    *,
    model_path: str | Path | None = None,
    base_model: str = "Qwen/Qwen2.5-0.5B-Instruct",
    adapter: str | Path | None = None,
    n_ctx: int = 2048,
    n_threads: int | None = None,
    n_gpu_layers: int = 0,
) -> Backend:
    kind = kind.lower()
    if kind == "mock":
        return MockBackend()
    if kind == "oracle":
        return OracleBackend()
    if kind == "llama":
        if not model_path:
            raise RouteBackendError("--model-path is required for the llama backend")
        return LlamaBackend(
            model_path,
            n_ctx=n_ctx,
            n_threads=n_threads,
            n_gpu_layers=n_gpu_layers,
        )
    if kind == "transformers":
        return TransformersBackend(base_model, adapter=adapter)
    if kind == "auto":
        path = Path(model_path) if model_path else None
        if path and path.suffix.lower() == ".gguf" and path.exists():
            return LlamaBackend(path, n_ctx=n_ctx, n_threads=n_threads, n_gpu_layers=n_gpu_layers)
        if adapter and Path(adapter).exists():
            return TransformersBackend(base_model, adapter=adapter)
        if path and path.exists() and path.is_dir():
            return TransformersBackend(str(path))
        return MockBackend()
    raise RouteBackendError(f"unknown backend: {kind}")


def route_text(
    backend: Backend,
    text: str,
    context: RouterContext | dict[str, Any] | None = None,
    gold: RouterIntent | dict[str, Any] | None = None,
) -> RouteResult:
    text = text.strip()
    if not text:
        raise ValueError("text cannot be empty")
    return backend.route(text, context, gold)


def result_payload(result: RouteResult) -> dict[str, Any]:
    return {
        "decision": result.decision.model_dump(mode="json"),
        "meta": result.meta.to_dict(),
        "raw": result.raw,
    }


def json_result(result: RouteResult) -> str:
    return json.dumps(result_payload(result), ensure_ascii=False, indent=2)
