# AGENTS.md — working on ONI

## What this is

Oni is a local **Agentic Development Environment**: a Rust daemon that runs CLI
coding agents in isolated git worktrees, verifies their work, and exposes a
mobile-first PWA for supervision.

Read first:

- `docs/MVP.md` — scope, state machine, guardrails, roadmap.
- `docs/BRAND.md` — palette, type, logo rules, licensing caveat.
- `README.md` — setup, config, API.

## Commands

```bash
# daemon (port 7766, data in ~/.oni)
cargo run --manifest-path oni-daemon/Cargo.toml

# PWA dev server (proxy /api -> 7766)
npm run dev

# production build
npm run build

# checks
cargo check --manifest-path oni-daemon/Cargo.toml
cargo test --manifest-path oni-daemon/Cargo.toml
npm run build --workspace @oni/pwa
```

The PWA is embedded into the daemon at compile time, so run `npm run build`
before `cargo build` when `clients/pwa/dist` changed. `ONI_PWA_DIR` overrides
the embed and serves that directory instead.

On Windows, `cargo run` fails to replace the binary while a daemon is running:
stop the process first or run `oni-daemon\target\debug\oni-daemon.exe` directly.

## Architecture map

| Path | Responsibility |
|---|---|
| `oni-daemon/src/api.rs` | REST routes, SSE, token middleware. |
| `oni-daemon/src/pwa.rs` | Embedded PWA assets, disk override, SPA fallback. |
| `oni-daemon/src/runner.rs` | Task state machine, agent spawn, log stream, verify + retry. |
| `oni-daemon/src/git.rs` | Worktree, merge, commit, diff, slug/branch. |
| `oni-daemon/src/store.rs` | JSON state persistence; interrupted-task recovery. |
| `oni-daemon/src/state.rs` | Shared state, runner handles, process-tree kill. |
| `clients/pwa/src/pages/` | Tasks, task detail, projects, settings. |
| `clients/pwa/src/components/ui.tsx` | Brand primitives: buttons, pills, sheets, toasts. |
| `clients/pwa/src/lib/api.ts` | Typed API client + token storage. |
| `clients/pwa/src/lib/useLiveEvents.ts` | SSE connection → React Query cache + log bus. |

## Invariants — do not break

1. **No agent writes to the base branch.** Every task gets a worktree and an
   `oni/<slug>` branch. Merge is an explicit human action.
2. **Verification is mandatory before review.** The loop is bounded
   (`maxIterations`, agent timeout, verify timeout).
3. **Stop must kill the process tree**, not just the shell. On Windows use
   `taskkill /T /F`; on Unix signal the process.
4. **No CORS.** The PWA is same-origin. Adding permissive CORS lets any web page
   drive a local daemon that executes commands.
5. **Secrets stay out of the repo.** Token lives in env and browser
   `localStorage`; never log it. SSE carries it as a query token by design.
6. **Brand:** sumi/hueso/kintsugi/ceniza/vermilion only. No gradients, glows,
   shadows. Gold is accent; vermilion is semantic (error/stop/destructive).
   Never render the wordmark with a font.
7. **Hannya licensing:** the symbol derives from a Noun Project reference. Do
   not claim commercial/trademark rights without licensing or a manual redraw.
8. **Mobile-first:** touch targets ≥ 44 px, one-hand actions, no hover-only
   affordances.

## Testing a change

- Unit tests: `cargo test`.
- UI: build, run daemon, then load <http://127.0.0.1:7766> at 390×844 and
  desktop width. Check console/network errors.
- E2E smoke currently needs a local test repo with a fake agent; the daemon's
  own `cargo test` covers git parsing and branch naming.

## Known gaps (v0.1)

- OpenCode is exercised end-to-end (smoke test, Windows npm shim resolution,
  `--auto`); Claude preset is implemented but not exercised end-to-end yet.
- JSON persistence is best-effort atomic; crash during save can lose the last
  write. Migrate to SQLite when attempt history matters.
- No push notifications, no interactive PTY, no Web Push yet. CI runs checks and
  a fake-agent smoke on Linux; no Windows/macOS matrix yet.
- Patch review is capped at 512 KB and only available while the worktree exists.
- Verification runs inside the agent's worktree, so an agent can edit the
  verify script or test config. Treat green checks as a signal, review those
  files, and add a protected verify copy or tamper check later.
- PWA install over LAN requires HTTPS (Tailscale Serve/TLS proxy).
