# Installation, update and removal

## Distribution

| File | What it is |
|---|---|
| `remote-http-action_<version>_x64_en-US.msi` | Installer with an English interface |
| `remote-http-action_<version>_x64_ru-RU.msi` | Installer with a Russian interface |
| `remote-http-action_<version>_x64_portable.zip` | Portable version, no installation |

Installers in both languages install the same application; choose either.
`SHA256SUMS.txt` in the release contains checksums for verifying downloaded files.

## Requirements

- Windows 10 or 11, 64-bit.
- WebView2 Runtime. It is part of Windows 11 and of updated Windows 10; if it is missing, the installer offers to download it.

## MSI installer

1. Run the MSI and follow the wizard. Installation requires administrator rights.
2. The application is installed to `C:\Program Files\Remote HTTP Action\`:
   - `remote-http-action.exe` — the application;
   - `remote-http-action-cli.exe` — the console CLI (see [Command line](cli.md)).
3. A **Remote HTTP Action** shortcut appears in the Start menu.

Data is stored in `%APPDATA%\remote-http-action\`: the database with keys, flows and settings, logs, the HTTPS certificate.

## Portable version

1. Extract the archive to any folder, for example a USB drive or `D:\Tools\`.
2. Run `remote-http-action.exe`.

A `portable.flag` file lies next to the executables. While it is present, data is stored in the `data\` folder next to them instead of the user profile.
Without `portable.flag` the application uses `%APPDATA%\remote-http-action\`.

The portable folder must be writable.

## Updating

The easiest way is from the application itself, in the **Update** section (see [Updates](update.md)).
Manually:

- **MSI:** run the installer of the new version. It replaces the program files; data in `%APPDATA%` is kept.
- **Portable:** quit the application (tray → Quit) and replace both executables. Keep the `data\` folder and `portable.flag`.

Quit the application before updating, otherwise Windows cannot replace the running executable.
If [autostart](autostart.md) is set up and the program folder has changed, reinstall autostart in the settings.

## Removal

1. If autostart is set up, remove it: **Settings → Autostart → Remove autostart**. The scheduled task is not removed together with the program.
2. Quit the application: tray → Quit.
3. MSI version: Settings → Apps → Installed apps → Remote HTTP Action → Uninstall.
   Portable version: delete the folder.
4. Uninstalling the MSI keeps the data. To remove it too, delete `%APPDATA%\remote-http-action\`.

## Installed and portable versions at the same time

Both versions use the same CLI channel name and, by default, the same port, so only one of them can run at a time.
