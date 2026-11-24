# Juniperus Router Contract v1

El contrato es la interfaz entre el SLM, el sidecar y Oni. La fuente de verdad
ejecutable es [`schema/intent_schema.py`](../schema/intent_schema.py). El daemon
replica las reglas de seguridad en `oni-daemon/src/router.rs`.

## Salida

Una decisión es **un único objeto JSON** con estas claves, en este orden:

| Campo | Tipo | Valores / límites | Semántica |
|---|---|---|---|
| `intent` | enum | ver tabla | Qué quiere hacer el usuario. |
| `agent_target` | enum | `opencode`, `claude`, `custom`, `auto` | Agente sugerido. `auto` deja que Oni use el preset del proyecto. |
| `model_override` | string? | libre, `null` | Override opcional de modelo para el agente. Oni lo ignora en v0.1. |
| `worktree_strategy` | enum | `isolated_branch`, `existing_branch` | **Nunca** `main_direct`. |
| `branch_name` | string? | `oni/<slug>` | Sugerencia; Oni genera la rama real con el id de tarea. |
| `project_ref` | string? | nombre de proyecto del contexto | Proyecto objetivo. |
| `task_ref` | string? | id corto, id completo o título | Tarea objetivo. |
| `task_prompt` | string? | ≤ 4000 chars | Pedido original para el agente. Sólo en `spawn_agent`. |
| `loop_status` | enum | `single_shot`, `continue_loop`, `completed`, `needs_human` | Qué debería hacer el loop. |
| `max_iterations` | int | 1..6 | Cap de reintentos del agente. |
| `success_criterion` | string? | comando de verificación | Criterio de aceptación. |
| `reasoning` | string | ≤ 400 chars | Una frase corta, en el idioma del usuario. |

### Intenciones

| `intent` | Acción esperada en Oni | Requiere `task_ref` |
|---|---|---|
| `spawn_agent` | Crear tarea + worktree + agente. | No (requiere proyecto) |
| `query_status` | Listar tareas o mostrar estado/log. | No |
| `merge_worktree` | Merge humano de una tarea en review. | Sí |
| `retry_task` | Reencolar una tarea fallida/stopped. | Sí |
| `cancel_task` | Stop real del árbol de procesos. | Sí |
| `discard_task` | Descartar tarea y worktree. | Sí |
| `list_projects` | Listar proyectos registrados. | No |
| `help` | Mostrar capacidades. | No |
| `unknown` | No se entendió; pedir aclaración. | No |

### `loop_status`

No es una autorización. Es una sugerencia para el loop de Oni:

- `single_shot`: correr el agente una vez.
- `continue_loop`: reencolar (retry).
- `completed`: la acción no requiere loop de agente (merge, cancel, query).
- `needs_human`: falta un dato, hay ambigüedad o el pedido viola una regla.

Aunque `loop_status` sea `completed`, un merge sigue necesitando confirmación
humana en la PWA. El router nunca mergea solo.

## Ejemplos

### Spawn

```json
{"intent":"spawn_agent","agent_target":"opencode","model_override":null,"worktree_strategy":"isolated_branch","branch_name":"oni/agrega-pricing","project_ref":"landing","task_ref":null,"task_prompt":"agregá una sección de precios con tres planes","loop_status":"single_shot","max_iterations":3,"success_criterion":"npm run build","reasoning":"Pedido de implementación delegado a un agente."}
```

### Merge con tarea

```json
{"intent":"merge_worktree","agent_target":"auto","model_override":null,"worktree_strategy":"isolated_branch","branch_name":null,"project_ref":"landing","task_ref":"3f2a1b9c","task_prompt":null,"loop_status":"completed","max_iterations":1,"success_criterion":null,"reasoning":"Se pidió integrar una tarea."}
```

### Ambigüedad

```json
{"intent":"merge_worktree","agent_target":"auto","model_override":null,"worktree_strategy":"isolated_branch","branch_name":null,"project_ref":null,"task_ref":null,"task_prompt":null,"loop_status":"needs_human","max_iterations":1,"success_criterion":null,"reasoning":"Falta identificar la tarea."}
```

### Pedido inseguro

Usuario: *“hacelo directo en main”*.

```json
{"intent":"spawn_agent","agent_target":"opencode","model_override":null,"worktree_strategy":"isolated_branch","branch_name":"oni/hacelo-directo-en-main","project_ref":"oni","task_ref":null,"task_prompt":"hacelo directo en main","loop_status":"needs_human","max_iterations":3,"success_criterion":"cargo test","reasoning":"La rama base está prohibida; se usará un worktree aislado."}
```

La decisión conserva la intención (crear algo), pero fuerza el worktree y pide
un humano. Nunca se ejecuta sobre `main`.

## Normalización defensiva

`sanitize_payload` corre antes de la validación estricta. El modelo puede
alucinar; el contrato no se rompe.

| Entrada del modelo | Resultado |
|---|---|
| `worktree_strategy: "main_direct"` | `isolated_branch`, `needs_human` |
| `branch_name: "main/foo"` | `oni/foo` (slug) |
| `branch_name` ausente en spawn | `oni/<prompt slug>` |
| `intent: "merge"` / `"merge_task"` | `merge_worktree` |
| `agent_target: "claude_code"` / `"aider"` | `claude` / `custom` |
| `max_iterations: 99` | `6` |
| `task_ref: "null"` | `null` |
| Acción de tarea sin `task_ref` | `needs_human` |
| `intent: "unknown"` | `needs_human` |
| `success_criterion` en una query | `null` |
| Texto con `main` en un spawn | Worktree aislado + `needs_human` |

## Contexto

El daemon envía sólo lo necesario: nombres, ids, rama, estado y comando de
verificación. **No** envía paths absolutos ni diffs.

```json
{
  "projects": [
    {"id":"aaaa1111-...","name":"landing","base_branch":"main","verify_command":"npm run build","agent":"opencode"}
  ],
  "tasks": [
    {"id":"3f2a1b9c-...","title":"Add pricing section","state":"review","project":"landing","branch":"oni/add-pricing-3f2a1b9c"}
  ],
  "selected_project_id":"aaaa1111-...",
  "selected_task_id":null
}
```

El prompt renderizado tiene este formato:

```text
CONTEXT
Proyectos:
- landing [base=main] verify="npm run build" agent=opencode id=aaaa1111
Tareas recientes:
- 3f2a1b9c [review] "Add pricing section" project=landing branch=oni/add-pricing-3f2a1b9c
Selección actual: project=aaaa1111 task=-

PEDIDO:
mergeá la tarea 3f2a1b9c
```

## Respuesta del sidecar

```json
{
  "decision": { "...": "contrato v1" },
  "meta": {
    "backend": "llama",
    "model": "models/juniperus-v1.gguf",
    "latency_ms": 812.4,
    "retries": 0,
    "degraded": false,
    "error": null,
    "prompt_tokens": 734,
    "completion_tokens": 61
  },
  "raw": "..."
}
```

`degraded=true` significa que el backend falló o devolvió JSON inválido tras el
reintento. Oni trata una respuesta degradada como fallo del sidecar y usa su
fallback determinístico.

## Versionado

- `CONTRACT_VERSION = "1"` en Python y `contract: "1"` en manifests.
- Agregar una intención o un campo es breaking: subir a `v2`.
- El daemon debe ignorar campos desconocidos (serde default) y validar los
  conocidos.
- El sidecar expone `GET /schema` con el JSON Schema generado por Pydantic.

## Seguridad

1. `main_direct` no es una salida válida.
2. Toda rama empieza con `oni/`.
3. El cap de iteraciones es 6, siempre.
4. Las acciones sobre tareas exigen `task_ref`.
5. El router es **read-only**: no crea tareas, no mergea, no mata procesos.
6. El sidecar bindea `127.0.0.1` por defecto y no tiene CORS.
7. Si el sidecar no está, Oni usa el fallback; el router no es un punto único de
   falla.
