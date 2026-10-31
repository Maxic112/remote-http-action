# Flows

A flow is a named sequence of [actions](actions.md) started by a `POST` request to the flow's URL.

## General

| Parameter | Description |
|---|---|
| Name | Shown in the window, in the history and in the list of allowed flows. |
| Path | The part of the URL after the prefix. The path `sleep` gives `POST /flow/sleep/`. Allowed characters are `a-z`, `0-9`, `-`, `_` and `/` (for example `pc/power-off`). Letters are lower-cased, leading and trailing slashes are dropped. |
| Description | Any text. |
| Enabled | A disabled flow cannot be called over HTTP (`404`) and is not listed as allowed. |

## Execution

| Parameter | Default | Description |
|---|---|---|
| Response mode | Wait for the result | **sync** — the server waits for the flow to finish and returns the result. **async** — the server answers `202` with a run id at once; the status is requested separately. |
| Flow timeout | 60 s | Maximum time for all steps (the start delay is not counted), up to 7 days. When it expires, execution stops and the run gets the `timeout` status. |
| Call while already running | Reject | **Reject** — answer `409` until the previous run finishes. **Queue** — runs are executed strictly one after another. **Run in parallel** — no limit. |
| Cooldown between runs | 0 | Minimum interval between starts. An earlier call gets `429`. |
| Start delay | 0 | Before execution the computer shows a countdown window with a cancel button. Useful for shutdown and restart. |
| Can be canceled | Yes | Allows cancelling the run over HTTP and the CLI. Any run can be canceled from the application window. |

Tip: for flows that shut down or restart the computer use **async** mode or a short start delay,
so the client is sure to receive the answer before the computer goes offline.

## Client data

Fields on the **Client** tab do not affect execution. They are returned in the list of allowed flows (`/get-allowed/`) for client applications:

- icon, colour, category, tags;
- "Client should ask for confirmation" — a hint for the client to ask the user before calling.

## Steps

Steps run strictly in order; the order is changed with the Move up / Move down buttons.
For each step you set:

- what to do **on error**: stop the flow or continue;
- its own **timeout**;
- **Requires administrator rights**. Such steps are marked with a shield icon, and so are flows containing them.

If at least one step fails, the run gets the `failed` status, even when the remaining steps ran because of "continue".
