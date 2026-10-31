<#
.SYNOPSIS
  Packs the portable version: both executables and portable.flag in a zip archive.

.DESCRIPTION
  Run after `pnpm tauri build`. The result is written next to the MSI bundles:
    src-tauri\target\release\bundle\portable\remote-http-action_<version>_x64_portable.zip
  With portable.flag next to the executables the application keeps its data in the `data`
  folder beside them instead of %APPDATA%.
#>
param(
    [string]$Release = "src-tauri\target\release"
)

$ErrorActionPreference = "Stop"
$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
$outDir = Join-Path $Release "bundle\portable"
$name = "remote-http-action_${version}_x64_portable"
$stage = Join-Path ([IO.Path]::GetTempPath()) ($name + "-" + [guid]::NewGuid())
$folder = Join-Path $stage "Remote HTTP Action"

New-Item -ItemType Directory -Force $folder, $outDir | Out-Null
try {
    foreach ($exe in "remote-http-action.exe", "remote-http-action-cli.exe") {
        $src = Join-Path $Release $exe
        if (-not (Test-Path $src)) { throw "$src not found; run 'pnpm tauri build' first" }
        Copy-Item $src $folder
    }
    Set-Content -Encoding UTF8 (Join-Path $folder "portable.flag") @"
This file switches Remote HTTP Action to portable mode: settings, keys, logs and
certificates are stored in the "data" folder next to the executables.
Delete this file to use %APPDATA%\remote-http-action instead.
"@
    Copy-Item LICENSE (Join-Path $folder "LICENSE.txt")

    $zip = Join-Path $outDir "$name.zip"
    if (Test-Path $zip) { Remove-Item $zip }
    Compress-Archive -Path $folder -DestinationPath $zip
    Write-Host "Portable archive: $zip"
} finally {
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
}
