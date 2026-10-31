# Authentication

## API key

Every request except the public status must carry an API key in one of these headers:

```
Authorization: Bearer rha_...
X-API-Key: rha_...
```

A key in URL parameters is not accepted.

| Situation | Response |
|---|---|
| No key | `401`, `UNAUTHORIZED`, "API key required" |
| Invalid or disabled key | `401`, `UNAUTHORIZED`, "invalid API key" |
| Valid key, but the flow is not in its groups | `403` (or `404` if hiding inaccessible flows is enabled) |

## Order of checks

Each request goes through these checks in order:

1. **Client address.** For requests from trusted reverse proxies the address is taken from `X-Forwarded-For`.
   The header is read right to left, skipping trusted proxies. For other requests the TCP peer address is used.
2. **Allowed addresses.** If the list is set and the address is not in it — `403`.
3. **Host header.** If the list of allowed values is set and the value does not match — `403`.
4. **Ban.** If the address is banned for guessing keys — `429` with a `Retry-After` header.
5. **API key and access rights.**

## Brute-force protection

Every invalid key counts as a failed attempt of the client address.
When the given number of attempts accumulates within the period, the address is banned. All requests from it are blocked, including those with a valid key.
A missing key does not count as an attempt.

Parameters and lifting bans — in [Settings](../user/settings.md#network-access).

## Transport

The server uses HTTP or HTTPS (see [HTTPS](../user/https.md)). For access from untrusted networks enable HTTPS or put the server behind a reverse proxy with HTTPS.
In the latter case add the proxy address to the trusted list.
