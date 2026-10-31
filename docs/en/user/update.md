# Updates

New versions are published as [releases of the project repository on GitHub](https://github.com/Maxic112/remote-http-action/releases). The application finds out about them itself and updates at the press of a button.

## Checking

- Once a day the application requests the latest release. The first check runs a minute after start if more than a day has passed since the previous one.
- When a new version is out, an orange dot appears next to the **Update** menu item.
- **Check now** checks immediately.
- Automatic checks can be turned off under **Update → Update settings**.

The update screen shows:
- the installed version and installation type (MSI or portable);
- the time of the last check;
- for a new version — its name, release date and release notes.

## Installing an update

### MSI installation

1. Click **Download and install**. The installer in the interface language is downloaded.
2. The application closes and opens the setup wizard: click Next and Install.
3. After installation the application starts again in the new version.

If the wizard is closed without installing, the application starts in the previous version.

### Portable version

1. Click **Download**. The archive of the new version is downloaded and extracted.
2. Click **Restart and update**. The application closes, both executables are replaced, and the application starts again.
   The `data\` folder and `portable.flag` are not touched.

The portable folder must be writable.

### After updating

Downloaded files are deleted right after installation. If an update was interrupted, leftovers are deleted at the next start.
The temporary folder is `%TEMP%\remote-http-action-update`.

The last update is logged to `logs\update.log` in the data folder.

## Security

- Every downloaded file is checked against the SHA-256 checksum from the release (`SHA256SUMS.txt` or the GitHub checksum of the file).
  A file that does not match, or is published without a checksum, is not installed.
- The MSI installer runs with the application's rights: if it runs as administrator, the wizard does not ask for UAC; otherwise Windows shows the prompt.
- Development builds are not updated.

## Update source

By default — the project's GitHub releases. **Update source** lets you set another address, for example an internal mirror.
It must answer in the format of the GitHub API `GET /repos/<owner>/<repo>/releases/latest`:
- `tag_name` — the version, for example `v1.2.0`;
- `body` — release notes;
- `assets` — files with `name`, `browser_download_url`, `size`.

Release files are recognised by the end of their names: `_x64_ru-RU.msi`, `_x64_en-US.msi`, `_x64_portable.zip`.

An empty field restores the default source.

## Command line

```bash
remote-http-action-cli.exe update status
remote-http-action-cli.exe update check
remote-http-action-cli.exe update download
remote-http-action-cli.exe update install
```
