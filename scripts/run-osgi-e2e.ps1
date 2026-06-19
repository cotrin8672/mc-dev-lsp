$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $gradleCmd = $env:GRADLE_CMD
    if (-not $gradleCmd) {
        $miseGradle = Join-Path $env:LOCALAPPDATA "mise/installs/gradle/9.5.1/gradle-9.5.1/bin/gradle.bat"
        if (Test-Path -LiteralPath $miseGradle) {
            $gradleCmd = $miseGradle
        } else {
            $gradleCmd = "gradle"
        }
    }

    if ($env:MCDEV_SKIP_BUNDLE_BUILD -ne "1") {
        & $gradleCmd :mcdev-jdtls-extension:jar :mcdev-jdtls-extension:checkBundle
        if ($LASTEXITCODE -ne 0) { throw "bundle build failed" }
    }

    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "prepare-e2e-workspace.ps1")
    if ($LASTEXITCODE -ne 0) { throw "workspace preparation failed" }

    $bundleJar = Get-ChildItem -LiteralPath (Join-Path $repoRoot "mcdev-jdtls-extension/build/libs") `
        -Filter "io.github.mcdev.jdtls-*.jar" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $bundleJar) { throw "bundle jar not found" }
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
    $env:JDTLS_CMD = $jdtlsCmd
    if ($env:MCDEV_JDTLS_JAVA_HOME) {
        $env:JAVA_HOME = $env:MCDEV_JDTLS_JAVA_HOME
        $env:PATH = (Join-Path $env:MCDEV_JDTLS_JAVA_HOME "bin") + [IO.Path]::PathSeparator + $env:PATH
    }

    $nvimCmd = $env:NVIM_CMD
    if (-not $nvimCmd) {
        $miseNvim = Join-Path $env:LOCALAPPDATA "mise/installs/neovim/0.12.2/nvim-win64/bin/nvim.exe"
        if (Test-Path -LiteralPath $miseNvim) {
            $nvimCmd = $miseNvim
        } else {
            $nvimCmd = "nvim"
        }
    }

    & $nvimCmd --headless `
        -u (Join-Path $repoRoot "mcdev-nvim/tests/e2e/minimal_init.lua") `
        -c "luafile mcdev-nvim/tests/e2e/jdtls_bundle_e2e.lua"
    if ($LASTEXITCODE -ne 0) { throw "osgi e2e failed" }

    Write-Host "osgi bundle e2e passed"
} finally {
    Pop-Location
}
