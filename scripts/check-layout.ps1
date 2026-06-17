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

$memberResolutionFiles = @(
    "mcdev-core/src/main/kotlin/io/github/mcdev/core/at/AtMemberResolver.kt",
    "mcdev-core/src/main/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionService.kt",
    "mcdev-core/src/main/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionService.kt",
    "mcdev-core/src/main/kotlin/io/github/mcdev/core/mixin/MixinDefinitionService.kt"
)

foreach ($path in $memberResolutionFiles) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing member resolution file: $path"
    }
    $source = Get-Content -Raw -LiteralPath $path
    if ($source -match "\.first\(\)" -or $source -match "firstOrNull\(\)\s*\?:" ) {
        throw "Forbidden first-overload fallback in member resolution file: $path"
    }
}

$resolverSource = Get-Content -Raw -LiteralPath "mcdev-core/src/main/kotlin/io/github/mcdev/core/mixin/MemberResolver.kt"
foreach ($marker in @("EXACT_DESCRIPTOR_REQUIRED", "SINGLE_CANDIDATE_IF_DESCRIPTOR_MISSING", "ALLOW_AMBIGUOUS_FOR_COMPLETION")) {
    if (-not $resolverSource.Contains($marker)) {
        throw "MemberResolver is missing ambiguity policy marker: $marker"
    }
}

Write-Host "layout check passed"
