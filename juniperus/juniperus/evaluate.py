"""Bench de intención y contrato para Juniperus."""

from __future__ import annotations

import json
import statistics
import time
from pathlib import Path
from typing import Any

from schema.intent_schema import CONTRACT_VERSION, Intent, RouterIntent

from .dataset import load_records
from .runtime import Backend, RouteBackendError, build_backend

CONTRACT_FIELDS = (
    "intent",
    "agent_target",
    "worktree_strategy",
    "project_ref",
    "task_ref",
    "max_iterations",
)


def _norm(value: Any) -> str:
    return str(value or "").strip().lower()


def _percentile(values: list[float], fraction: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, round((len(ordered) - 1) * fraction)))
    return ordered[index]


def _matches(gold: Any, got: Any, *, field: str) -> bool:
    if field in {"project_ref", "task_ref"}:
        left, right = _norm(gold), _norm(got)
        if not left and not right:
            return True
        return left == right or (bool(left) and bool(right) and (left in right or right in left))
    return _norm(gold) == _norm(got)


def evaluate(
    records: list[dict[str, Any]],
    backend: Backend,
    *,
    limit: int | None = None,
) -> dict[str, Any]:
    examples = records[:limit] if limit else records
    if not examples:
        raise ValueError("no examples to evaluate")

    started = time.perf_counter()
    results: list[dict[str, Any]] = []
    latencies: list[float] = []
    failures: list[dict[str, Any]] = []

    for index, record in enumerate(examples, start=1):
        gold = RouterIntent.model_validate(record["label"])
        try:
            routed = backend.route(record["text"], record["context"], gold)
            got = routed.decision
            meta = routed.meta.to_dict()
            error = meta.get("error")
        except RouteBackendError as backend_error:
            got = RouterIntent(
                intent=Intent.UNKNOWN,
                reasoning="Backend failure.",
                max_iterations=1,
            )
            meta = {"backend": backend.name, "degraded": True, "error": str(backend_error)}
            error = str(backend_error)

        latencies.append(float(meta.get("latency_ms") or 0.0))
        parse_ok = not bool(meta.get("degraded")) and not error
        schema_ok = True
        try:
            RouterIntent.model_validate(got.model_dump(mode="json"))
        except Exception:
            schema_ok = False

        field_results = {
            field: _matches(getattr(gold, field), getattr(got, field), field=field)
            for field in CONTRACT_FIELDS
        }
        exact = (
            field_results["intent"]
            and field_results["worktree_strategy"]
            and field_results["project_ref"]
            and field_results["task_ref"]
        )
        safety_violations = 0
        if got.worktree_strategy.value == "main_direct":
            safety_violations += 1
        if got.branch_name and not got.branch_name.startswith("oni/"):
            safety_violations += 1
        if (
            gold.intent
            in {
                Intent.MERGE_WORKTREE,
                Intent.RETRY_TASK,
                Intent.CANCEL_TASK,
                Intent.DISCARD_TASK,
            }
            and not got.task_ref
        ):
            safety_violations += 1

        result = {
            "index": index,
            "text": record["text"],
            "gold": gold.model_dump(mode="json"),
            "got": got.model_dump(mode="json"),
            "meta": meta,
            "parse_ok": parse_ok,
            "schema_ok": schema_ok,
            "fields": field_results,
            "exact": exact,
            "safety_violations": safety_violations,
        }
        results.append(result)

        if (not field_results["intent"] or not parse_ok or not schema_ok) and len(failures) < 12:
            failures.append(
                {
                    "text": record["text"],
                    "expected": {
                        "intent": gold.intent.value,
                        "task_ref": gold.task_ref,
                        "project_ref": gold.project_ref,
                    },
                    "got": {
                        "intent": got.intent.value,
                        "task_ref": got.task_ref,
                        "project_ref": got.project_ref,
                    },
                    "error": error,
                }
            )

    total = len(results)
    correct_intent = sum(1 for result in results if result["fields"]["intent"])
    per_intent: dict[str, dict[str, Any]] = {}
    for intent in Intent:
        support = sum(1 for result in results if result["gold"]["intent"] == intent.value)
        correct = sum(
            1
            for result in results
            if result["gold"]["intent"] == intent.value and result["got"]["intent"] == intent.value
        )
        predicted = sum(1 for result in results if result["got"]["intent"] == intent.value)
        true_positive = correct
        precision = true_positive / predicted if predicted else 0.0
        recall = true_positive / support if support else 0.0
        f1 = (2 * precision * recall / (precision + recall)) if precision + recall else 0.0
        per_intent[intent.value] = {
            "support": support,
            "correct": correct,
            "accuracy": correct / support if support else 0.0,
            "precision": precision,
            "recall": recall,
            "f1": f1,
        }

    confusion: dict[str, dict[str, int]] = {}
    for result in results:
        expected = result["gold"]["intent"]
        predicted = result["got"]["intent"]
        confusion.setdefault(expected, {}).setdefault(predicted, 0)
        confusion[expected][predicted] += 1

    field_accuracy = {
        field: sum(1 for result in results if result["fields"][field]) / total
        for field in CONTRACT_FIELDS
    }

    report = {
        "contract": CONTRACT_VERSION,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "backend": backend.name,
        "model": backend.model,
        "examples": total,
        "elapsed_secs": round(time.perf_counter() - started, 2),
        "metrics": {
            "parse_ok": sum(1 for result in results if result["parse_ok"]) / total,
            "schema_ok": sum(1 for result in results if result["schema_ok"]) / total,
            "intent_accuracy": correct_intent / total,
            "exact_match": sum(1 for result in results if result["exact"]) / total,
            "safety_violations": sum(result["safety_violations"] for result in results),
            "field_accuracy": field_accuracy,
        },
        "latency_ms": {
            "mean": round(statistics.fmean(latencies), 2) if latencies else 0.0,
            "p50": round(_percentile(latencies, 0.5), 2),
            "p95": round(_percentile(latencies, 0.95), 2),
            "max": round(max(latencies), 2) if latencies else 0.0,
        },
        "per_intent": per_intent,
        "confusion": confusion,
        "failures": failures,
        "results": results,
    }
    return report


def _markdown(report: dict[str, Any]) -> str:
    metrics = report["metrics"]
    lines = [
        "# Juniperus bench",
        "",
        f"- **Contrato:** v{report['contract']}",
        f"- **Backend:** `{report['backend']}` ({report['model'] or '-'})",
        f"- **Ejemplos:** {report['examples']}",
        f"- **Duración:** {report['elapsed_secs']}s",
        "",
        "## Métricas",
        "",
        "| Métrica | Valor |",
        "|---|---:|",
        f"| JSON parse OK | {metrics['parse_ok']:.1%} |",
        f"| Schema OK | {metrics['schema_ok']:.1%} |",
        f"| Intent accuracy | {metrics['intent_accuracy']:.1%} |",
        f"| Exact match | {metrics['exact_match']:.1%} |",
        f"| Safety violations | {metrics['safety_violations']} |",
        f"| Latencia p50 | {report['latency_ms']['p50']} ms |",
        f"| Latencia p95 | {report['latency_ms']['p95']} ms |",
        "",
        "## Campos",
        "",
        "| Campo | Accuracy |",
        "|---|---:|",
    ]
    for field, value in metrics["field_accuracy"].items():
        lines.append(f"| {field} | {value:.1%} |")

    lines += [
        "",
        "## Por intención",
        "",
        "| Intención | Support | Correct | Accuracy | Precision | Recall | F1 |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for intent, row in report["per_intent"].items():
        if row["support"] == 0:
            continue
        lines.append(
            f"| {intent} | {row['support']} | {row['correct']} | {row['accuracy']:.1%} | "
            f"{row['precision']:.1%} | {row['recall']:.1%} | {row['f1']:.1%} |"
        )

    if report["failures"]:
        lines += ["", "## Fallas", ""]
        for failure in report["failures"]:
            lines.append(f"- **{failure['text'][:160]}**")
            lines.append(
                f"  - esperado `{failure['expected']['intent']}` "
                f"task={failure['expected']['task_ref']} project={failure['expected']['project_ref']}"
            )
            lines.append(
                f"  - obtenido `{failure['got']['intent']}` "
                f"task={failure['got']['task_ref']} project={failure['got']['project_ref']}"
            )
            if failure.get("error"):
                lines.append(f"  - error: `{failure['error']}`")

    lines.append("")
    return "\n".join(lines)


def write_report(report: dict[str, Any], out_dir: Path) -> tuple[Path, Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "report.json"
    md_path = out_dir / "report.md"
    # Los results completos son útiles para depurar, pero pesados para el log.
    json_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    md_path.write_text(_markdown(report) + "\n", encoding="utf-8")
    return json_path, md_path


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="Evaluate Juniperus on the router bench")
    parser.add_argument("--dataset", type=Path, default=Path("data/processed/test.jsonl"))
    parser.add_argument(
        "--engine", choices=["auto", "oracle", "mock", "llama", "transformers"], default="auto"
    )
    parser.add_argument("--model-path", type=Path, default=Path("models/juniperus-v1.gguf"))
    parser.add_argument("--base-model", default="Qwen/Qwen2.5-0.5B-Instruct")
    parser.add_argument("--adapter", type=Path, default=Path("models/juniperus-lora"))
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--out", type=Path, default=Path("reports/eval"))
    parser.add_argument("--json", action="store_true", help="print the full JSON report")
    args = parser.parse_args(argv)

    records = load_records(args.dataset)
    backend = build_backend(
        args.engine,
        model_path=args.model_path,
        base_model=args.base_model,
        adapter=args.adapter,
    )
    report = evaluate(records, backend, limit=args.limit)
    json_path, md_path = write_report(report, args.out)

    metrics = report["metrics"]
    print(
        f"{report['examples']} examples · {report['backend']} · "
        f"intent {metrics['intent_accuracy']:.1%} · exact {metrics['exact_match']:.1%} · "
        f"schema {metrics['schema_ok']:.1%} · p95 {report['latency_ms']['p95']} ms"
    )
    print(f"report: {json_path}")
    print(f"report: {md_path}")
    if args.json:
        print(json.dumps(report, indent=2, ensure_ascii=False))
    return 0
