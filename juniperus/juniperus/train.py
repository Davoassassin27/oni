"""LoRA fine-tuning de un SLM <1B para el contrato de Juniperus.

Camino recomendado: GPU CUDA + Unsloth (si está instalado).
Camino portable: CPU/GPU con Transformers + PEFT (más lento, mismos datos).

Sólo se calcula loss sobre la respuesta JSON del assistant; el prompt queda
enmascarado con -100.
"""

from __future__ import annotations

import json
import math
import random
import time
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import torch
from datasets import Dataset as HFDataset
from torch.utils.data import DataLoader
from transformers import AutoModelForCausalLM, AutoTokenizer, get_cosine_schedule_with_warmup

from schema.intent_schema import CONTRACT_VERSION

from .dataset import load_records


@dataclass
class TrainArgs:
    base_model: str = "Qwen/Qwen2.5-0.5B-Instruct"
    dataset_dir: Path = Path("data/processed")
    out_dir: Path = Path("models/juniperus-lora")
    epochs: float = 3.0
    batch_size: int = 2
    grad_accum: int = 4
    lr: float = 2e-4
    weight_decay: float = 0.0
    max_seq_len: int = 1024
    max_steps: int = -1
    max_samples: int | None = None
    lora_r: int = 16
    lora_alpha: int = 32
    lora_dropout: float = 0.05
    warmup_ratio: float = 0.03
    eval_every: int = 50
    log_every: int = 10
    seed: int = 42
    device: str = "auto"
    use_unsloth: bool = True
    dry_run: bool = False


def _format_example(
    tokenizer: Any, record: dict[str, Any], max_seq_len: int
) -> dict[str, list[int]]:
    messages = record["messages"]
    prompt_messages = messages[:-1]
    completion = messages[-1]["content"]
    prompt_text = tokenizer.apply_chat_template(
        prompt_messages, tokenize=False, add_generation_prompt=True
    )
    completion_text = completion + (tokenizer.eos_token or "")
    prompt_ids = tokenizer(prompt_text, add_special_tokens=False)["input_ids"]
    completion_ids = tokenizer(completion_text, add_special_tokens=False)["input_ids"]

    total = len(prompt_ids) + len(completion_ids)
    if total > max_seq_len:
        overflow = total - max_seq_len
        if overflow < len(prompt_ids):
            prompt_ids = prompt_ids[overflow:]
        else:
            keep = max(0, max_seq_len - len(prompt_ids))
            completion_ids = completion_ids[:keep]
            prompt_ids = prompt_ids[:max_seq_len]
    input_ids = prompt_ids + completion_ids
    labels = [-100] * len(prompt_ids) + completion_ids
    return {"input_ids": input_ids, "labels": labels, "attention_mask": [1] * len(input_ids)}


def _collate(batch: list[dict[str, list[int]]], pad_token_id: int) -> dict[str, torch.Tensor]:
    width = max(len(item["input_ids"]) for item in batch)
    input_ids, labels, attention = [], [], []
    for item in batch:
        pad = width - len(item["input_ids"])
        input_ids.append(item["input_ids"] + [pad_token_id] * pad)
        labels.append(item["labels"] + [-100] * pad)
        attention.append(item["attention_mask"] + [0] * pad)
    return {
        "input_ids": torch.tensor(input_ids, dtype=torch.long),
        "labels": torch.tensor(labels, dtype=torch.long),
        "attention_mask": torch.tensor(attention, dtype=torch.long),
    }


def _target_modules(model: Any) -> list[str]:
    suffixes = (
        "q_proj",
        "k_proj",
        "v_proj",
        "o_proj",
        "gate_proj",
        "up_proj",
        "down_proj",
    )
    found: set[str] = set()
    for name, module in model.named_modules():
        if isinstance(module, torch.nn.Linear):
            for suffix in suffixes:
                if name.endswith(suffix):
                    found.add(suffix)
    return sorted(found) or ["q_proj", "v_proj"]


def _load_model(args: TrainArgs, device: torch.device) -> tuple[Any, Any, bool]:
    tokenizer = AutoTokenizer.from_pretrained(args.base_model, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "right"

    if args.use_unsloth and device.type == "cuda":
        try:
            from unsloth import FastLanguageModel

            model, tokenizer = FastLanguageModel.from_pretrained(
                model_name=args.base_model,
                max_seq_length=args.max_seq_len,
                dtype=None,
                load_in_4bit=False,
            )
            model = FastLanguageModel.get_peft_model(
                model,
                r=args.lora_r,
                target_modules=[
                    "q_proj",
                    "k_proj",
                    "v_proj",
                    "o_proj",
                    "gate_proj",
                    "up_proj",
                    "down_proj",
                ],
                lora_alpha=args.lora_alpha,
                lora_dropout=args.lora_dropout,
                bias="none",
                use_gradient_checkpointing="unsloth",
                random_state=args.seed,
            )
            return model, tokenizer, True
        except Exception as error:
            print(f"unsloth unavailable ({error}); falling back to transformers + peft")

    from peft import LoraConfig, get_peft_model

    dtype = torch.float32
    if device.type == "cuda":
        dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
    try:
        model = AutoModelForCausalLM.from_pretrained(
            args.base_model,
            dtype=dtype,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
        )
    except TypeError:
        model = AutoModelForCausalLM.from_pretrained(
            args.base_model,
            torch_dtype=dtype,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
        )
    model.to(device)
    targets = _target_modules(model)
    config = LoraConfig(
        r=args.lora_r,
        lora_alpha=args.lora_alpha,
        lora_dropout=args.lora_dropout,
        bias="none",
        task_type="CAUSAL_LM",
        target_modules=targets,
    )
    model = get_peft_model(model, config)
    return model, tokenizer, False


def _tokenize_records(
    tokenizer: Any, records: list[dict[str, Any]], args: TrainArgs
) -> list[dict[str, list[int]]]:
    tokenized = [_format_example(tokenizer, record, args.max_seq_len) for record in records]
    truncated = sum(
        1
        for item, record in zip(tokenized, records, strict=False)
        if len(item["input_ids"]) >= args.max_seq_len
    )
    if truncated:
        print(
            f"warning: {truncated}/{len(records)} examples hit max_seq_len={args.max_seq_len}; "
            "raise it if the labels look cut"
        )
    lengths = [len(item["input_ids"]) for item in tokenized]
    print(
        f"tokens: mean={sum(lengths) / len(lengths):.0f} max={max(lengths)} "
        f"p95={sorted(lengths)[int(len(lengths) * 0.95)]}"
    )
    return tokenized


def _evaluate_loss(
    model: Any, loader: DataLoader, device: torch.device, max_batches: int = 20
) -> float:
    model.eval()
    losses: list[float] = []
    with torch.no_grad():
        for index, batch in enumerate(loader):
            if index >= max_batches:
                break
            batch = {key: value.to(device) for key, value in batch.items()}
            output = model(**batch)
            losses.append(float(output.loss.detach().cpu()))
    model.train()
    return sum(losses) / len(losses) if losses else float("nan")


def train(args: TrainArgs) -> dict[str, Any]:
    random.seed(args.seed)
    torch.manual_seed(args.seed)
    torch.cuda.manual_seed_all(args.seed)

    device = torch.device(
        args.device if args.device != "auto" else ("cuda" if torch.cuda.is_available() else "cpu")
    )
    print(f"device: {device}")

    train_records = load_records(args.dataset_dir / "train.jsonl")
    val_path = args.dataset_dir / "val.jsonl"
    val_records = load_records(val_path) if val_path.exists() else []
    if args.max_samples:
        train_records = train_records[: args.max_samples]
        val_records = val_records[: max(8, args.max_samples // 10)]
    if not train_records:
        raise RuntimeError("no training records")

    tokenizer_for_check = AutoTokenizer.from_pretrained(args.base_model, trust_remote_code=True)
    if tokenizer_for_check.pad_token is None:
        tokenizer_for_check.pad_token = tokenizer_for_check.eos_token
    tokenized = _tokenize_records(tokenizer_for_check, train_records, args)
    tokenized_val = _tokenize_records(tokenizer_for_check, val_records, args) if val_records else []

    if args.dry_run:
        pad = tokenizer_for_check.pad_token_id
        sample = _collate(tokenized[:2], pad)
        print(
            "dry-run ok: "
            f"train={len(tokenized)} val={len(tokenized_val)} "
            f"batch_input_ids={tuple(sample['input_ids'].shape)} "
            f"batch_labels={tuple(sample['labels'].shape)}"
        )
        print(json.dumps(_config_dict(args, device), indent=2, ensure_ascii=False))
        return {"dry_run": True, "train": len(tokenized), "val": len(tokenized_val)}

    model, tokenizer, used_unsloth = _load_model(args, device)
    model.train()
    if hasattr(model, "config"):
        model.config.use_cache = False
    if hasattr(model, "print_trainable_parameters"):
        model.print_trainable_parameters()

    train_loader = DataLoader(
        HFDataset.from_list(tokenized).with_format("python"),
        batch_size=args.batch_size,
        shuffle=True,
        collate_fn=lambda batch: _collate(batch, tokenizer.pad_token_id),
        drop_last=False,
    )
    val_loader = (
        DataLoader(
            HFDataset.from_list(tokenized_val).with_format("python"),
            batch_size=args.batch_size,
            shuffle=False,
            collate_fn=lambda batch: _collate(batch, tokenizer.pad_token_id),
        )
        if tokenized_val
        else None
    )

    optimizer = torch.optim.AdamW(
        [parameter for parameter in model.parameters() if parameter.requires_grad],
        lr=args.lr,
        weight_decay=args.weight_decay,
    )
    updates_per_epoch = math.ceil(len(train_loader) / args.grad_accum)
    total_updates = max(1, int(updates_per_epoch * args.epochs))
    if args.max_steps > 0:
        total_updates = min(total_updates, args.max_steps)
    scheduler = get_cosine_schedule_with_warmup(
        optimizer,
        num_warmup_steps=max(1, int(total_updates * args.warmup_ratio)),
        num_training_steps=total_updates,
    )

    args.out_dir.mkdir(parents=True, exist_ok=True)
    started = time.time()
    global_step = 0
    micro_step = 0
    epochs_done = 0
    running_loss: list[float] = []
    best_eval = float("inf")
    history: list[dict[str, Any]] = []

    for epoch in range(math.ceil(args.epochs)):
        if global_step >= total_updates:
            break
        epochs_done = epoch + 1
        for batch in train_loader:
            if global_step >= total_updates:
                break
            batch = {key: value.to(device) for key, value in batch.items()}
            output = model(**batch)
            loss = output.loss / args.grad_accum
            loss.backward()
            running_loss.append(float(output.loss.detach().cpu()))
            micro_step += 1

            if micro_step % args.grad_accum == 0:
                torch.nn.utils.clip_grad_norm_(
                    [p for p in model.parameters() if p.requires_grad], 1.0
                )
                optimizer.step()
                scheduler.step()
                optimizer.zero_grad(set_to_none=True)
                global_step += 1

                if global_step % args.log_every == 0 or global_step == 1:
                    avg = sum(running_loss[-args.log_every :]) / min(
                        len(running_loss), args.log_every
                    )
                    print(
                        f"step {global_step}/{total_updates} "
                        f"loss {avg:.4f} lr {scheduler.get_last_lr()[0]:.2e}"
                    )
                if val_loader is not None and global_step % args.eval_every == 0:
                    eval_loss = _evaluate_loss(model, val_loader, device)
                    print(f"  eval loss {eval_loss:.4f}")
                    history.append({"step": global_step, "eval_loss": eval_loss})
                    if eval_loss < best_eval:
                        best_eval = eval_loss
                        model.save_pretrained(args.out_dir / "best")
                        tokenizer.save_pretrained(args.out_dir / "best")

    if micro_step % args.grad_accum != 0 and global_step < total_updates:
        optimizer.step()
        scheduler.step()
        optimizer.zero_grad(set_to_none=True)
        global_step += 1

    model.save_pretrained(args.out_dir)
    tokenizer.save_pretrained(args.out_dir)

    meta = {
        "contract": CONTRACT_VERSION,
        "created_at": datetime.now(UTC).isoformat(),
        "base_model": args.base_model,
        "dataset_dir": str(args.dataset_dir),
        "train_examples": len(tokenized),
        "val_examples": len(tokenized_val),
        "epochs": epochs_done,
        "updates": global_step,
        "final_loss": sum(running_loss[-50:]) / min(len(running_loss), 50)
        if running_loss
        else None,
        "best_eval_loss": None if best_eval == float("inf") else best_eval,
        "history": history,
        "elapsed_secs": round(time.time() - started, 1),
        "device": str(device),
        "unsloth": used_unsloth,
        "config": _config_dict(args, device),
    }
    (args.out_dir / "training_meta.json").write_text(
        json.dumps(meta, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"saved adapter: {args.out_dir}")
    print(
        f"updates={global_step} final_loss={meta['final_loss']} "
        f"best_eval={meta['best_eval_loss']} elapsed={meta['elapsed_secs']}s"
    )
    return meta


def _config_dict(args: TrainArgs, device: torch.device) -> dict[str, Any]:
    return {
        "base_model": args.base_model,
        "dataset_dir": str(args.dataset_dir),
        "out_dir": str(args.out_dir),
        "epochs": args.epochs,
        "batch_size": args.batch_size,
        "grad_accum": args.grad_accum,
        "lr": args.lr,
        "max_seq_len": args.max_seq_len,
        "lora_r": args.lora_r,
        "lora_alpha": args.lora_alpha,
        "seed": args.seed,
        "device": str(device),
        "use_unsloth": args.use_unsloth,
    }


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="LoRA fine-tune Juniperus")
    parser.add_argument("--base-model", default="Qwen/Qwen2.5-0.5B-Instruct")
    parser.add_argument("--dataset-dir", type=Path, default=Path("data/processed"))
    parser.add_argument("--out", type=Path, default=Path("models/juniperus-lora"))
    parser.add_argument("--epochs", type=float, default=3.0)
    parser.add_argument("--batch-size", type=int, default=2)
    parser.add_argument("--grad-accum", type=int, default=4)
    parser.add_argument("--lr", type=float, default=2e-4)
    parser.add_argument("--max-seq-len", type=int, default=1024)
    parser.add_argument("--max-steps", type=int, default=-1)
    parser.add_argument("--max-samples", type=int, default=None)
    parser.add_argument("--lora-r", type=int, default=16)
    parser.add_argument("--lora-alpha", type=int, default=32)
    parser.add_argument("--lora-dropout", type=float, default=0.05)
    parser.add_argument("--warmup-ratio", type=float, default=0.03)
    parser.add_argument("--eval-every", type=int, default=50)
    parser.add_argument("--log-every", type=int, default=10)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--device", default="auto")
    parser.add_argument("--no-unsloth", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    meta = train(
        TrainArgs(
            base_model=args.base_model,
            dataset_dir=args.dataset_dir,
            out_dir=args.out,
            epochs=args.epochs,
            batch_size=args.batch_size,
            grad_accum=args.grad_accum,
            lr=args.lr,
            max_seq_len=args.max_seq_len,
            max_steps=args.max_steps,
            max_samples=args.max_samples,
            lora_r=args.lora_r,
            lora_alpha=args.lora_alpha,
            lora_dropout=args.lora_dropout,
            warmup_ratio=args.warmup_ratio,
            eval_every=args.eval_every,
            log_every=args.log_every,
            seed=args.seed,
            device=args.device,
            use_unsloth=not args.no_unsloth,
            dry_run=args.dry_run,
        )
    )
    if not meta.get("dry_run"):
        print(json.dumps(meta, indent=2, ensure_ascii=False))
    return 0
