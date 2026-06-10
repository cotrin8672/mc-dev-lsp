$ErrorActionPreference = "Stop"

$required = @(
    "settings.gradle.kts",
    "build.gradle.kts",
    "mcdev-core/build.gradle.kts",
    "mcdev-protocol/build.gradle.kts",
    "mcdev-jdtls-extension/build.gradle.kts",
    "mcdev-nvim/lua/mcdev/init.lua",
    "mcdev-nvim/lua/mcdev/protocol.lua",
    "mcdev-test-fixtures/build.gradle.kts"
)

foreach ($path in $required) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing required path: $path"
    }
}

$manifestSource = Get-Content -Raw -LiteralPath "mcdev-jdtls-extension/build.gradle.kts"
foreach ($marker in @(
    "Bundle-SymbolicName",
    "io.github.mcdev.jdtls",
    "Bundle-Activator",
    "io.github.mcdev.jdtls.McdevPlugin"
)) {
    if (-not $manifestSource.Contains($marker)) {
        throw "Missing bundle manifest marker: $marker"
    }
}

$luaSource = Get-Content -Raw -LiteralPath "mcdev-nvim/lua/mcdev/protocol.lua"
foreach ($forbidden in @("parseDescriptor", "remap", "scanJar")) {
    if ($luaSource.Contains($forbidden)) {
        throw "Lua transport contains forbidden semantic marker: $forbidden"
    }
}

Write-Host "layout check passed"
