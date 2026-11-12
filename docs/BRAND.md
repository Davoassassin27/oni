# ONI — Sistema de marca para producto

Fuente de verdad: `images/30..40-ONI-brand-book-*.jpg` y los SVG en
`images/svg/`. Este documento traduce el brand book a tokens y reglas de UI.

## 1. Paleta

| Token | Nombre | Hex | Uso |
|---|---|---|---|
| `sumi` | Sumi | `#0B0B0C` | Fondo principal, tinta, texto sobre hueso. |
| `hueso` | Hueso | `#F3EFE6` | Texto principal sobre sumi, fondo claro. **No usar blanco puro.** |
| `kintsugi` | Kintsugi | `#C9A227` | Acento: grieta, línea, detalle, foco, descriptor. |
| `ceniza` | Ceniza | `#8B857A` | Texto secundario, metadatos, bordes suaves. |
| `vermilion` | Vermilion | `#E23B2E` | Solo alertas/estados (error, stop, destructivo). Nunca marca. |

Pares aprobados por el brand book:

- Hueso sobre Sumi — 17:1.
- Sumi sobre Hueso — 17:1.
- Oro sobre Sumi — 8,1:1.
- **No usar oro para texto chico sobre hueso.**
- **No usar vermilion como color de marca.** Es semántico.

### Tokens CSS

```css
:root {
  --oni-sumi: #0B0B0C;
  --oni-hueso: #F3EFE6;
  --oni-kintsugi: #C9A227;
  --oni-ceniza: #8B857A;
  --oni-vermilion: #E23B2E;

  /* derivados de UI, no de marca */
  --oni-sumi-raised: #141416;
  --oni-sumi-sunken: #080809;
  --oni-line: rgb(243 239 230 / 0.14);
  --oni-line-strong: rgb(243 239 230 / 0.28);
}
```

### Reglas

- El dorado es acento, no fondo de bloques grandes.
- El vermilion aparece solo cuando algo requiere atención: error, stop,
  destructivo, desconectado.
- Sin gradientes, sin sombras, sin filtros, sin glow. La identidad es plana.
- La UI oscura usa sumi; la clara usa hueso con tinta sumi. No mezclar.

## 2. Tipografía

El brand book ya resolvió la elección: **Inter** (UI/cuerpo) + **JetBrains Mono**
(código/terminal/descriptor). Ambas son open source bajo **SIL Open Font
License 1.1** y se auto-hospedan.

| Rol | Familia | Pesos | Regla |
|---|---|---|---|
| UI y cuerpo | Inter | 400 / 500 / 600 / 700 | Titulares 600/700; cuerpo 400/500. |
| Código, terminal, metadatos | JetBrains Mono | 400 / 500 | Logs, branches, IDs, labels. |
| Descriptor | JetBrains Mono | 500 | Mayúsculas, tracking +8 %, dorado. |

- **Wordmark:** está vectorizado. No se reescribe con una fuente.
- **Descriptor:** `AGENTIC DEVELOPMENT ENVIRONMENT`, mono, mayúsculas, tracking
  +8 %. No forma parte del logo.
- **Números y estados:** mono para que no bailen en vivo.

Implementación: `@fontsource-variable/inter` + `@fontsource/jetbrains-mono`,
self-hosted por Vite. Sin Google Fonts en runtime.

## 3. Símbolo

- **Shiro Hannya** (máscara blanca sobre sumi) para app icon, hero, superficies
  grandes. Mínimo 64 px.
- **Glifo de terminal** para 16–48 px: header, favicon, TUI, lockup compacto.
- La grieta es una sola y siempre dorada. No agregar grietas.
- No agregar pupilas, brillos, sombras ni degradados.
- No rotar, inclinar, estirar ni recolorear fuera de la paleta.
- No animar el logo en un header persistente.
- Animaciones respetan `prefers-reduced-motion`.

## 4. Voz

- Corta, directa, sin hype.
- En la UI: verbos concretos (`Run`, `Stop`, `Verify`, `Merge`, `Discard`).
- El estado siempre es visible. Nunca un spinner sin fase ni timestamp.
- Los errores dicen qué pasó, dónde y qué puede hacer el humano.
- No usar emojis en la UI del producto.

## 5. Aplicación a la PWA

- Fondo `sumi`; texto `hueso`; metadatos `ceniza`.
- Bordes hairline de 1 px, radio 10–14 px, densidad alta pero táctil (targets
  ≥ 44 px).
- El log es el objeto visual central: JetBrains Mono, fondo `sumi-sunken`.
- Estados:
  - `acting` / `verifying`: dorado.
  - `review`: hueso (acción disponible).
  - `failed` / `stopped`: vermilion.
  - `merged`: ceniza + check.
- La PWA usa la Hannya a 64 px en el estado vacío y el glifo a 24–32 px en el
  header.

## 6. Assets

Copiar a `clients/pwa/public/brand/` solo los necesarios:

- `01-ONI-logo-hannya-tile.svg` → app icon / empty state / hero.
- `06-ONI-icono-terminal-tile.svg` → header, favicon, compacto.
- `10-ONI-lockup-horizontal-oscuro.svg` → README, landing.
- `25-ONI-hero-horizontal.svg` → README/hero.
- PNGs `19–25` → manifest, apple-touch, favicons.

Los originales quedan en `images/` como fuente de verdad.

## 7. Licencia y autoría

El brand book (p. 10) indica:

> La Shiro Hannya es un redibujo por vectorización de la referencia de monnik
> (Noun Project): paths nuevos, cuencas vacías, recentrado y grieta kintsugi. La
> forma sigue basada en esa obra. Para uso comercial o registro marcario:
> licenciar el ícono o hacer un redibujo manual. El glifo de terminal, el
> wordmark y los lockups son originales.

**Acción antes de comercializar:** licenciar la referencia o encargar/hacer un
redibujo manual. El MVP personal no se bloquea por esto, pero el lanzamiento
público sí.
