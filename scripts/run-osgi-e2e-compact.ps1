param(
    [string] $LauncherPath,
    [string] $LogDirectory,
    [int] $TimeoutSeconds = 300,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $LauncherArgs
)

$ErrorActionPreference = 'Stop'
if (-not $LauncherPath) { $LauncherPath = Join-Path $PSScriptRoot 'run-osgi-e2e.ps1' }
if (-not $LogDirectory) { $LogDirectory = Join-Path (Split-Path -Parent $PSScriptRoot) 'build/e2e-logs' }
$null = New-Item -ItemType Directory -Force -Path $LogDirectory
$LogDirectory = (Resolve-Path -LiteralPath $LogDirectory).Path
$LauncherPath = [IO.Path]::GetFullPath($LauncherPath)
$runStamp = (Get-Date).ToString('yyyyMMdd-HHmmssfff')
$logPath = Join-Path $LogDirectory ("osgi-e2e-{0}.log" -f $runStamp)
$summaryPath = Join-Path $LogDirectory 'summary.txt'
$stdoutPath = Join-Path $LogDirectory ("osgi-e2e-{0}.stdout.log" -f $runStamp)
$stderrPath = Join-Path $LogDirectory ("osgi-e2e-{0}.stderr.log" -f $runStamp)
$payloadPath = Join-Path $LogDirectory ("osgi-e2e-{0}.args.clixml" -f $runStamp)
$powershellCmd = if ($PSVersionTable.PSEdition -eq 'Core') { 'pwsh' } else { 'powershell' }
$exitCode = 1
$timedOut = $false
$runnerProcess = $null
$runnerProcessId = $null

if ($TimeoutSeconds -le 0) {
    throw 'TimeoutSeconds must be a positive number of seconds'
}

function Stop-RunnerProcessTree([System.Diagnostics.Process] $Process) {
    if ($null -eq $Process) { return }
    if (-not $Process.HasExited) {
            if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
                $taskkillArgs = @('/PID', [string]$Process.Id, '/T', '/F')
                & taskkill.exe @taskkillArgs *> $null
                if ($LASTEXITCODE -ne 0 -and -not $Process.HasExited) {
                    throw "Could not terminate runner process tree at PID $($Process.Id)"
                }
            } else {
                $killWithTree = $Process.GetType().GetMethod('Kill', [Type[]]@([bool]))
                if ($null -ne $killWithTree) {
                    $Process.Kill($true)
                } else {
                    $Process.Kill()
                }
            }
    }
}

function Restore-ProcessEnvironmentValue([string] $Name, [AllowNull()][string] $Value) {
    if ($null -eq $Value) {
        Remove-Item -LiteralPath ("Env:{0}" -f $Name) -ErrorAction SilentlyContinue
    } else {
        [Environment]::SetEnvironmentVariable($Name, $Value, 'Process')
    }
}

function Append-CapturedOutput([string] $CapturePath, [string] $DestinationPath) {
    if (-not (Test-Path -LiteralPath $CapturePath)) { return }
    $captured = [IO.File]::ReadAllText($CapturePath)
    if ($captured.Length -gt 0) {
        [IO.File]::AppendAllText($DestinationPath, $captured)
    }
}

$null = New-Item -ItemType File -Force -Path $logPath
$runId = [guid]::NewGuid().ToString('N')
$argsEnvironmentName = "MCDEV_COMPACT_ARGS_FILE_$runId"
$oldArgsEnvironmentValue = [Environment]::GetEnvironmentVariable($argsEnvironmentName, 'Process')
$payload = [pscustomobject]@{
    PowerShellCommand = $powershellCmd
    LauncherPath = $LauncherPath
    LauncherArgs = @($LauncherArgs)
    LogPath = $logPath
}
$payload | Export-Clixml -LiteralPath $payloadPath -Force

$wrapper = @'
$ErrorActionPreference = 'Stop'
$payload = $null
$launcherLogPath = $null
try {
    $argsPath = [Environment]::GetEnvironmentVariable('__ARGS_ENVIRONMENT_NAME__', 'Process')
    if ([string]::IsNullOrWhiteSpace($argsPath)) {
        throw 'runner argument payload path was not provided'
    }
    $payload = Import-Clixml -LiteralPath $argsPath
    $launcherLogPath = [string]$payload.LogPath
    $launcherCommand = [string]$payload.PowerShellCommand
    $launcherPath = [string]$payload.LauncherPath
    $launcherArgs = @($payload.LauncherArgs)
    # Native tools may write warnings to stderr while exiting successfully.
    # Preserve that output and use the process exit code to determine success.
    $ErrorActionPreference = 'Continue'
    & $launcherCommand -NoProfile -ExecutionPolicy Bypass -File $launcherPath @launcherArgs *> $launcherLogPath
    $childExitCode = if ($null -ne $LASTEXITCODE) { [int]$LASTEXITCODE } else { 0 }
    exit $childExitCode
} catch {
    $errorText = $_ | Out-String
    if (-not [string]::IsNullOrWhiteSpace($launcherLogPath)) {
        $errorText | Out-File -LiteralPath $launcherLogPath -Append -Encoding utf8
    }
    exit 1
}
'@
$wrapper = $wrapper.Replace('__ARGS_ENVIRONMENT_NAME__', $argsEnvironmentName)
$encodedWrapper = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($wrapper))

try {
    [Environment]::SetEnvironmentVariable($argsEnvironmentName, $payloadPath, 'Process')
    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $powershellCmd
    $startInfo.Arguments = "-NoProfile -ExecutionPolicy Bypass -EncodedCommand $encodedWrapper"
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $runnerProcess = New-Object System.Diagnostics.Process
    $runnerProcess.StartInfo = $startInfo
    $null = $runnerProcess.Start()
    $runnerProcessId = $runnerProcess.Id
    $stdoutTask = $runnerProcess.StandardOutput.ReadToEndAsync()
    $stderrTask = $runnerProcess.StandardError.ReadToEndAsync()
    $timeoutMilliseconds = if ($TimeoutSeconds -ge ([int]::MaxValue / 1000)) {
        [int]::MaxValue
    } else {
        [int]($TimeoutSeconds * 1000)
    }
    if (-not $runnerProcess.WaitForExit($timeoutMilliseconds)) {
        $timedOut = $true
        Stop-RunnerProcessTree $runnerProcess
        if (-not $runnerProcess.WaitForExit(5000)) {
            throw "Runner PID $runnerProcessId did not exit after termination"
        }
        $exitCode = 124
        Add-Content -LiteralPath $logPath -Value ("timeout: exceeded {0} seconds; terminated process tree rooted at PID {1}" -f $TimeoutSeconds, $runnerProcessId)
    } else {
        $exitCode = [int]$runnerProcess.ExitCode
    }
} catch {
    $_ | Out-File -FilePath $logPath -Append -Encoding utf8
    if ($null -ne $runnerProcess) {
        Stop-RunnerProcessTree $runnerProcess
    }
} finally {
    Restore-ProcessEnvironmentValue $argsEnvironmentName $oldArgsEnvironmentValue
}

foreach ($capture in @(
        [pscustomobject]@{ Task = $stdoutTask; Path = $stdoutPath },
        [pscustomobject]@{ Task = $stderrTask; Path = $stderrPath })) {
    if ($null -eq $capture.Task) { continue }
    try {
        if ($capture.Task.Wait(5000)) {
            [IO.File]::WriteAllText($capture.Path, [string]$capture.Task.Result, (New-Object Text.UTF8Encoding($false)))
        }
    } catch {
        $_ | Out-File -FilePath $logPath -Append -Encoding utf8
    }
}
Append-CapturedOutput $stdoutPath $logPath
Append-CapturedOutput $stderrPath $logPath
if ($null -ne $runnerProcess) {
    $runnerProcess.Dispose()
}
Remove-Item -LiteralPath $payloadPath, $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue

function Limit-Text([string] $Value, [int] $Maximum = 300) {
    if ($null -eq $Value) { return '' }
    $compact = [regex]::Replace($Value, '\s+', ' ').Trim()
    if ($compact.Length -gt $Maximum) { return $compact.Substring(0, $Maximum) }
    return $compact
}

$compilerErrors = [System.Collections.Generic.List[string]]::new()
foreach ($line in @(Get-Content -LiteralPath $logPath -ErrorAction SilentlyContinue)) {
    $trimmed = $line.Trim()
    $isCompilerError = $trimmed -match '(?i)\.(java|kt|kts):\s*(?:\(\s*)?\d+(?::|\s*,)' -or
        $trimmed -match '(?i)^e:\s+.*\.(java|kt|kts):' -or
        $trimmed -match '(?i)compilation failed' -or
        $trimmed -match '(?i)execution failed for task .*compile'
    if ($isCompilerError -and $compilerErrors.Count -lt 5) {
        $compilerErrors.Add((Limit-Text $trimmed))
    }
}

$summaryLines = [System.Collections.Generic.List[string]]::new()
$summaryLines.Add("exit code: $exitCode")
$statusLine = if ($timedOut) { "status: timed out after $TimeoutSeconds seconds" } else { 'status: completed' }
$summaryLines.Add($statusLine)
if ($timedOut) {
    $summaryLines.Add("termination: terminated process tree rooted at PID $runnerProcessId")
}
$summaryLines.Add('updated tests: unavailable (E2E launcher)')
if ($exitCode -ne 0) {
    foreach ($compilerError in @($compilerErrors | Select-Object -First 5)) {
        $summaryLines.Add("compiler error: $compilerError")
    }
    if ($compilerErrors.Count -eq 0) {
        $summaryLines.Add('failure: see log for details')
    }
}
$summaryLines.Add("summary path: $summaryPath")
$summaryLines.Add("log path: $logPath")

[System.IO.File]::WriteAllLines($summaryPath, [string[]]$summaryLines, (New-Object System.Text.UTF8Encoding($false)))
$summaryLines | Write-Output
exit $exitCode
