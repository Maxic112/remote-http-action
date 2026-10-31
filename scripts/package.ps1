<#
.SYNOPSIS
  Builds the release: MSI installers and the portable zip, from a clean bundle folder.

.DESCRIPTION
  The files to publish are collected in src-tauri\target\release\bundle\release\:
    remote-http-action_<version>_x64_ru-RU.msi
    remote-http-action_<version>_x64_en-US.msi
    remote-http-action_<version>_x64_portable.zip
    SHA256SUMS.txt
  The application's updater finds its file by these name suffixes and verifies it with SHA256SUMS.txt.
#>
$ErrorActionPreference = "Stop"
$bundle = "src-tauri\target\release\bundle"

# Old installers would otherwise stay next to the new ones.
Remove-Item -Recurse -Force $bundle -ErrorAction SilentlyContinue

pnpm tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& "$PSScriptRoot\package-portable.ps1"

$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
$release = Join-Path $bundle "release"
New-Item -ItemType Directory -Force $release | Out-Null
foreach ($msi in Get-ChildItem "$bundle\msi\*.msi") {
    $lang = ($msi.BaseName -split "_")[-1]
    Copy-Item $msi.FullName (Join-Path $release "remote-http-action_${version}_x64_$lang.msi")
}
Copy-Item "$bundle\portable\remote-http-action_${version}_x64_portable.zip" $release

$sums = Get-ChildItem $release -File | Where-Object Name -ne "SHA256SUMS.txt" | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
}
# LF line endings and no BOM, as produced by sha256sum.
[IO.File]::WriteAllText((Join-Path $release "SHA256SUMS.txt"), (($sums -join "`n") + "`n"))

Write-Host "Release files:"
Get-ChildItem $release | Select-Object Name, Length | Format-Table -AutoSize | Out-String | Write-Host
