# Aislamiento y sandbox

## Recomendación corta

- **Oni (daemon/controlador) corre en el host**, porque necesita manejar git,
  worktrees, procesos y la PWA.
- **El agente corre en un sandbox por tarea** (contenedor, WSL2 o VM), con solo
  el worktree montado y el resto del host inaccesible.
- **Juniperus corre como sidecar aislado**, sin filesystem de escritura, sin
  red saliente y con salida validada. Nunca ejecuta una decisión.
- El modo `host` (agente con tus permisos) queda como opt-in explícito para
  desarrollo local, no como default para agentes no confiables.

## Por qué

El worktree de git aísla **commits y branches**, no el proceso. Un agente puede
leer `~/.ssh`, escribir en `C:\Windows\...` o crear `../fuera-del-repo` con una
ruta absoluta. Lo verificamos con una sonda controlada: Oni vio un diff vacío
mientras los archivos externos existían.

Sanitizar el prompt no alcanza: el agente puede descubrir rutas, usar symlinks,
variables de entorno o binarios del sistema. La única frontera confiable es el
sistema operativo.

## Modelo de amenaza

- Agente comprometido o prompt injection desde un repo/issue.
- Modelo local o router que devuelve una decisión maliciosa.
- Dependencias del repo que ejecutan código durante tests/build.
- Fuga de secretos del host (SSH, cloud, `.env`, tokens).

## Diseño propuesto

```text
host
├─ oni-daemon            controlador: git, worktrees, API, PWA
├─ juniperus sidecar     contenedor sellado, solo modelo/reglas
└─ task sandbox          contenedor/WSL/VM efímero por tarea
   ├─ /workspace         bind mount del worktree (rw)
   ├─ root               read-only
   ├─ usuario            no-root, sin sudo
   ├─ red                none o allowlist
   ├─ límites            CPU, memoria, pids, tiempo
   └─ secretos           ninguno del host
```

Reglas:

1. El sandbox recibe **solo** el worktree. Nada de `$HOME`, `~/.ssh`, Docker
   socket, claves cloud ni el repo base.
2. Root filesystem read-only; `/tmp` efímero; worktree rw.
3. Red `none` por defecto. Si el agente necesita un proveedor, allowlist de
   dominios y proxy con auditoría.
4. Límites de CPU/memoria/pids/tiempo; se destruye al terminar.
5. La verificación corre en el mismo sandbox o en uno limpio con el verificador
   restaurado desde la rama base. El agente no puede editar el verificador.
6. El merge sigue siendo humano y ocurre en el host.

## Juniperus

Juniperus es una función pura: texto + contexto → JSON. No necesita escribir
archivos ni salir a internet.

- Contenedor separado, imagen read-only, sin volúmenes salvo el modelo.
- `--network none` o UDS local; si hay red, solo loopback.
- Token opcional, timeouts y límite de tamaño de respuesta.
- Validación estricta del contrato v1; el daemon re-sanitiza.
- `main_direct` siempre se degrada a worktree aislado.
- La decisión es **preview**; el daemon no la ejecuta.

## Opciones por plataforma

| Plataforma | Opción recomendada |
|---|---|
| Linux | Podman/Docker rootless + `--network=none`, `--read-only`, `--cap-drop=ALL`, `--security-opt no-new-privileges`, límites. |
| macOS | Lima/Colima o contenedores; VM ligera si se necesita aislamiento fuerte. |
| Windows | WSL2 (rootless Podman/Docker) o Windows Sandbox; evitar montar `C:\Users` completo. |

Ejemplo Linux:

```bash
podman run --rm \
  --network=none \
  --read-only \
  --tmpfs /tmp:rw,size=256m \
  --cap-drop=ALL \
  --security-opt no-new-privileges \
  --user 1000:1000 \
  --memory=2g --cpus=2 --pids-limit=256 \
  -v "$WORKTREE:/workspace:rw" \
  -w /workspace \
  oni-agent:opencode \
  opencode run --auto "..."
```

## Política en Oni

Propuesta de variables:

```text
ONI_SANDBOX=auto|required|off     # default: auto
ONI_SANDBOX_ENGINE=podman|docker|wsl|host
ONI_SANDBOX_NETWORK=none|allowlist
ONI_SANDBOX_IMAGE=oni-agent:opencode
```

- `required`: si no hay motor de sandbox, la tarea falla antes de ejecutar.
- `auto`: usa sandbox si está disponible; registra el motor en la tarea.
- `off`: modo host explícito para desarrollo; mostrarlo en la UI.

La tarea debería guardar `executor`, `sandboxImage`, `network` y el resultado
del sandbox para auditoría.

## Roadmap

1. Documentar la política y agregar `ONI_SANDBOX` (sin motor todavía).
2. Implementar executor `wsl`/`docker` para OpenCode y custom.
3. Verificación en sandbox limpio con verificador restaurado.
4. Contenedor para Juniperus, sin red, con socket local.
5. Smoke E2E en CI con el executor sandbox.
6. Endurecer: allowlist de red, secretos, límites y auditoría.

## Lo que no hacer

- Confiar en “no uses rutas absolutas” en el prompt.
- Montar el repo base o `$HOME` “solo para que funcione”.
- Dar el socket de Docker al agente: equivale a root en el host.
- Dejar que Juniperus ejecute acciones o mergee.
- Presentar el worktree como sandbox de seguridad.
