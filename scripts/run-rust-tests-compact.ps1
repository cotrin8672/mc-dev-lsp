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
$editorProject = $null
try {
    $steps = @(
        @{ Name = 'fmt'; Command = 'cargo'; Arguments = @('fmt', '--all', '--', '--check'); Timeout = 120 },
        @{ Name = 'tests'; Command = 'cargo'; Arguments = @('test', '--workspace', '--all-targets', '--locked'); Timeout = 900 },
        @{ Name = 'build'; Command = 'cargo'; Arguments = @('build', '--bin', 'mcdev-ktlsp', '--locked'); Timeout = 900 }
    )
    if (-not $SkipEditor) {
        $binary = Join-Path $repo ('target/debug/mcdev-ktlsp' + $(if ($IsWindows) { '.exe' }))
        $env:MCDEV_SMOKE_BINARY = $binary
        # The host scans Git-visible sources; a fixture under ignored build/ would be invisible.
        $editorProject = Join-Path ([System.IO.Path]::GetTempPath()) ('mcdev-editor-' + [guid]::NewGuid())
        $env:MCDEV_SMOKE_PROJECT = $editorProject
        $env:MCDEV_SMOKE_ARTIFACTS = $logs
        $env:KTLSP_CACHE_DIR = Join-Path $logs 'cache'
        $env:XDG_STATE_HOME = Join-Path $logs 'editor-state'
        # Primitive-only smoke needs a bounded SDK fixture, rather than parsing the installed JDK.
        $env:KTLSP_JDK_SRC = Join-Path $logs 'jdk-src.zip'
        $jdk = [System.IO.Compression.ZipFile]::Open($env:KTLSP_JDK_SRC, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            $entry = $jdk.CreateEntry('java.base/java/lang/Object.java')
            $writer = [System.IO.StreamWriter]::new($entry.Open())
            try { $writer.Write('package java.lang; public class Object {}') } finally { $writer.Dispose() }
        } finally { $jdk.Dispose() }
        $steps += @{ Name = 'editor'; Command = 'nvim'; Arguments = @('--headless', '-u', 'NONE', '-l', (Join-Path $repo 'mcdev-ktlsp/tests/editor_smoke.lua')); Timeout = 120 }
    }
    foreach ($step in $steps) {
        $result = Invoke-RustCheck $step.Name $step.Command $step.Arguments $repo $logs $step.Timeout
        $line = '{0}: exit={1} timeout={2} seconds={3} pid={4}' -f $result.Name, $result.ExitCode, $result.TimedOut, $result.Seconds, $result.Pid
        $results.Add($line)
        Write-Output $line
        if ($result.ExitCode -ne 0) { throw "$($step.Name) failed; see complete logs" }
    }
    $exitCode = 0
} catch {
    $results.Add($_.Exception.Message)
    Write-Output $_.Exception.Message
} finally {
    if ($editorProject -and (Test-Path -LiteralPath $editorProject)) {
        $resolved = [System.IO.Path]::GetFullPath($editorProject)
        $temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
        if ((Split-Path -Parent $resolved) -eq $temporaryRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) -and
            (Split-Path -Leaf $resolved) -like 'mcdev-editor-*') {
            Remove-Item -LiteralPath $resolved -Recurse -Force
        } else { throw "Refusing to remove unexpected fixture path: $resolved" }
    }
    $results.Add("exit code: $exitCode")
    $results.Add("logs: $logs")
    $results | Set-Content -LiteralPath (Join-Path $logs 'summary.txt')
    Write-Output "logs: $logs"
}
exit $exitCode
