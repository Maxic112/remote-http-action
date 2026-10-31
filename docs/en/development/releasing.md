# Releasing

Releases are built and published by GitHub Actions on a version tag. Installed applications learn about a new release through the [updater](../user/update.md).

## Steps

1. **Version.** Raise the number in three files; it must match:
   - `package.json`
   - `src-tauri/Cargo.toml` (and refresh `Cargo.lock` with any cargo command, e.g. `cargo check`)
   - `src-tauri/tauri.conf.json`

   Check: `pwsh -File scripts/check-version.ps1`.
2. **Changelog.** Add a `## [X.Y.Z] - YYYY-MM-DD` section to `CHANGELOG.md`, in English. Its text becomes the release description and is shown to users on the Update screen.
   Check: `pwsh -File scripts/release-notes.ps1 -Version X.Y.Z`.
3. **Commit** to `main` and push. Wait for a green CI.
4. **Tag** and push the tag:

   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The `.github/workflows/release.yml` workflow:
- checks that the tag matches the version and that `CHANGELOG.md` has a section for it;
- runs the linters and tests;
- runs `pnpm package`;
- publishes the release with the files from `src-tauri/target/release/bundle/release/`.

## Release contents

| File | Purpose |
|---|---|
| `remote-http-action_X.Y.Z_x64_en-US.msi` | Installer (English) |
| `remote-http-action_X.Y.Z_x64_ru-RU.msi` | Installer (Russian) |
| `remote-http-action_X.Y.Z_x64_portable.zip` | Portable version |
| `SHA256SUMS.txt` | Checksums |

**This is a contract with installed copies of the application.** The updater finds its file by the end of the name (`_x64_ru-RU.msi`, `_x64_en-US.msi`, `_x64_portable.zip`) and installs it only if the SHA-256 matches `SHA256SUMS.txt` or the GitHub checksum of the file.
Renaming the files or dropping the checksums breaks updates for users.

## Pre-releases

A tag with a suffix (`v1.2.0-beta.1`) is published as a pre-release. The application does not offer it: it reads only the latest regular release (`/releases/latest`).

## Parameters that must not change

- `upgradeCode` in `src-tauri/tauri.conf.json` — the installer replaces the previous version by it.
- `identifier` in `src-tauri/tauri.conf.json` — single-instance behaviour depends on it.
- `REPOSITORY` in `src-tauri/src/update.rs` — where the application gets updates from.
