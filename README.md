# Remote HTTP Action

**English** · [Русский](README.ru.md)

A Windows application that turns a computer into a node controlled over HTTP.
You configure **flows** — sequences of actions such as shutdown, restart, sleep, delay, running a command or script, launching a program.
An authorized `POST /flow/<path>/` request with an API key runs the matching flow.

## Features

- HTTP API with API key authentication; keys are stored as hashes.
- Access groups: which flows each key may run.
- Sync and async responses, timeouts, queueing, cooldown between runs.
- Start delay with an on-screen countdown that can be canceled.
- Client IP and `Host` header restrictions, reverse proxy support, temporary bans on key guessing.
- HTTPS with a self-signed or your own certificate.
- Logs of HTTP requests, CLI commands and flow runs with rotation.
- Runs in the background with a tray icon; autostart with administrator rights through Task Scheduler.
- Administrator rights indicator and elevated restart.
- Full control from the command line (`remote-http-action-cli.exe <command>`), enabled in the settings.
- Automatic updates from GitHub releases.
- English and Russian interface.

## Installation

An MSI installer or a portable zip is available on the [Releases](https://github.com/Maxic112/remote-http-action/releases) page.
See [Installation, update and removal](docs/en/user/installation.md).

## Quick example

```bash
curl -X POST http://<computer-address>:8765/flow/sleep/ -H "Authorization: Bearer rha_XXXXXXXX..."
```

## Documentation

The full documentation is in [docs/en/](docs/en/index.md):

- [User guide](docs/en/user/index.md)
- [HTTP API](docs/en/api/index.md)
- [Architecture](docs/en/architecture/index.md)
- [Development](docs/en/development/index.md)

## License

[MIT](LICENSE)
