# Command line (CLI)

Every operation available in the window can be performed from a terminal:

```bash
remote-http-action-cli.exe <group> <command> [options]
```

`remote-http-action-cli.exe` is installed next to the main `remote-http-action.exe`. It is a console program:
the terminal waits for it, output appears immediately and the exit code is available (`%errorlevel%`, `$LASTEXITCODE`).
It stores and executes nothing itself: commands are passed to the running application.

The main `remote-http-action.exe` accepts the same commands, but it is a windowed application and `cmd`/PowerShell do not wait for it,
so the prompt may appear before the output. It is only convenient in scripts with redirected output
(`> file`, `| Out-String`). In a terminal use `remote-http-action-cli.exe`.

The examples below use `remote-http-action-cli.exe`.

The main `remote-http-action.exe` without options opens the application. `--minimized` starts it in the tray.
`--headless` starts it without a window and tray icon: only the HTTP server and the CLI work.

## Requirements

- The application must be **running**. The CLI passes commands to the running instance and never changes data directly.
- **Allow control from the command line** must be enabled in the settings.
  It is enabled by default in development builds and disabled in release builds.
- Commands are accepted only from the Windows user the application runs as (or from an administrator).
  A non-elevated terminal can control an application running as administrator.
- `cli.enabled` cannot be changed through the CLI.

`--help` and `--version` always work, even without a running application.

## Global options

| Option | Description |
|---|---|
| `--json` | JSON output instead of tables, convenient for scripts. |
| `--help` | Help for any group or command. |

## Commands

### API keys

```bash
remote-http-action-cli.exe keys list
remote-http-action-cli.exe keys create "Phone" --groups 1,2
remote-http-action-cli.exe keys update 3 --name "Tablet" --disable --groups 2
remote-http-action-cli.exe keys update 3 --no-groups
remote-http-action-cli.exe keys rotate 3
remote-http-action-cli.exe keys delete 3
```

`create` and `rotate` print the key secret once.

### Access groups

```bash
remote-http-action-cli.exe groups list
remote-http-action-cli.exe groups create "Power" --description "..." --flows 1,2 --keys 3
remote-http-action-cli.exe groups update 1 --flows 1,2,4 --no-keys
remote-http-action-cli.exe groups delete 1
```

### Flows

Flows are given as a JSON file in the format described in [Flows](flows.md) and [Actions](actions.md). Omitted fields get default values.
The output of `flows get <flow> --json` can be edited and passed to `flows update`.

```bash
remote-http-action-cli.exe flows list
remote-http-action-cli.exe flows get sleep --json > sleep.json
remote-http-action-cli.exe flows create --file sleep.json
remote-http-action-cli.exe flows update 5 --file sleep.json
remote-http-action-cli.exe flows enable sleep
remote-http-action-cli.exe flows disable 5
remote-http-action-cli.exe flows delete sleep
remote-http-action-cli.exe flows run sleep --wait
```

A flow is identified by its numeric id or its path.

### Runs

```bash
remote-http-action-cli.exe runs list --limit 20 --flow 5
remote-http-action-cli.exe runs get <id>
remote-http-action-cli.exe runs cancel <id>
```

### Settings

```bash
remote-http-action-cli.exe settings get
remote-http-action-cli.exe settings get server.port
remote-http-action-cli.exe settings set server.port 9000
remote-http-action-cli.exe settings set server.allowed_ips '["192.168.1.0/24"]'
```

The value is parsed as JSON; if it is not JSON, it is taken as a string.
Lists can be given comma-separated: `settings set server.allowed_ips 192.168.1.0/24,10.0.0.5`. An empty string `""` clears a list.

### Logs

```bash
remote-http-action-cli.exe logs info
remote-http-action-cli.exe logs tail http|cli|flow [--limit 20] [--grep text]
remote-http-action-cli.exe logs clear http|cli|flow
```

`logs tail` prints entries as JSON Lines, newest first.

### Autostart and HTTPS

```bash
remote-http-action-cli.exe autostart status|install|remove
remote-http-action-cli.exe tls info
remote-http-action-cli.exe tls regenerate
```

### Updates

```bash
remote-http-action-cli.exe update status|check|download|install
```

`update install` installs the downloaded version: the application closes and starts again (see [Updates](update.md)).

### Server, bans, system, application

```bash
remote-http-action-cli.exe server status|start|stop
remote-http-action-cli.exe bans list
remote-http-action-cli.exe bans remove 10.0.0.9
remote-http-action-cli.exe system info
remote-http-action-cli.exe system action-types
remote-http-action-cli.exe app show|quit|restart-elevated
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Internal error |
| 2 | Invalid arguments |
| 3 | The application is not running |
| 4 | Command-line control is disabled |
| 5 | Access denied |
| 6 | Not found |
| 7 | Validation error |
| 8 | Conflict (for example the flow is already running) or too frequent run |
