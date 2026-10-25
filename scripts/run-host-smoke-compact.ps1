#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $HostBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $HostSha256,
    [Parameter(Mandatory = $true)][string] $CompanionBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $CompanionSha256
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$logs = Join-Path $repo ('build/rust-test-logs/host-smoke-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$project = Join-Path $temporaryRoot ('mcdev-host-smoke-' + [guid]::NewGuid())
$exitCode = 1
try {
    $hostPath = (Resolve-Path -LiteralPath $HostBinary).Path
    $companionPath = (Resolve-Path -LiteralPath $CompanionBinary).Path
    if ((Get-FileHash -LiteralPath $hostPath -Algorithm SHA256).Hash -ne $HostSha256) {
        throw 'Stock host checksum mismatch'
    }
    if ((Get-FileHash -LiteralPath $companionPath -Algorithm SHA256).Hash -ne $CompanionSha256) {
        throw 'Companion checksum mismatch'
    }
    $env:MCDEV_SMOKE_BINARY = $hostPath
    $env:MCDEV_SMOKE_COMPANION = $companionPath
    $env:MCDEV_SMOKE_PROJECT = $project
    $env:MCDEV_SMOKE_ARTIFACTS = $logs
    $env:KTLSP_CACHE_DIR = Join-Path $logs 'cache'
    $env:XDG_STATE_HOME = Join-Path $logs 'editor-state'
    $env:KTLSP_JDK_SRC = Join-Path $logs 'jdk-src.zip'
    $jdk = [IO.Compression.ZipFile]::Open($env:KTLSP_JDK_SRC, [IO.Compression.ZipArchiveMode]::Create)
    try {
        $entry = $jdk.CreateEntry('java.base/java/lang/Object.java')
        $writer = [IO.StreamWriter]::new($entry.Open())
        try { $writer.Write('package java.lang; public class Object {}') } finally { $writer.Dispose() }
    } finally { $jdk.Dispose() }

    $editor = Join-Path $repo 'mcdev-companion/tests/editor_smoke.lua'
    $inputs = @($editor, (Join-Path $repo 'mcdev-companion/tests/support/accepted_trace.lua'),
        $hostPath, $companionPath, $PSCommandPath, (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1'))
    $before = Get-FileHash -LiteralPath $inputs -Algorithm SHA256
    $before | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.before.json')
    $result = Invoke-RustCheck -Name editor -Command nvim -Arguments @(
        '--headless', '-u', 'NONE', '-l', $editor
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds 120
    $result | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'summary.json')
    $after = Get-FileHash -LiteralPath $inputs -Algorithm SHA256
    $after | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.after.json')
    if (@(Compare-Object $before $after -Property Path,Hash).Count -ne 0) {
        throw 'Host smoke inputs changed during execution'
    }
    if ($result.ExitCode -ne 0 -or $result.TimedOut) { throw 'Host smoke failed; see complete logs' }
    $proof = Get-Content -LiteralPath (Join-Path $logs 'editor-proof.json') -Raw | ConvertFrom-Json
    if ($null -ne $proof.error -or $proof.checks -ne 112 -or @($proof.requests).Count -ne 29 -or
        -not $proof.shutdown.clean -or -not $proof.shutdown.exited -or
        $proof.shutdown.exit_code -ne 0 -or $proof.shutdown.signal -ne 0) {
        throw 'Host smoke did not complete every expected check and clean shutdown'
    }
    [ordered]@{
        checks = $proof.checks
        requests = @($proof.requests).Count
        accepted = @($proof.accepted).Count
        seconds = $result.Seconds
        pid = $result.Pid
        deadlineSeconds = 120
        hostSha256 = $HostSha256.ToLowerInvariant()
        companionSha256 = $CompanionSha256.ToLowerInvariant()
        inputHashesStable = $true
        scope = 'semantic-v1 Shadow slice; completion identity edits at fully typed names'
        fullMcCapabilitiesVerified = $false
    } | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'receipt.json')
    $result | ConvertTo-Json -Compress
    $exitCode = 0
} catch {
    Write-Output $_.Exception.Message
    $stderr = Join-Path $logs 'editor.stderr.log'
    if (Test-Path -LiteralPath $stderr) { Get-Content -LiteralPath $stderr -Tail 24 }
} finally {
    if (Test-Path -LiteralPath $project) {
        $resolved = [IO.Path]::GetFullPath($project)
        if ((Split-Path -Parent $resolved) -ne $temporaryRoot -or
            (Split-Path -Leaf $resolved) -notlike 'mcdev-host-smoke-*') {
            throw "Refusing unexpected fixture cleanup path: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    Write-Output "logs: $logs"
}
exit $exitCode
