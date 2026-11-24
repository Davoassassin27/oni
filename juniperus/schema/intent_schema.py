"""Juniperus Router Contract v1.

Este módulo es la única fuente de verdad del contrato: lo usan la generación
del dataset, el entrenamiento, la evaluación, el sidecar y el daemon de Oni
(que replica las mismas reglas de seguridad en Rust).

Reglas duras:

* La salida es un único objeto JSON con las claves de ``RouterIntent``.
* ``worktree_strategy`` nunca puede ser ``main_direct``. Si el modelo lo
  propone, ``sanitize_payload`` lo degrada a ``isolated_branch``.
* ``branch_name`` siempre empieza con ``oni/``.
* ``max_iterations`` está acotado a 1..6 (el hard cap de Oni).
* Las acciones sobre una tarea existente necesitan ``task_ref``. Si falta,
  la decisión queda en ``needs_human``.
"""

from __future__ import annotations

import json
import re
import unicodedata
from collections.abc import Iterable
from enum import Enum, StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator, model_validator

CONTRACT_VERSION = "1"

SYSTEM_PROMPT = """\
Eres Juniperus, el router local de Oni, un Agentic Development Environment.
Tu trabajo es convertir un pedido en lenguaje natural (español, inglés o mixto)
en UNA decisión JSON para que Oni sepa qué hacer.

Respondé SOLO un objeto JSON válido, sin markdown ni texto extra.

Contrato (usá exactamente estas claves y valores):
{
  "intent": "spawn_agent | query_status | merge_worktree | retry_task | cancel_task | discard_task | list_projects | help | unknown",
  "agent_target": "opencode | claude | custom | auto",
  "model_override": "string | null",
  "worktree_strategy": "isolated_branch | existing_branch",
  "branch_name": "oni/slug | null",
  "project_ref": "string | null",
  "task_ref": "string | null",
  "task_prompt": "string | null",
  "loop_status": "single_shot | continue_loop | completed | needs_human",
  "max_iterations": 1,
  "success_criterion": "string | null",
  "reasoning": "string"
}

Reglas:
- NUNCA uses main_direct ni propongas trabajar sobre la rama base. Todo
  trabajo va a un worktree aislado con una rama oni/.
- merge_worktree, retry_task, cancel_task y discard_task necesitan task_ref
  (id corto, id completo o título de una tarea del CONTEXT).
- No inventes proyectos ni tareas: usá solo los del CONTEXT.
- Si el pedido es ambiguo o falta un dato imprescindible, usá
  "loop_status": "needs_human" y explicá qué falta en reasoning.
- task_prompt conserva el pedido original del usuario, sin reescribirlo.
- success_criterion es el comando de verificación del proyecto cuando exista.
- reasoning: una sola frase corta, en el idioma del usuario.
- Si no entendés el pedido, usá "intent": "unknown".
"""


class Intent(StrEnum):
    SPAWN_AGENT = "spawn_agent"
    QUERY_STATUS = "query_status"
    MERGE_WORKTREE = "merge_worktree"
    RETRY_TASK = "retry_task"
    CANCEL_TASK = "cancel_task"
    DISCARD_TASK = "discard_task"
    LIST_PROJECTS = "list_projects"
    HELP = "help"
    UNKNOWN = "unknown"


class AgentTarget(StrEnum):
    OPENCODE = "opencode"
    CLAUDE = "claude"
    CUSTOM = "custom"
    AUTO = "auto"


class WorktreeStrategy(StrEnum):
    ISOLATED_BRANCH = "isolated_branch"
    EXISTING_BRANCH = "existing_branch"


class LoopStatus(StrEnum):
    SINGLE_SHOT = "single_shot"
    CONTINUE_LOOP = "continue_loop"
    COMPLETED = "completed"
    NEEDS_HUMAN = "needs_human"


TASK_ACTIONS = {
    Intent.MERGE_WORKTREE,
    Intent.RETRY_TASK,
    Intent.CANCEL_TASK,
    Intent.DISCARD_TASK,
}

DEFAULT_ITERATIONS = {
    Intent.SPAWN_AGENT: 3,
    Intent.RETRY_TASK: 3,
}


class RouterIntent(BaseModel):
    """La decisión que devuelve Juniperus. ``extra=forbid`` atrapa drift."""

    model_config = ConfigDict(extra="forbid", str_strip_whitespace=True)

    intent: Intent
    agent_target: AgentTarget = AgentTarget.AUTO
    model_override: str | None = None
    worktree_strategy: WorktreeStrategy = WorktreeStrategy.ISOLATED_BRANCH
    branch_name: str | None = None
    project_ref: str | None = None
    task_ref: str | None = None
    task_prompt: str | None = None
    loop_status: LoopStatus = LoopStatus.SINGLE_SHOT
    max_iterations: int = Field(default=3, ge=1, le=6)
    success_criterion: str | None = None
    reasoning: str = Field(default="", max_length=400)

    @field_validator("branch_name")
    @classmethod
    def _branch_is_oni(cls, value: str | None) -> str | None:
        if value is None:
            return None
        value = value.strip()
        if not value:
            return None
        if not value.startswith("oni/"):
            raise ValueError("branch_name must start with 'oni/'")
        if not re.fullmatch(r"oni/[a-z0-9][a-z0-9._/-]{0,79}", value):
            raise ValueError("branch_name must be a lowercase oni/ slug")
        return value

    @field_validator(
        "model_override", "project_ref", "task_ref", "task_prompt", "success_criterion"
    )
    @classmethod
    def _blank_to_none(cls, value: str | None) -> str | None:
        if value is None:
            return None
        value = value.strip()
        return value or None

    @field_validator("task_prompt")
    @classmethod
    def _prompt_limit(cls, value: str | None) -> str | None:
        if value is not None and len(value) > 4000:
            raise ValueError("task_prompt is too long (max 4000)")
        return value

    @model_validator(mode="after")
    def _coherent(self) -> RouterIntent:
        if self.worktree_strategy is WorktreeStrategy.EXISTING_BRANCH and not self.branch_name:
            raise ValueError("existing_branch requires branch_name")
        if self.intent is Intent.SPAWN_AGENT and not self.task_prompt:
            raise ValueError("spawn_agent requires task_prompt")
        if self.intent in TASK_ACTIONS and not self.task_ref:
            self.loop_status = LoopStatus.NEEDS_HUMAN
        if self.intent is Intent.UNKNOWN:
            self.loop_status = LoopStatus.NEEDS_HUMAN
        return self

    def to_json(self) -> str:
        return json.dumps(self.model_dump(mode="json"), ensure_ascii=False, separators=(",", ":"))


class ProjectContext(BaseModel):
    model_config = ConfigDict(extra="ignore")

    id: str
    name: str
    base_branch: str = "main"
    verify_command: str = ""
    agent: str = "opencode"


class TaskContext(BaseModel):
    model_config = ConfigDict(extra="ignore")

    id: str
    title: str
    state: str = "queued"
    project: str = ""
    branch: str = ""


class RouterContext(BaseModel):
    model_config = ConfigDict(extra="ignore")

    projects: list[ProjectContext] = Field(default_factory=list)
    tasks: list[TaskContext] = Field(default_factory=list)
    selected_project_id: str | None = None
    selected_task_id: str | None = None

    def short_project(self, project_id: str | None) -> ProjectContext | None:
        if not project_id:
            return None
        for project in self.projects:
            if project.id == project_id or project.id.startswith(project_id):
                return project
        return None

    def short_task(self, task_id: str | None) -> TaskContext | None:
        if not task_id:
            return None
        for task in self.tasks:
            if task.id == task_id or task.id.startswith(task_id):
                return task
        return None


class IntentParseError(ValueError):
    """El modelo no devolvió un objeto JSON utilizable."""


# ---------------------------------------------------------------------------
# Normalización defensiva: el modelo puede alucinar sinónimos, nulls o
# main_direct. Nada de eso debe llegar crudo al daemon.
# ---------------------------------------------------------------------------

_INTENT_ALIASES = {
    "spawn": Intent.SPAWN_AGENT,
    "create_task": Intent.SPAWN_AGENT,
    "new_task": Intent.SPAWN_AGENT,
    "dispatch": Intent.SPAWN_AGENT,
    "start_agent": Intent.SPAWN_AGENT,
    "run_agent": Intent.SPAWN_AGENT,
    "status": Intent.QUERY_STATUS,
    "query": Intent.QUERY_STATUS,
    "check_status": Intent.QUERY_STATUS,
    "list_tasks": Intent.QUERY_STATUS,
    "task_status": Intent.QUERY_STATUS,
    "merge": Intent.MERGE_WORKTREE,
    "merge_task": Intent.MERGE_WORKTREE,
    "retry": Intent.RETRY_TASK,
    "cancel": Intent.CANCEL_TASK,
    "stop": Intent.CANCEL_TASK,
    "stop_task": Intent.CANCEL_TASK,
    "discard": Intent.DISCARD_TASK,
    "delete_task": Intent.DISCARD_TASK,
    "projects": Intent.LIST_PROJECTS,
    "list_repos": Intent.LIST_PROJECTS,
    "capabilities": Intent.HELP,
    "chitchat": Intent.UNKNOWN,
    "other": Intent.UNKNOWN,
    "none": Intent.UNKNOWN,
    "": Intent.UNKNOWN,
}

_AGENT_ALIASES = {
    "claude_code": AgentTarget.CLAUDE,
    "claude-code": AgentTarget.CLAUDE,
    "claudecode": AgentTarget.CLAUDE,
    "opencode_cli": AgentTarget.OPENCODE,
    "opencode-cli": AgentTarget.OPENCODE,
    "aider": AgentTarget.CUSTOM,
    "hermes": AgentTarget.CUSTOM,
    "custom_cli": AgentTarget.CUSTOM,
    "custom-cli": AgentTarget.CUSTOM,
    "default": AgentTarget.AUTO,
    "any": AgentTarget.AUTO,
    "": AgentTarget.AUTO,
}

_STRATEGY_ALIASES = {
    "isolated": WorktreeStrategy.ISOLATED_BRANCH,
    "isolated_worktree": WorktreeStrategy.ISOLATED_BRANCH,
    "worktree": WorktreeStrategy.ISOLATED_BRANCH,
    "existing": WorktreeStrategy.EXISTING_BRANCH,
    "existing_worktree": WorktreeStrategy.EXISTING_BRANCH,
}

_LOOP_ALIASES = {
    "single": LoopStatus.SINGLE_SHOT,
    "one_shot": LoopStatus.SINGLE_SHOT,
    "once": LoopStatus.SINGLE_SHOT,
    "continue": LoopStatus.CONTINUE_LOOP,
    "loop": LoopStatus.CONTINUE_LOOP,
    "done": LoopStatus.COMPLETED,
    "complete": LoopStatus.COMPLETED,
    "finished": LoopStatus.COMPLETED,
    "needs-human": LoopStatus.NEEDS_HUMAN,
    "human": LoopStatus.NEEDS_HUMAN,
    "blocked": LoopStatus.NEEDS_HUMAN,
    "": LoopStatus.SINGLE_SHOT,
}

_NULLISH = {"", "null", "none", "nil", "n/a", "na", "auto", "undefined", "-"}


def _enum_value(value: Any, enum: type[Enum], aliases: dict[str, Any], default: Enum) -> Enum:
    if isinstance(value, enum):
        return value
    text = "" if value is None else str(value).strip().lower()
    text = text.replace(" ", "_")
    if text in aliases:
        return aliases[text]
    try:
        return enum(text)
    except ValueError:
        return default


def _clean_ref(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    value = value.strip().strip("\"'`")
    return None if value.lower() in _NULLISH else value


def _slugify(text: str, max_len: int = 60) -> str:
    normalized = unicodedata.normalize("NFKD", text)
    ascii_text = normalized.encode("ascii", "ignore").decode("ascii").lower()
    slug = re.sub(r"[^a-z0-9]+", "-", ascii_text).strip("-")
    slug = re.sub(r"-{2,}", "-", slug)
    return slug[:max_len].strip("-") or "task"


def _branch(value: Any, task_prompt: str | None) -> str | None:
    raw = _clean_ref(value)
    if raw:
        raw = raw.replace("\\", "/").strip("/")
        if raw.startswith("oni/"):
            return f"oni/{_slugify(raw[4:], 72)}"
        return f"oni/{_slugify(raw, 72)}"
    if task_prompt:
        return f"oni/{_slugify(task_prompt, 60)}"
    return None


def _iterations(value: Any, intent: Intent) -> int:
    try:
        number = int(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        number = DEFAULT_ITERATIONS.get(intent, 3)
    return max(1, min(6, number))


def sanitize_payload(raw: dict[str, Any]) -> dict[str, Any]:
    """Convierte una salida cruda del modelo en un payload seguro y canónico.

    No lanza por campos faltantes: completa defaults. La validación estricta
    ocurre después, en ``RouterIntent.model_validate``.
    """

    if not isinstance(raw, dict):
        raise IntentParseError("the model did not return a JSON object")

    intent = _enum_value(raw.get("intent"), Intent, _INTENT_ALIASES, Intent.UNKNOWN)
    agent = _enum_value(raw.get("agent_target"), AgentTarget, _AGENT_ALIASES, AgentTarget.AUTO)
    strategy = _enum_value(
        raw.get("worktree_strategy"),
        WorktreeStrategy,
        _STRATEGY_ALIASES,
        WorktreeStrategy.ISOLATED_BRANCH,
    )
    loop = _enum_value(raw.get("loop_status"), LoopStatus, _LOOP_ALIASES, LoopStatus.SINGLE_SHOT)

    reasoning = str(raw.get("reasoning") or "").strip()[:400]
    downgraded = False
    if str(raw.get("worktree_strategy", "")).strip().lower() in {"main_direct", "main", "direct"}:
        strategy = WorktreeStrategy.ISOLATED_BRANCH
        loop = LoopStatus.NEEDS_HUMAN
        downgraded = True
        reasoning = (
            (reasoning + " " if reasoning else "")
            + "Trabajar sobre la rama base está prohibido; se propone un worktree aislado."
        )[:400]
    if strategy is WorktreeStrategy.EXISTING_BRANCH and not _clean_ref(raw.get("branch_name")):
        strategy = WorktreeStrategy.ISOLATED_BRANCH

    task_prompt = _clean_ref(raw.get("task_prompt"))
    if intent is not Intent.SPAWN_AGENT:
        task_prompt = None
    if intent is Intent.SPAWN_AGENT and not task_prompt:
        task_prompt = _clean_ref(raw.get("text")) or None
    if task_prompt and len(task_prompt) > 4000:
        task_prompt = task_prompt[:4000].rstrip()

    project_ref = _clean_ref(raw.get("project_ref"))
    task_ref = _clean_ref(raw.get("task_ref"))
    if task_ref and len(task_ref) > 200:
        task_ref = task_ref[:200].rstrip()
    if project_ref and len(project_ref) > 200:
        project_ref = project_ref[:200].rstrip()
    branch_name = (
        _branch(raw.get("branch_name"), task_prompt) if intent is Intent.SPAWN_AGENT else None
    )

    if intent in TASK_ACTIONS and not task_ref:
        loop = LoopStatus.NEEDS_HUMAN
        reasoning = (reasoning + " " if reasoning else "") + "Falta identificar la tarea."[:400]
    if intent is Intent.UNKNOWN:
        loop = LoopStatus.NEEDS_HUMAN
    if loop is LoopStatus.SINGLE_SHOT and intent is not Intent.SPAWN_AGENT:
        loop = LoopStatus.COMPLETED

    model_override = _clean_ref(raw.get("model_override"))
    success = _clean_ref(raw.get("success_criterion"))
    if intent is not Intent.SPAWN_AGENT and not downgraded:
        success = None

    return {
        "intent": intent.value,
        "agent_target": agent.value,
        "model_override": model_override,
        "worktree_strategy": strategy.value,
        "branch_name": branch_name,
        "project_ref": project_ref,
        "task_ref": task_ref,
        "task_prompt": task_prompt,
        "loop_status": loop.value,
        "max_iterations": _iterations(raw.get("max_iterations"), intent),
        "success_criterion": success,
        "reasoning": reasoning or _default_reasoning(intent),
    }


def _default_reasoning(intent: Intent) -> str:
    return {
        Intent.SPAWN_AGENT: "Pedido de implementación delegado a un agente.",
        Intent.QUERY_STATUS: "Consulta de estado.",
        Intent.MERGE_WORKTREE: "Se pidió integrar una tarea.",
        Intent.RETRY_TASK: "Se pidió reintentar una tarea.",
        Intent.CANCEL_TASK: "Se pidió detener una tarea.",
        Intent.DISCARD_TASK: "Se pidió descartar una tarea.",
        Intent.LIST_PROJECTS: "Consulta de proyectos.",
        Intent.HELP: "El usuario pide ayuda.",
        Intent.UNKNOWN: "No se pudo interpretar el pedido.",
    }[intent]


def extract_json_object(text: str) -> dict[str, Any]:
    """Extrae el primer objeto JSON de una salida con fences o texto extra."""

    if not text or not text.strip():
        raise IntentParseError("the model returned an empty response")

    cleaned = text.strip()
    if cleaned.startswith("```"):
        cleaned = re.sub(r"^```[a-zA-Z0-9_-]*\s*", "", cleaned)
        cleaned = re.sub(r"\s*```$", "", cleaned).strip()

    decoder = json.JSONDecoder()
    for match in re.finditer(r"\{", cleaned):
        try:
            value, _ = decoder.raw_decode(cleaned[match.start() :])
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise IntentParseError("no JSON object found in the model output")


def parse_decision(text: str) -> RouterIntent:
    """Parsea, normaliza y valida una salida cruda del modelo."""

    payload = extract_json_object(text)
    try:
        return RouterIntent.model_validate(sanitize_payload(payload))
    except ValidationError as error:
        raise IntentParseError(f"invalid router decision: {error}") from error


def decision_schema() -> dict[str, Any]:
    schema = RouterIntent.model_json_schema()
    schema["$schema"] = "https://json-schema.org/draft/2020-12/schema"
    schema["title"] = "Juniperus Router Contract v1"
    return schema


def validate_dataset_record(label: dict[str, Any]) -> RouterIntent:
    """Valida un label de dataset sin tolerar coerciones silenciosas."""

    return RouterIntent.model_validate(label)


def majority_intents(labels: Iterable[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for label in labels:
        intent = str(label.get("intent", "unknown"))
        counts[intent] = counts.get(intent, 0) + 1
    return dict(sorted(counts.items(), key=lambda item: (-item[1], item[0])))


def main(argv: list[str] | None = None) -> int:
    import argparse
    import sys

    parser = argparse.ArgumentParser(description="Juniperus Router Contract v1")
    parser.add_argument("--write", help="write the JSON Schema to this path")
    parser.add_argument("--prompt", action="store_true", help="print the training system prompt")
    args = parser.parse_args(argv)

    if args.prompt:
        print(SYSTEM_PROMPT)
        return 0

    payload = json.dumps(decision_schema(), indent=2, ensure_ascii=False) + "\n"
    if args.write:
        from pathlib import Path

        Path(args.write).write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
