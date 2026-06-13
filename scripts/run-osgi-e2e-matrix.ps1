$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$fixtures = @(
    "fabric-basic",
    "fabric-mixinextras",
    "fabric-aw-at",
    "multi-source-set",
    "forge-basic"
)

Push-Location $repoRoot
try {
    & gradle :mcdev-jdtls-extension:jar :mcdev-jdtls-extension:checkBundle
    if ($LASTEXITCODE -ne 0) { throw "bundle build failed" }

    foreach ($fixture in $fixtures) {
        Write-Host "running osgi e2e for $fixture"
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "prepare-e2e-workspace.ps1") -Fixture $fixture
        if ($LASTEXITCODE -ne 0) { throw "workspace preparation failed for $fixture" }

        $bundleJar = Join-Path $repoRoot "mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar"
        $workspace = Join-Path $repoRoot "build/e2e-workspace"
        $jdtlsCmd = $env:JDTLS_CMD
        if (-not $jdtlsCmd) {
            $masonJdtls = Join-Path $env:LOCALAPPDATA "nvim-data/mason/bin/jdtls.cmd"
            if (Test-Path -LiteralPath $masonJdtls) {
                $jdtlsCmd = $masonJdtls
            } else {
                $jdtlsCmd = "jdtls"
            }
        }

        $env:MCDEV_BUNDLE_JAR = $bundleJar
        $env:MCDEV_E2E_WORKSPACE = $workspace
        $env:MCDEV_E2E_FIXTURE = $fixture
        $env:JDTLS_CMD = $jdtlsCmd

        nvim --headless `
            -u (Join-Path $repoRoot "mcdev-nvim/tests/e2e/minimal_init.lua") `
            -c "luafile mcdev-nvim/tests/e2e/jdtls_bundle_e2e.lua"
        if ($LASTEXITCODE -ne 0) { throw "osgi e2e failed for $fixture" }
    }

    Write-Host "osgi bundle e2e matrix passed"
} finally {
    Pop-Location
}
