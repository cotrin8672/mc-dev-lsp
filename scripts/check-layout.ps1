$ErrorActionPreference = "Stop"

$required = @(
    "Cargo.toml",
    "Cargo.lock",
    "mcdev-core/Cargo.toml",
    "mcdev-core/src/lib.rs",
    "mcdev-protocol/Cargo.toml",
    "mcdev-protocol/src/lib.rs",
    "mcdev-companion/Cargo.toml",
    "mcdev-companion/src/lib.rs",
    "mcdev-companion/src/main.rs",
    "mcdev-nvim/lua/mcdev/init.lua",
    "mcdev-nvim/lua/mcdev/protocol.lua",
    "mcdev-nvim/lua/mcdev/ktlsp.lua",
    "scripts/run-rust-tests-compact.ps1",
    "scripts/run-nvim-tests-compact.ps1",
    "scripts/build-native-release.ps1",
    "docs/native-distribution.md",
    "third-party/MixinExtras-LICENSE",
    "third-party/MixinExtras-PROVENANCE.md"
)

foreach ($path in $required) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing required native path: $path"
    }
}

$workspace = Get-Content -Raw -LiteralPath "Cargo.toml"
foreach ($member in @("mcdev-core", "mcdev-protocol", "mcdev-companion")) {
    if (-not $workspace.Contains('"' + $member + '"')) {
        throw "Native workspace member is missing: $member"
    }
}

$companionManifest = Get-Content -Raw -LiteralPath "mcdev-companion/Cargo.toml"
if ($companionManifest -match "(?i)\b(ktcore|ktlsp)\b") {
    throw "mcdev-companion manifest must not link the ktlsp/ktcore host"
}
foreach ($marker in @("mcdev-core", "mcdev-protocol", "serde_json")) {
    if (-not $companionManifest.Contains($marker)) {
        throw "mcdev-companion manifest is missing native dependency marker: $marker"
    }
}

$companionMain = Get-Content -Raw -LiteralPath "mcdev-companion/src/main.rs"
foreach ($marker in @("--stdio-completion", "mcdev_companion::run")) {
    if (-not $companionMain.Contains($marker)) {
        throw "mcdev-companion entrypoint is missing marker: $marker"
    }
}

$luaProtocol = Get-Content -Raw -LiteralPath "mcdev-nvim/lua/mcdev/protocol.lua"
foreach ($forbidden in @("parseDescriptor", "remap", "scanJar")) {
    if ($luaProtocol.Contains($forbidden)) {
        throw "Lua transport contains forbidden semantic marker: $forbidden"
    }
}

$luaHostBoundary = Get-Content -Raw -LiteralPath "mcdev-nvim/lua/mcdev/ktlsp.lua"
foreach ($marker in @("extend_config", "companion", "analysisProviders")) {
    if (-not $luaHostBoundary.Contains($marker)) {
        throw "Native Lua host boundary is missing marker: $marker"
    }
}

Write-Host "native layout check passed"
