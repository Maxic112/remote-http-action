# Troubleshooting

## The server does not start

The window header shows "Server error". The cause is shown in **Settings → HTTP server**.

| Message | What to do |
|---|---|
| `cannot listen on 0.0.0.0:8765: … (os error 10048)` | The port is used by another program or a second instance of the application. Choose another port or close the program using it (`netstat -ano \| findstr :8765` shows the PID). |
| `cannot listen on …: (os error 10049)` | The selected interface address no longer belongs to the computer, e.g. the IP has changed. Choose "All interfaces" or the current address. |
| `cannot load TLS certificate …` | Your certificate files are missing or not PEM, or the key does not match the certificate. Check the paths or switch to the self-signed certificate. |

## Requests from other devices do not arrive

1. Check from the computer itself: `curl http://127.0.0.1:8765/status/`. If there is no answer, see "The server does not start".
2. **Windows Firewall.** Allow incoming connections for `remote-http-action.exe`. In "Windows Defender Firewall → Allow an app through firewall" the needed network type must be checked.
   If the network is "Public" but access is allowed only for "Private", connections are blocked.
3. **Interface.** If `127.0.0.1` is selected in the settings, the server is reachable from this computer only.
4. **Allowed addresses and Host.** A `403` with `client address is not allowed` or `host is not allowed` means the request was rejected by the filters in **Settings → Network access**.
5. **Ban.** A `429` means the address is blocked after failed authentication attempts. Lift the ban in the settings or wait until it expires.

## 401 response

- The key is not sent in `Authorization: Bearer …` or `X-API-Key`.
- The key is disabled or was rotated. A key is shown only once; if it is lost, rotate it.

## 403 or 404 when calling a flow

- `403` — the key is not in any group that contains the flow (see [Access groups](access-groups.md)).
- `404` — the flow does not exist, is disabled, or "Answer 404 instead of 403" is enabled.
- Check the prefix: the default URL is `POST /flow/<path>/`, but the prefix can be changed in the settings.

## A step fails with `ADMIN_REQUIRED`

The step is marked "Requires administrator rights" but the application runs without them.
Restart the application elevated (yellow shield in the header) or set up [autostart](autostart.md), which starts it with administrator rights.

## Launching an application finishes at once while the program is still open

The program is a launcher: it handed the work over to another process and exited. Choose the wait mode "While the process and all its children run" or "While processes with a given name run" (see [Actions](actions.md#wait-modes)).

## HTTPS: the client does not trust the certificate

A self-signed certificate is not signed by a trusted authority. Compare the fingerprint from the settings and pin it on the client, or add `cert.pem` to the trusted store (see [HTTPS](https.md)).

## CLI

| Message | What to do |
|---|---|
| `remote-http-action is not running` (code 3) | Start the application. The CLI controls only a running instance. |
| `CLI access is disabled` (code 4) | Enable **Settings → Command-line control** in the application window. |
| Output appears after the terminal prompt, no exit code | Use `remote-http-action-cli.exe`, not `remote-http-action.exe`. |
| `access to the running instance was denied` | The command runs as a different Windows user. The CLI accepts commands only from the user who started the application and from administrators. |

## Autostart does not work

- **Settings → Autostart** shows "The task points to a different executable" — the program was moved. Click Reinstall.
- Open `taskschd.msc`, find the **Remote HTTP Action** task and check its History or Last Run Result.

## Where to find details

- The **Runs** page — result and output of each step.
- The **Logs** page — HTTP requests with response codes, CLI commands, the full course of flow runs (see [Logs](logs.md)).
