# Data model

Data is stored in SQLite (`data.db`). The schema is created and upgraded by the migrations in `src-tauri/src/db.rs`.
The number of the applied migration is stored in `PRAGMA user_version`.
If the database was created by a newer version of the application, start-up stops with an error.

## Tables

| Table | Content |
|---|---|
| `api_keys` | `name`, `key_prefix` (first 12 characters of the key), `key_hash` (SHA-256, unique), `enabled`, `created_at`, `last_used_at` |
| `access_groups` | `name` (unique), `description`, `created_at` |
| `api_key_groups` | key ↔ group link, cascading delete |
| `flows` | `path` (unique), `data` — JSON with the flow parameters and its actions, `created_at`, `updated_at` |
| `group_flows` | group ↔ flow link, cascading delete |
| `settings` | the `app` record with all settings as JSON, and other JSON state such as `update_state` |
| `runs` | run history: flow (`flow_id` is cleared when the flow is deleted; name and path are kept), source, key, client address, status, times, error, steps as JSON |

## Flows as JSON

Flow parameters (`FlowInput`) are stored as a single JSON document with the actions as an array inside.
This makes adding fields easy: missing fields get default values when read.
The CLI accepts the same format (`flows create --file`).

## Settings

Settings are one JSON document (`Settings`). Missing fields are filled with defaults.
The default of `cli.enabled` depends on the build: `true` in development builds, `false` in release builds.

## Files next to the database

| Path | Content |
|---|---|
| `logs\http.jsonl`, `cli.jsonl`, `flow.jsonl` (+ `.1`…`.N`) | Logs, see [Logs](../user/logs.md) |
| `logs\update.log` | Course of the last update |
| `tls\cert.pem`, `tls\key.pem` | Self-signed HTTPS certificate |

## Run history

After each run, finished records beyond `app.run_history_limit` are deleted.
At start-up, runs left unfinished by a previous process are marked `failed`.
