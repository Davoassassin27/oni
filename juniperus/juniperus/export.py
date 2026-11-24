"""Merge del adapter LoRA y export a GGUF cuantizado.

Requiere un checkout de llama.cpp con ``convert_hf_to_gguf.py`` y el binario
``llama-quantize``. El script puede clonar llama.cpp si se lo pedís, pero no
compila C++ por vos: documentamos ese paso en docs/PIPELINE.md.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from schema.intent_schema import CONTRACT_VERSION

DEFAULT_OUT = Path("models/juniperus-v1.gguf")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def find_llama_cpp(explicit: Path | None = None, clone: bool = False) -> Path | None:
    candidates: list[Path] = []
    if explicit:
        candidates.append(Path(explicit))
    if os.environ.get("LLAMA_CPP_DIR"):
        candidates.append(Path(os.environ["LLAMA_CPP_DIR"]))
    candidates += [
        Path("third_party/llama.cpp"),
        Path("llama.cpp"),
        Path.home() / "llama.cpp",
        Path("C:/llama.cpp"),
    ]
    for candidate in candidates:
        if (candidate / "convert_hf_to_gguf.py").is_file():
            return candidate.resolve()

    if clone:
        target = Path("third_party/llama.cpp").resolve()
        target.parent.mkdir(parents=True, exist_ok=True)
        print(f"cloning llama.cpp into {target}")
        subprocess.run(
            ["git", "clone", "--depth", "1", "https://github.com/ggerganov/llama.cpp", str(target)],
            check=True,
        )
        if (target / "convert_hf_to_gguf.py").is_file():
            return target
    return None


def find_quantizer(llama_cpp: Path | None) -> Path | None:
    env = os.environ.get("LLAMA_QUANTIZE")
    candidates: list[Path] = []
    if env:
        candidates.append(Path(env))
    if llama_cpp:
        candidates += [
            llama_cpp / "build" / "bin" / "llama-quantize",
            llama_cpp / "build" / "bin" / "Release" / "llama-quantize.exe",
            llama_cpp / "build" / "bin" / "llama-quantize.exe",
            llama_cpp / "build" / "bin" / "Release" / "quantize.exe",
        ]
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()

    for name in ("llama-quantize", "llama-quantize.exe"):
        found = shutil.which(name)
        if found:
            return Path(found)
    return None


def merge_adapter(
    base_model: str, adapter: Path, merged_dir: Path, *, dry_run: bool = False
) -> Path:
    if not adapter.exists():
        raise FileNotFoundError(f"adapter not found: {adapter}")
    if dry_run:
        print(f"[dry-run] merge {adapter} + {base_model} -> {merged_dir}")
        return merged_dir

    try:
        import torch
        from peft import PeftModel
        from transformers import AutoModelForCausalLM, AutoTokenizer
    except ImportError as error:
        raise RuntimeError("torch, transformers and peft are required to merge") from error

    print(f"loading base model {base_model}")
    tokenizer = AutoTokenizer.from_pretrained(base_model, trust_remote_code=True)
    try:
        model = AutoModelForCausalLM.from_pretrained(
            base_model,
            dtype=torch.float32,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
        )
    except TypeError:
        model = AutoModelForCausalLM.from_pretrained(
            base_model,
            torch_dtype=torch.float32,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
        )

    print(f"merging adapter {adapter}")
    model = PeftModel.from_pretrained(model, str(adapter))
    model = model.merge_and_unload()
    merged_dir.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(merged_dir, safe_serialization=True, max_shard_size="2GB")
    tokenizer.save_pretrained(merged_dir)
    return merged_dir


def convert_to_gguf(
    llama_cpp: Path,
    merged_dir: Path,
    out_f16: Path,
    *,
    outtype: str = "f16",
    dry_run: bool = False,
) -> Path:
    converter = llama_cpp / "convert_hf_to_gguf.py"
    command = [
        sys.executable,
        str(converter),
        str(merged_dir),
        "--outfile",
        str(out_f16),
        "--outtype",
        outtype,
    ]
    if dry_run:
        print("[dry-run] " + " ".join(command))
        return out_f16
    if not converter.is_file():
        raise FileNotFoundError(f"converter not found: {converter}")
    out_f16.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(command, check=True)
    return out_f16


def quantize(
    quantizer: Path,
    source: Path,
    target: Path,
    quant: str,
    *,
    dry_run: bool = False,
) -> Path:
    command = [str(quantizer), str(source), str(target), quant]
    if dry_run:
        print("[dry-run] " + " ".join(command))
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(command, check=True)
    return target


def write_manifest(
    out: Path,
    *,
    base_model: str,
    adapter: Path,
    merged_dir: Path,
    llama_cpp: Path | None,
    quant: str | None,
    source: Path,
) -> Path:
    manifest = {
        "contract": CONTRACT_VERSION,
        "created_at": datetime.now(UTC).isoformat(),
        "base_model": base_model,
        "adapter": str(adapter),
        "merged_dir": str(merged_dir),
        "gguf": str(out),
        "quant": quant,
        "size_bytes": out.stat().st_size,
        "sha256": _sha256(out),
        "source": str(source),
        "llama_cpp": str(llama_cpp) if llama_cpp else None,
        "llama_cpp_commit": _git_commit(llama_cpp),
    }
    path = out.with_suffix(".manifest.json")
    path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return path


def _git_commit(repo: Path | None) -> str | None:
    if not repo:
        return None
    try:
        result = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout.strip()
    except Exception:
        return None


def export(
    *,
    base_model: str = "Qwen/Qwen2.5-0.5B-Instruct",
    adapter: Path = Path("models/juniperus-lora"),
    merged_dir: Path = Path("models/juniperus-merged"),
    out: Path = DEFAULT_OUT,
    quant: str = "Q4_K_M",
    llama_cpp_dir: Path | None = None,
    clone: bool = False,
    outtype: str = "f16",
    skip_merge: bool = False,
    keep_f16: bool = False,
    require_quantize: bool = False,
    dry_run: bool = False,
) -> dict[str, Any]:
    llama_cpp = find_llama_cpp(llama_cpp_dir, clone=clone)
    if llama_cpp is None and not dry_run:
        raise FileNotFoundError(
            "llama.cpp not found. Set LLAMA_CPP_DIR, pass --llama-cpp-dir, "
            "or use --clone. See docs/PIPELINE.md for the build step."
        )
    print(f"llama.cpp: {llama_cpp or '(dry-run without checkout)'}")

    if not skip_merge:
        merge_adapter(base_model, adapter, merged_dir, dry_run=dry_run)
    elif not merged_dir.exists() and not dry_run:
        raise FileNotFoundError(f"merged dir not found: {merged_dir}")

    out_f16 = merged_dir.with_suffix(".f16.gguf")
    convert_to_gguf(
        llama_cpp or Path("llama.cpp"), merged_dir, out_f16, outtype=outtype, dry_run=dry_run
    )

    quantizer = find_quantizer(llama_cpp)
    final_source = out_f16
    used_quant: str | None = None
    if quantizer and quant:
        quantize(quantizer, out_f16, out, quant, dry_run=dry_run)
        final_source = out
        used_quant = quant
    else:
        message = (
            "llama-quantize not found; keeping f16 GGUF. Build llama.cpp "
            "(cmake --build build --config Release) or set LLAMA_QUANTIZE."
        )
        if require_quantize and not dry_run:
            raise FileNotFoundError(message)
        print(f"warning: {message}")
        if out != out_f16:
            if dry_run:
                print(f"[dry-run] copy {out_f16} -> {out}")
            else:
                out.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(out_f16, out)
            final_source = out

    if not dry_run:
        manifest_path = write_manifest(
            out,
            base_model=base_model,
            adapter=adapter,
            merged_dir=merged_dir,
            llama_cpp=llama_cpp,
            quant=used_quant,
            source=final_source,
        )
        print(f"gguf: {out} ({out.stat().st_size / 1024 / 1024:.1f} MB)")
        print(f"manifest: {manifest_path}")
    if not keep_f16 and not dry_run and out_f16.exists() and out_f16 != out:
        out_f16.unlink()

    return {
        "gguf": str(out),
        "quant": used_quant,
        "dry_run": dry_run,
        "llama_cpp": str(llama_cpp) if llama_cpp else None,
    }


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="Merge LoRA and export Juniperus to GGUF")
    parser.add_argument("--base-model", default="Qwen/Qwen2.5-0.5B-Instruct")
    parser.add_argument("--adapter", type=Path, default=Path("models/juniperus-lora"))
    parser.add_argument("--merged-dir", type=Path, default=Path("models/juniperus-merged"))
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--quant", default="Q4_K_M", help="Q4_K_M, Q5_K_M, Q8_0, or '' to skip")
    parser.add_argument("--llama-cpp-dir", type=Path, default=None)
    parser.add_argument("--clone", action="store_true", help="clone llama.cpp if missing")
    parser.add_argument("--outtype", default="f16")
    parser.add_argument("--skip-merge", action="store_true")
    parser.add_argument("--keep-f16", action="store_true")
    parser.add_argument("--require-quantize", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    result = export(
        base_model=args.base_model,
        adapter=args.adapter,
        merged_dir=args.merged_dir,
        out=args.out,
        quant=args.quant,
        llama_cpp_dir=args.llama_cpp_dir,
        clone=args.clone,
        outtype=args.outtype,
        skip_merge=args.skip_merge,
        keep_f16=args.keep_f16,
        require_quantize=args.require_quantize,
        dry_run=args.dry_run,
    )
    print(json.dumps(result, indent=2, ensure_ascii=False))
    return 0
