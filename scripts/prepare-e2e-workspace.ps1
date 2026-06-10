$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Join-Path $repoRoot "build/e2e-workspace"
$fixtureRoot = "fixtures/fabric-basic"

function Copy-FixtureResource {
    param(
        [string]$ResourcePath,
        [string]$Destination
    )

    $sourceJar = Join-Path $repoRoot "mcdev-test-fixtures/build/libs/mcdev-test-fixtures-0.1.0-SNAPSHOT.jar"
    if (-not (Test-Path -LiteralPath $sourceJar)) {
        Push-Location $repoRoot
        try {
            & gradle :mcdev-test-fixtures:jar
            if ($LASTEXITCODE -ne 0) { throw "failed to build mcdev-test-fixtures jar" }
        } finally {
            Pop-Location
        }
    }

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

if (Test-Path -LiteralPath $workspaceRoot) {
    Remove-Item -LiteralPath $workspaceRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $workspaceRoot -Force | Out-Null

$resources = @(
    "build.gradle",
    "fabric.mod.json",
    "mixins.json",
    "mappings.tiny",
    "src/main/java/com/example/mixin/ExampleMixin.java",
    "src/main/java/com/example/target/SimpleTarget.java"
)

foreach ($relative in $resources) {
    $resourcePath = "$fixtureRoot/$relative"
    $destination = Join-Path $workspaceRoot $relative
    Copy-FixtureResource -ResourcePath $resourcePath -Destination $destination
}

$classResource = "fixtures/shared/classes/com/example/target/SimpleTarget.class"
$classDestination = Join-Path $workspaceRoot "classpath/com/example/target/SimpleTarget.class"
Copy-FixtureResource -ResourcePath $classResource -Destination $classDestination

Write-Host "e2e workspace prepared at $workspaceRoot"
