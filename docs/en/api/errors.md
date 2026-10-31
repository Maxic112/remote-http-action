# Errors and status codes

Errors are returned in one format:

```json
{ "error": { "code": "UNAUTHORIZED", "message": "invalid API key" } }
```

`code` is a stable machine-readable code, `message` an explanation for people.

| HTTP | `code` | When |
|---|---|---|
| 400 | `VALIDATION` | Invalid request |
| 401 | `UNAUTHORIZED` | No key, invalid or disabled key |
| 403 | `FORBIDDEN` | Address or Host not allowed; no access to the flow; cancellation not allowed |
| 404 | `NOT_FOUND` | Path, flow or run not found |
| 405 | `VALIDATION` | Unsupported method for an existing path |
| 409 | `CONFLICT` | The flow is already running; the run has already finished |
| 429 | `TOO_MANY_REQUESTS` | The address is banned (with `Retry-After`) or the cooldown has not passed |
| 500 | `INTERNAL` | Internal error |

Answers to running a flow in sync mode are a run object, not an error object. Their codes are described in [Endpoints](endpoints.md#sync-mode).
