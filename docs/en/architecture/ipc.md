# CLI and IPC

## Overview

```
remote-http-action-cli.exe keys list
        │ clap: argument parsing, --help/--version handled locally
        ▼
   Request (JSON)  ──►  \\.\pipe\remote-http-action  ──►  running instance
                                                          ├─ cli.enabled? otherwise CLI_DISABLED
                                                          └─ Core::handle(Caller::Cli, request)
   table / JSON output  ◄──  IpcResponse { data | error }
```

The CLI never touches the database: every operation is performed by the running instance.
This keeps the data consistent, and the window sees changes at once (`data_changed` event).

## Protocol

One connection, one request. The client writes one JSON line terminated by a newline:

```json
{"request": {"op": "key_create", "name": "Phone", "group_ids": [1]}}
```

The server answers with one line:

```json
{"data": {...}, "error": null}
```

or

```json
{"data": null, "error": {"code": "NOT_FOUND", "message": "..."}}
```

The `request` format is the one the UI passes to the `api` Tauri command (the `Request` enum, tagged by `op`).

## Transport

| OS | Transport | Name |
|---|---|---|
| Windows | Named pipe | `\\.\pipe\remote-http-action`, `\\.\pipe\remote-http-action-dev` in development builds |
| Other (development) | Unix socket | `<data folder>/ipc.sock` |

The name can be overridden with the `RHA_IPC_ENDPOINT` environment variable.

### Pipe access rights

Security descriptor:

- full access: `SYSTEM`, `Administrators` and the SID of the user who started the application;
- Medium integrity label: a non-elevated terminal can connect to an instance running as administrator;
- the first pipe instance is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`, so no other process can take over the name;
- remote pipe clients are rejected.

## Console output

The CLI client exists in two executables sharing the same code (`cli::run`):

| File | Subsystem | Behaviour in a terminal |
|---|---|---|
| `remote-http-action-cli.exe` (`src/bin/remote-http-action-cli.rs`) | console | The terminal waits for it; output and exit code work like any console utility. |
| `remote-http-action.exe` with arguments | GUI (release) | Attaches to the parent console (`AttachConsole`) if output is not redirected. `cmd` and PowerShell do not wait for GUI processes, so output may appear after the prompt. With redirection (`> file`, a pipeline) it works correctly. |

The subsystem of an executable is fixed at build time, and the shell decides whether to wait based on it. That is why interactive use needs a separate console executable.
