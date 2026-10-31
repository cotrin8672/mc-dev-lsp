$ErrorActionPreference = 'Stop'
$fixtures = @('fabric-basic', 'fabric-mixinextras', 'fabric-aw-at', 'multi-source-set', 'forge-basic')
foreach ($fixture in $fixtures) {
    Write-Host "running native e2e for $fixture"
    $env:MCDEV_E2E_FIXTURE = $fixture
    & (Join-Path $PSScriptRoot 'run-osgi-e2e.ps1')
}
Write-Host 'native e2e matrix passed'
