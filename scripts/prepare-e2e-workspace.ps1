param(
    [ValidateSet("fabric-basic", "fabric-mixinextras", "fabric-aw-at", "multi-source-set", "forge-basic", "broken-diagnostics")]
    [string]$Fixture = "fabric-basic"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Join-Path $repoRoot "build/e2e-workspace"
$awAtFixtureRoot = "fixtures/fabric-aw-at"

function Get-FixtureJar {
    if ($script:FixtureJar) { return $script:FixtureJar }

    $jarDir = Join-Path $repoRoot "mcdev-test-fixtures/build/libs"
    Push-Location $repoRoot
    try {
        & gradle :mcdev-test-fixtures:jar 2>&1 | ForEach-Object { Write-Host $_ }
        if ($LASTEXITCODE -ne 0) { throw "failed to build mcdev-test-fixtures jar" }
    } finally {
        Pop-Location
    }

    $rootBuild = Get-Content -LiteralPath (Join-Path $repoRoot "build.gradle.kts") -Raw
    if ($rootBuild -notmatch 'version\s*=\s*"([^"]+)"') {
        throw "root project version not found"
    }
    $version = $Matches[1]
    $sourceJar = Join-Path $jarDir "mcdev-test-fixtures-$version.jar"
    if (-not (Test-Path -LiteralPath $sourceJar)) {
        throw "mcdev-test-fixtures jar not found: $sourceJar"
    }
    $script:FixtureJar = $sourceJar
    return $sourceJar
}

function Copy-FixtureResource {
    param(
        [string]$ResourcePath,
        [string]$Destination
    )

    $sourceJar = Get-FixtureJar

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($sourceJar)
    try {
        $entry = $zip.Entries | Where-Object { $_.FullName -eq $ResourcePath } | Select-Object -First 1
        if (-not $entry) {
            throw "fixture resource not found in jar: $ResourcePath"
        }
        $destinationParent = Split-Path -Parent $Destination
        if ($destinationParent -and -not (Test-Path -LiteralPath $destinationParent)) {
            New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
        }
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $Destination, $true)
    } finally {
        $zip.Dispose()
    }
}

function Copy-FixtureTree {
    param(
        [string]$FixtureRoot,
        [string]$DestinationRoot
    )

    $sourceJar = Get-FixtureJar

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($sourceJar)
    try {
        $prefix = "$FixtureRoot/"
        foreach ($entry in $zip.Entries) {
            if (-not $entry.FullName.StartsWith($prefix)) { continue }
            $relative = $entry.FullName.Substring($prefix.Length).Replace('/', [IO.Path]::DirectorySeparatorChar)
            if ([string]::IsNullOrWhiteSpace($relative)) { continue }
            if ($relative.EndsWith([IO.Path]::DirectorySeparatorChar)) { continue }
            if ($entry.FullName.EndsWith("/")) { continue }
            $destination = Join-Path $DestinationRoot $relative
            $destinationParent = Split-Path -Parent $destination
            if ($destinationParent -and -not (Test-Path -LiteralPath $destinationParent)) {
                New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
            }
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $destination, $true)
        }
    } finally {
        $zip.Dispose()
    }
}

if (Test-Path -LiteralPath $workspaceRoot) {
    Remove-Item -LiteralPath $workspaceRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $workspaceRoot -Force | Out-Null

$fixtureRoot = "fixtures/$Fixture"
Copy-FixtureTree -FixtureRoot $fixtureRoot -DestinationRoot $workspaceRoot

$classResource = "fixtures/shared/classes/com/example/target/SimpleTarget.class"
$classDestination = Join-Path $workspaceRoot "classpath/com/example/target/SimpleTarget.class"
Copy-FixtureResource -ResourcePath $classResource -Destination $classDestination

if ($Fixture -eq "fabric-basic") {
    $awAtResources = @(
        "src/main/resources/mod.accesswidener",
        "src/main/resources/mod_at.cfg"
    )

    foreach ($relative in $awAtResources) {
        $resourcePath = "$awAtFixtureRoot/$relative"
        $destination = Join-Path $workspaceRoot $relative
        Copy-FixtureResource -ResourcePath $resourcePath -Destination $destination
    }

    $fabricModPath = Join-Path $workspaceRoot "fabric.mod.json"
    if (Test-Path -LiteralPath $fabricModPath) {
        $fabricMod = Get-Content -LiteralPath $fabricModPath -Raw | ConvertFrom-Json
        $fabricMod | Add-Member -NotePropertyName "accessWidener" -NotePropertyValue "mod.accesswidener" -Force
        $fabricMod | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $fabricModPath -Encoding utf8
    }
}

if ($Fixture -eq "fabric-aw-at") {
    Copy-FixtureResource `
        -ResourcePath "fixtures/fabric-basic/src/main/java/com/example/target/SimpleTarget.java" `
        -Destination (Join-Path $workspaceRoot "src/main/java/com/example/target/SimpleTarget.java")
}

Write-Host "e2e workspace prepared for $Fixture at $workspaceRoot"
