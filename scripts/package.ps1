<#
.SYNOPSIS
  Builds the release: MSI installers and the portable zip, from a clean bundle folder.
#>
$ErrorActionPreference = "Stop"

# Old installers would otherwise stay next to the new ones.
Remove-Item -Recurse -Force "src-tauri\target\release\bundle" -ErrorAction SilentlyContinue

pnpm tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& "$PSScriptRoot\package-portable.ps1"
Get-ChildItem "src-tauri\target\release\bundle" -Recurse -Include *.msi, *.zip | Select-Object FullName, Length
