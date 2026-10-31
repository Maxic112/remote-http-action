# Testing

## Commands

| Command | What it checks |
|---|---|
| `pnpm test:all` | TypeScript type check, frontend tests and all Rust tests |
| `pnpm test` | Frontend tests (Vitest) |
| `pnpm test:rust` | Rust tests (`cargo test`) |
| `pnpm lint` | ESLint, Prettier, rustfmt, clippy (warnings are errors) |

## Rust

| Where | What |
|---|---|
| `#[cfg(test)] mod tests` next to the code | Unit tests: settings, key generation, action parameter parsing, network filters, brute-force protection, SQLite repositories (in-memory), runner, CLI parsing |
| `src/http/tests.rs` | HTTP API integration tests over a real socket (`127.0.0.1:0`) with a reqwest client |
| `src/ipc.rs` | Exchange over the real transport: a named pipe on Windows, a Unix socket elsewhere |
| `src/platform/autostart.rs` | Task XML generation. `autostart_round_trip` (marked `#[ignore]`) creates and removes a real task and needs an elevated terminal: `cargo test -- --ignored autostart_round_trip` |
| `src/tls.rs`, `src/http/tests.rs` | Certificate generation, HTTPS server over a real socket |
| `src/logs.rs` | Writing, rotation, reading across rotated files |
| `src/update/tests.rs` | Release feed parsing, file selection, checksums, update helper script, archive extraction, check and download against a local feed server |
| `src/platform/system.rs` (Windows only) | Real `cmd` and PowerShell runs, output encoding, working directory, process termination, shell detection, waiting for process trees |

Operating-system actions in core tests go through `MockPlatform` (`src/platform/mock.rs`), which:

- records power calls and processes;
- returns scripted answers with a delay;
- simulates administrator rights.

`SystemPlatform` is always created with `dry_run_power: true` in tests.

Test helpers: `core::tests::core_with(platform)` creates a core with an in-memory database, `core::tests::sample_flow(path)` creates a minimal flow.

## Frontend

Tests live next to the code (`*.test.ts`, `*.test.tsx`) and run in jsdom.

- `src/test/utils.tsx` mocks `@tauri-apps/api/core`, `@tauri-apps/api/event` and `@tauri-apps/plugin-dialog`.
- `mockApi({ op: handler })` defines core answers by `op` and returns the list of received requests, which is used to check that the UI sent the right `Request`.
- `renderWithProviders` wraps a component in QueryClient and TooltipProvider.
- Texts are looked up in the English locale, enabled in `src/test/setup.ts`.
- For Base UI checkboxes use `getByRole("checkbox", { name })`: `getByLabelText` finds two elements.

## End-to-end smoke test

`scripts/smoke.ps1` runs the real executable in `--headless` mode with:

- a temporary data folder;
- a separate pipe name;
- `RHA_DRY_RUN_POWER=1`.

It sets up a flow, a group and a key through the CLI and then checks:

- the HTTP API and the run history;
- CLI exit codes;
- running a PowerShell script with arguments;
- the logs (including the absence of the secret);
- switching to HTTPS and back;
- the autostart status;
- process exit.

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bins
pwsh -File scripts/smoke.ps1
```

CLI commands go through `remote-http-action-cli.exe`; one command is sent through the main executable to cover that path too.

The script needs a debug build, where command-line control is enabled by default. No window is needed, so it also runs in non-interactive sessions.

## Manual checks

Scenarios that need a window in the user session are checked manually on a built application:

- tray and window close behaviour;
- elevated restart and UAC;
- the countdown window;
- real power actions;
- installing an update through the installer wizard.
