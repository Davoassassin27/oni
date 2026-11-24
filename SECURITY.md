# Security policy

## Supported versions

`v0.1.x` receives security fixes while it is the current release.

## Reporting a vulnerability

Please report vulnerabilities privately by email to **davosoler@gmail.com**.
Do not open a public issue.

Include:

- the version and platform;
- a minimal reproduction;
- the impact you believe it has;
- whether the daemon was exposed to the LAN or the internet.

## Scope and model

Oni is a local daemon that executes agent commands and verification commands by
design. It is not a sandbox:

- worktrees isolate Git branches, not the filesystem;
- agents run with the permissions of the daemon user;
- an agent can write outside its worktree using absolute paths or `..`;
- do not expose the daemon to the internet; use localhost, a token, and a VPN
  such as Tailscale if you need remote access.

The verification guard restores file-based verifiers from the base branch and
flags sensitive changes, but it is not a substitute for OS-level isolation.
See `docs/SANDBOX.md` for the recommended container/WSL/VM model.

## Out of scope

- vulnerabilities that require the daemon to be exposed publicly without a
  token or VPN;
- prompt-injection content in a repository or model output;
- agent behavior that the user explicitly approved with `--auto` in a trusted
  local environment.
