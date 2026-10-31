# HTTP API

The HTTP API runs flows and answers service requests. The configuration cannot be changed over the network.

- [Authentication](authentication.md) — sending the API key, network restrictions, bans.
- [Endpoints](endpoints.md) — running a flow, run status, cancellation, allowed flows, application status.
- [Errors and status codes](errors.md) — error format and HTTP statuses.

The base URL is `http://<host>:<port>`, port `8765` by default.
The `/flow/`, `/get-allowed/` and `/status/` paths are configurable (see [Settings](../user/settings.md#http-server)).
The examples use the defaults.
A trailing slash in paths is optional.
