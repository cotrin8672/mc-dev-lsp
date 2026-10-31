param(
    [ValidateSet("fabric-basic", "fabric-mixinextras", "fabric-aw-at", "multi-source-set", "forge-basic", "broken-diagnostics")]
    [string]$Fixture = "fabric-basic"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')

$repoRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Assert-E2eBuildPath (Join-Path $repoRoot "build/e2e-workspace")
$awAtFixtureRoot = "fixtures/fabric-aw-at"
$null = Resolve-E2eExecutable 'GRADLE_CMD'

if (Test-Path -LiteralPath $workspaceRoot) {
    Remove-Item -LiteralPath $workspaceRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $workspaceRoot -Force | Out-Null

$fixtureRoot = "fixtures/$Fixture"
Copy-FixtureTree -FixtureRoot $fixtureRoot -DestinationRoot $workspaceRoot

$fixedDependencies = -not [string]::IsNullOrWhiteSpace($env:MCDEV_E2E_INPUTS_RECEIPT)
if ($fixedDependencies) {
    Copy-E2eFixedDependencies $env:MCDEV_E2E_INPUTS_RECEIPT $workspaceRoot
} else {
    Write-Host 'fixture dependencies: Maven coordinates (MCDEV_E2E_INPUTS_RECEIPT is unset)'
}
$mixinDependency = if ($fixedDependencies) { 'files("libs/mixin-0.8.7.jar")' }
    else { '"org.spongepowered:mixin:0.8.7"' }
$extrasDependency = if ($fixedDependencies) { 'files("libs/mixinextras-fabric-0.5.5.jar")' }
    else { '"io.github.llamalad7:mixinextras-fabric:0.5.5"' }

if ($Fixture -eq "fabric-mixinextras") {
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

$platformMarker = if ($Fixture -eq "forge-basic") { "net.minecraftforge.gradle" } else { "fabric-loom" }
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
    $clientDependencies = @"
    clientImplementation files("classpath")
    clientCompileOnly $mixinDependency
"@
}
$extraDependencies = if ($Fixture -eq 'fabric-mixinextras') {
    '    compileOnly ' + $extrasDependency
} else { '' }
$repositories = if ($fixedDependencies) { '' } else { @'
repositories {
    mavenCentral()
    maven { url "https://repo.spongepowered.org/repository/maven-public/" }
}
'@ }

$e2eBuildGradlePath = Join-Path $workspaceRoot "build.gradle"
$e2eBuildGradleContent = @"
plugins {
    id 'java'
    id 'maven-publish'
}

// $platformMarker marker preserves the original fixture's platform selection.
java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}
tasks.withType(JavaCompile).configureEach {
    options.encoding = 'UTF-8'
    options.compilerArgs = ['-proc:none']
}
$repositories

$sourceSets
dependencies {
    implementation files("classpath")
    compileOnly $mixinDependency
$extraDependencies
$clientDependencies}
"@
[System.IO.File]::WriteAllText($e2eBuildGradlePath, $e2eBuildGradleContent, [System.Text.UTF8Encoding]::new($false))

$settingsGradlePath = Join-Path $workspaceRoot "settings.gradle"
$settingsGradleContent = @'
rootProject.name = "mcdev-e2e-workspace"
'@
[System.IO.File]::WriteAllText($settingsGradlePath, $settingsGradleContent, [System.Text.UTF8Encoding]::new($false))

Copy-FixtureTargetClass -DestinationRoot (Join-Path $workspaceRoot 'classpath')
Write-E2eGradleWrapper $workspaceRoot

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
