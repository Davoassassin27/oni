# Dogfood — 2026-11-18

Primera corrida real de Oni contra un repo de ejemplo, con OpenCode y sin
agente fake. El objetivo era ver el ciclo completo desde la PWA: dispatch,
worktree, agente, verificación, review del patch y merge.

## Setup

| Pieza | Valor |
|---|---|
| Daemon | `ONI_DATA_DIR=.oni/dogfood`, puerto `7766` |
| Repo demo | `C:\Users\davos\orca\projects\oni-demo-page` (repo git separado) |
| Agente | preset `opencode`, OpenCode v2.2.7 |
| Modelo | `opencode-go/beam-free` (gratuito) |
| Verificación | `node verify.mjs <feature>` |
| Iteraciones máximas | 2 |
| Base branch | `main` |

El repo demo es una landing estática sin dependencias: `index.html`,
`styles.css`, `app.js`, `verify.mjs` y `serve.mjs`. El baseline se commiteó
antes de despachar las tareas.

## Tareas

| # | Tarea | Verify | Intentos | Duración | Diff |
|---|---|---|---|---|---|
| 1 | Add pricing section | `node verify.mjs pricing` | 1/2 | 49 s | +145 |
| 2 | Add theme toggle | `node verify.mjs theme` | 1/2 | 69 s | +123 |
| 3 | Add contact form | `node verify.mjs contact` | 1/2 | 46 s | +238 |

Las tres llegaron a `review` en el primer intento y se mergearon con
`--no-ff` después de revisar el patch.

Commits del repo demo:

```text
7e78d2a demo: reveal content without JavaScript
434eb68 oni: merge oni/add-contact-form-afac645b
f36f24a oni: Add contact form
b124f00 oni: merge oni/add-theme-toggle-5de7ecd2
8de7a22 oni: Add theme toggle
bf4c301 oni: merge oni/add-pricing-section-5c5735fc
7b16e99 oni: Add pricing section
da52744 demo: add favicon
```

## Qué se validó

- El daemon creó un worktree y una branch `oni/<slug>` por tarea.
- OpenCode corrió headless con `--auto`, sin preguntas interactivas.
- La verificación se ejecutó en el worktree y bloqueó el pase a review.
- El patch real se pudo revisar por API y por la PWA.
- El merge dejó `main` limpio y con el contenido esperado.
- La PWA mostró el estado en vivo y el log completo de cada intento.
- El modelo gratuito resolvió las tres tareas sin reintentos.

## Hallazgos

1. **Modelo por defecto de OpenCode.** Al quitar el modelo de LM Studio, el
   default automático fue `openai/gpt-6.2-sol` vía OpenRouter, cuya key no
   tenía crédito. Se dejó `opencode-go/beam-free` como default en
   `~/.config/opencode/opencode.jsonc` (backup: `opencode.jsonc.bak-oni`).
   Oni todavía no fija modelo por tarea; eso queda para Juniperus.
2. **Retry no ejercitado.** Las tres tareas pasaron a la primera. El camino de
   fallo y reintento sigue cubierto por el test con agente fake, pero falta
   forzarlo en un repo real.
3. **Animación de reveal.** Las secciones debajo del fold no aparecían en un
   screenshot full-page porque el `IntersectionObserver` no las había visto.
   Se agregó un fallback `<noscript>` y el screenshot se toma forzando
   `is-visible`. No es un bug de Oni.
4. **Detalle mobile.** El nombre de branch se trunca en la tarjeta de la tarea.
   Conviene un botón de copiar o permitir scroll horizontal.
5. **Formulario de contacto.** El estado de éxito deshabilita el submit hasta
   recargar; es aceptable para la demo, pero se puede resetear.

## Reproducir

```powershell
# daemon
$env:ONI_DATA_DIR="C:\Users\davos\orca\projects\oni-ade\.oni\dogfood"
$env:ONI_PORT="7766"
.\oni-daemon\target\debug\oni-daemon.exe

# página demo
cd C:\Users\davos\orca\projects\oni-demo-page
node serve.mjs
```

Luego abrir <http://127.0.0.1:7766> (PWA) y <http://127.0.0.1:8080> (demo).

## Próximos pasos

- Forzar un fallo de verificación para ejercitar retry y el presupuesto de
  iteraciones en un repo real.
- Probar `stop` y `discard` con un agente colgado.
- Smoke E2E en CI con agente fake, sin tokens.
- Probar el preset de Claude Code.
- Embeber la PWA en el binario release.
