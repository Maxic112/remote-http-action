# Logs

The application keeps three independent logs. Actions of the user in the application window are not logged.

| Category | File | What is written |
|---|---|---|
| HTTP requests | `http.jsonl` | Time, client address, method, path, response code, API key **name**, duration, User-Agent. The key itself is never written. |
| CLI commands | `cli.jsonl` | The command and its parameters, success or error code, duration. Responses (including issued keys) are not written. |
| Flow runs | `flow.jsonl` | Two entries per run: `started` and `finished`. The second has the final status and, for each step, its type, parameters, administrator flag, start and end time, exit code, error and command output (up to a configurable limit). |

Files are stored in `<data folder>\logs\`. Each line is a separate JSON object (JSON Lines); the `ts` field is the time in UTC.

## Rotation

For each category, **Settings → Logging** has:

| Parameter | Default | Description |
|---|---|---|
| Enabled | yes | A disabled category is not written. |
| File size, MB | 5 | When exceeded, the current file is renamed to `<category>.1.jsonl` and older ones shift (`.1` → `.2` …). |
| Files to keep | 5 | Number of rotated files kept; older ones are deleted. |

A separate setting limits the command output written to the flow log (default 4096 characters per stream). The full output (up to 16 KB) is kept in the run history.

## Viewing

The **Logs** page:

- tabs per category, newest entries first;
- text search;
- clicking a row shows the whole entry;
- Open folder and Clear (deletes all files of the category).

From the command line:

```bash
remote-http-action-cli.exe logs info
remote-http-action-cli.exe logs tail flow --limit 20
remote-http-action-cli.exe logs tail http --grep 10.0.0.5
remote-http-action-cli.exe logs clear cli
```
