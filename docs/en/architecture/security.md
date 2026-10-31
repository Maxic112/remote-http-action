# Security

The application executes commands on a computer in response to network requests. The main goal is to never run actions without the owner's permission.

## Measures

| Threat | Measure |
|---|---|
| Calls without permission | Every run requires a valid API key with access to the flow through a group. |
| Database leak | Only the SHA-256 of a key is stored; the secret is shown once. |
| Key leak through URLs and logs | Keys are accepted only in headers. Lists and history contain the key name, not the secret. |
| Key guessing | A key has about 238 bits of entropy; addresses are banned temporarily after a series of failed attempts. |
| Command injection | Requests do not pass parameters to actions: flows are static and defined only by the owner. |
| Access from foreign networks | Interface selection, allowed address list, `Host` header check. |
| Spoofing through `X-Forwarded-For` | The header is honoured only from trusted proxies and read right to left. |
| Exposure of command output | Output is returned to the client only for steps with "Return output to the client" explicitly enabled. |
| Key interception on the network | HTTPS with a self-signed or own certificate; a SHA-256 fingerprint for pinning on the client. |
| Secrets in logs | The HTTP log contains the key name; the CLI log contains requests only (not responses, which may contain secrets). |
| Tampered updates | A downloaded file is checked against the SHA-256 from the release; without a checksum the update is not installed. The default source is the GitHub API over HTTPS. |
| Configuration changes over the network | The HTTP API has no operations that change the configuration. |
| Control by another Windows user | The CLI named pipe is accessible only to SYSTEM, administrators and the user who started the application. Remote pipe clients are rejected. |
| Unexpected shutdown | A start delay with a countdown window and a cancel button on the computer. |

## The CLI switch

"Allow control from the command line" is a convenience, not a security boundary:
a process of the same user can change the application's files directly.
That is why the switch cannot be changed through the CLI itself and is off by default in release builds.

## Administrator rights

The application never asks for elevation on a remote call.
Steps marked "Requires administrator rights" run only if the process is already elevated.
By running the application as administrator, the owner deliberately allows such steps.

## Deployment recommendations

- Restrict the interface or the allowed address list to the local network.
- For access over the internet use a reverse proxy with HTTPS and add it to the trusted proxies.
- Issue a separate key to each client with the smallest set of groups.
- Enable "Return output to the client" only where needed.
