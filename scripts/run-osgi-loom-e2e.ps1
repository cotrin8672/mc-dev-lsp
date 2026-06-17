$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    & gradle :mcdev-jdtls-extension:jar :mcdev-jdtls-extension:checkBundle :mcdev-test-fixtures:jar
    if ($LASTEXITCODE -ne 0) { throw "bundle build failed" }

    $runGenSources = $false
    if ($args -contains "-RunGenSources" -or $env:MCDEV_LOOM_RUN_GEN_SOURCES -eq "1") {
        $runGenSources = $true
    }

    if ($runGenSources) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "prepare-loom-e2e-workspace.ps1") -RunGenSources
    } else {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "prepare-loom-e2e-workspace.ps1")
    }
    if ($LASTEXITCODE -ne 0) { throw "loom workspace preparation failed" }

    $bundleJar = Get-ChildItem -LiteralPath (Join-Path $repoRoot "mcdev-jdtls-extension/build/libs") `
        -Filter "io.github.mcdev.jdtls-*.jar" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $bundleJar) { throw "bundle jar not found" }
    $workspace = Join-Path $repoRoot "build/e2e-loom-workspace"
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
    $env:JDTLS_CMD = $jdtlsCmd
    if ($env:MCDEV_JDTLS_JAVA_HOME) {
        $env:JAVA_HOME = $env:MCDEV_JDTLS_JAVA_HOME
        $env:PATH = (Join-Path $env:MCDEV_JDTLS_JAVA_HOME "bin") + [IO.Path]::PathSeparator + $env:PATH
    }

    nvim --headless `
        -u (Join-Path $repoRoot "mcdev-nvim/tests/e2e/minimal_init.lua") `
        -c "luafile mcdev-nvim/tests/e2e/jdtls_loom_e2e.lua"
    if ($LASTEXITCODE -ne 0) { throw "loom osgi e2e failed" }

    Write-Host "loom osgi bundle e2e passed"
} finally {
    Pop-Location
}
