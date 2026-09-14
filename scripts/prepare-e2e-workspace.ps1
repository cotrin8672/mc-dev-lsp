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
    $rootBuild = Get-Content -LiteralPath (Join-Path $repoRoot "build.gradle.kts") -Raw
    if ($rootBuild -notmatch 'version\s*=\s*"([^"]+)"') {
        throw "root project version not found"
    }
    $version = $Matches[1]
    $sourceJar = Join-Path $jarDir "mcdev-test-fixtures-$version.jar"
    if (Test-Path -LiteralPath $sourceJar) {
        $script:FixtureJar = $sourceJar
        return $sourceJar
    }

    Push-Location $repoRoot
    try {
        & gradle :mcdev-test-fixtures:jar 2>&1 | ForEach-Object { Write-Host $_ }
        if ($LASTEXITCODE -ne 0) { throw "failed to build mcdev-test-fixtures jar" }
    } finally {
        Pop-Location
    }

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

if ($Fixture -eq "fabric-mixinextras") {
    $e2eBuildGradlePath = Join-Path $workspaceRoot "build.gradle"
    $e2eBuildGradleContent = @'
plugins {
    id 'java'
    id 'maven-publish'
}

repositories {
    mavenCentral()
    maven { url "https://repo.spongepowered.org/repository/maven-public/" }
}

dependencies {
    // fabric-loom marker for mcdev platform detection; this fixture is imported
    // by JDT LS without resolving the live Loom plugin.
    compileOnly "org.spongepowered:mixin:0.8.7"
    compileOnly "io.github.llamalad7:mixinextras-fabric:0.5.5"
}
'@
    [System.IO.File]::WriteAllText($e2eBuildGradlePath, $e2eBuildGradleContent, [System.Text.UTF8Encoding]::new($false))

    $sharedMixinPath = Join-Path $workspaceRoot "src/main/java/com/example/mixin/MixinExtrasSharedExample.java"
    $sharedMixinContent = @'
package com.example.mixin;

import com.example.target.SimpleTarget;
import com.llamalad7.mixinextras.sugar.Share;
import com.llamalad7.mixinextras.sugar.ref.LocalIntRef;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(SimpleTarget.class)
public abstract class MixinExtrasSharedExample {
    @Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))
    private void mcdev$shareAcrossFiles(
        CallbackInfo ci,
        @Share(value = "speedAcrossFiles", namespace = "shared") LocalIntRef speed
    ) {}
}
'@
    [System.IO.File]::WriteAllText($sharedMixinPath, $sharedMixinContent, [System.Text.UTF8Encoding]::new($false))

    $mixinsJsonPath = Join-Path $workspaceRoot "mixins.json"
    $mixinsJson = Get-Content -LiteralPath $mixinsJsonPath -Raw | ConvertFrom-Json
    $mixinsJson.mixins = @($mixinsJson.mixins) + "MixinExtrasSharedExample"
    $mixinsJsonContent = $mixinsJson | ConvertTo-Json -Depth 10
    [System.IO.File]::WriteAllText($mixinsJsonPath, $mixinsJsonContent, [System.Text.UTF8Encoding]::new($false))
}

if ($Fixture -in @("multi-source-set", "forge-basic")) {
    $platformMarker = if ($Fixture -eq "multi-source-set") { "fabric-loom" } else { "net.minecraftforge.gradle" }
    $sourceSets = ""
    $clientDependencies = ""
    if ($Fixture -eq "multi-source-set") {
        $sourceSets = @'
sourceSets {
    main {
        java.srcDirs = ["src/main/java"]
        resources.srcDirs = ["src/main/resources"]
    }
    client {
        java.srcDirs = ["src/client/java"]
        resources.srcDirs = ["src/client/resources"]
        compileClasspath += sourceSets.main.output
        runtimeClasspath += sourceSets.main.output
    }
}
'@
        $clientDependencies = @'
    clientCompileOnly files("classpath")
    clientCompileOnly "org.spongepowered:mixin:0.8.7"
'@
    }

    $e2eBuildGradlePath = Join-Path $workspaceRoot "build.gradle"
    $e2eBuildGradleContent = @"
plugins {
    id 'java'
    id 'maven-publish'
}

// $platformMarker marker for mcdev platform detection; this fixture is imported
// by JDT LS without resolving the live platform plugin.
repositories {
    mavenCentral()
    maven { url "https://repo.spongepowered.org/repository/maven-public/" }
}

$sourceSets
dependencies {
    compileOnly files("classpath")
    compileOnly "org.spongepowered:mixin:0.8.7"
$clientDependencies}
"@
    [System.IO.File]::WriteAllText($e2eBuildGradlePath, $e2eBuildGradleContent, [System.Text.UTF8Encoding]::new($false))
}

$settingsGradlePath = Join-Path $workspaceRoot "settings.gradle"
$settingsGradleContent = @'
rootProject.name = "mcdev-e2e-workspace"
'@
[System.IO.File]::WriteAllText($settingsGradlePath, $settingsGradleContent, [System.Text.UTF8Encoding]::new($false))

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

    $sourceOnlyTargetDir = Join-Path $workspaceRoot "src/main/java/com/example/target"
    New-Item -ItemType Directory -Path $sourceOnlyTargetDir -Force | Out-Null
    @'
package com.example.target;

public class SourceOnlyTarget {
    public void pulse() {}

    public int measure(String label) {
        return label.length();
    }
}
'@ | Set-Content -LiteralPath (Join-Path $sourceOnlyTargetDir "SourceOnlyTarget.java") -Encoding utf8
}

if ($Fixture -in @("fabric-aw-at", "fabric-mixinextras")) {
    Copy-FixtureResource `
        -ResourcePath "fixtures/fabric-basic/src/main/java/com/example/target/SimpleTarget.java" `
        -Destination (Join-Path $workspaceRoot "src/main/java/com/example/target/SimpleTarget.java")
}

Write-Host "e2e workspace prepared for $Fixture at $workspaceRoot"
