# Overview and concepts

Remote HTTP Action runs in the background on a Windows computer and keeps an HTTP server running.
When an authorized request arrives at a flow's URL, the application performs preconfigured actions on that computer.

## Entities

| Entity | Purpose |
|---|---|
| **API key** | A secret the client sends with every request. Without a valid key the server answers `401`. |
| **Access group** | Connects API keys with a set of flows. A key may run a flow only if they share a group. |
| **Flow** | A named sequence of actions with its own URL, for example `POST /flow/sleep/`. |
| **Action** | One step of a flow: shutdown, restart, sleep, delay, command, script, program launch. |
| **Run** | One execution of a flow with the result of each step. Kept in the history. |

```
API key ──┐            ┌── Flow ── action 1, action 2, …
          ├─ Group ────┤
API key ──┘            └── Flow ── …
```

Keys and groups are linked many-to-many: a key may belong to several groups, a group may contain several keys.
Groups and flows are linked the same way.

## Ways to control the application

- **Application window** — configuration of all entities and parameters.
- **Command line** — the same operations through `remote-http-action-cli.exe <command>` (see [CLI](cli.md)).
- **HTTP API** — running flows and service requests only (see [HTTP API](../api/index.md)). The configuration cannot be changed over the network.

## Running in the background

On start the window opens and an icon appears in the Windows notification area.
By default closing the window only hides it: the server keeps running.
To quit completely, use the tray icon menu (**Quit**).
This behaviour is configured in [Settings](settings.md#application).
