#requires -Version 7.0
param([string] $Name = 'adapters', [string] $Entry = 'mcdev-nvim/tests/run.lua',
      [int] $TimeoutSeconds = 90, [switch] $Suite)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$logs = Join-Path $repo 'build/nvim-test-logs'
$null = New-Item -ItemType Directory -Force -Path $logs
function Invoke-ClientEntry([string] $RunName, [string] $RunEntry, [int] $Deadline) {
    $env:MCDEV_ASSERTION_LOG = Join-Path $logs "$RunName.assertions.json"
    $result = Invoke-RustCheck -Name $RunName -Command 'nvim' -Arguments @(
        '--headless', '-u', 'mcdev-nvim/tests/minimal_init.lua', '-l', $RunEntry
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds $Deadline
    # Neovim may report a VimLeavePre error while still returning exit code zero.
    $errorText = Get-Content -LiteralPath (Join-Path $logs "$RunName.stderr.log") -Raw
    if ($result.ExitCode -eq 0 -and $errorText -match '(?m)(^Error detected|E\d{3,}:|stack traceback:)') {
        $result.ExitCode = 1
    }
    $result | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs "$RunName.result.json")
    $result | ConvertTo-Json -Compress
    if ($result.ExitCode -ne 0) {
        Get-Content -LiteralPath (Join-Path $logs "$RunName.stderr.log") -Tail 18
        exit $result.ExitCode
    }
}
if ($Suite) {
    $null = New-Item -ItemType Directory -Force -Path (Join-Path $repo 'target/debug/examples')
    $fixtureName = 'teardown_process' + $(if ($IsWindows) { '.exe' } else { '' })
    $build = Invoke-RustCheck -Name "$Name-fixture-build" -Command 'rustc' -Arguments @(
        '--edition', '2021', 'mcdev-companion/examples/teardown_process.rs',
        '-o', (Join-Path 'target/debug/examples' $fixtureName)
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds 60
    $build | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs "$Name-fixture-build.result.json")
    $build | ConvertTo-Json -Compress
    if ($build.ExitCode -ne 0) { exit $build.ExitCode }
    $oldCompanionBinary = [Environment]::GetEnvironmentVariable('MCDEV_COMPANION_BINARY', 'Process')
    if ($null -eq $oldCompanionBinary) {
        $companionName = 'mcdev-companion' + $(if ($IsWindows) { '.exe' } else { '' })
        $env:MCDEV_COMPANION_BINARY = Join-Path $repo ('target/debug/' + $companionName)
    }
    Invoke-ClientEntry "$Name-syntax" 'mcdev-nvim/tests/syntax_load.lua' 20
    Invoke-ClientEntry "$Name-adapters" 'mcdev-nvim/tests/run.lua' 90
    Invoke-ClientEntry "$Name-helper" 'mcdev-nvim/tests/native_helper.lua' 30
    Invoke-ClientEntry "$Name-teardown-unit" 'mcdev-nvim/tests/e2e/teardown_test.lua' 60
    Invoke-ClientEntry "$Name-teardown-process" 'mcdev-nvim/tests/e2e/teardown_process_test.lua' 110
    if ($null -eq $oldCompanionBinary) { Remove-Item Env:MCDEV_COMPANION_BINARY -ErrorAction SilentlyContinue }
} else {
    Invoke-ClientEntry $Name $Entry $TimeoutSeconds
}
