# ONI — MVP v0.1

> **Tesis:** tu máquina trabaja; tu teléfono supervisa.
>
> Oni es un Agentic Development Environment headless: un daemon local que corre
> agentes de código en worktrees aislados, verifica su trabajo y te deja aprobar,
> reintentar o cortar desde una PWA pensada para una mano.

## 0. Estado de implementación (v0.1)

El **walking skeleton end-to-end ya funciona** y fue probado en Windows con
OpenCode real y un modelo gratuito:

- proyecto → tarea → worktree → agente → verify → retry automático → review →
  merge / discard;
- stop real (mata el árbol de procesos); el worktree queda;
- dispatch y merge desde la PWA mobile;
- revisión con patch unificado y edición/borrado de proyectos;
- deep links del SPA, SSE, persistencia entre reinicios y token de API;
- dogfood con 3 tareas reales sobre una landing (pricing, theme, contact) y
  pruebas forzadas de retry, stop y discard;
- CI en Linux con formato, tests, build de la PWA y smoke con agente fake;
- PWA embebida en el binario: un solo ejecutable sirve API y app.
- CLI `oni` como cliente de la API: health, projects, tasks, task, log, new,
  watch, patch, stop, retry, merge y discard.
- TUI en Ratatui: lista, detalle, log en vivo, patch y acciones; intro opcional
  con TerminalTextEffects.

Pendiente antes de llamarlo MVP cerrado:

- instalación real de la PWA en un teléfono (requiere HTTPS/Tailscale) y Web
  Push;
- dogfood de 10 tareas y métricas;
- matriz Windows/macOS y empaquetado release/instalador.

Claude Code queda implementado pero no probado end-to-end, fuera de prioridad
por ahora.

---

## 1. El problema

Hoy, para usar agentes de código en serio, tenés que quedarte frente a la
terminal: aprobar permisos, leer salidas, decidir si el parche sirve, correr los
tests y mergear. Eso convierte al agente en una tarea de escritorio.

Oni separa **ejecución** de **supervisión**:

- La máquina (laptop, mini PC, homelab) ejecuta.
- El teléfono (o cualquier navegador) supervisa.
- El daemon garantiza aislamiento, verificación y límites.

### Usuario inicial

Un dev solo, cómodo con git y CLI, que ya usa OpenCode / Claude Code /
Aider y quiere:

1. Lanzar una tarea desde el teléfono sin abrir una terminal.
2. Ver que el agente avanza sin mirar cada token.
3. Recibir un resultado **verificado**, no una promesa.
4. Mergear o descartar con un toque, sin romper `main`.

---

## 2. La hipótesis a validar

> Un dev puede delegar una tarea real a un agente y completarla desde el
> teléfono en menos de 3 intervenciones, con el repositorio intacto.

Si esto no se cumple, ninguna cantidad de features (router local, DAGs,
notificaciones, terminal interactiva) salva al producto.

### Métricas de éxito del MVP

| Métrica | Objetivo |
|---|---|
| Tareas reales completadas | 10 |
| Tareas mergeadas sin tocar la terminal | ≥ 8 |
| Intervenciones humanas por tarea | ≤ 3 |
| Repos corruptos o commits en `main` por accidente | 0 |
| Tiempo desde "listo" hasta revisión en el teléfono | < 10 s |
| Recuperación tras cerrar la PWA / perder red | 100 % (estado persistido) |

---

## 3. Flujo principal (el único que importa)

1. **Conectar repo** — indicás path, rama base, comando de verificación y preset
   de agente. Oni valida que el repo esté limpio.
2. **Crear tarea** — título + prompt + criterio de aceptación (el comando de
   verificación). Oni crea `oni/<slug>` en un worktree aparte.
3. **Act** — el agente corre headless en ese worktree; la PWA muestra estado y
   log en vivo.
4. **Verify** — al terminar, Oni corre el comando de verificación. Si falla,
   reinyecta el error al agente y reintenta (máx. 3). Si pasa, commitea lo que
   quedó sucio y pasa a **Review**.
5. **Review** — la PWA muestra archivos cambiados, diff stat y salida de tests.
   Acciones: **Merge**, **Retry**, **Discard**, **Stop**.
6. **Merge** — solo si la rama base está limpia y chequeada. Oni hace
   `merge --no-ff`, marca la tarea y limpia el worktree.

**Regla de oro:** ningún agente escribe en la rama principal. Nunca.

---

## 4. Alcance

### Must (v0.1)

- [x] Daemon local en Rust + Axum, binario único.
- [x] Registro, edición y borrado de proyectos (path, base branch, verify command, agente).
- [x] Tarea con worktree aislado y branch `oni/<slug>`.
- [x] Agente headless no interactivo con presets: OpenCode, Claude Code, custom.
- [x] Streaming de log por SSE + persistencia de log en disco.
- [x] Loop Plan → Act → Verify con máximo de iteraciones y presupuesto de tiempo.
- [x] Stop real (mata el árbol de procesos).
- [x] Review con diff stat, archivos, patch unificado y salida de verificación.
- [x] Merge / Discard atómicos, con chequeo de repo limpio.
- [x] Verificación protegida: verificador restaurado desde la rama base y merge
  bloqueado si cambian tests/CI/config hasta que un humano lo acepte.
- [x] PWA mobile-first instalable, estado en vivo, reconexión.
- [x] PWA embebida en el binario; `ONI_PWA_DIR` permite servir desde disco.
- [x] Persistencia de tareas en SQLite o JSON atómico (sobrevive reinicios).
- [x] Bind a `127.0.0.1` por defecto; modo LAN explícito con token.

### Should (v0.2)

- [ ] Push notifications (Web Push) para "needs human" y "ready".
- [ ] Adjuntar imágenes/screenshots a la tarea.
- [ ] Múltiples verificaciones por proyecto (test, lint, typecheck).
- [ ] Historial de intentos con diff entre iteraciones.
- [ ] Adjuntar terminal interactiva para debugging (PTY + xterm.js).
- [ ] Exportar sesión como PR / patch.

### Won't (por ahora)

- **Juniperus en el camino crítico del MVP.** El router SLM se desarrolla en
  `feat/juniperus` y se integra después, con shadow mode y fallback. El MVP usa
  el router determinístico y no depende de que el modelo esté listo.
- **Orquestación multiagente / DAGs.** Una tarea, un agente, un worktree.
- **Terminal interactiva como camino principal.** Los agentes corren en modo
  headless; los permisos se resuelven por preset. La PTY queda para debugging.
- **Editar código desde el teléfono.** El teléfono supervisa, no reemplaza al
  editor.
- **Cuentas, cloud, multiusuario, marketplace.**
- **Tailscale embebido.** Documentamos cómo exponerlo; no lo empaquetamos.
- **Auto-merge.** El merge siempre es una acción humana explícita.

---

## 5. Máquina de estados

```text
queued
  └─> acting ──> verifying ──ok──> review ──> merged
        │            │                 └──> discarded
        │            └──fail──> retrying ──> acting (iter+1)
        │                          └──max──> failed
        ├──stop──> stopped
        └──error─> failed
```

Invariantes:

- `iteration <= max_iterations` (default 3, hard cap 6).
- Una tarea `merged`, `discarded`, `failed` o `stopped` es terminal.
- El worktree vive hasta merge/discard; se puede inspeccionar después.
- El log se escribe antes de emitir el evento (si la PWA se reconecta, no
  pierde nada).

---

## 6. Arquitectura v0.1

```text
┌─────────────────────────────┐         ┌──────────────────────────┐
│  PWA (React + Vite)         │  HTTPS  │  oni-daemon (Rust/Axum)  │
│  - feed de tareas           │◄───────►│  - REST + SSE            │
│  - detalle + log            │  token  │  - task runner           │
│  - review / merge / stop    │         │  - git worktrees         │
└─────────────────────────────┘         │  - verify + retry loop   │
                                        │  - persistencia          │
                                        └────────────┬─────────────┘
                                                     │ spawn
                                        ┌────────────▼─────────────┐
                                        │ agente CLI (headless)    │
                                        │ opencode / claude / ...  │
                                        └──────────────────────────┘
```

### Decisiones técnicas

| Área | Decisión | Por qué |
|---|---|---|
| Daemon | Rust + Axum + Tokio | Un binario, bajo consumo, streaming simple. |
| Git | CLI de git para worktrees, merge y diff; `git2` queda para v0.2 | El CLI es el ground truth y evita edge cases de libgit2. La lectura nativa llega después. |
| Procesos | `tokio::process` + kill de árbol (`taskkill /T` en Windows) | Los agentes headless no necesitan PTY. Menos superficie. |
| Persistencia | JSON con escritura atómica best-effort | Simple para v0.1; SQLite cuando haya concurrencia o consultas. |
| Frontend | React + Vite + Tailwind v4 + TanStack Query | Mobile-first, bundle chico, sin framework pesado. |
| Logs | Archivo por tarea + ring buffer en memoria | Reconnect sin pérdida; el archivo sirve para auditar. |
| Auth | Token bearer + query token para SSE | Simple y suficiente para un usuario. |
| CORS | Deshabilitado | La PWA es same-origin; evita que un sitio malicioso maneje el daemon local. |

### API v0.1

```text
GET    /api/health
GET    /api/projects
POST   /api/projects
PUT    /api/projects/:id
DELETE /api/projects/:id
GET    /api/tasks
POST   /api/tasks
GET    /api/tasks/:id
GET    /api/tasks/:id/log
GET    /api/tasks/:id/diff
GET    /api/tasks/:id/patch
POST   /api/tasks/:id/stop
POST   /api/tasks/:id/retry
POST   /api/tasks/:id/merge
POST   /api/tasks/:id/discard
GET    /api/events            (SSE, todos los eventos)
```

---

## 7. Guardrails (no negociables)

1. **Nunca `main_direct`.** Si el worktree no se pudo crear, la tarea falla.
2. **Repo limpio para mergear.** Si hay cambios locales, Oni explica y no toca
   nada.
3. **Cap de iteraciones + cap de tiempo + cap de output.** Un agente colgado no
   puede consumir la máquina indefinidamente.
4. **No hay "infinite loop".** Hay un loop acotado. El nombre es aspiracional;
   el comportamiento es finito y visible.
5. **Todo comando es code execution.** La UI lo dice al conectar un repo. El
   daemon no se expone a internet sin token.
6. **Un worktree por tarea.** Nada de dos agentes sobre el mismo directorio.
7. **El log es append-only.** La UI puede perder la conexión; el estado no.

---

## 8. Roadmap de construcción

| Hito | Entregable | Criterio de salida |
|---|---|---|
| **M0 — Fundación** | Brand tokens, fuentes, monorepo, daemon health, PWA shell | `cargo run` + `npm run dev` levantan y se ven. |
| **M1 — Walking skeleton** | Proyecto + tarea + worktree + agente fake que streamea + verify | Una tarea llega a `review` sin tocar git a mano. |
| **M2 — Agente real** | Preset OpenCode, loop de retry, stop real | 3 tareas reales completadas en un repo de prueba. |
| **M3 — Review/Merge** | Diff, merge, discard, limpieza | 3 merges limpios; `main` intacto. |
| **M3.5 — CI** | Checks + smoke con agente fake en Linux | Los jobs pasan en GitHub Actions sin tokens. |
| **M4 — Phone-ready** | PWA install, token, reconexión, LAN + HTTPS | Tarea lanzada y mergeada desde el teléfono con la PWA instalada (requiere HTTPS: Tailscale Serve o proxy TLS). |
| **M5 — Dogfood** | 10 tareas reales, métricas, fixes | Hipótesis validada o pivot. |

**Primer corte vendible:** M0–M3. **Primer corte usable:** M4. **Primer corte
público:** M5 + licencia + onboarding de 5 minutos.

---

## 9. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| El agente "pasa" tests pero rompe el diseño | Review humana obligatoria; mostrar diff, no solo verde. Verificador restaurado desde la base y cambios sensibles bloquean el merge. |
| Loop quema tokens sin avanzar | Detectar output idéntico / sin cambios en el worktree; cortar. |
| Agentes interactivos piden permisos | Presets headless documentados; nunca prometer TUI interactiva en v0.1. |
| Windows/macOS/Linux divergen | CI en Linux desde M3.5; matriz Windows/macOS y kill de proceso/paths desde M4. |
| El logo deriva de una referencia con licencia | Ver `docs/BRAND.md`: licenciar o redibujar antes de uso comercial. |
| Exponer el daemon a internet | Bind localhost por defecto; token; guía Tailscale, no túnel propio. |

---

## 10. Decisiones

Tomadas para v0.1:

1. **Persistencia:** JSON con escritura atómica best-effort. Migrar a SQLite en
   M2, cuando el historial de intentos y las consultas lo justifiquen.
2. **Git:** CLI para worktrees, merge y diff. `git2` no entró en v0.1.
3. **Idioma:** UI en inglés; docs y brand en español.
4. **Transporte:** REST + SSE, sin CORS. La PWA se sirve same-origin.
5. **Auth:** token opcional por header; query token solo para SSE.

Pendientes:

1. **Agente primario:** OpenCode (más fácil de automatizar y ya lo usás) vs
   Claude Code (mejor calidad). Recomiendo probar OpenCode primero y Claude como
   preset.
2. **Licencia del código:** MIT o Apache-2.0. La marca (logo/wordmark) va aparte,
   todos los derechos reservados.
3. **Uso comercial del símbolo:** la página 10 del brand book dice que la Hannya
   deriva de una referencia de Noun Project. Antes de vender/registrar marca hay
   que licenciarla o hacer un redibujo manual.
