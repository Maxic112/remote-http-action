# Actions

An action is one step of a [flow](flows.md). All actions share common parameters, and each type has its own.

## Common parameters

| Parameter | Description |
|---|---|
| On error | `stop` — stop the flow (default), `continue` — go on with the next step. |
| Step timeout | Maximum step time in seconds. Empty — only the flow timeout applies. |
| Requires administrator rights | The step runs only if the application runs with administrator rights; otherwise it fails with `ADMIN_REQUIRED` (see [Administrator rights](administrator-rights.md)). |
| Return output to the client | Commands and scripts only: stdout and stderr are included in the HTTP response. By default the client receives only the status and exit code. |

## Shut down — `shutdown`

| Parameter | Description |
|---|---|
| `force` | Do not wait for applications to close (unsaved data is lost). |

## Restart — `reboot`

| Parameter | Description |
|---|---|
| `force` | As for `shutdown`. |

## Sleep — `sleep`

| Parameter | Description |
|---|---|
| `hibernate` | Hibernate instead of sleep. |

The computer goes to sleep about 1.5 seconds after the step, so the client has time to receive the answer.

## Delay — `delay`

| Parameter | Description |
|---|---|
| `seconds` | Pause before the next step, from 0 to 86400 (fractions allowed). |

## Run command — `cli_command`

| Parameter | Description |
|---|---|
| `shell` | `cmd`, `powershell` (Windows PowerShell), `pwsh` (PowerShell 7), `wsl`. Available shells are detected automatically. |
| `command` | The command text. |
| `working_dir` | Working directory (optional). |

The command runs without a console window. The output encoding is detected automatically: UTF-8 or the OEM code page of the Windows console,
so non-Latin text is shown correctly for both `cmd` built-ins and PowerShell.
A non-zero exit code fails the step.
The output is kept in the run history (up to 16 KB per stream).

## Run script — `run_script`

| Parameter | Description |
|---|---|
| `shell` | What runs the script: `cmd` (`.bat`, `.cmd`), `powershell` / `pwsh` (`.ps1`, started with `-ExecutionPolicy Bypass`), `wsl` (`sh` script). |
| `script_path` | Path to the script file, selectable with the Browse… button in the window. For `wsl` a Windows path (`C:\dir\job.sh`) is converted to `/mnt/c/dir/job.sh`; a path inside WSL is used as is. |
| `args` | List of arguments, one per line in the window. Arguments are passed as is, without shell interpretation. |
| `working_dir` | Working directory; defaults to the script's folder. |

If the file does not exist, the step fails before starting.
Output, exit code and "Return output to the client" work as for `cli_command`.

## Launch application — `launch_app`

| Parameter | Description |
|---|---|
| `path` | Path to the program (`.exe`, `.bat`, `.cmd`), selectable with the Browse… button. |
| `args` | List of arguments, one per line. |
| `working_dir` | Working directory; defaults to the program's folder. |
| `wait` | Wait mode, see below. Default `none`. |
| `wait_process_name` | For `wait: name`: process name, for example `steam.exe`. Defaults to the file name from `path`. |
| `wait_start_timeout_sec` | For `wait: name`: how many seconds to wait for the process to appear, default 60. If it does not appear, the step fails. |

The program starts with its own window on the desktop of the user Remote HTTP Action runs as, with the same rights.
If the application runs as administrator, the launched program gets administrator rights too.

### Wait modes

| `wait` | The step finishes | Suitable for |
|---|---|---|
| `none` | Right after the program has started. | Starting something in the background. |
| `process` | When the started process exits. A non-zero exit code fails the step. | Regular programs and utilities. |
| `tree` | When the started process and all processes it started have exited (tracked with a Windows job object). | Launchers that start the main program and exit. |
| `name` | When no process named `wait_process_name` is left. The step first waits for the process to appear, then for it to disappear; it counts as gone after three consecutive checks one second apart, so a program restarting itself (for example after an update) does not end the wait. | Programs that may already be running, run as a single instance or restart themselves: Steam, Notepad on Windows 11. |

Cancelling the run or the flow timeout stops **waiting**, but does not close the program.

Many programs are launchers: the started file hands the work over to another process and exits at once.
Notepad on Windows 11 (starts the Microsoft Store version) and Steam (if it is already open, a new `steam.exe` passes the command to the running instance) behave this way.
For them `process` finishes immediately; use `name`.

Example: start Steam and run the next steps after it is closed.

```json
{
  "name": "Gaming session",
  "path": "gaming",
  "response_mode": "async",
  "timeout_sec": 43200,
  "actions": [
    {
      "type": "launch_app",
      "params": {
        "path": "C:\\Program Files (x86)\\Steam\\steam.exe",
        "wait": "name",
        "wait_process_name": "steam.exe"
      }
    },
    { "type": "sleep", "params": {} }
  ]
}
```

For flows that wait for a long-running program choose the **async** response mode (otherwise the HTTP client waits all that time) and a sufficient flow timeout.

Example of a flow in JSON (see [CLI](cli.md#flows)):

```json
{
  "name": "Restart print spooler",
  "path": "restart-spooler",
  "actions": [
    {
      "type": "cli_command",
      "params": { "shell": "powershell", "command": "Restart-Service Spooler" },
      "run_as_admin": true,
      "capture_output": true
    }
  ]
}
```
