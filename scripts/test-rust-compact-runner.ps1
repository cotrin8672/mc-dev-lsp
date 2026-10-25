#requires -Version 7.0
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo ('build/rust-test-logs/runner-check-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$pwsh = (Get-Process -Id $PID).Path
$failure = Invoke-RustCheck 'failure' $pwsh @('-NoProfile', '-Command', '[Console]::Out.WriteLine("retained stdout"); [Console]::Error.WriteLine("retained stderr"); exit 17') $repo $logs 10
if ($failure.ExitCode -ne 17 -or $failure.TimedOut) { throw 'Nonzero exit was not preserved' }
if ((Get-Content (Join-Path $logs 'failure.stdout.log') -Raw) -notmatch 'retained stdout' -or
    (Get-Content (Join-Path $logs 'failure.stderr.log') -Raw) -notmatch 'retained stderr') { throw 'Full streams were not retained' }
$timeoutScript = Join-Path $logs 'timeout-child.ps1'
@'
param($ChildPidFile)
$info = [System.Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
foreach ($argument in @('-NoProfile', '-Command', 'Start-Sleep -Seconds 60')) { $info.ArgumentList.Add($argument) }
$child = [System.Diagnostics.Process]::Start($info)
[System.IO.File]::WriteAllText($ChildPidFile, [string]$child.Id)
Start-Sleep -Seconds 60
'@ | Set-Content -LiteralPath $timeoutScript
$childPidFile = Join-Path $logs 'child.pid'
$timeout = Invoke-RustCheck 'timeout' $pwsh @('-NoProfile', '-File', $timeoutScript, $childPidFile) $repo $logs 3
if ($timeout.ExitCode -ne 124 -or -not $timeout.TimedOut -or $timeout.Seconds -gt 10) { throw 'Timeout was not bounded' }
if (Get-Process -Id $timeout.Pid -ErrorAction SilentlyContinue) { throw 'Owned timed-out process survived' }
$childPid = [int](Get-Content -LiteralPath $childPidFile -Raw)
if (Get-Process -Id $childPid -ErrorAction SilentlyContinue) { throw 'Owned descendant survived timeout' }
foreach ($name in @('MCDEV_FACADE_CASE', 'MCDEV_TWO_OPS_TEST_PORT')) {
    $previous = [Environment]::GetEnvironmentVariable($name, 'Process')
    try {
        Set-Item -LiteralPath "Env:$name" -Value 'runner-guard-check'
        $guard = Invoke-RustCheck "guard-$name" $pwsh @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1'), '-SkipEditor') $repo $logs 10
        if ($guard.ExitCode -ne 1 -or $guard.TimedOut) { throw "$name did not reject an incomplete full-suite run" }
        if ((Get-Content -LiteralPath (Join-Path $logs "guard-$name.stdout.log") -Raw) -notlike "*requires $name to be absent*") {
            throw "$name did not fail at the full-suite environment gate"
        }
    } finally {
        if ($null -eq $previous) { Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue }
        else { Set-Item -LiteralPath "Env:$name" -Value $previous }
    }
}
Write-Output "runner checks: PASS (failure, retained streams, timeout, owned process tree exit, unfiltered suite gate); logs: $logs"
