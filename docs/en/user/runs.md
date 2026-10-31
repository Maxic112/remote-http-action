# Runs and cancellation

Every execution of a flow is stored as a **run**.

## Statuses

| Status | Meaning |
|---|---|
| `pending` | Accepted and waiting (for example in the queue). |
| `delayed` | The start delay countdown is running. |
| `running` | Steps are executing. |
| `success` | All steps succeeded. |
| `failed` | At least one step failed. |
| `canceled` | The run was canceled. |
| `timeout` | The flow timeout was exceeded. |

## History

The **Runs** page shows recent runs with their source (HTTP, CLI, window), API key name and client address.
Clicking a row opens the details: result, exit code and output of each step.
The number of kept runs is set in the settings (default 200).

Runs interrupted by closing the application are marked `failed` at the next start.

## Running from the window

The ▶ button in the flow list runs a flow immediately, without an API key — handy for checking a configuration.

## Start delay and cancellation

If a flow has a start delay, the computer shows a countdown window with a **Cancel run** button,
so a person at the computer can cancel a remotely requested shutdown or restart.

An active run can be canceled:

- with the button in the countdown window or on the **Runs** page (always possible);
- with `POST /runs/{id}/cancel` using the key that created the run, if the flow has "Can be canceled" enabled;
- with `remote-http-action-cli.exe runs cancel <id>`, if the flow has "Can be canceled" enabled.

Cancelling a running command terminates its process.
