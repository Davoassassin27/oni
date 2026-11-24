"""Generación, validación y split del dataset sintético de Juniperus.

Modos:

* ``offline``: plantillas + catálogo de escenarios. Determinista, sin red.
* ``teacher``: el profesor (OpenAI-compatible) genera paráfrasis; las
  etiquetas las calculamos nosotros.
* ``hybrid`` / ``auto``: combina ambos.

El profesor nunca produce labels: sólo reescribe el pedido del usuario. Así una
alucinación del profesor no puede contaminar el contrato.
"""

from __future__ import annotations

import hashlib
import json
import os
import random
import re
import sys
import time
from collections.abc import Iterable
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from schema.intent_schema import (
    CONTRACT_VERSION,
    Intent,
    RouterContext,
    RouterIntent,
    decision_schema,
    extract_json_object,
    validate_dataset_record,
)

from .prompts import build_messages, format_context
from .scenarios import PROJECTS, TASKS, sample_example

DEFAULT_COUNT = 1600
DEFAULT_TEACHER_PROTOTYPES = 60
DEFAULT_TEACHER_PER_PROTOTYPE = 6

# Distribución objetivo. El router ve mucho más "hacé X" que "mergeá Y", pero
# sin datos balanceados las acciones de supervisión se aprenden mal.
INTENT_WEIGHTS = {
    "spawn_agent": 0.35,
    "query_status": 0.18,
    "merge_worktree": 0.10,
    "retry_task": 0.09,
    "cancel_task": 0.09,
    "discard_task": 0.08,
    "list_projects": 0.03,
    "help": 0.03,
    "unknown": 0.05,
}


@dataclass
class DatasetArgs:
    mode: str = "auto"
    count: int = DEFAULT_COUNT
    seed: int = 42
    out_dir: Path = Path("data/processed")
    teacher_api_key: str | None = None
    teacher_base_url: str = "https://api.deepseek.com/v1"
    teacher_model: str = "deepseek-chat"
    teacher_prototypes: int = DEFAULT_TEACHER_PROTOTYPES
    teacher_per_prototype: int = DEFAULT_TEACHER_PER_PROTOTYPE
    teacher_concurrency: int = 4
    teacher_temperature: float = 0.8
    allow_offline_fallback: bool = True


def _norm(value: str) -> str:
    return re.sub(r"\s+", " ", value.strip().lower())


def _hash_id(*parts: str) -> str:
    return hashlib.sha1("\x1f".join(parts).encode("utf-8")).hexdigest()[:12]


def make_record(
    text: str,
    context: RouterContext,
    label: RouterIntent,
    meta: dict[str, Any] | None = None,
) -> dict[str, Any]:
    label = RouterIntent.model_validate(label.model_dump(mode="json"))
    context = RouterContext.model_validate(context.model_dump(mode="json"))
    messages = build_messages(text, context) + [{"role": "assistant", "content": label.to_json()}]
    record = {
        "id": _hash_id(text, context.model_dump_json(), label.to_json()),
        "text": text.strip(),
        "context": context.model_dump(mode="json"),
        "label": label.model_dump(mode="json"),
        "messages": messages,
        "meta": {
            "contract": CONTRACT_VERSION,
            "source": "offline",
            **(meta or {}),
        },
    }
    return record


def _key(record: dict[str, Any]) -> tuple[str, str, str, str]:
    label = record["label"]
    return (
        _norm(record["text"]),
        str(label.get("intent")),
        str(label.get("task_ref") or ""),
        str(label.get("project_ref") or ""),
    )


def dedupe(records: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    seen: set[tuple[str, str, str, str]] = set()
    unique: list[dict[str, Any]] = []
    for record in records:
        key = _key(record)
        if key in seen:
            continue
        seen.add(key)
        unique.append(record)
    return unique


def _quotas(count: int) -> dict[str, int]:
    raw = {intent: count * weight for intent, weight in INTENT_WEIGHTS.items()}
    quotas = {intent: int(value) for intent, value in raw.items()}
    remainder = count - sum(quotas.values())
    for intent, _ in sorted(raw.items(), key=lambda item: item[1] - int(item[1]), reverse=True):
        if remainder <= 0:
            break
        quotas[intent] += 1
        remainder -= 1
    return quotas


def _offline_record(
    scenario_id: str, family: str, lang: str, context: Any, label: Any, text: str, seed: int
) -> dict[str, Any]:
    return make_record(
        text,
        context,
        label,
        meta={
            "scenario": scenario_id,
            "family": family,
            "lang": lang,
            "source": "offline",
            "seed": seed,
        },
    )


def generate_offline(count: int = DEFAULT_COUNT, seed: int = 42) -> list[dict[str, Any]]:
    rng = random.Random(seed)
    quotas = _quotas(count)
    counts: dict[str, int] = {intent: 0 for intent in quotas}
    records: list[dict[str, Any]] = []
    seen: set[tuple[str, str, str, str]] = set()
    attempts = 0
    max_attempts = max(20_000, count * 80)

    while len(records) < count and attempts < max_attempts:
        attempts += 1
        scenario_id, family, lang, context, label, text = sample_example(rng)
        intent = label.intent.value
        if counts.get(intent, 0) >= quotas.get(intent, 0):
            continue
        record = _offline_record(scenario_id, family, lang, context, label, text, seed)
        key = _key(record)
        if key in seen:
            continue
        seen.add(key)
        records.append(record)
        counts[intent] += 1

    # Si alguna cuota es inalcanzable por falta de variedad, completamos con
    # cualquier intención en vez de fallar el pipeline entero.
    while len(records) < count and attempts < max_attempts:
        attempts += 1
        scenario_id, family, lang, context, label, text = sample_example(rng)
        record = _offline_record(scenario_id, family, lang, context, label, text, seed)
        key = _key(record)
        if key in seen:
            continue
        seen.add(key)
        records.append(record)
        counts[label.intent.value] = counts.get(label.intent.value, 0) + 1

    if len(records) < count:
        print(
            f"warning: offline generator produced {len(records)}/{count} unique examples "
            "(rare intents saturated); continuing",
            file=sys.stderr,
        )
        if len(records) < count * 0.9:
            raise RuntimeError(
                f"offline generator produced only {len(records)} unique examples "
                f"out of {count} requested"
            )
    return records


# ---------------------------------------------------------------------------
# Profesor
# ---------------------------------------------------------------------------


def _protected_terms(text: str) -> set[str]:
    terms: set[str] = set()
    lowered = _norm(text)
    for project in PROJECTS:
        if re.search(rf"\b{re.escape(project.name)}\b", lowered):
            terms.add(project.name)
    for task in TASKS:
        if task.id.lower() in lowered:
            terms.add(task.id)
        if _norm(task.title) in lowered:
            terms.add(task.title)
    return terms


def _references_intact(original: str, candidate: str, label: dict[str, Any]) -> bool:
    protected = _protected_terms(original)
    task_ref = label.get("task_ref")
    project_ref = label.get("project_ref")
    if task_ref:
        protected.add(str(task_ref))
    if project_ref and _norm(str(project_ref)) in _norm(original):
        protected.add(str(project_ref))

    lowered = _norm(candidate)
    return all(_norm(term) in lowered for term in protected)


def _teacher_prompt(record: dict[str, Any], n: int) -> str:
    context = RouterContext.model_validate(record["context"])
    label = record["label"]
    return (
        "Sos un generador de paráfrasis para Juniperus, el router de Oni.\n"
        "Tu única tarea es reescribir el pedido del usuario sin cambiar su intención.\n\n"
        f"Intención etiquetada: {label['intent']}\n"
        f"Comando original: {record['text']}\n\n"
        f"Contexto disponible:\n{format_context(context)}\n\n"
        f"Devolvé exactamente {n} paráfrasis distintas en un objeto JSON:\n"
        '{"paraphrases": ["...", "..."]}\n\n'
        "Reglas:\n"
        "- Conservá literalmente ids, nombres de proyecto y títulos de tarea.\n"
        "- Variá el registro: español rioplatense, inglés, mixto, dictado de "
        "teléfono, typos leves, urgencia.\n"
        "- No agregues pedidos nuevos ni cambies la intención.\n"
        "- Máximo 280 caracteres por paráfrasis, una sola línea.\n"
        "- Nada de markdown ni explicaciones."
    )


def _paraphrases_from_content(content: str) -> list[str]:
    try:
        parsed = json.loads(content.strip())
    except json.JSONDecodeError:
        parsed = None
    if isinstance(parsed, list):
        return [str(item) for item in parsed if isinstance(item, (str, int, float))]
    payload = parsed if isinstance(parsed, dict) else extract_json_object(content)

    for key in ("paraphrases", "rewrites", "commands", "items", "results"):
        value = payload.get(key)
        if isinstance(value, list):
            return [str(item) for item in value if isinstance(item, (str, int, float))]
    for value in payload.values():
        if isinstance(value, list):
            return [str(item) for item in value if isinstance(item, (str, int, float))]
    return []


def _call_teacher(client: Any, model: str, prompt: str, temperature: float) -> list[str]:
    kwargs = {
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
        "max_tokens": 1200,
    }
    try:
        response = client.chat.completions.create(**kwargs, response_format={"type": "json_object"})
    except Exception:
        response = client.chat.completions.create(**kwargs)
    content = response.choices[0].message.content or ""
    return _paraphrases_from_content(content)


def _teacher_batch(
    client: Any,
    record: dict[str, Any],
    n: int,
    model: str,
    temperature: float,
    retries: int = 3,
) -> list[dict[str, Any]]:
    prompt = _teacher_prompt(record, n)
    last_error: Exception | None = None
    for attempt in range(retries):
        try:
            candidates = _call_teacher(client, model, prompt, temperature)
            break
        except Exception as error:  # network/provider errors are expected
            last_error = error
            time.sleep(1.5 * (attempt + 1))
    else:
        print(f"  ! teacher failed: {last_error}", file=sys.stderr)
        return []

    out: list[dict[str, Any]] = []
    for candidate in candidates:
        candidate = re.sub(r"\s+", " ", candidate).strip().strip("\"'")
        if not (3 <= len(candidate) <= 400):
            continue
        if "```" in candidate or candidate.startswith("{"):
            continue
        if not _references_intact(record["text"], candidate, record["label"]):
            continue
        label = RouterIntent.model_validate(record["label"])
        if label.intent is Intent.SPAWN_AGENT:
            label.task_prompt = candidate
            label.branch_name = None  # se recalcula abajo, sin arrastrar el original
            from .scenarios import branch_for

            label.branch_name = branch_for(candidate)
        meta = {
            **record["meta"],
            "source": "teacher",
            "teacher_model": model,
            "original_id": record["id"],
        }
        try:
            out.append(
                make_record(candidate, RouterContext.model_validate(record["context"]), label, meta)
            )
        except Exception:
            continue
    return out


def _select_prototypes(
    records: list[dict[str, Any]], limit: int, seed: int
) -> list[dict[str, Any]]:
    rng = random.Random(seed)
    by_intent: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        by_intent.setdefault(record["label"]["intent"], []).append(record)

    for group in by_intent.values():
        rng.shuffle(group)

    selected: list[dict[str, Any]] = []
    intents = list(by_intent)
    rng.shuffle(intents)
    index = 0
    while len(selected) < limit and any(by_intent[intent] for intent in intents):
        intent = intents[index % len(intents)]
        index += 1
        if by_intent[intent]:
            selected.append(by_intent[intent].pop())
    return selected


def generate_teacher(
    base_records: list[dict[str, Any]],
    args: DatasetArgs,
    client: Any | None = None,
) -> list[dict[str, Any]]:
    if client is None:
        from openai import OpenAI

        if not args.teacher_api_key:
            raise ValueError("TEACHER_API_KEY is required for teacher mode")
        client = OpenAI(api_key=args.teacher_api_key, base_url=args.teacher_base_url)

    prototypes = _select_prototypes(base_records, args.teacher_prototypes, args.seed)
    if not prototypes:
        return []

    print(
        f"teacher: {len(prototypes)} prototypes x {args.teacher_per_prototype} "
        f"paraphrases with {args.teacher_model}",
        file=sys.stderr,
    )
    generated: list[dict[str, Any]] = []
    with ThreadPoolExecutor(max_workers=max(1, args.teacher_concurrency)) as pool:
        futures = {
            pool.submit(
                _teacher_batch,
                client,
                record,
                args.teacher_per_prototype,
                args.teacher_model,
                args.teacher_temperature,
            ): record
            for record in prototypes
        }
        for done, future in enumerate(as_completed(futures), start=1):
            batch = future.result()
            generated.extend(batch)
            if done % 10 == 0 or done == len(futures):
                print(
                    f"  {done}/{len(futures)} prototypes, {len(generated)} paraphrases",
                    file=sys.stderr,
                )

    if not generated:
        print("teacher produced no valid paraphrases; using offline data only", file=sys.stderr)
    return dedupe(generated)


# ---------------------------------------------------------------------------
# Split y escritura
# ---------------------------------------------------------------------------


def split_records(
    records: list[dict[str, Any]], seed: int = 42, train_ratio: float = 0.8, val_ratio: float = 0.1
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    rng = random.Random(seed)
    groups: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        groups.setdefault(record["label"]["intent"], []).append(record)

    train: list[dict[str, Any]] = []
    val: list[dict[str, Any]] = []
    test: list[dict[str, Any]] = []

    for group in groups.values():
        rng.shuffle(group)
        size = len(group)
        if size < 4:
            train.extend(group)
            continue
        n_test = max(1, round(size * (1 - train_ratio - val_ratio)))
        n_val = max(1, round(size * val_ratio))
        if n_test + n_val >= size:
            n_test = 1
            n_val = 1
        cut_train = size - n_test - n_val
        train.extend(group[:cut_train])
        val.extend(group[cut_train : cut_train + n_val])
        test.extend(group[cut_train + n_val :])

    rng.shuffle(train)
    rng.shuffle(val)
    rng.shuffle(test)
    return train, val, test


def _distribution(records: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for record in records:
        intent = record["label"]["intent"]
        counts[intent] = counts.get(intent, 0) + 1
    return dict(sorted(counts.items()))


def _write_jsonl(path: Path, records: list[dict[str, Any]]) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        for record in records:
            handle.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return digest


def write_dataset(
    records: list[dict[str, Any]],
    out_dir: Path,
    args: DatasetArgs,
    extra_manifest: dict[str, Any] | None = None,
) -> dict[str, Any]:
    out_dir.mkdir(parents=True, exist_ok=True)
    train, val, test = split_records(records, args.seed)
    if not train:
        raise RuntimeError("no training records generated")

    files = {
        "train": _write_jsonl(out_dir / "train.jsonl", train),
        "val": _write_jsonl(out_dir / "val.jsonl", val),
        "test": _write_jsonl(out_dir / "test.jsonl", test),
    }
    schema_path = out_dir / "intent.schema.json"
    schema_path.write_text(
        json.dumps(decision_schema(), indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    sources: dict[str, int] = {}
    for record in records:
        source = record["meta"].get("source", "unknown")
        sources[source] = sources.get(source, 0) + 1

    manifest = {
        "contract": CONTRACT_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "mode": args.mode,
        "seed": args.seed,
        "counts": {"total": len(records), "train": len(train), "val": len(val), "test": len(test)},
        "intents": {
            "total": _distribution(records),
            "train": _distribution(train),
            "val": _distribution(val),
            "test": _distribution(test),
        },
        "sources": sources,
        "teacher": {
            "model": args.teacher_model if "teacher" in sources else None,
            "prototypes": args.teacher_prototypes,
            "per_prototype": args.teacher_per_prototype,
        },
        "sha256": files,
        **(extra_manifest or {}),
    }
    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return manifest


def generate(args: DatasetArgs) -> dict[str, Any]:
    mode = args.mode
    if mode not in {"auto", "offline", "teacher", "hybrid"}:
        raise ValueError(f"unknown generation mode: {mode}")

    key = args.teacher_api_key or os.environ.get("TEACHER_API_KEY")
    use_teacher = mode in {"teacher", "hybrid"} or (mode == "auto" and bool(key))
    if mode == "teacher" and not key:
        raise ValueError("teacher mode requires TEACHER_API_KEY (or use --mode offline)")

    offline = generate_offline(args.count, args.seed)
    records = list(offline)
    teacher_records: list[dict[str, Any]] = []

    if use_teacher and key:
        args.teacher_api_key = key
        try:
            teacher_records = generate_teacher(offline, args)
            records.extend(teacher_records)
        except Exception as error:
            if mode == "teacher" and not args.allow_offline_fallback:
                raise
            print(f"teacher unavailable, continuing offline: {error}", file=sys.stderr)
    elif use_teacher:
        print("no TEACHER_API_KEY found; continuing offline", file=sys.stderr)

    records = dedupe(records)
    manifest = write_dataset(
        records,
        args.out_dir,
        args,
        extra_manifest={
            "teacher_records": len(teacher_records),
            "offline_records": len(offline),
        },
    )
    return manifest


def load_records(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
                validate_dataset_record(record["label"])
                records.append(record)
            except Exception as error:
                raise ValueError(f"{path}:{line_number}: invalid record: {error}") from error
    return records


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="Generate the Juniperus router dataset")
    parser.add_argument("--mode", choices=["auto", "offline", "teacher", "hybrid"], default="auto")
    parser.add_argument("--count", type=int, default=DEFAULT_COUNT)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--out", type=Path, default=Path("data/processed"))
    parser.add_argument("--teacher-model", default=os.environ.get("TEACHER_MODEL", "deepseek-chat"))
    parser.add_argument(
        "--teacher-base-url",
        default=os.environ.get("TEACHER_BASE_URL", "https://api.deepseek.com/v1"),
    )
    parser.add_argument("--teacher-prototypes", type=int, default=DEFAULT_TEACHER_PROTOTYPES)
    parser.add_argument("--teacher-per-prototype", type=int, default=DEFAULT_TEACHER_PER_PROTOTYPE)
    parser.add_argument("--teacher-concurrency", type=int, default=4)
    parser.add_argument("--teacher-temperature", type=float, default=0.8)
    parser.add_argument("--no-offline-fallback", action="store_true")
    args = parser.parse_args(argv)

    manifest = generate(
        DatasetArgs(
            mode=args.mode,
            count=args.count,
            seed=args.seed,
            out_dir=args.out,
            teacher_api_key=os.environ.get("TEACHER_API_KEY"),
            teacher_base_url=args.teacher_base_url,
            teacher_model=args.teacher_model,
            teacher_prototypes=args.teacher_prototypes,
            teacher_per_prototype=args.teacher_per_prototype,
            teacher_concurrency=args.teacher_concurrency,
            teacher_temperature=args.teacher_temperature,
            allow_offline_fallback=not args.no_offline_fallback,
        )
    )
    print(json.dumps(manifest, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
