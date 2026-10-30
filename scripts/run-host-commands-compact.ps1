#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $HostBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $HostSha256,
    [Parameter(Mandatory = $true)][string] $CompanionBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $CompanionSha256,
    [string] $NvimBinary = 'nvim',
    [string] $GradleBinary = 'gradle',
    # Cold indexing, two Gradle model refreshes, and restart each have finite phase budgets.
    [ValidateRange(30, 600)][int] $EditorTimeoutSeconds = 600
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$logs = Join-Path $repo ('build/rust-test-logs/host-commands-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$project = Join-Path $temporaryRoot ('mcdev-host-commands-' + [guid]::NewGuid())
$exitCode = 1
$lf = [string][char]10
function Write-FixtureFile([string] $relative, [string] $content) {
    $path = Join-Path $project $relative
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path)
    [IO.File]::WriteAllText($path, $content, [Text.UTF8Encoding]::new($false))
}
try {
    if (-not [IO.Path]::IsPathRooted($HostBinary) -or -not [IO.Path]::IsPathRooted($CompanionBinary)) {
        throw 'HostBinary and CompanionBinary must be absolute paths'
    }
    $hostPath = (Resolve-Path -LiteralPath $HostBinary).Path
    $companionPath = (Resolve-Path -LiteralPath $CompanionBinary).Path
    if ((Get-FileHash -LiteralPath $hostPath -Algorithm SHA256).Hash -ne $HostSha256) { throw 'Stock host checksum mismatch' }
    if ((Get-FileHash -LiteralPath $companionPath -Algorithm SHA256).Hash -ne $CompanionSha256) { throw 'Companion checksum mismatch' }
    $editorPath = (Get-Command $NvimBinary -ErrorAction Stop).Source
    $gradlePath = (Get-Command $GradleBinary -ErrorAction Stop).Source
    if (-not [IO.Path]::IsPathRooted($editorPath) -or -not [IO.Path]::IsPathRooted($gradlePath)) { throw 'Editor/Gradle paths must resolve absolutely' }
    $null = New-Item -ItemType Directory -Force -Path $project
    Write-FixtureFile 'settings.gradle' ("rootProject.name = 'mcdev-command-proof'" + $lf)
    Write-FixtureFile 'build.gradle' (@(
        "plugins { id 'java' }",
        'java { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }',
        "tasks.withType(JavaCompile).configureEach { options.encoding = 'UTF-8' }",
        ''
    ) -join $lf)
    Write-FixtureFile 'gradle.properties' (@('org.gradle.daemon=false', 'org.gradle.parallel=false', 'org.gradle.workers.max=2', '') -join $lf)
    if ($IsWindows) {
        Write-FixtureFile 'gradlew.bat' (@('@echo off', ('call "' + $gradlePath + '" %*'), 'exit /b %ERRORLEVEL%', '') -join $lf)
    } else {
        Write-FixtureFile 'gradlew' (@('#!/bin/sh', ('exec "' + $gradlePath + '" "$@"'), '') -join $lf)
        & rtk proxy chmod +x (Join-Path $project 'gradlew')
        if ($LASTEXITCODE -ne 0) { throw 'Cannot set fixture wrapper executable' }
    }
    Write-FixtureFile 'src/main/java/org/spongepowered/asm/mixin/Mixin.java' (@('package org.spongepowered.asm.mixin;', 'public @interface Mixin { Class<?>[] value() default {}; }', '') -join $lf)
    Write-FixtureFile 'src/main/java/org/spongepowered/asm/mixin/injection/Inject.java' (@('package org.spongepowered.asm.mixin.injection;', 'public @interface Inject { String[] method() default {}; }', '') -join $lf)
    Write-FixtureFile 'src/main/java/org/spongepowered/asm/mixin/injection/At.java' (@('package org.spongepowered.asm.mixin.injection;', 'public @interface At { String value(); }', '') -join $lf)
    Write-FixtureFile 'src/main/java/com/llamalad7/mixinextras/expression/Definition.java' (@('package com.llamalad7.mixinextras.expression;', 'public @interface Definition { String id(); }', '') -join $lf)
    Write-FixtureFile 'src/main/java/com/llamalad7/mixinextras/expression/Expression.java' (@('package com.llamalad7.mixinextras.expression;', 'public @interface Expression { String value(); }', '') -join $lf)
    Write-FixtureFile 'src/main/java/com/llamalad7/mixinextras/injector/ModifyExpressionValue.java' (@('package com.llamalad7.mixinextras.injector;', 'import org.spongepowered.asm.mixin.injection.At;', 'public @interface ModifyExpressionValue { String[] method(); At at(); }', '') -join $lf)
    Write-FixtureFile 'src/main/java/fixture/Target.java' (@('package fixture;', 'public class Target { public int tick(int value) { return value; } public int draw(String s, float x, float y) { return 1; } }', '') -join $lf)
    Write-FixtureFile 'src/main/java/fixture/Patch.java' (@('package fixture; import org.spongepowered.asm.mixin.Mixin; @Mixin(Target.class) class Patch {}', '') -join $lf)
    Write-FixtureFile 'src/main/java/fixture/ExampleMixin.java' (@(
        'package fixture;',
        'import org.spongepowered.asm.mixin.Mixin;',
        'import org.spongepowered.asm.mixin.injection.At;',
        'import com.llamalad7.mixinextras.expression.Definition;',
        'import com.llamalad7.mixinextras.expression.Expression;',
        'import com.llamalad7.mixinextras.injector.ModifyExpressionValue;',
        '@Mixin(Target.class)',
        'abstract class ExampleMixin {',
        '    @Definition(id = "drawCall")',
        '    @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)I", at = @At("MIXINEXTRAS:EXPRESSION"))',
        '    @Expression("drawCall")',
        '    private int mcdev$handler(int original) { return original; }',
        '}',
        ''
    ) -join $lf)
    Write-FixtureFile 'src/main/resources/fixture.mixins.json' '{"package":"fixture","mixins":[]}'
    Write-FixtureFile 'src/main/resources/access.aw' ('accessWidener v2 named' + $lf)
    Write-FixtureFile 'src/main/java/fixture/Owner.java' (@(
        'package fixture;',
        'import org.spongepowered.asm.mixin.Mixin;',
        'import org.spongepowered.asm.mixin.injection.Inject;',
        '// saved',
        '@Mixin(Target.class)',
        'abstract class Owner {',
        '    @Inject(method = "tick")',
        '    void onTick() {}',
        '}',
        ''
    ) -join $lf)

    $env:MCDEV_COMMAND_HOST = $hostPath
    $env:MCDEV_COMMAND_COMPANION = $companionPath
    $env:MCDEV_COMMAND_PROJECT = $project
    $env:MCDEV_COMMAND_ARTIFACTS = $logs
    $env:KTLSP_CACHE_DIR = Join-Path $logs 'host-cache'
    $env:XDG_CONFIG_HOME = Join-Path $logs 'editor-config'
    $env:XDG_DATA_HOME = Join-Path $logs 'editor-data'
    $env:XDG_STATE_HOME = Join-Path $logs 'editor-state'
    $env:XDG_CACHE_HOME = Join-Path $logs 'editor-cache'
    $env:GRADLE_USER_HOME = Join-Path $logs 'gradle-user'
    $env:NVIM_LOG_FILE = Join-Path $logs 'nvim.log'
    $env:KTLSP_JDK_SRC = Join-Path $logs 'jdk-src.zip'
    $jdk = [IO.Compression.ZipFile]::Open($env:KTLSP_JDK_SRC, [IO.Compression.ZipArchiveMode]::Create)
    try {
        $entry = $jdk.CreateEntry('java.base/java/lang/Object.java')
        $writer = [IO.StreamWriter]::new($entry.Open())
        try { $writer.Write('package java.lang; public class Object {}') } finally { $writer.Dispose() }
    } finally { $jdk.Dispose() }

    $editor = Join-Path $repo 'mcdev-companion/tests/command_editor_smoke.lua'
    $startupSource = 'mcdev-nvim/tests/e2e/minimal_init.lua'
    $startup = Join-Path $repo $startupSource
    $sourceFiles = @('mcdev-core/src', 'mcdev-protocol/src', 'mcdev-companion/src', 'mcdev-nvim/lua') |
        ForEach-Object { Get-ChildItem -LiteralPath (Join-Path $repo $_) -Recurse -File } |
        Where-Object { $_.Extension -in @('.rs', '.lua') } | ForEach-Object FullName
    $inputs = @($sourceFiles) + @($editor, $startup, $hostPath, $companionPath, $editorPath, $gradlePath,
        $PSCommandPath, (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1'),
        (Join-Path $repo 'Cargo.toml'), (Join-Path $repo 'Cargo.lock'),
        (Join-Path $repo 'mcdev-companion/Cargo.toml'),
        (Join-Path $repo 'mcdev-companion/tests/support/accepted_trace.lua'))
    $before = @(Get-FileHash -LiteralPath $inputs -Algorithm SHA256)
    $startupHash = ($before | Where-Object { $_.Path -eq $startup }).Hash.ToLowerInvariant()
    $before | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.before.json')
    $head = (& rtk proxy git -C $repo rev-parse HEAD | Select-Object -Last 1).Trim()
    $gradleArguments = @('--offline', '--no-daemon', '--max-workers=2', '--console=plain', 'classes')
    $gradleCommand = $gradlePath
    if ($IsWindows -and $gradlePath.EndsWith('.bat', [StringComparison]::OrdinalIgnoreCase)) {
        $gradleCommand = Join-Path ([Environment]::SystemDirectory) 'cmd.exe'
        $gradleArguments = @('/d', '/c', $gradlePath) + $gradleArguments
    }
    $gradle = Invoke-RustCheck -Name gradle-build -Command $gradleCommand -Arguments $gradleArguments -Directory $project -LogDirectory $logs -TimeoutSeconds 180
    $gradle | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'gradle-summary.json')
    $fixtureArchive = Join-Path $logs 'fixture-inputs'
    New-Item -ItemType Directory -Path $fixtureArchive | Out-Null
    Copy-Item -LiteralPath (Join-Path $project 'src') -Destination (Join-Path $fixtureArchive 'src') -Recurse
    foreach ($fixtureFile in @('settings.gradle', 'build.gradle', 'gradle.properties', 'gradlew.bat')) {
        $fixturePath = Join-Path $project $fixtureFile
        if (Test-Path -LiteralPath $fixturePath) { Copy-Item -LiteralPath $fixturePath -Destination $fixtureArchive }
    }
    $targetClass = Join-Path $project 'build/classes/java/main/fixture/Target.class'
    if (Test-Path -LiteralPath $targetClass) { Copy-Item -LiteralPath $targetClass -Destination $fixtureArchive }
    $env:MCDEV_COMMAND_GRADLE_BUILD_OK = if ($gradle.ExitCode -eq 0 -and -not $gradle.TimedOut) { '1' } else { '0' }
    $editorRun = Invoke-RustCheck -Name editor -Command $editorPath -Arguments @(
        '--headless', '-u', $startup, '-i', 'NONE', '-l', $editor
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds $EditorTimeoutSeconds
    $editorRun | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'summary.json')
    $after = @(Get-FileHash -LiteralPath $inputs -Algorithm SHA256)
    $after | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.after.json')
    $stable = @(Compare-Object $before $after -Property Path,Hash).Count -eq 0
    $proofPath = Join-Path $logs 'editor-proof.json'
    $proof = if (Test-Path -LiteralPath $proofPath) { Get-Content -LiteralPath $proofPath -Raw | ConvertFrom-Json } else { $null }
    [ordered]@{
        head = $head; hostSha256 = $HostSha256.ToLowerInvariant(); companionSha256 = $CompanionSha256.ToLowerInvariant()
        gradle = $gradle; editor = $editorRun; deadlineSeconds = $EditorTimeoutSeconds
        inputHashesStable = $stable; project = $project; proof = $proof
        startupSupport = [ordered]@{
            source = $startupSource; path = $startup; sha256 = $startupHash; role = 'editor-startup'
            verified = [bool]($stable -and $proof -and $proof.startup -and $proof.startup.verified)
        }
        requestedDomainCommands = 11; fullMcCapabilitiesVerified = $false
        sdkSourceFixture = 'reduced Object.java; selected runtime SDK remains host-owned'
        fixtureBuild = 'Java-only Gradle project with an isolated Gradle user directory'
    } | ConvertTo-Json -Depth 100 | Set-Content -Encoding utf8 (Join-Path $logs 'receipt.json')
    if (-not $stable) { throw 'E2E input sources changed during execution; the binary pin still identifies the measured executable' }
    if ($editorRun.ExitCode -ne 0 -or $editorRun.TimedOut -or -not $proof -or $null -ne $proof.error -or -not $proof.scopePassed) {
        throw 'Command E2E failed or was incomplete; see the full receipt'
    }
    Write-Output ("editor exit=0 checks={0} matrix={1} pid={2}" -f $proof.checks, @($proof.matrix).Count, $editorRun.Pid)
    $exitCode = 0
} catch {
    Write-Output $_.Exception.Message
    $stderr = Join-Path $logs 'editor.stderr.log'
    if (Test-Path -LiteralPath $stderr) { Get-Content -LiteralPath $stderr -Tail 24 }
} finally {
    if (Test-Path -LiteralPath $project) {
        $resolved = [IO.Path]::GetFullPath($project)
        if ((Split-Path -Parent $resolved) -ne $temporaryRoot -or
            (Split-Path -Leaf $resolved) -notlike 'mcdev-host-commands-*') {
            throw "Refusing unexpected fixture cleanup path: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    Write-Output "logs: $logs"
}
exit $exitCode
