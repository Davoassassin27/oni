# ONI 鬼

> **Tu máquina trabaja. Tu teléfono supervisa.**

Oni es un **Agentic Development Environment** headless: un daemon local escrito
en Rust que ejecuta agentes de código en worktrees aislados, verifica su trabajo
y te deja revisar, reintentar, cortar o mergear desde una PWA pensada para una
mano.

- **Daemon:** Rust + Axum + Tokio.
- **Aislamiento:** un worktree y una rama `oni/<slug>` por tarea.
- **Loop:** Act → Verify → retry acotado. Nada escribe en `main` hasta que vos
  lo apruebes.
- **Interfaz:** React + Vite + Tailwind, mobile-first, instalable como PWA.
- **Router:** Juniperus, un SLM local <1B opcional que convierte lenguaje
  natural en intenciones validadas. Si no está, el router determinístico
  responde igual. Ver [`juniperus/README.md`](juniperus/README.md).
- **Marca:** Shiro Hannya, sumi / hueso / kintsugi / vermilion. Ver
  [`docs/BRAND.md`](docs/BRAND.md).

**Estado:** MVP funcional (M0–M3). El flujo completo — proyecto, worktree,
agente, verificación, review, merge y discard — corre end-to-end. Le faltan
pulido, tests de integración y las piezas marcadas en
[`docs/MVP.md`](docs/MVP.md).

---

## Qué ya funciona

- Registrar un repo local con rama base, comando de verificación y preset de
  agente.
- Crear una tarea desde la PWA; Oni crea un worktree aislado.
- Ejecutar OpenCode, Claude Code o un comando custom headless.
- Streamear el log en vivo por SSE y persistirlo a disco.
- Correr la verificación después de cada intento, reinyectar el error y
  reintentar hasta el límite.
- Ver diff stat, archivos cambiados y el **patch unificado** con hunks.
- Editar y borrar proyectos; borrar mantiene el historial de tareas.
- Verificación protegida: el verificador se restaura desde la rama base y los
  cambios en tests/CI/config requieren aceptación humana antes del merge.
- Stop real (mata el árbol de procesos), retry, discard y merge `--no-ff`.
- Persistencia de estado entre reinicios; las tareas interrumpidas se marcan
  como `failed` en vez de quedar colgadas.
- PWA instalable, dark-first, con reconexión y token opcional para LAN.
- La PWA queda embebida en el binario: no hace falta `dist/` en runtime.
- CLI `oni` para health, proyectos, tareas, dispatch, watch, log, patch, merge,
  stop y discard.
- TUI en Ratatui con lista, detalle, log, patch y acciones; intro opcional con
  TerminalTextEffects.

### Smoke test automatizado

Con el daemon corriendo:

```bash
npm run smoke            # agente fake, sin tokens
npm run smoke:opencode   # OpenCode real, usa el modelo configurado en OpenCode
```

`smoke:opencode` corre OpenCode con `--auto` (headless, sin aprobaciones
interactivas). Para forzar un modelo sin tocar la config global del CLI, arrancá
el daemon con `ONI_OPENCODE_MODEL=<provider/model>`:

```powershell
$env:ONI_OPENCODE_MODEL="opencode-go/beam-free"
.\oni-daemon\target\debug\oni-daemon.exe
```

El script crea un repo temporal, registra el proyecto, lanza una tarea, espera
`review`, valida el patch, mergea y comprueba que el archivo llegó a `main`.
Si falla, deja el repo y el log para inspeccionar.

En GitHub Actions, `.github/workflows/ci.yml` corre formato, tests, build de la
PWA, este smoke con agente fake en Linux y los tests/dataset offline de
Juniperus; no consume tokens de modelo.

### Dogfood

Primera corrida real con tres tareas sobre una landing estática: pricing, theme
toggle y formulario de contacto. Resultados y hallazgos en
[`docs/DOGFOOD.md`](docs/DOGFOOD.md).

## Inicio rápido

Requisitos: **Rust 1.88+**, **Node 22+**, **git** y al menos un agente CLI en el
`PATH` (`opencode` o `claude`).

```bash
npm install
npm run build
cargo run --manifest-path oni-daemon/Cargo.toml
```

Abrí <http://127.0.0.1:7766>. Los datos viven en `~/.oni` (en Windows,
`C:\Users\<vos>\.oni`).

### Binario único

La PWA se compila dentro del daemon, así que un solo ejecutable sirve la API y
la app; no necesitás `clients/pwa/dist` en runtime:

```bash
npm run build                                      # genera dist/ antes de compilar
cargo build --release --manifest-path oni-daemon/Cargo.toml
.\oni-daemon\target\release\oni-daemon.exe
```

`ONI_PWA_DIR` sirve para sobreescribir eso y servir un build desde disco.

### CLI

El mismo build genera `oni-daemon` y `oni`. El CLI habla con el daemon por HTTP:

```bash
cargo build --manifest-path oni-daemon/Cargo.toml

.\oni-daemon\target\debug\oni.exe health
.\oni-daemon\target\debug\oni.exe projects
.\oni-daemon\target\debug\oni.exe tasks
.\oni-daemon\target\debug\oni.exe task <id>
.\oni-daemon\target\debug\oni.exe log <id> --tail 40
.\oni-daemon\target\debug\oni.exe new --project <proyecto> --watch "agregar FAQ"
.\oni-daemon\target\debug\oni.exe patch <id> --stat
.\oni-daemon\target\debug\oni.exe accept <id>
.\oni-daemon\target\debug\oni.exe merge <id>
.\oni-daemon\target\debug\oni.exe stop <id>
.\oni-daemon\target\debug\oni.exe discard <id>
.\oni-daemon\target\debug\oni.exe route "mergeá la tarea <id>"
```

Acepta `--url` o `ONI_URL`, `--token` o `ONI_TOKEN`, y `--json` para scripting.
Los IDs de tarea se pueden abreviar a 8 caracteres. `accept` solo hace falta
cuando la tarea quedó marcada con cambios sensibles en la verificación.

### TUI

La TUI es la interfaz principal de terminal. Se abre con `oni` sin subcomando,
o explícitamente con `oni tui`:

```bash
cargo build --manifest-path oni-daemon/Cargo.toml
.\oni-daemon\target\debug\oni.exe
```

Para tener el comando en el PATH:

```bash
cargo install --path oni-daemon --bin oni --locked
oni
```

Muestra la lista de tareas, el detalle, el log en vivo, el diff y permite
merge, discard, stop, retry y aceptar cambios de verificación. Teclas: `?` para
la ayuda completa. `oni tui --no-intro` saltea la animación.

El intro y el cierre usan **TerminalTextEffects** con una hannya ASCII blanca y
el wordmark ONI:

```bash
uv tool install terminaltexteffects
# o: pipx install terminaltexteffects
```

Sin TTE la TUI funciona igual. `--no-intro` o `ONI_TUI_INTRO=0` lo desactivan;
`ONI_TTE_BIN` permite apuntar a otro binario.

También existe el binario `oni-tui` si querés abrir la TUI sin pasar por el CLI.

Guía completa de teclas y layout: [`docs/TUI.md`](docs/TUI.md).

### Router opcional (Juniperus)

Juniperus vive en [`juniperus/`](juniperus/README.md) como un sidecar local. El
daemon lo usa si `ONI_JUNIPERUS_URL` está definido; si no, cae al router
determinístico en Rust. La decisión es sólo un **preview**: no ejecuta nada.

```powershell
# sidecar sin modelo (reglas), para probar la integración
cd juniperus
uv run juniperus serve --backend mock --port 7767

# en otra terminal
$env:ONI_JUNIPERUS_URL="http://127.0.0.1:7767"
.\oni-daemon\target\debug\oni-daemon.exe

# preview de una decisión
.\oni-daemon\target\debug\oni.exe route "mergeá la tarea 3f2a1b9c"
```

Pipeline del modelo (dataset, LoRA, GGUF, bench) en
[`juniperus/docs/PIPELINE.md`](juniperus/docs/PIPELINE.md). Contrato en
[`juniperus/docs/CONTRACT.md`](juniperus/docs/CONTRACT.md).

### Desarrollo

Dos terminales:

```bash
# terminal 1 — daemon
cargo run --manifest-path oni-daemon/Cargo.toml

# terminal 2 — PWA con HMR y proxy a /api
npm run dev
```

Vite queda en <http://127.0.0.1:5173> y proxya `/api` al daemon en `:7766`.

### Primer uso

1. **Projects → Add project**: path absoluto, rama base, comando de
   verificación y agente.
2. **Tasks → New task**: título, prompt, criterio de aceptación. Oni crea el
   worktree y arranca.
3. Mirá el log en vivo. Cuando el estado pasa a **Review**, revisá el diff.
4. **Merge** (exige repo limpio y rama base chequeada), **Retry** o
   **Discard**.

## Presets de agente

| Preset | Comando | Notas |
|---|---|---|
| OpenCode | `opencode run <prompt>` | Recomendado para el primer dogfood. |
| Claude Code | `claude -p <prompt> --permission-mode acceptEdits` | Puede rechazar tools que requieran aprobación; revisá el log. |
| Custom | shell command | Corre en el worktree y recibe el prompt por **stdin**. |

Oni está diseñado para agentes **headless**: no espera una TUI interactiva. Si
tu agente pide permisos, configurá su modo no interactivo o usá un wrapper. El
aislamiento es un worktree, no una sandbox de seguridad.

## Cómo funciona

```text
PWA ──REST + SSE──> oni-daemon ──spawn──> agente CLI
                       │
                       ├── git worktree add -b oni/<slug>
                       ├── stream log + persist
                       ├── verify command (retry acotado)
                       ├── commit de lo que dejó el agente
                       └── review → merge / discard
```

Estados:

```text
queued → acting → verifying → review → merged
            │         │          └──→ discarded
            │         └──fail──→ retrying → acting
            └──stop───────────→ stopped
```

La rama base nunca se toca hasta el merge. El merge exige que el repo esté
limpio y que la rama base esté chequeada; si hay conflicto, se aborta y la tarea
queda en review.

## Seguridad

- El daemon escucha en `127.0.0.1` por defecto.
- Para LAN: `ONI_HOST=0.0.0.0` + `ONI_TOKEN=<secreto>` y abrí la IP de la
  máquina desde el teléfono. La PWA guarda el token en `localStorage`.
- No hay CORS: la API solo está pensada para el mismo origen. En desarrollo,
  Vite proxya `/api`.
- **No expongas el daemon a internet.** Ejecuta comandos arbitrarios por diseño.
  Si necesitás acceso remoto, usá Tailscale o una VPN.
- Instalar la PWA en el teléfono requiere **HTTPS** (service worker). HTTP plano
  funciona como web app; para instalar, usá Tailscale Serve o un proxy TLS.
- Los comandos de agente y de verificación son code execution. Conectá solo
  repos que controlás.
- La verificación restaura desde la rama base el archivo del verificador
  referenciado por el acceptance command. Los cambios en tests, CI, lockfiles o
  configuración se muestran como sensibles y bloquean el merge hasta que un
  humano los acepta explícitamente.

## Configuración

| Variable | Default | Descripción |
|---|---|---|
| `ONI_HOST` | `127.0.0.1` | Interfaz de bind. |
| `ONI_PORT` | `7766` | Puerto HTTP. |
| `ONI_TOKEN` | vacío | Si está, protege `/api` con bearer token. |
| `ONI_DATA_DIR` | `~/.oni` | Estado, logs y worktrees. |
| `ONI_PWA_DIR` | vacío | Sirve la PWA desde disco; vacío usa la PWA embebida. |
| `ONI_AGENT_TIMEOUT_SECS` | `1800` | Timeout por intento de agente. |
| `ONI_VERIFY_TIMEOUT_SECS` | `600` | Timeout de la verificación. |
| `ONI_OPENCODE_MODEL` | vacío | `provider/model` opcional para `opencode run --model`. |
| `ONI_OPENCODE_BIN` | autodetectado | Ruta explícita al binario de OpenCode. |
| `ONI_JUNIPERUS_URL` | vacío | Sidecar Juniperus; vacío usa el fallback determinístico. |
| `ONI_JUNIPERUS_TOKEN` | vacío | Bearer token opcional para el sidecar. |
| `ONI_JUNIPERUS_TIMEOUT_MS` | `1500` | Timeout del sidecar antes de caer al fallback. |
| `ONI_LOG` | `oni_daemon=info,tower_http=info` | Filtro de `tracing`. |

## API

```text
GET    /api/health
GET    /api/projects          POST /api/projects
PUT    /api/projects/:id      DELETE /api/projects/:id
GET    /api/tasks             POST /api/tasks
GET    /api/tasks/:id
GET    /api/tasks/:id/log
GET    /api/tasks/:id/diff
GET    /api/tasks/:id/patch
POST   /api/tasks/:id/stop
POST   /api/tasks/:id/retry
POST   /api/tasks/:id/merge
POST   /api/tasks/:id/accept-verification
POST   /api/tasks/:id/discard
POST   /api/router/route      (preview de Juniperus, no ejecuta)
GET    /api/router/status
GET    /api/events            (SSE)
```

El contrato completo y el alcance del MVP están en
[`docs/MVP.md`](docs/MVP.md).

## Estructura

```text
oni/
├── docs/                 MVP + sistema de marca
├── oni-daemon/           daemon Rust (Axum, git, runner, router fallback)
├── clients/pwa/          PWA React + Vite + Tailwind
├── juniperus/            SLM router (contrato, dataset, LoRA, GGUF, sidecar)
├── images/               brand book y assets originales
├── package.json          workspace raíz
└── README.md
```

### Brand y fuentes

- **Inter** (UI) + **JetBrains Mono** (código/metadatos), ambas **SIL OFL 1.1**,
  self-hosted en `clients/pwa/public/fonts/`.
- Paleta: sumi `#0B0B0C`, hueso `#F3EFE6`, kintsugi `#C9A227`, ceniza
  `#8B857A`, vermilion `#E23B2E`.
- El wordmark está vectorizado: no se reescribe con una fuente.
- La Hannya deriva de una referencia de Noun Project. **Antes de uso comercial
  o registro marcario hay que licenciarla o redibujarla.** Ver
  [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## Roadmap

- **M4 — Phone-ready:** probar LAN + token + instalación real en iOS/Android.
- **M5 — Dogfood:** 10 tareas reales, medir intervenciones y merges limpios.
- **Después:** push notifications, múltiples verificaciones, PTY interactiva,
  edición/borrado de proyectos, SQLite, CI.

Juniperus ya vive en `juniperus/` como router local opcional en modo preview
(con fallback determinístico y sin ejecutar acciones). Siguen fuera del MVP:
multiagente/DAGs, edición de código desde el teléfono, cuentas/cloud y
auto-merge. Ver [`juniperus/docs/ONI_INTEGRATION.md`](juniperus/docs/ONI_INTEGRATION.md).

## Licencia

Código: decisión pendiente (**MIT sugerida**). Marca, logo y brand book: todos
los derechos reservados. Fuentes y assets de terceros: ver
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
