# ONI TUI

Cliente de terminal para supervisar tareas sin abrir la PWA. Está hecho con
**Ratatui** y habla con el daemon por la misma API REST.

```bash
cargo build --manifest-path oni-tui/Cargo.toml
.\oni-tui\target\debug\oni-tui.exe
```

`npm run tui` hace lo mismo. Acepta `--url`/`ONI_URL`, `--token`/`ONI_TOKEN`,
`--interval` (segundos de refresh) y `--no-intro`.

## Layout

```text
◆ ONI  v0.1.0  ● daemon   ·  0 active  1 review  ? help
project all projects   filter ALL   12 tasks · 1 project
──────────────────────────────────────────────────────────────
TASKS                         │ TASK                        REVIEW
▌ 0340ad2d REVIEW    1/3 !    │ Verification guard demo
  Verification guard demo     │ oni-demo-page · Custom · 1/3
  afac645b MERGED    1/2      │ branch  oni/...
  Add contact form            │ ! verification files changed
                              │ merge blocked · press a
                              │ changes 2 files +6 −0
                              │ log · ...
──────────────────────────────────────────────────────────────
j/k move · Tab filter · p project · r refresh · v patch · m merge ...
```

## Teclas

| Tecla | Acción |
|---|---|
| `j` / `k`, `↑` / `↓` | Mover selección. |
| `g` / `G`, Home/End | Primera / última tarea. |
| `Tab`, `1`–`5` | Filtrar: ALL, ACTIVE, REVIEW, DONE, FAILED. |
| `p` | Ciclar filtro de proyecto. |
| `r` | Refrescar ahora. |
| `v` | Ver el patch unificado de la tarea. |
| `J` / `K`, PgUp/PgDn | Scrollear el log. |
| `m` | Merge (con confirmación; bloqueado si hay cambios sensibles sin aceptar). |
| `d` | Discard (con confirmación). |
| `s` | Stop del agente (con confirmación). |
| `t` | Retry (con confirmación). |
| `a` | Aceptar cambios de verificación. |
| `?` | Ayuda. |
| `q` | Cerrar overlay o salir. |

## Colores

- Fondo sumi, texto hueso, acento kintsugi.
- Fail/stop/peligro en vermilion.
- El estado de cada tarea se colorea en la lista; `!` marca verificación
  sensible sin aceptar.
- Los logs resaltan `== verify` en oro, `-` en vermilion y `+` en oro.

## Intro con TTE

Si `tte` (TerminalTextEffects) está instalado, la TUI reproduce una animación
`decrypt` con el wordmark ONI antes de entrar al dashboard:

```bash
uv tool install terminaltexteffects
# o: pipx install terminaltexteffects
```

- `--no-intro` o `ONI_TUI_INTRO=0` la desactivan.
- `ONI_TTE_BIN` apunta a un binario específico.
- Si TTE no está instalado, la TUI arranca directo, sin error.

## Límites

- La TUI **supervisa**; para dispatch de tareas nuevas usá la PWA o `oni new`.
- El patch se corta a 512 KB, igual que la API.
- La verificación sensible se detecta por heurística; el review humano sigue
  siendo obligatorio.
