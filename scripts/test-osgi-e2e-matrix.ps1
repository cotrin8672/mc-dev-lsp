#requires -Version 7.0
param([Parameter(Mandatory = $true)][string] $InputsReceipt)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo ('build/rust-test-logs/matrix-receipt-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$receipt = (Resolve-Path -LiteralPath $InputsReceipt).Path
# Verify real receipt bytes and copy both physical JARs using the production preparation boundary.
Copy-E2eFixedDependencies $receipt (Join-Path $logs 'physical-inputs')
$mirror = Join-Path $logs 'launcher-mock'
$null = New-Item -ItemType Directory -Force -Path $mirror
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'run-osgi-e2e-matrix.ps1') -Destination $mirror
$entry = @'
# Mock observer of launcher forwarding only: no host, Gradle, compiler or E2E is executed.
[IO.File]::AppendAllText($env:MCDEV_MATRIX_TEST_PROOF,
    $env:MCDEV_E2E_FIXTURE + "`t" + $env:MCDEV_E2E_INPUTS_RECEIPT + "`n")
'@
$entry | Set-Content -LiteralPath (Join-Path $mirror 'run-osgi-e2e.ps1') -Encoding utf8
$matrix = Join-Path $mirror 'run-osgi-e2e-matrix.ps1'
$proof = Join-Path $logs 'forwarded.tsv'
$pwsh = (Get-Process -Id $PID).Path
$savedReceipt = [Environment]::GetEnvironmentVariable('MCDEV_E2E_INPUTS_RECEIPT', 'Process')
$savedProof = [Environment]::GetEnvironmentVariable('MCDEV_MATRIX_TEST_PROOF', 'Process')
$results = [System.Collections.Generic.List[object]]::new()
try {
    $env:MCDEV_MATRIX_TEST_PROOF = $proof
    [Environment]::SetEnvironmentVariable('MCDEV_E2E_INPUTS_RECEIPT', $null, 'Process')
    $missing = Invoke-RustCheck 'missing-receipt' $pwsh @('-NoProfile', '-File', $matrix) $repo $logs 20
    $results.Add($missing)
    $failure = [IO.File]::ReadAllText((Join-Path $logs 'missing-receipt.stderr.log'))
    if ($missing.TimedOut -or $missing.ExitCode -eq 0 -or
        -not $failure.Contains('MCDEV_E2E_INPUTS_RECEIPT or -InputsReceipt') -or (Test-Path -LiteralPath $proof)) {
        throw 'Missing receipt must fail at launcher prerequisite before any fixture starts'
    }
    $invalid = Invoke-RustCheck 'nonexistent-receipt' $pwsh @('-NoProfile', '-File', $matrix,
        '-InputsReceipt', (Join-Path $logs 'absent-receipt.json')) $repo $logs 20
    $results.Add($invalid)
    $failure = [IO.File]::ReadAllText((Join-Path $logs 'nonexistent-receipt.stderr.log'))
    if ($invalid.TimedOut -or $invalid.ExitCode -eq 0 -or
        -not $failure.Contains('fixed-input receipt is not a file') -or (Test-Path -LiteralPath $proof)) {
        throw 'Nonexistent receipt must fail at launcher prerequisite before any fixture starts'
    }
    foreach ($mode in @('flag', 'environment')) {
        $arguments = @('-NoProfile', '-File', $matrix)
        if ($mode -eq 'flag') {
            $env:MCDEV_E2E_INPUTS_RECEIPT = Join-Path $logs 'wrong-environment.json'
            $arguments += @('-InputsReceipt', $receipt)
        } else {
            $env:MCDEV_E2E_INPUTS_RECEIPT = $receipt
        }
        $result = Invoke-RustCheck "forward-$mode" $pwsh $arguments $repo $logs 20
        $results.Add($result)
        if ($result.ExitCode -ne 0 -or $result.TimedOut) { throw "Receipt $mode forwarding failed" }
        $rows = @(Get-Content -LiteralPath $proof)
        $fixtures = @('fabric-basic', 'fabric-mixinextras', 'fabric-aw-at', 'multi-source-set', 'forge-basic')
        if ($rows.Count -ne $fixtures.Count) { throw 'Matrix must preserve all original fixture invocations' }
        for ($index = 0; $index -lt $fixtures.Count; $index++) {
            if ($rows[$index] -ne ($fixtures[$index] + "`t" + $receipt)) {
                throw "Matrix did not forward the selected real receipt to $($fixtures[$index])"
            }
        }
        Remove-Item -LiteralPath $proof
    }
    $results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $logs 'results.json') -Encoding utf8
    Write-Output "matrix receipt support: 2 prerequisite failures + flag/env forwarding through 5 mocked fixtures each; real physical JAR inputs verified; logs: $logs"
} finally {
    [Environment]::SetEnvironmentVariable('MCDEV_E2E_INPUTS_RECEIPT', $savedReceipt, 'Process')
    [Environment]::SetEnvironmentVariable('MCDEV_MATRIX_TEST_PROOF', $savedProof, 'Process')
}
