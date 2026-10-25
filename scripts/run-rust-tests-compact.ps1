#requires -Version 7.0
param([switch] $SkipEditor)

$ErrorActionPreference = 'Stop'

function Invoke-RustCheck {
    param([string] $Name, [string] $Command, [string[]] $Arguments,
          [string] $Directory, [string] $LogDirectory, [int] $TimeoutSeconds)
    $stdout = Join-Path $LogDirectory "$Name.stdout.log"
    $stderr = Join-Path $LogDirectory "$Name.stderr.log"
    $info = [System.Diagnostics.ProcessStartInfo]::new($Command)
    $info.WorkingDirectory = $Directory
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $info
    $outFile = [System.IO.File]::Create($stdout)
    $errFile = [System.IO.File]::Create($stderr)
    $started = $false
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $started = $process.Start()
        $outCopy = $process.StandardOutput.BaseStream.CopyToAsync($outFile)
        $errCopy = $process.StandardError.BaseStream.CopyToAsync($errFile)
        $timedOut = -not $process.WaitForExit($TimeoutSeconds * 1000)
        if ($timedOut) {
            $process.Kill($true)
            if (-not $process.WaitForExit(5000)) { throw "$Name did not stop after timeout" }
        }
        if (-not [System.Threading.Tasks.Task]::WaitAll(@($outCopy, $errCopy), 5000)) {
            throw "$Name log streams did not close"
        }
        $code = if ($timedOut) { 124 } else { $process.ExitCode }
        return [pscustomobject]@{
            Name = $Name; ExitCode = $code; TimedOut = $timedOut; Pid = $process.Id
            Seconds = [math]::Round($watch.Elapsed.TotalSeconds, 1)
        }
    } finally {
        if ($started -and -not $process.HasExited) {
            $process.Kill($true)
            $null = $process.WaitForExit(5000)
        }
        $outFile.Dispose()
        $errFile.Dispose()
        $process.Dispose()
    }
}

if ($MyInvocation.InvocationName -eq '.') { return }

$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo ('build/rust-test-logs/' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$results = [System.Collections.Generic.List[string]]::new()
$exitCode = 1
try {
    foreach ($name in @('MCDEV_FACADE_CASE', 'MCDEV_TWO_OPS_TEST_PORT')) {
        if ([Environment]::GetEnvironmentVariables('Process').Contains($name)) {
            throw "Full-suite runner requires $name to be absent, including an empty value"
        }
    }
    $steps = @(
        @{ Name = 'fmt'; Command = 'cargo'; Arguments = @('fmt', '--all', '--', '--check'); Timeout = 120 },
        @{ Name = 'tests'; Command = 'cargo'; Arguments = @('test', '--workspace', '--locked', '--all-targets', '--', '--test-threads=1'); Timeout = 900 },
        @{ Name = 'doc'; Command = 'cargo'; Arguments = @('test', '--workspace', '--locked', '--doc', '--', '--test-threads=1'); Timeout = 900 },
        @{ Name = 'companion-build'; Command = 'cargo'; Arguments = @('build', '--locked', '-p', 'mcdev-companion', '--bin', 'mcdev-companion'); Timeout = 900 }
    )
    if (-not $SkipEditor) {
        $pwsh = (Get-Process -Id $PID).Path
        $steps += @{ Name = 'nvim-suite'; Command = $pwsh; Arguments = @(
            '-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
            (Join-Path $repo 'scripts/run-nvim-tests-compact.ps1'), '-Name', 'native-ci', '-Suite'
        ); Timeout = 420 }
    }
    foreach ($step in $steps) {
        $result = Invoke-RustCheck $step.Name $step.Command $step.Arguments $repo $logs $step.Timeout
        $line = '{0}: exit={1} timeout={2} seconds={3} pid={4}' -f $result.Name, $result.ExitCode, $result.TimedOut, $result.Seconds, $result.Pid
        $results.Add($line)
        Write-Output $line
        if ($result.ExitCode -ne 0) {
            Get-Content -LiteralPath (Join-Path $logs "$($step.Name).stderr.log") -Tail 40
            throw "$($step.Name) failed; see complete logs"
        }
    }
    $exitCode = 0
} catch {
    $results.Add($_.Exception.Message)
    Write-Output $_.Exception.Message
} finally {
    $results.Add("exit code: $exitCode")
    $results.Add("logs: $logs")
    $results | Set-Content -LiteralPath (Join-Path $logs 'summary.txt')
    Write-Output "logs: $logs"
}
exit $exitCode
