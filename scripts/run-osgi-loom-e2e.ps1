param([switch] $RunGenSources)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $env:MCDEV_E2E_KTLSP = Resolve-E2eExecutable 'MCDEV_E2E_KTLSP'
    $env:MCDEV_E2E_COMPANION = Resolve-E2eExecutable 'MCDEV_E2E_COMPANION'
    $env:NVIM_CMD = Resolve-E2eExecutable 'NVIM_CMD'
    $env:GRADLE_CMD = Resolve-E2eExecutable 'GRADLE_CMD'
    & (Join-Path $PSScriptRoot 'prepare-loom-e2e-workspace.ps1') -RunGenSources:$RunGenSources
    $env:MCDEV_E2E_WORKSPACE = Join-Path $repoRoot 'build/e2e-loom-workspace'
    $env:MCDEV_E2E_FIXTURE = 'fabric-loom-e2e'
    & $env:NVIM_CMD --headless -u (Join-Path $repoRoot 'mcdev-nvim/tests/e2e/minimal_init.lua') `
        -c 'luafile mcdev-nvim/tests/e2e/jdtls_loom_e2e.lua'
    if ($LASTEXITCODE -ne 0) { throw 'native Loom e2e failed' }
    Write-Host 'native Loom e2e passed'
} finally { Pop-Location }
