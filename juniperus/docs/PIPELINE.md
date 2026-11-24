# Pipeline de Juniperus

```text
escenarios + profesor
        â”‚
        â–¼
data/processed/{train,val,test}.jsonl
        â”‚
        â–¼
LoRA adapter  â”€â”€â–º  merged HF  â”€â”€â–º  GGUF f16  â”€â”€â–º  Q4_K_M
        â”‚                                           â”‚
        â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–º bench â—„â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

## 0. Setup

Requisitos: Python 3.11, `uv`, git. Para exportar: un checkout de llama.cpp con
`convert_hf_to_gguf.py` y `llama-quantize`.

```bash
cd juniperus
uv venv --python 3.11
uv sync --extra dev              # core + tests (sin torch)
uv sync --extra train            # training / export
# opcional para inferencia local GGUF:
uv sync --extra llm
```

En Windows, `llama-cpp-python` puede requerir Visual C++ Build Tools si no hay
wheel para tu versiÃ³n de Python. El entrenamiento y la evaluaciÃ³n con
Transformers no lo necesitan.

## 1. Dataset

```bash
# sin red ni API key
uv run python scripts/01_generate_dataset.py --mode offline --count 1600

# hÃ­brido: offline + parÃ¡frasis del profesor
uv run python scripts/01_generate_dataset.py --mode auto --count 1600 \
  --teacher-prototypes 60 --teacher-per-prototype 6 --teacher-concurrency 6

# sÃ³lo profesor (falla si no hay key)
uv run python scripts/01_generate_dataset.py --mode teacher --no-offline-fallback
```

ConfiguraciÃ³n del profesor (`.env`):

```env
TEACHER_API_KEY=...
TEACHER_BASE_URL=https://opencode.ai/inference/openai/v1
TEACHER_MODEL=deepseek-v4.1-flash
```

`TEACHER_MODEL` debe existir en el provider. Si ves `404`, revisÃ¡ el base URL:
OpenCode Console/Go usa `https://opencode.ai/inference/openai/v1`; DeepSeek
directo usa `https://api.deepseek.com/v1` con `deepseek-chat`.

### QuÃ© genera

Cada registro:

```json
{
  "id": "â€¦",
  "text": "che, mergeÃ¡ 3f2a1b9c",
  "context": { "projects": [], "tasks": [] },
  "label": { "intent": "merge_worktree", "...": "..." },
  "messages": [
    {"role": "system", "content": "â€¦"},
    {"role": "user", "content": "CONTEXTâ€¦\nPEDIDOâ€¦"},
    {"role": "assistant", "content": "{\"intent\":\"merge_worktree\",â€¦}"}
  ],
  "meta": {"source": "offline", "family": "merge_worktree", "lang": "mixed"}
}
```

- **Offline**: 45+ escenarios de spawn, 13 tareas, 4 acciones, queries, help,
  unknown y casos inseguros (`main`). Plantillas bilingÃ¼es y ruido de dictado.
- **Profesor**: parÃ¡frasis del pedido, no de la etiqueta. Se validan referencias
  (ids, nombres de proyecto, tÃ­tulos de tarea) antes de aceptar.
- **Split**: 80/10/10 estratificado por intenciÃ³n.
- **Manifest**: conteos, distribuciÃ³n, fuentes, hashes SHA-256.

El generador aplica cuotas por intenciÃ³n (`INTENT_WEIGHTS`) para que
`spawn_agent` no aplaste a `merge/retry/cancel/discard`. Si una intenciÃ³n rara
satura, avisa y sigue.

## 2. Training

```bash
# GPU (recomendado)
uv run --extra train python scripts/02_train_lora.py \
  --base-model Qwen/Qwen2.5-0.5B-Instruct \
  --epochs 3 --batch-size 4 --grad-accum 4 --lr 2e-4

# CPU / smoke
uv run --extra train python scripts/02_train_lora.py \
  --base-model Qwen/Qwen2.5-0.5B-Instruct \
  --batch-size 1 --grad-accum 8 --max-steps 30 --max-samples 200 --no-unsloth

# validar tokenizaciÃ³n sin bajar pesos
uv run --extra train python scripts/02_train_lora.py --dry-run --max-samples 8

# smoke real de 2 pasos en CPU (baja el modelo, ~1 minuto)
uv run --extra train python scripts/02_train_lora.py --max-samples 16 --max-steps 2 \
  --batch-size 1 --max-seq-len 1024 --no-unsloth --out models/smoke-lora
```

| Flag | Default | Notas |
|---|---|---|
| `--base-model` | `Qwen/Qwen2.5-0.5B-Instruct` | Apache-2.0, multilingÃ¼e. |
| `--max-seq-len` | 1024 | El dataset promedia ~760 tokens. |
| `--lora-r` / `--lora-alpha` | 16 / 32 | Suficiente para una tarea de formato. |
| `--batch-size` / `--grad-accum` | 2 / 4 | En CPU bajar batch a 1. |
| `--max-steps` | -1 | Ãštil para smoke y presupuesto. |
| `--no-unsloth` | off | Fuerza Transformers+PEFT. |
| `--device` | auto | `cuda` o `cpu`. |

Se enmascara el prompt con `-100`: la loss sÃ³lo ve el JSON del assistant. Se
guarda el adapter en `models/juniperus-lora/`, el mejor checkpoint en `best/` y
`training_meta.json` con loss, updates, device y config.

**Memoria**: Qwen2.5-0.5B en float32 ~2 GB de pesos + activaciones. Con
`--max-seq-len 512` y batch 1 entra en 8 GB de RAM. En GPU, Unsloth reduce
VRAM; si no estÃ¡, PEFT usa LoRA sobre fp16/bf16.

## 3. Export GGUF

### 3.1 llama.cpp

```bash
git clone --depth 1 https://github.com/ggerganov/llama.cpp third_party/llama.cpp
cmake -S third_party/llama.cpp -B third_party/llama.cpp/build
cmake --build third_party/llama.cpp/build --config Release -j
```

En Windows, el binario queda en
`third_party/llama.cpp/build/bin/Release/llama-quantize.exe`. En Linux/macOS,
`build/bin/llama-quantize`. TambiÃ©n podÃ©s bajar un release prebuilt y setear
`LLAMA_QUANTIZE`.

### 3.2 Merge + convert + quantize

```bash
uv run --extra train python scripts/03_export_gguf.py \
  --adapter models/juniperus-lora \
  --merged-dir models/juniperus-merged \
  --out models/juniperus-v1.gguf \
  --quant Q4_K_M \
  --llama-cpp-dir third_party/llama.cpp
```

El script:

1. carga el base en float32, aplica el adapter y hace `merge_and_unload()`;
2. corre `convert_hf_to_gguf.py --outtype f16`;
3. corre `llama-quantize â€¦ Q4_K_M`;
4. escribe `models/juniperus-v1.manifest.json` con SHA-256, tamaÃ±o, base,
   adapter, cuantizaciÃ³n y commit de llama.cpp.

Si no encuentra `llama-quantize`, deja el f16 y avisa. Con
`--require-quantize` falla en vez de continuar. `--clone` clona llama.cpp si
falta, pero **no compila**: eso es un paso manual.

TamaÃ±o esperado de Qwen2.5-0.5B Q4_K_M: ~400 MB.

## 4. Bench

```bash
# harness (no usa modelo)
uv run python scripts/04_eval_bench.py --engine oracle

# fallback determinÃ­stico
uv run python scripts/04_eval_bench.py --engine mock

# adapter
uv run --extra train python scripts/04_eval_bench.py --engine transformers \
  --base-model Qwen/Qwen2.5-0.5B-Instruct --adapter models/juniperus-lora

# GGUF
uv run --extra llm python scripts/04_eval_bench.py --engine llama \
  --model-path models/juniperus-v1.gguf
```

Reporta:

- JSON parse OK / schema OK.
- Intent accuracy y exact match (intenciÃ³n + proyecto + tarea + estrategia).
- Accuracy por campo (`agent_target`, `worktree_strategy`, `project_ref`,
  `task_ref`, `max_iterations`).
- Precision/recall/F1 por intenciÃ³n + matriz de confusiÃ³n.
- Latencia mean/p50/p95/max.
- Violaciones de seguridad (deberÃ­an ser 0).
- Primeras 12 fallas con entrada, esperado y obtenido.

Escribe `reports/eval/report.json` y `report.md`.

### Baseline actual

Sobre 190 ejemplos de test:

| Backend | Intent | Exact | Schema | p95 |
|---|---:|---:|---:|---:|
| oracle | 100% | 100% | 100% | 0 ms |
| mock | 73.7% | 73.7% | 100% | 0.04 ms |

El oracle valida el harness. El mock es el fallback real y el piso que el SLM
debe superar.

## 5. Troubleshooting

| SÃ­ntoma | Causa / soluciÃ³n |
|---|---|
| `404` del profesor | Base URL o modelo incorrecto. OpenCode: `https://opencode.ai/inference/openai/v1` + `deepseek-v4.1-flash`. |
| `429 Free usage limit reached` | El modelo free se agotÃ³; usÃ¡ un modelo Go/paid o esperÃ¡. |
| `teacher produced no valid paraphrases` | El profesor devolviÃ³ texto sin JSON o cambiÃ³ ids/tÃ­tulos; se usa offline. |
| `CUDA out of memory` | BajÃ¡ `--batch-size`, `--max-seq-len`, usÃ¡ `--no-unsloth` o 4-bit. |
| `llama-quantize not found` | CompilÃ¡ llama.cpp o seteÃ¡ `LLAMA_QUANTIZE`. |
| `llama-cpp-python` no instala | UsÃ¡ Python 3.11 con wheel, o evaluÃ¡ con `--engine transformers`. |
| El modelo emite markdown | El runtime lo limpia; si persiste, revisÃ¡ el system prompt y reentrenÃ¡ con mÃ¡s ejemplos. |
| El modelo inventa proyectos | Ya se valida contra el contexto? No: el contrato no lo impide. Revisar `project_ref` y agregar ejemplos negativos. |

## 6. Reproducibilidad

- Semilla default: 42 (dataset, split, training).
- `manifest.json` incluye hashes de train/val/test.
- `training_meta.json` incluye dataset dir, conteos, config y loss.
- El GGUF lleva un manifest con SHA-256.

Para comparar dos corridas, usÃ¡ el mismo `--seed` y el mismo `--count`. El
profesor es no determinÃ­stico: `--mode offline` es la referencia reproducible.
