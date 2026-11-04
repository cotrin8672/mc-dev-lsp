$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')

$repoRoot = Split-Path -Parent $PSScriptRoot
$readinessEnvironment = @{}
foreach ($name in @('MCDEV_E2E_ACCEPTANCE_PROOF', 'KTLSP_TRACE')) {
    $readinessEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
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
    $proofDirectory = Assert-E2eBuildPath (Join-Path $repoRoot ('build/e2e-proofs/' + $fixture + '-' + [guid]::NewGuid().ToString('N')))
    $null = New-Item -ItemType Directory -Force -Path $proofDirectory
    if ([string]::IsNullOrWhiteSpace($env:MCDEV_E2E_ACCEPTANCE_PROOF)) {
        $env:MCDEV_E2E_ACCEPTANCE_PROOF = Join-Path $proofDirectory 'editor-proof.json'
    }
    if ([string]::IsNullOrWhiteSpace($env:KTLSP_TRACE)) {
        $env:KTLSP_TRACE = Join-Path $proofDirectory 'host.trace.jsonl'
    }
    Write-Host ('editor acceptance proof: ' + $env:MCDEV_E2E_ACCEPTANCE_PROOF)

    & $nvimCmd --headless `
        -u (Join-Path $repoRoot "mcdev-nvim/tests/e2e/minimal_init.lua") `
        -c "luafile mcdev-nvim/tests/e2e/run_bundle_e2e.lua"
    if ($LASTEXITCODE -ne 0) { throw "native ktlsp companion e2e failed" }

    Write-Host "native ktlsp companion e2e passed"
} finally {
    foreach ($name in $readinessEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $readinessEnvironment[$name], 'Process')
    }
    Pop-Location
}
