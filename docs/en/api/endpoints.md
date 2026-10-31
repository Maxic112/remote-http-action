# Endpoints

| Method | Path | Key | Purpose |
|---|---|---|---|
| `POST` | `/flow/{path}/` | yes | Run a flow |
| `GET` | `/runs/{run_id}` | yes | Run status |
| `POST` | `/runs/{run_id}/cancel` | yes | Cancel a run |
| `GET` | `/get-allowed/` | yes | Flows available to the key |
| `GET` | `/status/` | no* | Application status |

\* Without a key — a minimal answer, if the public status is enabled.

A request to an existing path with another method gets `405`.

## Running a flow

```
POST /flow/{path}/
```

No request body is needed.

### Sync mode

The server waits for the flow to finish (start delay + flow timeout + 5 seconds) and returns the result:

```json
{
  "run_id": "4f1c…",
  "flow": "restart-spooler",
  "status": "success",
  "created_at": "2026-01-01T10:00:00.000Z",
  "started_at": "2026-01-01T10:00:00.010Z",
  "finished_at": "2026-01-01T10:00:01.200Z",
  "error": null,
  "steps": [
    {
      "index": 0,
      "type": "cli_command",
      "status": "success",
      "started_at": "…",
      "finished_at": "…",
      "exit_code": 0,
      "stdout": "…",
      "stderr": ""
    }
  ]
}
```

`stdout` and `stderr` are present only for steps with "Return output to the client" enabled.

| Run status | HTTP code |
|---|---|
| `success` | `200` |
| `failed` | `500` |
| `timeout` | `504` |
| `canceled` | `409` |
| not finished within the wait time | `202` (as in async mode) |

### Async mode

```
HTTP/1.1 202 Accepted

{ "run_id": "4f1c…", "status": "pending", "status_url": "/runs/4f1c…" }
```

### Errors

| Code | Cause |
|---|---|
| `404` | the flow does not exist or is disabled |
| `403` | the key has no access to the flow |
| `409` | the flow is already running ("Reject" policy) |
| `429` | the cooldown between runs has not passed |

## Run status

```
GET /runs/{run_id}
```

Returns the run in the same format as the sync answer.
A run is visible only to the key that created it; other keys get `404`.

## Cancelling a run

```
POST /runs/{run_id}/cancel
```

```json
{ "run_id": "4f1c…", "status": "canceling" }
```

| Code | Cause |
|---|---|
| `403` | the flow has "Can be canceled" disabled |
| `404` | the run is not found or was created by another key |
| `409` | the run has already finished |

## Allowed flows

```
GET /get-allowed/
```

Returns the enabled flows available to the key. Client applications use it to build their interface.

```json
{
  "key": "Phone",
  "flows": [
    {
      "path": "sleep",
      "url": "/flow/sleep/",
      "method": "POST",
      "name": "Sleep",
      "description": "",
      "response_mode": "sync",
      "start_delay_sec": 0,
      "cancelable": true,
      "client_meta": { "icon": "moon", "color": null, "category": "Power", "tags": [], "confirm_on_client": false }
    }
  ]
}
```

## Status

```
GET /status/
```

Without a key (if the public status is enabled):

```json
{ "status": "ok" }
```

With a valid key:

```json
{ "status": "ok", "version": "1.0.0", "uptime_sec": 3600, "elevated": true }
```

`elevated` tells whether the application runs with administrator rights.
