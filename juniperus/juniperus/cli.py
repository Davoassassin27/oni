"""Entrypoint único: ``juniperus <dataset|train|export|eval|serve|route|schema|doctor>``."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from schema.intent_schema import CONTRACT_VERSION, RouterContext

from . import __version__

VERSION = __version__

HELP = f"""\
juniperus {VERSION} · contrato v{CONTRACT_VERSION}

Uso:
  juniperus dataset [--mode auto|offline|teacher|hybrid] [--count 1600]
  juniperus train   [--dry-run] [--max-steps N]
  juniperus export  [--quant Q4_K_M] [--dry-run]
  juniperus eval    [--engine auto|oracle|mock|llama|transformers]
  juniperus serve   [--backend auto|mock|llama|transformers] [--port 7767]
  juniperus route   "mergeá la tarea 3f2a1b9c" [--context context.json]
  juniperus schema  [--write schema.json] [--prompt]
  juniperus doctor

Docs: juniperus/README.md · docs/CONTRACT.md · docs/PIPELINE.md
"""


def _doctor() -> int:
    import importlib.util

    print(f"juniperus {VERSION} · contrato v{CONTRACT_VERSION}")
    print(f"python {sys.version.split()[0]} · cwd {Path.cwd()}")
    print()

    def check(label: str, ok: bool, detail: str = "") -> bool:
        mark = "OK " if ok else "!! "
        print(f"  [{mark}] {label}{(' · ' + detail) if detail else ''}")
        return ok

    core = True
    for module in ("pydantic", "openai", "dotenv"):
        core &= check(f"python:{module}", importlib.util.find_spec(module) is not None)
    train = True
    for module in ("torch", "transformers", "peft", "datasets"):
        present = importlib.util.find_spec(module) is not None
        train &= check(f"python:{module} (train)", present)
    if not train:
        print("      falta el extra train: uv sync --extra train")
    check("python:llama_cpp (opcional)", importlib.util.find_spec("llama_cpp") is not None)
    check("python:unsloth (opcional)", importlib.util.find_spec("unsloth") is not None)

    key = bool(os.environ.get("TEACHER_API_KEY"))
    check("env:TEACHER_API_KEY", key, "configurada" if key else "sólo offline")
    model_path = Path(os.environ.get("JUNIPERUS_MODEL_PATH", "models/juniperus-v1.gguf"))
    adapter = Path(os.environ.get("JUNIPERUS_ADAPTER", "models/juniperus-lora"))
    check("modelo GGUF", model_path.is_file(), str(model_path))
    check("adapter LoRA", adapter.exists(), str(adapter))

    for split in ("train", "val", "test"):
        path = Path("data/processed") / f"{split}.jsonl"
        count = 0
        if path.is_file():
            with path.open("r", encoding="utf-8") as handle:
                count = sum(1 for line in handle if line.strip())
        check(f"dataset:{split}", path.is_file(), f"{count} ejemplos")

    print()
    if not core:
        print("Faltan dependencias. Corré: uv sync --extra dev")
        return 1
    print(
        "Listo. Sin GPU: `uv run python scripts/01_generate_dataset.py --mode offline` + "
        "`uv run python scripts/04_eval_bench.py --engine oracle`."
    )
    print(
        "Con GPU: `uv run --extra train python scripts/02_train_lora.py` y luego "
        "`uv run --extra train python scripts/03_export_gguf.py`."
    )
    if not model_path.is_file():
        print("Todavía no hay GGUF: generá el dataset, entrená y exportá.")
    return 0


def _route(argv: list[str]) -> int:
    import argparse

    from .runtime import build_backend, json_result, route_text

    parser = argparse.ArgumentParser(prog="juniperus route", description="Route one command")
    parser.add_argument("text", nargs="+", help="natural-language command")
    parser.add_argument(
        "--backend", choices=["auto", "mock", "llama", "transformers"], default="auto"
    )
    parser.add_argument(
        "--model-path",
        type=Path,
        default=Path(os.environ.get("JUNIPERUS_MODEL_PATH", "models/juniperus-v1.gguf")),
    )
    parser.add_argument(
        "--base-model",
        default=os.environ.get("JUNIPERUS_BASE_MODEL", "Qwen/Qwen2.5-0.5B-Instruct"),
    )
    parser.add_argument(
        "--adapter",
        type=Path,
        default=Path(os.environ.get("JUNIPERUS_ADAPTER", "models/juniperus-lora")),
    )
    parser.add_argument("--context", type=Path, help="JSON file with projects/tasks")
    parser.add_argument("--json", action="store_true", help="print full JSON with meta")
    args = parser.parse_args(argv)

    context: RouterContext | None = None
    if args.context:
        context = RouterContext.model_validate_json(args.context.read_text(encoding="utf-8"))
    backend = build_backend(
        args.backend,
        model_path=args.model_path,
        base_model=args.base_model,
        adapter=args.adapter,
    )
    result = route_text(backend, " ".join(args.text), context)
    if args.json:
        print(json_result(result))
    else:
        print(result.decision.to_json())
    return 0


def main(argv: list[str] | None = None) -> int:
    from dotenv import load_dotenv

    load_dotenv()
    args = list(sys.argv[1:] if argv is None else argv)
    if not args or args[0] in {"-h", "--help", "help"}:
        print(HELP)
        return 0
    if args[0] in {"-V", "--version", "version"}:
        print(VERSION)
        return 0

    command, rest = args[0], args[1:]
    if command == "dataset":
        from . import dataset

        return dataset.main(rest)
    if command == "train":
        from . import train

        return train.main(rest)
    if command == "export":
        from . import export

        return export.main(rest)
    if command == "eval":
        from . import evaluate

        return evaluate.main(rest)
    if command == "serve":
        from . import server

        return server.main(rest)
    if command == "route":
        return _route(rest)
    if command == "doctor":
        return _doctor()
    if command == "schema":
        from schema import intent_schema

        return intent_schema.main(rest)
    if command == "show-schema":
        from schema.intent_schema import decision_schema

        print(json.dumps(decision_schema(), indent=2, ensure_ascii=False))
        return 0

    print(f"unknown command: {command}\n", file=sys.stderr)
    print(HELP, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
