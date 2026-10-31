# Settings

Settings are saved with the **Save** button at the bottom of the page. Network parameters apply without restarting the application.

## HTTP server

| Parameter | Default | Description |
|---|---|---|
| HTTP server enabled | yes | A disabled server accepts no connections. The Start and Stop buttons change the same parameter. |
| Interface | `0.0.0.0` (all) | Address the server listens on. `127.0.0.1` — this computer only. |
| Port | `8765` | TCP port of the server. |
| Flow call path | `/flow/` | Prefix of flow URLs. |
| Allowed flows path | `/get-allowed/` | See [API](../api/endpoints.md#allowed-flows). |
| Status path | `/status/` | See [API](../api/endpoints.md#status). |
| Answer status requests without a key | yes | Without a key only `{"status":"ok"}` is returned. When disabled, the status requires a key. |
| Answer 404 instead of 403 | no | Hides the existence of flows the key has no access to. |

Prefixes must not overlap with each other or with the reserved `/runs/`.

## Network access

| Parameter | Description |
|---|---|
| Allowed client addresses | IP addresses or CIDR subnets, one per line (`192.168.1.0/24`, `10.0.0.5`). Empty — everyone is allowed. Requests from other addresses get `403`. |
| Allowed Host header values | For example `pc.local` or `*.example.com`. Empty — any. Protects against DNS rebinding. |
| Trusted reverse proxies | Proxy addresses. Only for requests from them the client address is taken from `X-Forwarded-For`; it is used for the address filter, bans and the history. |
| Block after failed attempts | After the given number of invalid keys within a period, the address is blocked for the given time (default 10 attempts in 300 s → 900 s). A blocked address gets `429`. |

Current bans are listed under **Blocked addresses** and can be lifted manually.

## HTTPS

Enabling HTTPS and choosing a certificate are described in [HTTPS](https.md).

## Autostart

The Set up autostart and Remove autostart buttons, see [Autostart](autostart.md).

## Logging

Enabling logs, file size and number of rotated files per category, see [Logs](logs.md#rotation).

## Command-line control

"Allow control from the command line" enables the [CLI](cli.md).
It can be changed only in the application window, not through the CLI.

## Application

| Parameter | Default | Description |
|---|---|---|
| Keep running in the tray when the window is closed | yes | When disabled, closing the window quits the application. |
| Language | system | English or Russian. |
| Runs to keep | 200 | Size of the run history. |
