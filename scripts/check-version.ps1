<#
.SYNOPSIS
  Checks that package.json, src-tauri/Cargo.toml and src-tauri/tauri.conf.json carry the same
  version and, if -Tag is given, that it matches the tag (v1.2.3).
#>
param([string]$Tag = "")

$ErrorActionPreference = "Stop"
$versions = [ordered]@{
    "package.json"              = (Get-Content package.json -Raw | ConvertFrom-Json).version
    "src-tauri/tauri.conf.json" = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
    "src-tauri/Cargo.toml"      = ([regex]::Match((Get-Content src-tauri/Cargo.toml -Raw), '(?m)^version\s*=\s*"([^"]+)"')).Groups[1].Value
}
$versions.GetEnumerator() | ForEach-Object { "{0,-27} {1}" -f $_.Key, $_.Value }

$unique = @($versions.Values | Select-Object -Unique)
if ($unique.Count -ne 1) {
    Write-Error "Versions differ between files."
    exit 1
}
if ($Tag -and $Tag.TrimStart("v") -ne $unique[0]) {
    Write-Error "Tag $Tag does not match version $($unique[0])."
    exit 1
}
"OK: $($unique[0])"
