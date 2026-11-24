# IntegraciÃ³n con Oni

Juniperus entra en Oni como un **router de preview**. No reemplaza al agente ni
ejecuta acciones: convierte lenguaje natural en una decisiÃ³n validada que la PWA
o el CLI pueden mostrar y, en una fase posterior, ejecutar con confirmaciÃ³n.

## Arquitectura

```text
PWA / CLI
   â”‚  POST /api/router/route { text, projectId?, taskId? }
   â–¼
oni-daemon â”€â”€â–º ONI_JUNIPERUS_URL? â”€â”€â–º juniperus serve (127.0.0.1:7767)
   â”‚                                      â”‚
   â”‚                                      â””â”€ llama.cpp + juniperus-v1.gguf
   â””â”€ si no hay URL, falla o degraded â”€â”€â–º fallback determinÃ­stico en Rust
```

- El daemon arma el contexto desde su store: proyectos, tareas recientes,
  selecciÃ³n actual. No manda paths ni diffs.
- El sidecar devuelve `decision` + `meta`.
- El daemon valida de nuevo: `main_direct` se degrada, ramas se prefijan con
  `oni/`, iteraciones se acotan, acciones sin tarea quedan en `needs_human`.
- Si el sidecar no responde en `ONI_JUNIPERUS_TIMEOUT_MS` (default 1500 ms) o
  devuelve `degraded`, se usa el fallback. El endpoint siempre responde 200 si
  pudo construir una decisiÃ³n.

## Arranque

```powershell
# 1. sidecar (mock para probar ya; llama para el modelo real)
cd juniperus
uv run juniperus serve --backend mock --port 7767
# uv run juniperus serve --backend llama --model-path models/juniperus-v1.gguf

# 2. daemon
$env:ONI_JUNIPERUS_URL="http://127.0.0.1:7767"
# si el sidecar tiene token:
$env:ONI_JUNIPERUS_TOKEN="..."
cargo run --manifest-path oni-daemon/Cargo.toml
```

Variables:

| Variable | Default | DescripciÃ³n |
|---|---|---|
| `ONI_JUNIPERUS_URL` | vacÃ­o | Base URL del sidecar. VacÃ­o = fallback. |
| `ONI_JUNIPERUS_TOKEN` | vacÃ­o | Bearer token opcional. |
| `ONI_JUNIPERUS_TIMEOUT_MS` | `1500` | Timeout por request. |
| `JUNIPERUS_MODEL_PATH` | `models/juniperus-v1.gguf` | GGUF del sidecar. |
| `JUNIPERUS_ADAPTER` | `models/juniperus-lora` | Adapter para el backend Transformers. |
| `JUNIPERUS_HOST` / `JUNIPERUS_PORT` | `127.0.0.1` / `7767` | Bind del sidecar. |
| `JUNIPERUS_TOKEN` | vacÃ­o | Token del sidecar. |

## API

### `POST /api/router/route`

Request:

```json
{"text":"mergeÃ¡ la tarea 3f2a1b9c","projectId":null,"taskId":null}
```

`projectId`/`taskId` son opcionales; se usan como selecciÃ³n para el contexto.
Si se envÃ­an, deben existir.

Response:

```json
{
  "decision": {
    "intent": "merge_worktree",
    "agent_target": "auto",
    "model_override": null,
    "worktree_strategy": "isolated_branch",
    "branch_name": null,
    "project_ref": "landing",
    "task_ref": "3f2a1b9c",
    "task_prompt": null,
    "loop_status": "completed",
    "max_iterations": 1,
    "success_criterion": null,
    "reasoning": "Se pidiÃ³ integrar una tarea."
  },
  "routing": {
    "source": "juniperus",
    "latencyMs": 812,
    "degraded": false,
    "error": null,
    "backend": "llama",
    "model": "models/juniperus-v1.gguf"
  },
  "context": {"projects": 2, "tasks": 5}
}
```

- `source`: `juniperus` o `fallback`.
- `degraded`: el sidecar fallÃ³ o devolviÃ³ algo invÃ¡lido. La decisiÃ³n es del
  fallback.
- La decisiÃ³n es **preview**; no se ejecutÃ³ nada.

### `GET /api/router/status`

```json
{
  "enabled": true,
  "url": "http://127.0.0.1:7767",
  "reachable": true,
  "sidecar": {"ok": true, "backend": "llama", "model": "â€¦", "contract": "1"}
}
```

Si `ONI_JUNIPERUS_URL` estÃ¡ vacÃ­o: `enabled: false`, `reachable: false`.

## CLI

```powershell
oni route "mergeÃ¡ la tarea 3f2a1b9c"
oni route --project landing "agregÃ¡ una secciÃ³n de precios"
oni route --task 3f2a1b9c --json "estado"
```

El CLI resuelve el proyecto por id/nombre y la tarea por id/prefijo, y llama al
daemon. La salida incluye `preview only; nothing was executed`.

## Fallback determinÃ­stico

Vive en `oni-daemon/src/router.rs`. Es conservador:

- Detecta merge/retry/cancel/discard/query/list/help/spawn por palabras clave
  bilingÃ¼es.
- Resuelve tarea por id corto, id completo, tÃ­tulo o selecciÃ³n.
- Resuelve proyecto por nombre o selecciÃ³n (si hay un solo proyecto, lo elige).
- Si no puede, devuelve `unknown` + `needs_human`.
- Si el pedido menciona `main`/`master`/`rama principal`, fuerza worktree
  aislado y `needs_human`.

No pretende superar al SLM; es el piso de disponibilidad. En el test set rinde
~74% de intent accuracy.

## Rollout

1. **Ahora â€” preview**: `POST /api/router/route` + `oni route`. Nada se ejecuta.
2. **Sombra**: la PWA muestra la decisiÃ³n junto al formulario de nueva tarea y
   registra si el humano la corrige. El formulario sigue mandando.
3. **Sugerencia**: la PWA prellena proyecto, tÃ­tulo, agente y criterio con la
   decisiÃ³n; el humano confirma.
4. **AcciÃ³n con confirmaciÃ³n**: para merge/stop/retry/discard, un botÃ³n
   â€œAplicarâ€ con confirmaciÃ³n explÃ­cita. Nunca auto-merge.

El paso 4 no estÃ¡ implementado a propÃ³sito: la decisiÃ³n del router no es una
autorizaciÃ³n.

## Presupuesto de latencia

| Componente | Objetivo |
|---|---|
| Sidecar (Qwen 0.5B Q4, CPU) | 300â€“1200 ms |
| Sidecar (GPU) | 50â€“250 ms |
| Timeout del daemon | 1500 ms |
| Fallback | < 1 ms |

Si el sidecar tarda mÃ¡s que el timeout, el usuario ve el fallback y `degraded:
true`; no se bloquea la PWA.

## Privacidad y seguridad

- El sidecar bindea `127.0.0.1` por defecto y no tiene CORS.
- El contexto no incluye paths absolutos, diffs ni logs.
- Si apuntÃ¡s `ONI_JUNIPERUS_URL` a un host remoto, el texto del usuario y el
  contexto salen de la mÃ¡quina. PreferÃ­ localhost o una red privada.
- El token del sidecar es opcional; en LAN, usalo.
- El daemon no expone el sidecar a internet.
- La decisiÃ³n se valida dos veces (Python y Rust). `main_direct` no puede
  atravesar ninguna capa.
- El router no crea worktrees, no commitea, no mergea y no mata procesos.

## Troubleshooting

| SÃ­ntoma | Causa |
|---|---|
| `source: fallback`, `degraded: true` | Sidecar apagado, timeout o JSON invÃ¡lido. MirÃ¡ `routing.error`. |
| `source: juniperus` pero `intent: unknown` | El modelo no entendiÃ³; no es un fallo tÃ©cnico. |
| `404` en `/api/router/route` | El daemon es viejo; recompilÃ¡ con este branch. |
| `401` del sidecar | `JUNIPERUS_TOKEN` y `ONI_JUNIPERUS_TOKEN` no coinciden. |
| Latencia alta en CPU | BajÃ¡ `n_ctx`, usÃ¡ Q4_K_M y `--threads` igual a nÃºcleos fÃ­sicos. |
| El sidecar no carga el GGUF | `llama-cpp-python` no instalado (`uv sync --extra llm`) o path mal. |
