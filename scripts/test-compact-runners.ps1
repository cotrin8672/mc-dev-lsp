$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$powershellCmd = if ($PSVersionTable.PSEdition -eq 'Core') { 'pwsh' } else { 'powershell' }
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('mcdev-compact-' + [guid]::NewGuid().ToString('N'))

function Assert-True([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw $Message }
}

function Assert-Contains([string] $Text, [string] $Needle, [string] $Message) {
    Assert-True ($Text.Contains($Needle)) ("{0}: missing '{1}'" -f $Message, $Needle)
}

function Wait-ProcessExit([int] $ProcessId, [int] $TimeoutMilliseconds = 5000) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($null -eq (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)) {
            return $true
        }
        Start-Sleep -Milliseconds 100
    }
    return $null -eq (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
}

function Assert-PathUnderRoot([string] $Root, [string] $Target, [string] $Message) {
    $rootPath = [IO.Path]::GetFullPath($Root)
    while ($rootPath.EndsWith('\') -or $rootPath.EndsWith('/')) {
        $rootPath = $rootPath.Substring(0, $rootPath.Length - 1)
    }
    $targetPath = [IO.Path]::GetFullPath($Target)
    $rootPrefix = $rootPath + [IO.Path]::DirectorySeparatorChar
    Assert-True $targetPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase) $Message
}

function Get-SummaryPath([string] $Text) {
    $line = @($Text -split "`r?`n" | Where-Object { $_ -like 'summary path: *' } | Select-Object -First 1)
    if ($line.Count -eq 0) { throw 'summary path was not printed' }
    return $line[0].Substring('summary path: '.Length).Trim()
}

function Invoke-CompactGradle([string] $CaseName) {
    $caseRoot = Join-Path $testRoot ('gradle-' + $CaseName)
    $scriptRoot = Join-Path $caseRoot 'scripts'
    $null = New-Item -ItemType Directory -Force -Path $scriptRoot
    Copy-Item -LiteralPath (Join-Path $repoRoot 'scripts/run-gradle-tests-compact.ps1') -Destination (Join-Path $scriptRoot 'run-gradle-tests-compact.ps1')

    $fakeGradle = Join-Path $caseRoot 'fake-gradle.ps1'
    $fakeGradleText = @'
$xmlDirectory = Join-Path $env:FAKE_ROOT 'module/build/test-results/test'
switch ($env:FAKE_CASE) {
    'success' {
        New-Item -ItemType Directory -Force -Path $xmlDirectory | Out-Null
        $xml = "<testsuite name='fake' tests='2' failures='0' errors='0' skipped='0'><testcase classname='FakeTest' name='passes'/><testcase classname='FakeTest' name='alsoPasses'/></testsuite>"
        [IO.File]::WriteAllText((Join-Path $xmlDirectory 'TEST-fake.xml'), $xml, (New-Object Text.UTF8Encoding($false)))
    }
    'failure' {
        New-Item -ItemType Directory -Force -Path $xmlDirectory | Out-Null
        $message = 'x' * 400
        $cases = ''
        foreach ($number in 1..4) {
            $cases += "<testcase classname='FakeTest' name='fails$number'><failure message='$message'>detail</failure></testcase>"
        }
        $xml = "<testsuite name='fake' tests='4' failures='4' errors='0' skipped='0'>$cases</testsuite>"
        [IO.File]::WriteAllText((Join-Path $xmlDirectory 'TEST-fake.xml'), $xml, (New-Object Text.UTF8Encoding($false)))
        exit 1
    }
    'malformed' {
        New-Item -ItemType Directory -Force -Path $xmlDirectory | Out-Null
        [IO.File]::WriteAllText((Join-Path $xmlDirectory 'TEST-fake.xml'), '<testsuite', (New-Object Text.UTF8Encoding($false)))
        exit 1
    }
    'compile' {
        foreach ($number in 1..7) { Write-Output "C:\src\Thing.java:$number`:3: error: broken" }
        Write-Output 'compilation failed'
        exit 1
    }
    'old' { }
}
exit 0
'@
    [IO.File]::WriteAllText($fakeGradle, $fakeGradleText, (New-Object Text.UTF8Encoding($false)))

    if ($CaseName -eq 'old') {
        $oldDirectory = Join-Path $caseRoot 'old/build/test-results/test'
        $null = New-Item -ItemType Directory -Force -Path $oldDirectory
        $oldXml = "<testsuite name='old' tests='9' failures='0' errors='0' skipped='0'></testsuite>"
        [IO.File]::WriteAllText((Join-Path $oldDirectory 'TEST-old.xml'), $oldXml, (New-Object Text.UTF8Encoding($false)))
    }

    $oldGradle = $env:GRADLE_CMD
    $oldCase = $env:FAKE_CASE
    $oldRoot = $env:FAKE_ROOT
    try {
        $env:GRADLE_CMD = $fakeGradle
        $env:FAKE_CASE = $CaseName
        $env:FAKE_ROOT = $caseRoot
        $output = @(& $powershellCmd -NoProfile -ExecutionPolicy Bypass -File (Join-Path $scriptRoot 'run-gradle-tests-compact.ps1') 2>&1)
        $code = [int]$LASTEXITCODE
        return [pscustomobject]@{ Code = $code; Text = (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine); Root = $caseRoot }
    } finally {
        $env:GRADLE_CMD = $oldGradle
        $env:FAKE_CASE = $oldCase
        $env:FAKE_ROOT = $oldRoot
    }
}

function Invoke-CompactE2e(
    [string] $CaseName,
    [string] $LauncherPath,
    [string] $LogDirectory,
    [Nullable[int]] $TimeoutSeconds = $null,
    [string] $ChildPidFile = $null
) {
    $oldCase = $env:FAKE_E2E_CASE
    $oldChildPidFile = $env:FAKE_E2E_CHILD_PID_FILE
    try {
        $env:FAKE_E2E_CASE = $CaseName
        if ($null -eq $ChildPidFile) {
            Remove-Item Env:FAKE_E2E_CHILD_PID_FILE -ErrorAction SilentlyContinue
        } else {
            $env:FAKE_E2E_CHILD_PID_FILE = $ChildPidFile
        }
        $runnerArgs = @('-LauncherPath', $LauncherPath, '-LogDirectory', $LogDirectory)
        if ($null -ne $TimeoutSeconds) {
            $runnerArgs += @('-TimeoutSeconds', [string]$TimeoutSeconds)
        }
        $output = @(& $powershellCmd -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repoRoot 'scripts/run-osgi-e2e-compact.ps1') @runnerArgs 2>&1)
        return [pscustomobject]@{ Code = [int]$LASTEXITCODE; Text = (($output | ForEach-Object { [string]$_ }) -join [Environment]::NewLine) }
    } finally {
        $env:FAKE_E2E_CASE = $oldCase
        if ($null -eq $oldChildPidFile) {
            Remove-Item Env:FAKE_E2E_CHILD_PID_FILE -ErrorAction SilentlyContinue
        } else {
            $env:FAKE_E2E_CHILD_PID_FILE = $oldChildPidFile
        }
    }
}

try {
    $null = New-Item -ItemType Directory -Force -Path $testRoot

    $success = Invoke-CompactGradle 'success'
    Assert-True ($success.Code -eq 0) 'success case should exit zero'
    Assert-Contains $success.Text 'updated XML: 1' 'success case'
    Assert-Contains $success.Text 'updated tests: 2, failures: 0, errors: 0, skipped: 0' 'success case'
    $successSummary = Get-SummaryPath $success.Text
    Assert-True (Test-Path -LiteralPath $successSummary) 'success summary should exist'
    Assert-Contains (Get-Content -Raw -LiteralPath $successSummary) 'updated tests: 2' 'success summary'

    $failure = Invoke-CompactGradle 'failure'
    Assert-True ($failure.Code -ne 0) 'failure case should fail'
    $failureLines = @($failure.Text -split "`r?`n" | Where-Object { $_ -like 'failed: *' })
    Assert-True ($failureLines.Count -eq 3) 'failure case should report three tests'
    foreach ($line in $failureLines) {
        Assert-True ($line.Length -le 330) 'failure message should be bounded'
    }

    $compile = Invoke-CompactGradle 'compile'
    Assert-True ($compile.Code -ne 0) 'compile case should fail'
    $compileLines = @($compile.Text -split "`r?`n" | Where-Object { $_ -like 'compiler error: *' })
    Assert-True ($compileLines.Count -eq 5) 'compile case should report five compiler errors'
    Assert-True (-not $compile.Text.Contains('Thing.java:6')) 'compile case should omit later compiler errors'
    $compileLog = @($compile.Text -split "`r?`n" | Where-Object { $_ -like 'log path: *' })[0].Substring('log path: '.Length).Trim()
    Assert-True (Test-Path -LiteralPath $compileLog) 'compile log should exist'

    $malformed = Invoke-CompactGradle 'malformed'
    Assert-True ($malformed.Code -ne 0) 'malformed case should fail'
    Assert-Contains $malformed.Text 'report unavailable: 1' 'malformed case'
    Assert-Contains $malformed.Text 'counts: unavailable (updated JUnit XML unreadable)' 'malformed case'

    $old = Invoke-CompactGradle 'old'
    Assert-True ($old.Code -eq 0) 'up-to-date case should exit zero'
    Assert-Contains $old.Text 'updated XML: 0' 'up-to-date case'
    Assert-Contains $old.Text 'counts: unavailable (no updated JUnit XML; up-to-date)' 'up-to-date case'
    Assert-True (-not $old.Text.Contains('updated tests: 9')) 'old XML must not be reported as current'

    $fakeLauncher = Join-Path $testRoot 'fake e2e launcher.ps1'
    $fakeLauncherText = @'
$powershell = if ($PSVersionTable.PSEdition -eq 'Core') { 'pwsh' } else { 'powershell' }
if ($env:FAKE_E2E_CASE -eq 'hung') {
    $child = Start-Process -FilePath $powershell -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', 'Start-Sleep -Seconds 600') -WindowStyle Hidden -PassThru
    [IO.File]::WriteAllText($env:FAKE_E2E_CHILD_PID_FILE, [string]$child.Id, (New-Object Text.UTF8Encoding($false)))
    while ($true) { Start-Sleep -Seconds 1 }
}
if ($env:FAKE_E2E_CASE -eq 'compile') {
    foreach ($number in 1..7) { Write-Output "C:\src\Mixin.java:$number`:1: error: broken" }
    Write-Output 'compilation failed'
    exit 1
}
Write-Output 'e2e passed'
[Console]::Error.WriteLine('non-fatal tool warning')
exit 0
'@
    [IO.File]::WriteAllText($fakeLauncher, $fakeLauncherText, (New-Object Text.UTF8Encoding($false)))
    $e2eLogs = Join-Path $testRoot 'e2e logs with spaces'
    $e2eFailure = Invoke-CompactE2e 'compile' $fakeLauncher $e2eLogs
    Assert-True ($e2eFailure.Code -ne 0) 'E2E compile case should fail'
    $e2eCompilerLines = @($e2eFailure.Text -split "`r?`n" | Where-Object { $_ -like 'compiler error: *' })
    Assert-True ($e2eCompilerLines.Count -eq 5) 'E2E should report five compiler errors'
    $e2eSuccess = Invoke-CompactE2e 'success' $fakeLauncher $e2eLogs
    Assert-True ($e2eSuccess.Code -eq 0) 'E2E success case should exit zero'

    $hungChildPidFile = Join-Path $testRoot 'hung child.pid'
    $hungChildPid = $null
    $unrelated = $null
    try {
        $unrelated = Start-Process -FilePath $powershellCmd -ArgumentList @(
            '-NoProfile',
            '-ExecutionPolicy', 'Bypass',
            '-Command', 'Start-Sleep -Seconds 600'
        ) -WindowStyle Hidden -PassThru
        $hung = Invoke-CompactE2e 'hung' $fakeLauncher $e2eLogs 3 $hungChildPidFile
        Assert-True ($hung.Code -eq 124) 'hung E2E case should exit 124'
        Assert-Contains $hung.Text 'status: timed out after 3 seconds' 'hung E2E case'
        $hungSummary = Get-SummaryPath $hung.Text
        Assert-True (Test-Path -LiteralPath $hungSummary) 'hung summary should exist'
        $hungSummaryText = Get-Content -Raw -LiteralPath $hungSummary
        Assert-Contains $hungSummaryText 'exit code: 124' 'hung summary'
        Assert-Contains $hungSummaryText 'terminated process tree' 'hung summary'
        $hungLog = @($hung.Text -split "`r?`n" | Where-Object { $_ -like 'log path: *' })[0].Substring('log path: '.Length).Trim()
        Assert-True (Test-Path -LiteralPath $hungLog) 'hung log should exist'
        Assert-True (Test-Path -LiteralPath $hungChildPidFile) 'hung launcher should report its child PID'
        [int]$hungChildPid = 0
        $hungChildPidText = (Get-Content -Raw -LiteralPath $hungChildPidFile).Trim()
        Assert-True ([int]::TryParse($hungChildPidText, [ref]$hungChildPid)) 'hung child PID should be numeric'
        Assert-True ($hungChildPid -ne $unrelated.Id) 'hung child should differ from unrelated process'
        Assert-True (Wait-ProcessExit $hungChildPid 5000) 'hung launcher child should be terminated'
        $unrelated.Refresh()
        Assert-True (-not $unrelated.HasExited) 'unrelated process should remain alive'
    } finally {
        if ($null -ne $hungChildPid -and $hungChildPid -gt 0 -and ($null -eq $unrelated -or $hungChildPid -ne $unrelated.Id)) {
            Stop-Process -Id $hungChildPid -Force -ErrorAction SilentlyContinue
        }
        if ($null -ne $unrelated) {
            Stop-Process -Id $unrelated.Id -Force -ErrorAction SilentlyContinue
        }
    }

    Write-Output 'compact runner checks passed'
} finally {
    Assert-PathUnderRoot ([IO.Path]::GetTempPath()) $testRoot 'test root must stay under the system temp root'
    $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
    if (Test-Path -LiteralPath $resolvedTestRoot) {
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
