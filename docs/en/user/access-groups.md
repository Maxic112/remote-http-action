# Access groups

An access group defines which flows the API keys in it may run.

## Group properties

| Property | Description |
|---|---|
| Name | A unique group name. |
| Description | Any comment. |
| Flows | Flows available to the group. |
| API keys | Keys that belong to the group. |

Key membership can be changed on either side, in the group or in the key; it is the same link.

## Access rule

A key may run a flow if at least one of the key's groups contains the flow.
In addition:

- the key is enabled;
- the flow is enabled.

Without access the server answers `403 Forbidden`.
With "Answer 404 instead of 403" enabled in the settings it answers `404 Not Found` and does not reveal that the flow exists.

## Example

| Group | Flows | Keys |
|---|---|---|
| "Power" | `shutdown`, `reboot`, `sleep` | "Phone" |
| "Media" | `open-player` | "Phone", "Remote" |

The "Remote" key may run only `open-player`; the "Phone" key may run all four flows.
