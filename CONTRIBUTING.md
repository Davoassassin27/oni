# Contributing to Oni

Oni is an early, local-first project. Small, focused changes with a clear
problem statement are the easiest to review.

## Before you start

- Read `docs/MVP.md` for the scope and the state machine.
- Read `AGENTS.md` if you are changing the daemon or the task runner.
- For a large feature, open an issue first so we can agree on the shape.

## Development

```bash
npm install
npm run build
cargo test --manifest-path oni-daemon/Cargo.toml
cargo test --manifest-path oni-tui/Cargo.toml
```

Juniperus is a Python subproject:

```bash
cd juniperus
uv sync --extra dev
uv run pytest
uv run ruff check .
uv run ruff format --check .
```

Run the daemon and the TUI in separate terminals:

```bash
cargo run --manifest-path oni-daemon/Cargo.toml
cargo install --path oni-daemon --bin oni --locked
oni
```

## Pull requests

- Keep commits focused.
- Sign off your commits with the Developer Certificate of Origin:

  ```bash
  git commit -s -m "your message"
  ```

- Add or update tests for behavior changes.
- Do not commit secrets, `.env` files, model weights, or generated data.
- Describe what you ran to verify the change.

## Security

Please do not open a public issue for a vulnerability. See `SECURITY.md`.
