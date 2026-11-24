# Juniperus

Router SLM ultraliviano (<1B) para **Oni ADE**. Convierte un pedido en lenguaje
natural â€”â€œmergeÃ¡ la de pricingâ€, â€œhow is the login task going?â€, â€œagregÃ¡ dark
mode a la PWAâ€â€” en una decisiÃ³n JSON validada que el daemon puede previsualizar
y, mÃ¡s adelante, ejecutar con confirmaciÃ³n humana.

Juniperus **no es un agente de cÃ³digo**: es el enrutador. El agente sigue siendo
OpenCode / Claude / un comando custom. El router decide intenciÃ³n, proyecto,
tarea, agente y criterio de verificaciÃ³n; nunca escribe en un worktree.

```text
PWA / CLI â”€â”€â–º GET/POST /api/router/* â”€â”€â–º oni-daemon
                                             â”‚
                         ONI_JUNIPERUS_URL?  â”œâ”€â”€â–º juniperus serve  (GGUF local)
                                             â””â”€â”€â–º fallback determinÃ­stico
```

## Estado

| Pieza | Estado |
|---|---|
| Contrato v1 + validaciÃ³n de seguridad | listo y testeado |
| Dataset sintÃ©tico (offline + profesor) | listo; 1.9k ejemplos generados localmente (no commiteados) |
| LoRA training (Transformers/PEFT + Unsloth opcional) | listo; `--dry-run` y smoke de 10 pasos verificados en CPU |
| Export GGUF + cuantizaciÃ³n | listo; requiere llama.cpp |
| Bench de intenciÃ³n / contrato / latencia | listo |
| Sidecar HTTP `/route` | listo |
| Fallback determinÃ­stico en Oni | listo; 74% intent accuracy en el test set |
| IntegraciÃ³n Oni (`/api/router/route`, `oni route`) | lista |
| Artefacto `juniperus-v1.gguf` | **no se commitea**; se genera con el pipeline |

El dataset y los pesos estÃ¡n en `.gitignore`. El repo guarda el contrato, los
scripts y los tests; el modelo se reproduce con tres comandos.

## Quickstart sin GPU

```bash
cd juniperus
uv sync --extra dev            # contrato, dataset, bench, tests (sin torch)

# smoke sin API key ni modelo: dataset offline + bench orÃ¡culo
uv run python scripts/01_generate_dataset.py --mode offline --count 240 --seed 7 --out data/demo
uv run python scripts/04_eval_bench.py --engine oracle --dataset data/demo/test.jsonl --out reports/demo

uv run juniperus route "mergeÃ¡ la tarea 3f2a1b9c" --backend mock --context examples/context.json
uv run juniperus serve --backend mock --port 7767
```

Para entrenar: `uv sync --extra train`. Para inferencia GGUF:
`uv sync --extra llm`. En Linux/macOS hay atajos equivalentes en el `Makefile`
(`make demo`, `make train`, ...).

Ese smoke no necesita API key ni modelo: valida el harness de punta a punta.
En Windows usÃ¡ los comandos `uv run`; el `Makefile` es para Unix/CI.

## Pipeline completo

```bash
# 1. dataset (usa el profesor si TEACHER_API_KEY estÃ¡; si no, offline)
uv run python scripts/01_generate_dataset.py --mode auto --count 1600

# 2. LoRA (GPU recomendada; en CPU usar --max-steps para un smoke)
uv run --extra train python scripts/02_train_lora.py --base-model Qwen/Qwen2.5-0.5B-Instruct

# 3. merge + GGUF (requiere llama.cpp; ver docs/PIPELINE.md)
uv run --extra train python scripts/03_export_gguf.py --quant Q4_K_M

# 4. bench
uv run --extra llm python scripts/04_eval_bench.py --engine llama \
  --model-path models/juniperus-v1.gguf
```

El profesor es OpenAI-compatible. Para OpenCode Console/Go:

```env
TEACHER_BASE_URL=https://opencode.ai/inference/openai/v1
TEACHER_MODEL=deepseek-v4.1-flash
```

El profesor **sÃ³lo parafrasea**; las etiquetas las calcula `juniperus/dataset.py`
y se validan contra el contrato. Una alucinaciÃ³n del profesor no puede romper el
schema.

## Contrato en 20 segundos

```json
{
  "intent": "spawn_agent",
  "agent_target": "opencode",
  "worktree_strategy": "isolated_branch",
  "branch_name": "oni/agrega-pricing",
  "project_ref": "landing",
  "task_prompt": "agregÃ¡ una secciÃ³n de precios",
  "loop_status": "single_shot",
  "max_iterations": 3,
  "success_criterion": "npm run build",
  "reasoning": "Pedido de implementaciÃ³n delegado a un agente."
}
```

Intenciones: `spawn_agent`, `query_status`, `merge_worktree`, `retry_task`,
`cancel_task`, `discard_task`, `list_projects`, `help`, `unknown`.

Reglas duras:

- `main_direct` **no existe** como salida vÃ¡lida. Si el modelo la propone, se
  degrada a worktree aislado y la decisiÃ³n queda en `needs_human`.
- `branch_name` siempre empieza con `oni/`.
- `max_iterations` estÃ¡ acotado a 1..6.
- Las acciones sobre una tarea exigen `task_ref`; si falta, no se adivina.
- La decisiÃ³n es **preview**: Oni no ejecuta nada desde el router.

Detalle completo en [docs/CONTRACT.md](docs/CONTRACT.md).

## IntegraciÃ³n con Oni

```powershell
# terminal 1: sidecar (mock para probar sin modelo)
uv run juniperus serve --backend mock --port 7767

# terminal 2: daemon con el sidecar
$env:ONI_JUNIPERUS_URL="http://127.0.0.1:7767"
cargo run --manifest-path ..\oni-daemon\Cargo.toml

# terminal 3: preview
oni route "mergeÃ¡ la tarea 3f2a1b9c"
```

Si el sidecar no estÃ¡, el daemon usa su router determinÃ­stico. Si el sidecar
devuelve `degraded`, tambiÃ©n. El MVP nunca depende del SLM.

Detalle en [docs/ONI_INTEGRATION.md](docs/ONI_INTEGRATION.md).

## Estructura

```text
juniperus/
â”œâ”€â”€ schema/intent_schema.py     # contrato, validaciÃ³n, sanitizaciÃ³n
â”œâ”€â”€ juniperus/
â”‚   â”œâ”€â”€ scenarios.py            # catÃ¡logo bilingÃ¼e de pedidos
â”‚   â”œâ”€â”€ dataset.py              # offline + profesor + split + manifest
â”‚   â”œâ”€â”€ train.py                # LoRA (Transformers/PEFT, Unsloth opcional)
â”‚   â”œâ”€â”€ export.py               # merge + convert + quantize
â”‚   â”œâ”€â”€ evaluate.py             # bench y reportes
â”‚   â”œâ”€â”€ runtime.py              # llama.cpp, transformers, mock, orÃ¡culo
â”‚   â”œâ”€â”€ server.py               # sidecar HTTP
â”‚   â””â”€â”€ cli.py                  # `juniperus <cmd>`
â”œâ”€â”€ scripts/01..04              # entrypoints del pipeline
â”œâ”€â”€ tests/                      # contrato, dataset, runtime, server, bench
â”œâ”€â”€ data/                       # generado (gitignored)
â”œâ”€â”€ models/                     # adapters/GGUF (gitignored)
â””â”€â”€ examples/context.json       # contexto de prueba
```

## Bench

Sobre el test set sintÃ©tico (190 ejemplos):

| Backend | Intent accuracy | Exact match | Schema OK | p95 |
|---|---:|---:|---:|---:|
| `oracle` (harness) | 100% | 100% | 100% | 0 ms |
| `mock` (reglas) | 73.7% | 73.7% | 100% | 0.04 ms |

El `oracle` valida el harness, no el modelo. El `mock` es el fallback real de
Oni cuando el SLM no estÃ¡: por eso importa que sea conservador y que nunca
proponga `main_direct`.

El bench del SLM se corre con `--engine llama` o `--engine transformers` y deja
`report.md` + `report.json` con matriz de confusiÃ³n, accuracy por campo y
muestras de fallas.

## Decisiones

- **Sidecar, no FFI (por ahora).** El daemon no compila llama.cpp ni arrastra un
  runtime C++ al binario. Habla HTTP con un proceso local opcional. Si el
  sidecar no estÃ¡, el daemon sigue funcionando. FFI queda para cuando el modelo
  justifique el costo de build y empaquetado.
- **Qwen2.5-0.5B-Instruct** como base default: licencia Apache-2.0, multilingÃ¼e,
  Q4_K_M ~400 MB. Se puede cambiar por Qwen3-0.6B o SmolLM2 sin tocar el
  contrato.
- **JSON estricto + validaciÃ³n + reintento.** El modelo puede fallar; el
  contrato no.
- **El router no ejecuta.** Oni muestra la decisiÃ³n; el merge/stop/retry sigue
  siendo una acciÃ³n humana explÃ­cita.

## Limitaciones

- El dataset es sintÃ©tico y bilingÃ¼e (es/en), con proyectos y tareas ficticias.
  El modelo puede fallar con jerga muy especÃ­fica de un repo.
- `branch_name` es una sugerencia: Oni genera la rama real con el id de tarea.
- La calidad del SLM depende del entrenamiento; el fallback determinÃ­stico no
  reemplaza un modelo entrenado.
- El contexto no incluye paths de repos ni diffs (privacidad y latencia).

## Licencia

El cÃ³digo del router sigue la licencia del repo Oni (decisiÃ³n pendiente en el
README raÃ­z; MIT sugerida). El modelo base Qwen2.5-0.5B-Instruct es Apache-2.0.
Pesos y datasets generados no se distribuyen en este repo.
