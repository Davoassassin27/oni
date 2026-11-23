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
2. **Reintento verificado en la segunda ronda.** Las tres tareas limpias
   pasaron a la primera; el camino de fallo y reinyección del error se forzó
   después con un agente custom. Ver «Segunda ronda: fallos controlados».
3. **Animación de reveal.** Las secciones debajo del fold no aparecían en un
   screenshot full-page porque el `IntersectionObserver` no las había visto.
   Se agregó un fallback `<noscript>` y el screenshot se toma forzando
   `is-visible`. No es un bug de Oni.
4. **Detalle mobile.** El nombre de branch se trunca en la tarjeta de la tarea.
   Conviene un botón de copiar o permitir scroll horizontal.
5. **Formulario de contacto.** El estado de éxito deshabilita el submit hasta
   recargar; es aceptable para la demo, pero se puede resetear.
6. **La verificación vive en el worktree.** `verify.mjs` está en el repo y el
   agente podría editarlo para hacer pasar el check. En esta corrida el prompt
   lo prohibió y el diff no lo tocó, pero no es una garantía técnica. Para el
   MVP: revisar siempre los archivos de test/CI y, más adelante, correr una copia
   protegida del verificador o marcar sus cambios como bloqueantes.

## Segunda ronda: fallos controlados

Después de los tres tasks limpios se probaron los caminos de fallo con el mismo
daemon y el mismo repo:

### Reintento por verificación

- Task `3418d9a4-87fd-46bf-859f-c8fe6e6fffb6`, agente custom, `maxIterations: 3`.
- El verificador generó un token al azar en su primera llamada, falló a propósito
  y lo devolvió en el error.
- Log de Oni:

```text
== attempt 1/3 · custom ==
agent: prompt received, 556 bytes
agent: retry token in prompt = none

== verify == 
verify: first check rejected on purpose. Add exactly "RETRY_RAL0IG" ...

== verification failed · retrying (2/3) ==

== attempt 2/3 · custom ==
agent: prompt received, 781 bytes
agent: retry token in prompt = RETRY_RAL0IG

== verify ==
verify: retry token RETRY_RAL0IG present, ok
```

- El prompt pasó de 556 a 781 bytes: el error de verificación se reinyectó y el
  agente lo usó. Estado final `review` en el intento 2/3. Se descartó después.

**Hallazgo:** con OpenCode, el agente ejecutó el verificador por su cuenta,
leyó el error y corrigió antes de que Oni hiciera su propia verificación, así
que no hubo reintento. Para probar el loop de forma determinista se usó un
agente custom que no corre el acceptance command.

### Stop con proceso colgado

- Task `07ccd3f0-0d4e-4aec-9285-d8fb956d772f`, agente custom con
  `setInterval(() => {}, 1000)`.
- Antes de stop: `node.exe` PID 17784 corriendo el script.
- Después de stop: ningún proceso con ese script; estado `stopped`; log con
  `== stopped by user ==`. El worktree quedó intacto.
- Detalle de API: `POST /stop` devolvió `acting` porque el stop es asíncrono;
  el estado se asentó en `stopped` ~2 s después vía SSE.

### Discard

- Task `64012aba-f143-46db-9bc7-7dbdd1c6726d`, en `review` con worktree.
- Tras `POST /discard`: estado `discarded`, worktree eliminado, branch borrada,
  `GET /patch` responde 409 (`the patch is no longer available...`) y el repo
  quedó limpio.

## Tercera ronda: verificación protegida

Se probó el guard con un repo temporal (`oni-guard-repo`) cuyo `verify.mjs`
exige `feature.txt`.

### Agente que implementa y debilita el verificador

- Task `478aaf02-...`, agente custom: escribe `feature.txt` y reemplaza
  `verify.mjs` por un script que siempre pasa.
- Antes de verificar, Oni restauró `verify.mjs` desde `main`:

```text
== verify · node verify.mjs ==
== oni ==
verifier verify.mjs restored from main
verify: feature.txt ok
```

- El diff final incluyó `verify.mjs`; Oni lo marcó como sensible y bloqueó el
  merge:

```text
== warn ==
verification-sensitive files changed:
  verify.mjs
merge requires explicit acceptance
```

- `oni merge` → `409 verification-sensitive files changed`.
- `oni accept <id>` → `acceptedByHuman: true`; `oni merge` → `merged`.
- En la PWA se vio la tarjeta roja “Verification files changed”, el botón Merge
  deshabilitado, y después de aceptar, la tarjeta dorada y el merge habilitado.

### Agente que solo debilita el verificador

- Task `e030b1a1-...`, agente custom que solo reemplaza `verify.mjs` sin crear
  `feature.txt`.
- Oni restauró el verificador base, que falló con `verify: missing feature.txt`;
  la tarea terminó `failed` en 1/1. Se descartó.

Esto cubre el bypass más directo: cambiar el script de verificación. La
detección de archivos sensibles (tests, CI, lockfiles, configs) sigue siendo una
heurística; un helper importado por el verificador podría no detectarse, por eso
el review humano sigue siendo obligatorio.

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

- Agregar el guard smoke a CI (hoy se probó a mano).
- Dogfood de 10 tareas y métricas.
- Instalación de la PWA en teléfono con HTTPS y Web Push.
- Matriz Windows/macOS y empaquetado release/instalador.
- Hacer que `POST /stop` devuelva el estado ya detenido (o `202`) en lugar del
  estado previo.
