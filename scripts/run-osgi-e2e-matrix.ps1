param([string] $InputsReceipt = $env:MCDEV_E2E_INPUTS_RECEIPT)
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($InputsReceipt)) {
    throw 'Native E2E matrix prerequisite: MCDEV_E2E_INPUTS_RECEIPT or -InputsReceipt must select the real fixed-input receipt for physical Mixin 0.8.7 and MixinExtras Fabric 0.5.5 JARs'
}
if (-not (Test-Path -LiteralPath $InputsReceipt -PathType Leaf)) {
    throw "Native E2E matrix prerequisite: fixed-input receipt is not a file: $InputsReceipt"
}
$env:MCDEV_E2E_INPUTS_RECEIPT = (Resolve-Path -LiteralPath $InputsReceipt).Path

$fixtures = @('fabric-basic', 'fabric-mixinextras', 'fabric-aw-at', 'multi-source-set', 'forge-basic')
foreach ($fixture in $fixtures) {
    Write-Host "running native e2e for $fixture"
    $env:MCDEV_E2E_FIXTURE = $fixture
    & (Join-Path $PSScriptRoot 'run-osgi-e2e.ps1')
}
Write-Host 'native e2e matrix passed'
