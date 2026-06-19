param(
    [switch]$RunGenSources
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Join-Path $repoRoot "build/e2e-loom-workspace"
$fixtureRoot = "fixtures/fabric-loom-e2e"

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
        [string]$FixtureRootName,
        [string]$DestinationRoot
    )

    $sourceJar = Get-FixtureJar

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($sourceJar)
    try {
        $prefix = "$FixtureRootName/"
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

Copy-FixtureTree -FixtureRootName $fixtureRoot -DestinationRoot $workspaceRoot

$classResource = "fixtures/shared/classes/com/example/target/SimpleTarget.class"
$classpathRoot = Join-Path $workspaceRoot "classpath-staging/com/example/target"
New-Item -ItemType Directory -Path $classpathRoot -Force | Out-Null
Copy-FixtureResource `
    -ResourcePath $classResource `
    -Destination (Join-Path $classpathRoot "SimpleTarget.class")

$loomJarDir = Join-Path $workspaceRoot ".gradle/loom-cache/remapped_working"
New-Item -ItemType Directory -Path $loomJarDir -Force | Out-Null
$loomJar = Join-Path $loomJarDir "minecraft-client-mapped.jar"
if (Test-Path -LiteralPath $loomJar) {
    Remove-Item -LiteralPath $loomJar -Force
}

Push-Location $workspaceRoot
try {
    & jar --create --file $loomJar -C "classpath-staging" "com/example/target/SimpleTarget.class"
    if ($LASTEXITCODE -ne 0) { throw "failed to create loom remapped jar" }
} finally {
    Pop-Location
}

Remove-Item -LiteralPath (Join-Path $workspaceRoot "classpath-staging") -Recurse -Force

if ($RunGenSources -or $env:MCDEV_LOOM_RUN_GEN_SOURCES -eq "1") {
    Write-Host "MCDEV_LOOM_RUN_GEN_SOURCES is set; attempting gradlew genSources (requires network)"
    if (-not (Test-Path -LiteralPath (Join-Path $workspaceRoot "gradlew"))) {
        Write-Warning "gradlew is not bundled in the loom fixture; skipping live genSources"
    } else {
        Push-Location $workspaceRoot
        try {
            & .\gradlew.bat --no-daemon genSources
            if ($LASTEXITCODE -ne 0) { throw "gradlew genSources failed" }
        } finally {
            Pop-Location
        }
    }
}

Write-Host "loom e2e workspace prepared at $workspaceRoot"
