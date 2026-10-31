# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).
A version's section is published as the GitHub release description and shown in the application on the Update screen.

## [1.0.0] - 2026-11-01

First release.

### Features

- HTTP server that runs flows — preconfigured sequences of actions — on requests with an API key.
- API keys (stored only as hashes) and access groups for flows.
- Actions: shutdown, restart, sleep, delay, command (cmd, PowerShell, pwsh, WSL), script, application launch
  with waiting for the process, the process tree or processes by name.
- Sync and async responses, timeouts, queueing, cooldown between runs, start delay that can be canceled on the computer.
- Client IP and Host header restrictions, trusted reverse proxies, temporary bans on key guessing, HTTPS.
- Logs of HTTP requests, CLI commands and flow runs with rotation and an in-app viewer.
- Tray mode, autostart with administrator rights through Task Scheduler, administrator rights indicator and elevated restart.
- Full control through the console `remote-http-action-cli.exe`.
- Automatic updates from GitHub releases for the MSI and portable versions.
- English and Russian interface.
