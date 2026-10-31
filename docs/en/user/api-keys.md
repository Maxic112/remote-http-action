# API keys

An API key is the secret a client uses to prove it may run flows. Without a valid key the server answers `401 Unauthorized`.

## Key properties

| Property | Description |
|---|---|
| Name | A readable name: "Phone", "Smart home". Shown in the run history. |
| Key | A string `rha_` + 40 characters. The list shows only the first characters (`rha_AbCdEfGh…`). |
| Access groups | One or more groups; they define which flows the key may run. |
| Enabled | A disabled key is rejected like an invalid one. |
| Last used | Time of the last successful request with this key. |

## Storage

The application stores only a hash of the key (SHA-256). The key itself is shown **once**, right after it is created or rotated.
A lost key cannot be recovered; it can only be rotated.

## Operations

- **Create** — enter a name and groups; copy the key from the dialog that opens.
- **Edit** — name and groups.
- **Enable / disable** — the switch in the list.
- **Rotate** — issues a new key. The old one stops working at once.
- **Delete** — removes the key together with its group links.

## Sending the key

```
Authorization: Bearer rha_...
```

or

```
X-API-Key: rha_...
```

A key in the query string (`?key=...`) is not accepted: such URLs often end up in proxy logs and browser history.
