<#
.SYNOPSIS
  End-to-end smoke test: starts the real executable with an isolated data folder,
  configures it through the CLI and exercises the HTTP API.

.DESCRIPTION
  Power actions run in dry-run mode (RHA_DRY_RUN_POWER=1), so the machine is not shut down.
  Requires a debug build (CLI access is enabled by default in debug builds):
    cargo build --manifest-path src-tauri/Cargo.toml --bins
#>
param(
    [string]$Exe = "src-tauri\target\debug\remote-http-action.exe",
    [string]$CliExe = "src-tauri\target\debug\remote-http-action-cli.exe",
    [int]$Port = 18765
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$Exe = (Resolve-Path $Exe).Path
$CliExe = (Resolve-Path $CliExe).Path
$failures = 0

function Check([string]$name, [bool]$ok, [string]$details = "") {
    if ($ok) { Write-Host "PASS  $name" }
    else { Write-Host "FAIL  $name $details"; $script:failures++ }
}

function Rha {
    $out = & $CliExe @args --json 2>&1 | Out-String
    $code = $LASTEXITCODE
    $json = $null
    try { $json = $out | ConvertFrom-Json } catch { }
    [pscustomobject]@{ Code = $code; Json = $json; Raw = $out }
}

function Http([string]$method, [string]$path, [string]$key) {
    $headers = @{}
    if ($key) { $headers["Authorization"] = "Bearer $key" }
    try {
        $r = Invoke-WebRequest -Method $method -Uri "http://127.0.0.1:$Port$path" -Headers $headers -UseBasicParsing
        [pscustomobject]@{ Status = [int]$r.StatusCode; Json = ($r.Content | ConvertFrom-Json) }
    } catch {
        $resp = $_.Exception.Response
        if (-not $resp) { throw }
        $body = $_.ErrorDetails.Message
        $json = $null
        try { $json = $body | ConvertFrom-Json } catch { }
        [pscustomobject]@{ Status = [int]$resp.StatusCode; Json = $json }
    }
}

$data = Join-Path $env:TEMP ("rha-smoke-" + [guid]::NewGuid())
New-Item -ItemType Directory $data | Out-Null
$env:RHA_DATA_DIR = $data
$env:RHA_DRY_RUN_POWER = "1"
$env:RHA_IPC_ENDPOINT = "\\.\pipe\rha-smoke-" + [guid]::NewGuid()

$before = Rha system info
Check "CLI reports a missing instance (exit 3)" ($before.Code -eq 3) $before.Raw

# Headless mode: the CLI and HTTP server without a window, so the test also runs in non-interactive sessions.
$proc = Start-Process -FilePath $Exe -ArgumentList "--headless" -PassThru -WindowStyle Hidden
try {
    $info = $null
    for ($i = 0; $i -lt 60; $i++) {
        $info = Rha system info
        if ($info.Code -eq 0) { break }
        Start-Sleep -Milliseconds 500
    }
    Check "instance answers over the CLI pipe" ($info.Code -eq 0) $info.Raw
    if ($info.Code -ne 0) { throw "application did not start" }
    Check "data folder override is used" ($info.Json.paths.data_dir -eq $data)

    $r = Rha settings set server.port $Port
    Check "settings set server.port" ($r.Code -eq 0 -and $r.Json.server.port -eq $Port) $r.Raw
    Start-Sleep -Milliseconds 500

    $flowFile = Join-Path $data "flow.json"
    @{
        name    = "Smoke"
        path    = "smoke/test"
        actions = @(
            @{ type = "cli_command"; params = @{ shell = "cmd"; command = "echo Привет" }; capture_output = $true },
            @{ type = "delay"; params = @{ seconds = 0 } },
            @{ type = "sleep"; params = @{} }
        )
    } | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 $flowFile
    $flow = Rha flows create --file $flowFile
    Check "flows create" ($flow.Code -eq 0 -and $flow.Json.path -eq "smoke/test") $flow.Raw

    $group = Rha groups create smoke --flows $flow.Json.id
    Check "groups create" ($group.Code -eq 0) $group.Raw
    $key = Rha keys create phone --groups $group.Json.id
    Check "keys create returns a secret" ($key.Code -eq 0 -and $key.Json.secret -like "rha_*") $key.Raw
    $secret = $key.Json.secret

    $s = Http GET "/status/" $null
    Check "public status" ($s.Status -eq 200 -and $s.Json.status -eq "ok")
    $s = Http GET "/status/" $secret
    Check "detailed status with key" ($s.Status -eq 200 -and $s.Json.version)

    $s = Http POST "/flow/smoke/test/" $null
    Check "flow without key is 401" ($s.Status -eq 401)
    $s = Http POST "/flow/smoke/test/" "rha_wrong"
    Check "flow with wrong key is 401" ($s.Status -eq 401)

    $s = Http POST "/flow/smoke/test/" $secret
    Check "flow runs over HTTP" ($s.Status -eq 200 -and $s.Json.status -eq "success") ($s | ConvertTo-Json -Depth 6)
    Check "command output is returned and decoded" ($s.Json.steps[0].stdout -like "*Привет*") $s.Json.steps[0].stdout
    Check "all steps reported" ($s.Json.steps.Count -eq 3)

    $s = Http GET "/get-allowed/" $secret
    Check "get-allowed lists the flow" ($s.Status -eq 200 -and $s.Json.flows[0].path -eq "smoke/test")

    $runs = Rha runs list --limit 5
    Check "run history records key name" ($runs.Code -eq 0 -and $runs.Json[0].api_key_name -eq "phone" -and $runs.Json[0].source -eq "http") $runs.Raw

    $run = Rha flows run smoke/test --wait
    Check "flows run --wait over CLI" ($run.Code -eq 0 -and $run.Json.status -eq "success" -and $run.Json.source -eq "cli") $run.Raw

    $main = & $Exe keys list --json 2>&1 | Out-String
    Check "main executable also accepts CLI commands" ($LASTEXITCODE -eq 0 -and $main -like "*phone*") $main

    $r = Rha keys delete 99999
    Check "missing item gives exit 6" ($r.Code -eq 6) $r.Raw
    $r = Rha settings set cli.enabled false
    Check "CLI cannot disable itself (exit 5)" ($r.Code -eq 5) $r.Raw
    $r = Rha settings set server.port '"x"'
    Check "invalid value gives exit 7" ($r.Code -eq 7) $r.Raw

    $r = Rha app quit
    Check "app quit" ($r.Code -eq 0) $r.Raw
    Check "process exits" ($proc.WaitForExit(15000))
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
}

if ($failures -gt 0) { Write-Host "$failures check(s) failed"; exit 1 }
Write-Host "All smoke checks passed"
