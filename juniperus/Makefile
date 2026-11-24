.PHONY: setup setup-llm dataset dataset-offline train train-smoke export eval demo test lint doctor serve

setup:
	uv venv --python 3.11
	uv sync --extra dev --extra train

setup-llm:
	uv sync --extra dev --extra train --extra llm

# Dataset sintético. Usa el profesor si TEACHER_API_KEY está definido y cae a
# generación offline por templates si no.
dataset:
	uv run python scripts/01_generate_dataset.py --mode auto

dataset-offline:
	uv run python scripts/01_generate_dataset.py --mode offline --count 1600

train:
	uv run --extra train python scripts/02_train_lora.py

# Valida el loop de training en CPU sin intentar converger (2 pasos).
train-smoke:
	uv run --extra train python scripts/02_train_lora.py --max-samples 16 --max-steps 2 --batch-size 1 --max-seq-len 1024 --no-unsloth --out models/smoke-lora

export:
	uv run --extra train python scripts/03_export_gguf.py

eval:
	uv run python scripts/04_eval_bench.py --engine auto

# Sin descargas ni GPU: dataset offline + eval con el backend oráculo.
# Usa data/demo para no pisar el dataset real de data/processed.
demo:
	uv run python scripts/01_generate_dataset.py --mode offline --count 240 --seed 7 --out data/demo
	uv run python scripts/04_eval_bench.py --engine oracle --dataset data/demo/test.jsonl --out reports/demo

test:
	uv run --extra dev pytest

lint:
	uv run ruff check .
	uv run ruff format --check .

doctor:
	uv run juniperus doctor

serve:
	uv run --extra llm juniperus serve
