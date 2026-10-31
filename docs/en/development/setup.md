# Environment and builds

## Requirements

- Windows 10/11 x64.
- [Rust](https://rustup.rs/) stable, toolchain `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools with the "Desktop development with C++" workload (MSVC).
- WebView2 Runtime.
- Node.js 22+ and pnpm 10 (`corepack enable`).

## First setup

```bash
pnpm install
```

## Development mode

```bash
pnpm tauri dev
```

Starts Vite on port 1420 and a debug build of the application. In a debug build:

- data is stored in `%APPDATA%\remote-http-action-dev\`;
- the CLI pipe is `\\.\pipe\remote-http-action-dev`;
- command-line control is enabled by default;
- the executable uses the console subsystem, so CLI output is visible at once.

CLI of the debug build:

```bash
src-tauri\target\debug\remote-http-action-cli.exe keys list
```

## Environment variables

| Variable | Purpose |
|---|---|
| `RHA_DATA_DIR` | Data folder instead of the default one |
| `RHA_IPC_ENDPOINT` | Pipe name (or socket path) |
| `RHA_DRY_RUN_POWER=1` | Do not shut down, restart or sleep; only report it to stderr |

`RHA_DRY_RUN_POWER=1` lets you test flows with power actions without switching the machine off.

## Release build

```bash
pnpm package
```

The command (`scripts/package.ps1`) cleans `bundle\`, runs `pnpm tauri build` and `scripts/package-portable.ps1`. Result:

| Path | What it is |
|---|---|
| `src-tauri\target\release\remote-http-action.exe` | Application |
| `src-tauri\target\release\remote-http-action-cli.exe` | Console CLI |
| `src-tauri\target\release\bundle\msi\*_ru-RU.msi`, `*_en-US.msi` | Installers (install both executables) |
| `src-tauri\target\release\bundle\portable\*_portable.zip` | Portable version: both executables and `portable.flag` |
| `src-tauri\target\release\bundle\release\` | **Files to publish in a release**: MSI files with names without spaces, the zip and `SHA256SUMS.txt` |

File names in `bundle\release\` are part of the contract with the updater: the application finds its file by the end of the name
(`_x64_ru-RU.msi`, `_x64_en-US.msi`, `_x64_portable.zip`) and verifies it with `SHA256SUMS.txt`.

Quit the application before building: a running executable cannot be overwritten.

## Version

The version number is set in three places and must match: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
Never change `upgradeCode` in `tauri.conf.json`: the installer of a new version uses it to find and replace the old one.

## Icon

The source is `assets/app-icon.svg`. After changing it, regenerate the icon set:

```bash
pnpm tauri icon assets/app-icon.svg
```

Delete the generated `src-tauri/icons/android` and `src-tauri/icons/ios` folders: mobile platforms are not used.
