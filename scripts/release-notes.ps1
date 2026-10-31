<#
.SYNOPSIS
  Extracts the section of a version from CHANGELOG.md (the "## [1.2.3] - date" heading up to the
  next "## " heading) and writes it to -Out. Fails if the version has no section.
#>
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Out = "release-notes.md"
)

$ErrorActionPreference = "Stop"
$Version = $Version.TrimStart("v")
$lines = Get-Content CHANGELOG.md -Encoding UTF8
$start = -1
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match "^## \[$([regex]::Escape($Version))\]") { $start = $i + 1; break }
}
if ($start -lt 0) {
    Write-Error "CHANGELOG.md has no section for $Version."
    exit 1
}
$end = $lines.Count
for ($i = $start; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match "^## ") { $end = $i; break }
}
$notes = ($lines[$start..($end - 1)] -join "`n").Trim()
if (-not $notes) {
    Write-Error "The CHANGELOG.md section for $Version is empty."
    exit 1
}
[IO.File]::WriteAllText($Out, $notes + "`n")
Write-Host $notes
