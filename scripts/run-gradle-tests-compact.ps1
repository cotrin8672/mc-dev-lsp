param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $GradleArgs
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$logDirectory = Join-Path $repoRoot 'build/test-logs'
$runStarted = Get-Date
$runStamp = $runStarted.ToString('yyyyMMdd-HHmmssfff')
$logPath = Join-Path $logDirectory ("gradle-tests-{0}.log" -f $runStamp)
$summaryPath = Join-Path $logDirectory 'summary.txt'
$null = New-Item -ItemType Directory -Force -Path $logDirectory
$xmlPattern = Join-Path $repoRoot '*/build/test-results/test/TEST-*.xml'

function Get-XmlFingerprint([System.IO.FileInfo] $File) {
    try {
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $File.FullName).Hash
    } catch {
        $hash = ''
    }
    return "{0}|{1}|{2}" -f $File.LastWriteTimeUtc.Ticks, $File.Length, $hash
}

function Limit-Text([AllowNull()][string] $Value, [int] $Maximum = 300) {
    if ($null -eq $Value) {
        return ''
    }
    $compact = [regex]::Replace($Value, '\s+', ' ').Trim()
    if ($compact.Length -gt $Maximum) {
        return $compact.Substring(0, $Maximum)
    }
    return $compact
}

$xmlSnapshot = @{}
foreach ($xmlFile in @(Get-ChildItem -Path $xmlPattern -File -ErrorAction SilentlyContinue)) {
    $xmlSnapshot[$xmlFile.FullName] = Get-XmlFingerprint $xmlFile
}

$gradleCmd = if ($env:GRADLE_CMD) { $env:GRADLE_CMD } else { 'gradle' }
$invokeArgs = @($GradleArgs) + @('--console=plain', '--no-daemon')
$exitCode = 1

Push-Location $repoRoot
try {
    $savedErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $global:LASTEXITCODE = 1
        & $gradleCmd @invokeArgs *> $logPath
        if ($null -ne $LASTEXITCODE) {
            $exitCode = [int]$LASTEXITCODE
        }
    } catch {
        $_ | Out-File -FilePath $logPath -Append -Encoding utf8
    } finally {
        $ErrorActionPreference = $savedErrorActionPreference
    }
} finally {
    Pop-Location
}

$xmlFiles = @(Get-ChildItem -Path $xmlPattern -File -ErrorAction SilentlyContinue |
    Where-Object {
        $before = $xmlSnapshot[$_.FullName]
        $null -eq $before -or $before -ne (Get-XmlFingerprint $_)
    } | Sort-Object FullName)

$totals = [ordered]@{ tests = [long]0; failures = [long]0; errors = [long]0; skipped = [long]0 }
$failedTests = [System.Collections.Generic.List[object]]::new()
$suiteCount = 0
$reportUnavailable = 0

foreach ($xmlFile in $xmlFiles) {
    try {
        $document = New-Object System.Xml.XmlDocument
        $document.Load($xmlFile.FullName)
        $suites = @($document.SelectNodes('//testsuite'))
        if ($suites.Count -eq 0) {
            throw 'JUnit XML has no testsuite element'
        }
        foreach ($suite in $suites) {
            if ($null -eq $suite) { continue }
            $suiteCount++
            foreach ($key in @($totals.Keys)) {
                $attribute = $suite.Attributes.GetNamedItem($key)
                [long] $value = 0
                if ($attribute -and [long]::TryParse([string]$attribute.Value, [ref]$value)) {
                    $totals[$key] += $value
                }
            }
            foreach ($testCase in @($suite.SelectNodes('./testcase'))) {
                $failure = $testCase.SelectSingleNode('./failure')
                $errorNode = $testCase.SelectSingleNode('./error')
                if ($failure -or $errorNode) {
                    $detail = if ($failure) { $failure } else { $errorNode }
                    $message = [string]$detail.GetAttribute('message')
                    if ([string]::IsNullOrWhiteSpace($message)) {
                        $message = [string]$detail.InnerText
                    }
                    $name = '{0}/{1}' -f [string]$testCase.GetAttribute('classname'), [string]$testCase.GetAttribute('name')
                    $failedTests.Add([pscustomobject]@{ Name = $name; Message = Limit-Text $message })
                }
            }
        }
    } catch {
        $reportUnavailable++
        Add-Content -LiteralPath $logPath -Value ("report unavailable: {0}: {1}" -f $xmlFile.FullName, $_.Exception.Message)
    }
}

$logLines = @(Get-Content -LiteralPath $logPath -ErrorAction SilentlyContinue)
if ($exitCode -ne 0 -and $failedTests.Count -eq 0) {
    foreach ($line in $logLines) {
        $trimmed = $line.Trim()
        if ($trimmed.Contains(' > ') -and $trimmed.EndsWith(' FAILED')) {
            $failedTests.Add([pscustomobject]@{ Name = $trimmed.Substring(0, $trimmed.Length - 7).Trim(); Message = '' })
        }
    }
}

$compilerErrors = [System.Collections.Generic.List[string]]::new()
foreach ($line in $logLines) {
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
$summaryLines.Add("updated XML: $($xmlFiles.Count)")
if ($xmlFiles.Count -eq 0) {
    if ($exitCode -eq 0) {
        $summaryLines.Add('counts: unavailable (no updated JUnit XML; up-to-date)')
    } else {
        $summaryLines.Add('counts: unavailable (no updated JUnit XML)')
    }
} elseif ($suiteCount -eq 0) {
    $summaryLines.Add('counts: unavailable (updated JUnit XML unreadable)')
} else {
    $summaryLines.Add("updated tests: $($totals.tests), failures: $($totals.failures), errors: $($totals.errors), skipped: $($totals.skipped)")
}
if ($reportUnavailable -gt 0) {
    $summaryLines.Add("report unavailable: $reportUnavailable")
}
if ($exitCode -ne 0) {
    foreach ($failedTest in @($failedTests | Select-Object -First 3)) {
        $message = if ([string]::IsNullOrWhiteSpace($failedTest.Message)) { 'no failure message' } else { $failedTest.Message }
        $summaryLines.Add("failed: $($failedTest.Name) - $message")
    }
    foreach ($compilerError in @($compilerErrors | Select-Object -First 5)) {
        $summaryLines.Add("compiler error: $compilerError")
    }
    if ($failedTests.Count -eq 0 -and $compilerErrors.Count -eq 0) {
        $summaryLines.Add('failure: see log for details')
    }
}
$summaryLines.Add("summary path: $summaryPath")
$summaryLines.Add("log path: $logPath")

[System.IO.File]::WriteAllLines($summaryPath, [string[]]$summaryLines, (New-Object System.Text.UTF8Encoding($false)))
$summaryLines | Write-Output
exit $exitCode
