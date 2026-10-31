# AGENTS.md

Instructions for AI agents and developers working on the **remote-http-action** repository.
Details are in [docs/en/](docs/en/index.md), extended agent rules in [docs/en/agents/](docs/en/agents/index.md).

## What the project is

A Windows desktop application (Tauri 2 + React/TypeScript). It keeps an HTTP server running:
a request with an API key to a flow's URL runs a preconfigured sequence of actions on the computer.
Everything available in the window is also available through the CLI (`remote-http-action-cli.exe`).

## Stack

- Backend: Rust (`src-tauri/`), axum, tokio, rusqlite (SQLite), windows-rs.
- Frontend: React 19 + TypeScript (`src/`), Vite, Tailwind CSS v4, shadcn/ui on Base UI, TanStack Query, i18next (en/ru).
- Tests: `cargo test` (unit and integration), Vitest + Testing Library.
- Package manager: **pnpm**.

## Commands

| Task | Command |
|---|---|
| Install dependencies | `pnpm install` |
| Development mode | `pnpm tauri dev` |
| All type checks and tests | `pnpm test:all` |
| Linters (ESLint, Prettier, rustfmt, clippy) | `pnpm lint` |
| Auto-format | `pnpm format` |
| Release build (MSI) | `pnpm tauri build` |
| Full release: MSI + portable zip | `pnpm package` |
| Publish a release on GitHub | tag `vX.Y.Z`, see [docs/en/development/releasing.md](docs/en/development/releasing.md) |

Before finishing any task, `pnpm lint` and `pnpm test:all` must pass.

## Code map

| Path | Purpose |
|---|---|
| `src-tauri/src/core.rs` | `Core` and the `Request` enum: **the single set of operations** for the UI and the CLI |
| `src-tauri/src/http/` | HTTP API: routing, authentication, network filters, brute-force protection |
| `src-tauri/src/runner.rs` | Flow execution: queue, cooldown, start delay, timeouts, cancellation, history |
| `src-tauri/src/actions.rs` | Action types: parameter validation and execution |
| `src-tauri/src/platform/` | OS integration behind the `Platform` trait (real implementation and a test mock) |
| `src-tauri/src/ipc.rs`, `cli.rs` | CLI: argument parsing, named pipe to the running instance |
| `src-tauri/src/bin/remote-http-action-cli.rs` | Console executable for the CLI (terminals wait for it) |
| `src-tauri/src/headless.rs` | `--headless` mode: server and CLI without a window |
| `src-tauri/src/logs.rs` | http/cli/flow logs in JSON Lines, rotation |
| `src-tauri/src/tls.rs` | HTTPS: self-signed certificate, PEM loading, fingerprint |
| `src-tauri/src/platform/autostart.rs` | Autostart through Task Scheduler |
| `src-tauri/src/update.rs` | Updates from GitHub releases: check, SHA-256 verified download, install helper |
| `src-tauri/src/db.rs` | SQLite schema, migrations, repositories |
| `src-tauri/src/settings.rs` | Settings, defaults, validation |
| `src-tauri/src/gui.rs` | Window, tray, `api` command, event forwarding to the UI |
| `src/lib/` | Types (mirror of the Rust models), API client, helpers |
| `src/features/` | Pages: flows, keys, groups, runs, logs, update, settings |
| `src/i18n/` | en/ru translations (key sets must match) |
| `scripts/` | `smoke.ps1` end-to-end test of the real executable; packaging and release scripts |
| `assets/app-icon.svg` | Icon source; `src-tauri/icons/` is generated with `pnpm tauri icon assets/app-icon.svg` |

## Rules

1. **UI and CLI parity.** A new user operation is added as a `Request` variant in `core.rs`,
   then gets a CLI command in `cli.rs` and a UI call through `call()`. Logic is never duplicated in the UI or the CLI.
2. **Tests for every feature.** Rust: unit tests next to the code, HTTP scenarios in `src-tauri/src/http/tests.rs`.
   Frontend: `*.test.ts(x)` next to the component. OS actions are tested only through `MockPlatform`.
   Tests must never shut down, restart or suspend the machine.
3. **Documentation with the code.** Every behaviour change updates the matching pages in both `docs/en/` and `docs/ru/`.
   If the repository rules change, this file is updated too.
4. **Documentation reads like a standalone product.** No references to the development process, specific machines, networks or test dates.
5. **Code comments** are in English and only explain what is not obvious from the code.
6. **Security.**
   - No secrets in logs: the HTTP log has the key name, the CLI log only requests.
   - API key secrets are never stored (only SHA-256), logged or returned by list operations.
   - The CLI cannot change `cli.*` settings.
   - New endpoints require a key unless there is an explicit reason not to.
7. **Type sync.** Changes to the Rust models (`model.rs`, `settings.rs`, `Request`) are repeated in `src/lib/types.ts`.
8. **Translations.** New UI strings go into `src/i18n/en.ts` and `src/i18n/ru.ts` together; a test checks that the keys match.
9. **New actions** follow [docs/en/development/adding-action.md](docs/en/development/adding-action.md).
10. **Releases.** The version in `package.json`, `Cargo.toml` and `tauri.conf.json` matches; every version has an English entry in `CHANGELOG.md`.
    Release file names and `SHA256SUMS.txt` are a contract with the updater and must not change.
11. **Git.** Branch `main`, Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `ci:`).

## Documentation

- [docs/en/index.md](docs/en/index.md) — contents ([Russian](docs/ru/index.md)).
- [docs/en/user/](docs/en/user/index.md) — user guide.
- [docs/en/api/](docs/en/api/index.md) — HTTP API.
- [docs/en/architecture/](docs/en/architecture/index.md) — design.
- [docs/en/development/](docs/en/development/index.md) — builds, tests, conventions, releasing.
- [docs/en/agents/](docs/en/agents/index.md) — extended rules for agents.
