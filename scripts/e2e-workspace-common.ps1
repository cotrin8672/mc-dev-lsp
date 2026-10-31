#requires -Version 7.0
$ErrorActionPreference = 'Stop'

function Resolve-E2eExecutable([string] $Name) {
    $value = [Environment]::GetEnvironmentVariable($Name, 'Process')
    if ([string]::IsNullOrWhiteSpace($value)) { throw "$Name must explicitly name an existing executable" }
    $command = Get-Command -Name $value -CommandType Application, ExternalScript -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $command) { throw "$Name executable not found: $value" }
    return $command.Source
}

function Assert-E2eBuildPath([string] $Path) {
    $buildRoot = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $PSScriptRoot) 'build')).TrimEnd('\', '/')
    $target = [IO.Path]::GetFullPath($Path)
    $comparison = if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        [StringComparison]::OrdinalIgnoreCase
    } else { [StringComparison]::Ordinal }
    if (-not $target.StartsWith($buildRoot + [IO.Path]::DirectorySeparatorChar, $comparison)) {
        throw "E2E path must stay under $buildRoot`: $target"
    }
    $ancestor = $target
    while ($ancestor.Length -ge $buildRoot.Length) {
        if (Test-Path -LiteralPath $ancestor) {
            if ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "E2E path must not traverse a reparse point: $ancestor"
            }
        }
        $ancestor = Split-Path -Parent $ancestor
    }
    return $target
}

function Copy-FixtureResource([string] $ResourcePath, [string] $Destination) {
    $source = Join-Path (Split-Path -Parent $PSScriptRoot) ('mcdev-test-fixtures/src/main/resources/' + $ResourcePath)
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "fixture resource not found: $ResourcePath" }
    $destinationPath = Assert-E2eBuildPath $Destination
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destinationPath)
    Copy-Item -LiteralPath $source -Destination $destinationPath -Force
}

function Copy-FixtureTree([string] $FixtureRoot, [string] $DestinationRoot) {
    $sourceRoot = Join-Path (Split-Path -Parent $PSScriptRoot) ('mcdev-test-fixtures/src/main/resources/' + $FixtureRoot)
    if (-not (Test-Path -LiteralPath $sourceRoot -PathType Container)) { throw "fixture directory not found: $FixtureRoot" }
    $destinationPath = Assert-E2eBuildPath $DestinationRoot
    foreach ($file in Get-ChildItem -LiteralPath $sourceRoot -Recurse -Force -File) {
        $relative = [IO.Path]::GetRelativePath($sourceRoot, $file.FullName)
        Copy-FixtureResource ($FixtureRoot + '/' + $relative) (Join-Path $destinationPath $relative)
    }
}

function Copy-FixtureTargetClass([string] $DestinationRoot) {
    $repo = Split-Path -Parent $PSScriptRoot
    $destinationPath = Assert-E2eBuildPath $DestinationRoot
    $null = New-Item -ItemType Directory -Force -Path $destinationPath
    . (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
    $logs = Join-Path $repo ('build/e2e-fixture-logs/javac-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff') + '-' + $PID)
    $null = New-Item -ItemType Directory -Force -Path $logs
    $executable = if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) { 'javac.exe' } else { 'javac' }
    $javac = if ($env:JAVA_HOME_21_X64 -and (Test-Path -LiteralPath (Join-Path $env:JAVA_HOME_21_X64 ('bin/' + $executable)))) {
        Join-Path $env:JAVA_HOME_21_X64 ('bin/' + $executable)
    } else { (Get-Command javac -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source }
    $version = Invoke-RustCheck 'javac-version' $javac @('-version') $repo $logs 15
    $versionText = [IO.File]::ReadAllText((Join-Path $logs 'javac-version.stdout.log')) +
        [IO.File]::ReadAllText((Join-Path $logs 'javac-version.stderr.log'))
    if ($version.ExitCode -ne 0 -or $versionText -notmatch '^javac 21(?:\.|\s|$)') {
        throw "JDK 21 javac is required for original fixtures; see $logs"
    }
    $source = Join-Path $repo 'mcdev-test-fixtures/src/fixture-java/com/example/target/SimpleTarget.java'
    $result = Invoke-RustCheck 'javac-target' $javac @('-g', '-encoding', 'UTF-8', '--release', '21', '-proc:none',
        '-d', $destinationPath, $source) $repo $logs 30
    $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $logs 'result.json') -Encoding utf8
    if ($result.ExitCode -ne 0) { throw "original fixture javac failed; see $logs" }
    Write-Host "original fixture javac: passed; logs: $logs"
}

function Copy-E2eFixedDependencies([string] $ReceiptPath, [string] $WorkspaceRoot) {
    $receipt = (Resolve-Path -LiteralPath $ReceiptPath -ErrorAction Stop).Path
    $receiptSha = (Get-FileHash -LiteralPath $receipt -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifest = Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
    if (@($manifest.failures).Count -ne 0) { throw 'The fixed-input receipt contains failures' }
    $selected = [ordered]@{}
    foreach ($name in @('mixin-0.8.7.jar', 'mixinextras-fabric-0.5.5.jar')) {
        $artifact = $manifest.artifacts.$name
        if (-not $artifact -or $artifact.sha256 -isnot [string] -or $artifact.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
            -not ($artifact.size -is [long] -or $artifact.size -is [int]) -or $artifact.size -le 0 -or
            $artifact.readOnly -isnot [bool] -or -not $artifact.readOnly -or $artifact.path -isnot [string] -or
            -not [IO.Path]::IsPathFullyQualified($artifact.path)) {
            throw "Invalid fixed fixture dependency: $name"
        }
        $expectedPath = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $receipt) $name))
        $source = (Resolve-Path -LiteralPath $artifact.path -ErrorAction Stop).Path
        $comparison = if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
            [StringComparison]::OrdinalIgnoreCase
        } else { [StringComparison]::Ordinal }
        if (-not $source.Equals($expectedPath, $comparison)) {
            throw "Fixed fixture dependency must be the exact copy beside its receipt: $name"
        }
        $file = Get-Item -LiteralPath $source
        if ($file.Length -ne $artifact.size -or
            (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $artifact.sha256) {
            throw "Fixed fixture dependency checksum/size mismatch: $name"
        }
        $selected[$name] = [ordered]@{ source = $source; sha256 = $artifact.sha256.ToLowerInvariant(); size = $file.Length }
    }
    $workspace = Assert-E2eBuildPath $WorkspaceRoot
    $libs = Assert-E2eBuildPath (Join-Path $workspace 'libs')
    $null = New-Item -ItemType Directory -Force -Path $libs
    foreach ($name in $selected.Keys) {
        $artifact = $selected[$name]
        $copy = Join-Path $libs $name
        Copy-Item -LiteralPath $artifact.source -Destination $copy -Force
        if ((Get-Item -LiteralPath $copy).Length -ne $artifact.size -or
            (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash.ToLowerInvariant() -ne $artifact.sha256) {
            throw "Fixed fixture dependency copy mismatch: $name"
        }
        $artifact.copy = $copy
    }
    $receiptCopy = Join-Path $workspace 'fixed-inputs-receipt.json'
    Copy-Item -LiteralPath $receipt -Destination $receiptCopy
    if ((Get-FileHash -LiteralPath $receiptCopy -Algorithm SHA256).Hash.ToLowerInvariant() -ne $receiptSha) {
        throw 'The fixed-input receipt changed during fixture preparation'
    }
    [ordered]@{ receipt = $receipt; receiptSha256 = $receiptSha; artifacts = $selected } |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $workspace 'fixed-dependencies.json') -Encoding utf8
    Write-Host "fixed fixture dependencies: two JARs verified from $receipt"
}

function Write-E2eGradleWrapper([string] $WorkspaceRoot) {
    $workspacePath = Assert-E2eBuildPath $WorkspaceRoot
    $gradle = Resolve-E2eExecutable 'GRADLE_CMD'
    $batch = @('@echo off', ('call "' + $gradle.Replace('%', '%%') + '" %*'), '') -join "`r`n"
    [IO.File]::WriteAllText((Join-Path $workspacePath 'gradlew.bat'), $batch, [Text.UTF8Encoding]::new($false))
    $quotedGradle = "'" + $gradle.Replace("'", "'\''") + "'"
    $shell = @('#!/bin/sh', ('exec ' + $quotedGradle + ' "$@"'), '') -join "`n"
    $wrapper = Join-Path $workspacePath 'gradlew'
    [IO.File]::WriteAllText($wrapper, $shell, [Text.UTF8Encoding]::new($false))
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        [IO.File]::SetUnixFileMode($wrapper, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor
            [IO.UnixFileMode]::UserExecute -bor [IO.UnixFileMode]::GroupRead -bor [IO.UnixFileMode]::GroupExecute -bor
            [IO.UnixFileMode]::OtherRead -bor [IO.UnixFileMode]::OtherExecute)
    }
}
