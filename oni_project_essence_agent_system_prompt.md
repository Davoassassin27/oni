# ONI (鬼) — System Blueprint & AI Agent Context

## 1. Visión General del Proyecto
**Oni** es un Agentic Development Environment (ADE) headless, ultraliviano y mobile-first diseñado en **Rust**. Su objetivo principal es orquestar múltiples agentes de código (Claude Code, OpenCode, Aider) en **Git Worktrees** aislados, ofreciendo ejecución continua mediante loops autónomos de verificación ("flujos infinitos") y supervisión remota en tiempo real desde una interfaz PWA táctil.

## 2. Pilares de Arquitectura e Ingeniería

### I. Core Daemon Headless (Rust Engine)
- **Runtime:** Rust con el framework web `Axum` para API REST y transporte de WebSockets bidireccionales.
- **Gestión de Terminales (PTY):** Supervisión de subprocesos CLI mediante `portable-pty`.
- **Aislamiento Git:** Manejo atómico y directo de ramas y worktrees mediante bindings nativos de `libgit2` (`git2-rs`).
- **Inferencia de Router (Juniperus):** Integración nativa C/FFI con `llama.cpp` para la carga local del binario `.gguf` (<500 MB).

### II. Mobile-First PWA (Frontend)
- **Stack:** React + Vite + Tailwind CSS + Shadcn UI.
- **Visualización:** Emulación de consola táctil con `xterm.js` y renderizado de diffs ultraliviano con `diff2html`.
- **Modo de Distribución:** Embebido directamente dentro del binario compilado de Rust (`rust-embed` / Axum static serving) y consumido como PWA Standalone vía Tailscale P2P.

### III. Orquestación y Flujos Infinitos (Autonomous Loop Control)
- Oni ejecuta un bucle de tres fases por cada tarea: **Plan → Act → Verify**.
- Tras la ejecución de un agente CLI, el daemon ejecuta verificaciones automáticas de tests/linters (`cargo test`, `pytest`, `npm test`). Si falla, re-escribe la tarea y re-ejecuta el agente de forma autónoma.
- Pausa y notificación push en PWA ante excepciones o umbrales de re-intentos alcanzados.

---

## 3. Estructura Proyectada del Monorepo (`oni`)

```text
oni/
├── docs/                      # Documentación del proyecto y contratos
├── oni-daemon/                # Engine principal en Rust
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # Entrypoint Axum + WebSockets
│       ├── pty/               # Supervisión de procesos PTY (portable-pty)
│       ├── git/               # Gestión de Git Worktrees (git2)
│       ├── router/            # C-bindings / FFI con llama.cpp para Juniperus
│       └── loop_engine/       # Máquina de estados para los bucles autónomos
├── clients/
│   └── pwa/                   # React + Vite + Tailwind (Interrupción & Feed mobile)
├── models/                    # Ubicación del asset binario `juniperus-v1.gguf`
├── Makefile
└── README.md
```

---

## 4. Contrato de Entrada/Salida del Router (Juniperus Contract v1)

El daemon interactúa con el router esperando la siguiente estructura Pydantic / JSON Schema validada:

```json
{
  "intent": "spawn_agent | query_status | merge_worktree | cancel_task",
  "agent_target": "opencode | claude_code | aider | hermes | custom_cli",
  "model_override": "claude-3-5-sonnet | gpt-4o | ollama/qwen2.5-coder | auto",
  "worktree_strategy": "isolated_branch | existing_branch | main_direct",
  "branch_name": "string (ej: oni/refactor-auth)",
  "task_prompt": "string",
  "loop_status": "single_shot | continue_loop | completed | needs_human",
  "max_iterations": 5,
  "success_criterion": "string (ej: 'cargo test')",
  "reasoning": "string"
}
```

---

## 5. Directivas Generales para el Agente de Desarrollo (System Prompt)

Al desarrollar componentes para Oni, seguí estas reglas de diseño e implementación:

1. **Rendimiento y Cero Bloat:** Priorizá bajo consumo de memoria y CPU. No uses procesos secundarios innecesarios. El daemon de Rust debe ser instantáneo y el frontend debe mantenerse ligero.
2. **Mobile First real:** Toda vista en React debe pensarse para resoluciones táctiles de smartphone (usabilidad con una sola mano, acciones claras para `merge`, `retry` y `stop`).
3. **Aislamiento en Git:** Ningún agente modifica directamente la rama principal. Todo cambio ocurre en un `worktree` secundario que el daemon puede inspeccionar, fusionar o purgar de manera atómica.
4. **Resiliencia en el Loop:** Diseñá el manejo de errores pensando en que los agentes CLI pueden colgarse, tirar códigos de salida no nulos o entrar en bucles de parches incorrectos.