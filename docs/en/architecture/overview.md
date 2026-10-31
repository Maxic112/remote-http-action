# Components

## Executables and modes

`remote-http-action.exe` without arguments starts the application (window, tray, HTTP server, IPC server).
With arguments it works as a CLI client: it parses the command, passes it to the running instance and prints the result.
The same CLI client is also built as the console `remote-http-action-cli.exe` (see [CLI and IPC](ipc.md#console-output)).

Application arguments:

| Argument | Purpose |
|---|---|
| `--minimized` | Start without showing the window (tray only). |
| `--headless` | Start without window and tray: only the HTTP server and the CLI channel. Stopped with `app quit` or Ctrl+C. For automation and environments without a desktop. |
| `--wait-pid <pid>` | Wait for the given process to exit before starting (used by the elevated restart). |

## Layers

```
┌──────────── UI (React) ────────────┐   ┌──── CLI client ────┐   ┌──── HTTP clients ────┐
│ invoke("api", Request)             │   │ clap → Request     │   │ POST /flow/...        │
└───────────────┬────────────────────┘   └─────────┬──────────┘   └──────────┬────────────┘
                │ Tauri command                     │ named pipe              │ axum
                ▼                                   ▼                         ▼
        ┌──────────────────────── Core (core.rs) ─────────────────────────────────────┐
        │ handle(Caller, Request)        settings        HttpServer    BruteForceGuard │
        └───────┬───────────────────────────┬──────────────────────┬──────────────────┘
                ▼                           ▼                      ▼
             Db (SQLite)              Runner (runner.rs)      EventSink → UI
                                            │
                                            ▼
                                  actions.rs → Platform (OS)
```

- **Core** — the single entry point for operations. The UI and the CLI send the same `Request`s, so their capabilities match by construction.
  The only difference: the CLI cannot change `cli.*` settings.
- **HttpServer** — axum with a single handler. Path prefixes are read from the settings on every request, so changing them needs no restart.
  The server restarts only when the address, port or HTTPS settings change.
- **Runner** — runs flows in background tokio tasks. Handles the queue, cooldown, start delay, timeouts and cancellation (`CancellationToken`),
  and stores the run history.
- **actions.rs** — parameter validation and execution of each action type.
- **Platform** — a trait for operating-system actions.
  - `SystemPlatform` performs them on Windows.
  - `MockPlatform` records calls in tests.
  - The `RHA_DRY_RUN_POWER=1` environment variable replaces power actions with a log message.
- **Logger** (`logs.rs`) — `http`, `cli` and `flow` logs in JSON Lines with size-based rotation. Written by the HTTP handler (the key name is passed through a task-local), the IPC dispatcher and the runner.
- **TLS** (`tls.rs`) — a self-signed certificate (rcgen) or the user's PEM files, rustls with the `ring` provider. The server is built on `axum-server`: one code path for HTTP and HTTPS, shutdown through a `Handle`.
- **Updates** (`update.rs`) — release check (GitHub API format), download with SHA-256 verification, installation through a PowerShell helper. The helper waits for the application to exit, runs `msiexec` or replaces the portable executables, restarts the application and deletes temporary files. The check result is stored in the `settings` table under `update_state`.
- **Autostart** (`platform/autostart.rs`) — Task Scheduler task XML and `schtasks.exe` calls.
- **EventSink** — core events (data changed, run progress, start delay, server state, update state). The GUI forwards them to the window as `app-event`.

## Flow call path

1. axum accepts the request and determines the client address, taking trusted proxies into account.
2. The address list, `Host` header, ban, API key and group access are checked.
3. `Runner::start` checks that the flow is enabled, validates the actions, applies the concurrency policy and cooldown, then creates the run record and a background task.
4. The task waits out the start delay (the countdown window receives `run_delayed`) and executes the steps.
5. In sync mode the handler waits for completion; in async mode it answers `202` at once.

## Window and tray

- The window is created hidden and shown after initialisation; with `--minimized` it stays hidden.
- Closing the window hides it or quits the application, depending on `app.close_to_tray`.
- Tray menu: Open, HTTP server switch, Quit. The menu follows the interface language.
- Starting the executable again without arguments shows the window of the running instance (single-instance plugin).

## Elevated restart

`Request::AppRestartElevated` calls `ShellExecuteExW` with the `runas` verb and `--wait-pid <current pid>`,
then the current process exits. The new process waits for it to exit before taking the single-instance lock, the pipe and the port.

## Data storage

| Mode | Data folder |
|---|---|
| Installed | `%APPDATA%\remote-http-action\` (`-dev` suffix in development builds) |
| Portable | `data\` next to the executable when `portable.flag` is present |
| Override | the `RHA_DATA_DIR` environment variable |

The folder contains:

- `data.db` — SQLite in WAL mode;
- `logs\` — log files;
- `tls\` — the self-signed certificate, once created.
