$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $env:MCDEV_E2E_KTLSP = Resolve-E2eExecutable 'MCDEV_E2E_KTLSP'
    $env:MCDEV_E2E_COMPANION = Resolve-E2eExecutable 'MCDEV_E2E_COMPANION'
    $nvimCmd = Resolve-E2eExecutable 'NVIM_CMD'
    $env:NVIM_CMD = $nvimCmd
    $gradleCmd = Resolve-E2eExecutable 'GRADLE_CMD'
    $env:GRADLE_CMD = $gradleCmd

    $fixture = if ($env:MCDEV_E2E_FIXTURE) { $env:MCDEV_E2E_FIXTURE } else { "fabric-basic" }
    if ($fixture -in @('real-cem', 'real-sodium', 'create-drill-definition-diagnostics')) {
        if (-not $env:MCDEV_E2E_WORKSPACE -or -not (Test-Path -LiteralPath $env:MCDEV_E2E_WORKSPACE -PathType Container)) {
            throw "MCDEV_E2E_WORKSPACE must point to an existing $fixture workspace"
        }
        $workspace = (Resolve-Path -LiteralPath $env:MCDEV_E2E_WORKSPACE).Path
    } else {
        & (Join-Path $PSScriptRoot "prepare-e2e-workspace.ps1") -Fixture $fixture
        $workspace = Join-Path $repoRoot "build/e2e-workspace"
    }

    $env:MCDEV_E2E_WORKSPACE = $workspace
    $env:MCDEV_E2E_FIXTURE = $fixture

    & $nvimCmd --headless `
        -u (Join-Path $repoRoot "mcdev-nvim/tests/e2e/minimal_init.lua") `
        -c "luafile mcdev-nvim/tests/e2e/run_bundle_e2e.lua"
    if ($LASTEXITCODE -ne 0) { throw "native ktlsp companion e2e failed" }

    Write-Host "native ktlsp companion e2e passed"
} finally {
    Pop-Location
}
