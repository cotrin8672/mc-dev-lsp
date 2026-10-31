#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+$')]
    [string] $Version,
    [string] $OutputDirectory = 'build/release/native'
)

$ErrorActionPreference = 'Stop'

$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$output = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
    [IO.Path]::GetFullPath($OutputDirectory)
} else {
    [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
}
$logs = Join-Path $repo ('build/rust-test-logs/native-release-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs

function Assert-PathUnderRoot([string] $Root, [string] $Target) {
    $rootPath = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    $targetPath = [IO.Path]::GetFullPath($Target)
    $prefix = $rootPath + [IO.Path]::DirectorySeparatorChar
    if (-not $targetPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing path outside root: $targetPath"
    }
}

Assert-PathUnderRoot $repo $output
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')

$summary = [System.Collections.Generic.List[string]]::new()
$stage = $null
$buildResult = $null
$smokeResult = $null
$exitCode = 1
try {
    $metadataResult = Invoke-RustCheck -Name 'native-release-metadata' -Command 'cargo' -Arguments @(
        'metadata', '--locked', '--offline', '--no-deps', '--format-version', '1'
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds 60
    if ($metadataResult.ExitCode -ne 0) {
        throw 'native Cargo metadata failed; see complete logs'
    }
    $metadata = Get-Content -LiteralPath (Join-Path $logs 'native-release-metadata.stdout.log') -Raw | ConvertFrom-Json
    $companionPackage = @($metadata.packages | Where-Object name -eq 'mcdev-companion')
    if ($companionPackage.Count -ne 1) { throw 'native companion package was not found' }
    $companionPackageVersion = [string]$companionPackage[0].version
    $productVersion = $companionPackageVersion
    if ($Version -ne $productVersion) {
        throw "release version $Version does not match the native Cargo product version $productVersion"
    }

    $buildCommit = $null
    $buildCommitOverride = [Environment]::GetEnvironmentVariable('MCDEV_BUILD_COMMIT', 'Process')
    try {
        $gitCommitOutput = & git -C $repo rev-parse HEAD 2>$null
        $gitCommit = ([string](@($gitCommitOutput) | Select-Object -First 1)).Trim()
        if ($LASTEXITCODE -eq 0 -and $gitCommit -match '^[0-9a-fA-F]{40}$') {
            $dirtyOutput = & git -C $repo status --porcelain 2>$null
            if ($LASTEXITCODE -eq 0) {
                $buildCommit = $gitCommit
                if (-not [string]::IsNullOrWhiteSpace(($dirtyOutput -join [Environment]::NewLine))) {
                    $buildCommit += '+dirty'
                }
            }
        }
    } catch {
        $buildCommit = $null
    }
    if ([string]::IsNullOrWhiteSpace($buildCommit)) {
        $buildCommit = $buildCommitOverride
    }
    if ([string]::IsNullOrWhiteSpace($buildCommit)) {
        $buildCommit = $null
    }
    $buildTime = [DateTimeOffset]::UtcNow.ToString(
        'yyyy-MM-ddTHH:mm:ss.fffZ',
        [Globalization.CultureInfo]::InvariantCulture)

    $buildTimeout = 900
    $oldBuildCommit = [Environment]::GetEnvironmentVariable('MCDEV_BUILD_COMMIT', 'Process')
    $oldBuildTime = [Environment]::GetEnvironmentVariable('MCDEV_BUILD_TIME', 'Process')
    $oldBuildVersion = [Environment]::GetEnvironmentVariable('MCDEV_BUILD_VERSION', 'Process')
    try {
        if ($null -eq $buildCommit) { Remove-Item Env:MCDEV_BUILD_COMMIT -ErrorAction SilentlyContinue }
        else { $env:MCDEV_BUILD_COMMIT = $buildCommit }
        $env:MCDEV_BUILD_TIME = $buildTime
        $env:MCDEV_BUILD_VERSION = $Version
        $buildResult = Invoke-RustCheck -Name 'native-release-build' -Command 'cargo' -Arguments @(
            'build', '--release', '--locked', '-p', 'mcdev-companion', '--bin', 'mcdev-companion'
        ) -Directory $repo -LogDirectory $logs -TimeoutSeconds $buildTimeout
    } finally {
        if ($null -eq $oldBuildCommit) { Remove-Item Env:MCDEV_BUILD_COMMIT -ErrorAction SilentlyContinue }
        else { $env:MCDEV_BUILD_COMMIT = $oldBuildCommit }
        if ($null -eq $oldBuildTime) { Remove-Item Env:MCDEV_BUILD_TIME -ErrorAction SilentlyContinue }
        else { $env:MCDEV_BUILD_TIME = $oldBuildTime }
        if ($null -eq $oldBuildVersion) { Remove-Item Env:MCDEV_BUILD_VERSION -ErrorAction SilentlyContinue }
        else { $env:MCDEV_BUILD_VERSION = $oldBuildVersion }
    }
    $buildResult | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'native-release-build.result.json')
    $summary.Add(('build: exit={0} timeout={1} seconds={2} pid={3} deadline={4}s' -f
        $buildResult.ExitCode, $buildResult.TimedOut, $buildResult.Seconds, $buildResult.Pid, $buildTimeout))
    $summary.Add(('build metadata: commit={0}; time={1}; version={2}' -f
        ($buildCommit ?? 'unknown'), $buildTime, $Version))
    if ($buildResult.ExitCode -ne 0) {
        Get-Content -LiteralPath (Join-Path $logs 'native-release-build.stderr.log') -Tail 40
        throw 'native release build failed; see complete logs'
    }

    $binaryName = 'mcdev-companion'
    if ($IsWindows) { $binaryName += '.exe' }
    if ($IsWindows) { $platform = 'windows' }
    elseif ($IsLinux) { $platform = 'linux' }
    else { throw 'native release packaging supports only Windows and Linux hosts' }
    $architecture = switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
        'X64' { 'x86_64' }
        'Arm64' { 'aarch64' }
        'X86' { 'x86' }
        'Arm' { 'armv7' }
        default { throw "unsupported host architecture: $($_)" }
    }
    $binary = Join-Path $repo ('target/release/' + $binaryName)
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        throw "release binary was not produced: $binary"
    }

    $stage = Join-Path $logs 'package'
    Assert-PathUnderRoot $logs $stage
    $null = New-Item -ItemType Directory -Force -Path $stage

    function Copy-Payload([string] $Source, [string] $RelativeDestination) {
        $destination = Join-Path $stage $RelativeDestination
        Assert-PathUnderRoot $stage $destination
        $parent = Split-Path -Parent $destination
        $null = New-Item -ItemType Directory -Force -Path $parent
        if (Test-Path -LiteralPath $Source -PathType Container) {
            Copy-Item -LiteralPath $Source -Destination $destination -Recurse -Force
        } else {
            Copy-Item -LiteralPath $Source -Destination $destination -Force
        }
    }

    Copy-Payload $binary $binaryName
    Copy-Payload (Join-Path $repo 'docs/native-distribution.md') 'README-native.md'

    $runtimeDirectories = [System.Collections.Generic.List[string]]::new()
    foreach ($runtimeRoot in @('lua', 'plugin', 'ftdetect', 'after')) {
        $source = Join-Path $repo ('mcdev-nvim/' + $runtimeRoot)
        if (Test-Path -LiteralPath $source -PathType Container) {
            Copy-Payload $source ('mcdev-nvim/' + $runtimeRoot)
            $runtimeDirectories.Add('mcdev-nvim/' + $runtimeRoot)
        }
    }
    if (-not $runtimeDirectories.Contains('mcdev-nvim/lua')) {
        throw 'mcdev-nvim/lua is required for a native package'
    }

    foreach ($notice in @('MixinExtras-LICENSE', 'MixinExtras-PROVENANCE.md')) {
        $source = Join-Path $repo ('third-party/' + $notice)
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "required third-party notice is missing: $source"
        }
        Copy-Payload $source ('third-party/' + $notice)
    }

    $payloadFiles = @(Get-ChildItem -LiteralPath $stage -File -Recurse)
    $forbiddenPath = '(?i)(^|/)(tests?|corpora?|target|mcdev-test-fixtures|mcdev-jdtls-extension|host|ktlsp)(/|$)|\.(jar|class)$'
    foreach ($file in $payloadFiles) {
        $relative = [IO.Path]::GetRelativePath($stage, $file.FullName).Replace('\', '/')
        if ($relative -match $forbiddenPath) {
            throw "forbidden release payload path: $relative"
        }
    }

    $entries = @(
        foreach ($file in ($payloadFiles | Sort-Object FullName)) {
            $relative = [IO.Path]::GetRelativePath($stage, $file.FullName).Replace('\', '/')
            [ordered]@{
                path = $relative
                bytes = [int64]$file.Length
                sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            }
        }
    )
    $manifest = [ordered]@{
        artifact = 'mcdev-companion'
        releaseVersion = $Version
        releaseVersionSource = 'existing release tag or workflow_dispatch input'
        productVersion = $productVersion
        companionPackageVersion = $companionPackageVersion
        platform = $platform
        architecture = $architecture
        # Keep the required base compatible with semantic-only hosts.
        capabilities = @('semantic-v1')
        optionalCapabilities = @(
            'project-class-facts-v1', 'source-snapshots-v1', 'owned-resources-v1',
            'runtime-class-bytes-v1', 'jvm-catalog-v1', 'provider-commands-v1',
            'source-locations-v1', 'source-facts-v2', 'project-inputs-v1', 'binary-annotations-v1',
            'root-files-v1'
        )
        supportedOps = [ordered]@{
            'project-class-facts-v1' = @('projectContext', 'typeJvm')
            'source-snapshots-v1' = @('sourceInventory', 'sourceSnapshot')
            'owned-resources-v1' = @('resourceInventory', 'resourceLookup', 'resourceSnapshot')
            'runtime-class-bytes-v1' = @('runtimeClass', 'classBytes')
            'jvm-catalog-v1' = @('jvmCatalog', 'jvmClass')
            'source-locations-v1' = @('sourceLocation')
            'source-facts-v2' = @('sourceFacts', 'annotationSites')
            'project-inputs-v1' = @('projectInputs')
            'root-files-v1' = @('rootFileInventory', 'rootFileSnapshot')
        }
        runtimeDirectories = @($runtimeDirectories)
        files = $entries
        manifestFilesExcludedFromSelfHash = $true
    }
    $manifestPath = Join-Path $stage 'manifest.json'
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8NoBOM -LiteralPath $manifestPath

    $manifestRead = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $manifestEntries = @($manifestRead.files)
    if ($manifestEntries.Count -ne $entries.Count) {
        throw 'release manifest entry count does not match the payload'
    }
    foreach ($entry in $manifestEntries) {
        $filePath = Join-Path $stage ($entry.path -replace '/', [IO.Path]::DirectorySeparatorChar)
        if (-not (Test-Path -LiteralPath $filePath -PathType Leaf)) {
            throw "manifest entry is missing from the package: $($entry.path)"
        }
        $actual = Get-FileHash -LiteralPath $filePath -Algorithm SHA256
        $length = (Get-Item -LiteralPath $filePath).Length
        if ($length -ne [int64]$entry.bytes -or $actual.Hash.ToLowerInvariant() -ne $entry.sha256) {
            throw "manifest hash or size mismatch: $($entry.path)"
        }
    }

    $null = New-Item -ItemType Directory -Force -Path $output
    $archiveName = 'mcdev-companion-{0}-{1}-{2}.zip' -f $Version, $platform, $architecture
    $archive = Join-Path $output $archiveName
    Assert-PathUnderRoot $output $archive
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -CompressionLevel Optimal -Force

    function Read-ZipEntry([IO.Compression.ZipArchiveEntry] $Entry) {
        $memory = [IO.MemoryStream]::new()
        $stream = $Entry.Open()
        try { $stream.CopyTo($memory) } finally { $stream.Dispose() }
        try {
            $sha = [Security.Cryptography.SHA256]::Create()
            try { $digest = $sha.ComputeHash($memory.ToArray()) } finally { $sha.Dispose() }
            return [pscustomobject]@{
                Bytes = [int64]$memory.Length
                Sha256 = ([Convert]::ToHexString($digest)).ToLowerInvariant()
            }
        } finally {
            $memory.Dispose()
        }
    }

    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $expectedFiles = @($manifestEntries | ForEach-Object { [string]$_.path }) + @('manifest.json')
        $zipFiles = @($zip.Entries | Where-Object { -not $_.FullName.EndsWith('/') } | ForEach-Object FullName)
        $missing = @($expectedFiles | Where-Object { $zipFiles -notcontains $_ })
        $extra = @($zipFiles | Where-Object { $expectedFiles -notcontains $_ })
        if ($missing.Count -gt 0 -or $extra.Count -gt 0 -or $zipFiles.Count -ne $expectedFiles.Count) {
            throw ('archive file entries differ from the manifest; missing={0}; extra={1}' -f
                ($missing -join ', '), ($extra -join ', '))
        }
        foreach ($directory in @($zip.Entries | Where-Object { $_.FullName.EndsWith('/') } | ForEach-Object FullName)) {
            if (-not @($expectedFiles | Where-Object { $_.StartsWith($directory, [StringComparison]::Ordinal) })) {
                throw "archive contains an unowned directory entry: $directory"
            }
        }
        foreach ($entry in $zipFiles) {
            if ($entry -match $forbiddenPath) {
                throw "archive contains forbidden payload path: $entry"
            }
        }
        foreach ($manifestEntry in $manifestEntries) {
            $zipEntry = $zip.GetEntry([string]$manifestEntry.path)
            if ($null -eq $zipEntry -or $zipEntry.FullName.EndsWith('/')) {
                throw "archive manifest entry is not a file: $($manifestEntry.path)"
            }
            $actual = Read-ZipEntry $zipEntry
            if ($actual.Bytes -ne [int64]$manifestEntry.bytes -or $actual.Sha256 -ne $manifestEntry.sha256) {
                throw "archive bytes or SHA-256 mismatch: $($manifestEntry.path)"
            }
        }
        $summary.Add(('zip selfcheck: {0} manifest files opened and verified; no extra file entries' -f $manifestEntries.Count))
    } finally {
        $zip.Dispose()
    }

    $smokeTimeout = 120
    $oldCompanion = [Environment]::GetEnvironmentVariable('MCDEV_COMPANION_BINARY', 'Process')
    $oldManifest = [Environment]::GetEnvironmentVariable('MCDEV_NATIVE_MANIFEST', 'Process')
    try {
        $env:MCDEV_COMPANION_BINARY = $binary
        $env:MCDEV_NATIVE_MANIFEST = $manifestPath
        $capabilityResult = Invoke-RustCheck -Name 'native-release-capability-smoke' -Command 'nvim' -Arguments @(
            '--headless', '-u', 'mcdev-nvim/tests/minimal_init.lua', '-l', 'mcdev-nvim/tests/native_package.lua'
        ) -Directory $repo -LogDirectory $logs -TimeoutSeconds 30
        $capabilityResult | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'native-release-capability-smoke.result.json')
        $summary.Add(('capability-smoke: exit={0} timeout={1} seconds={2} pid={3} deadline=30s' -f
            $capabilityResult.ExitCode, $capabilityResult.TimedOut, $capabilityResult.Seconds, $capabilityResult.Pid))
        if ($capabilityResult.ExitCode -ne 0) {
            Get-Content -LiteralPath (Join-Path $logs 'native-release-capability-smoke.stderr.log') -Tail 40
            throw 'native release capability metadata smoke failed; see complete logs'
        }
        $smokeResult = Invoke-RustCheck -Name 'native-release-stdio-smoke' -Command 'nvim' -Arguments @(
            '--headless', '-u', 'mcdev-nvim/tests/minimal_init.lua', '-l', 'mcdev-nvim/tests/native_helper.lua'
        ) -Directory $repo -LogDirectory $logs -TimeoutSeconds $smokeTimeout
    } finally {
        if ($null -eq $oldCompanion) { Remove-Item Env:MCDEV_COMPANION_BINARY -ErrorAction SilentlyContinue }
        else { $env:MCDEV_COMPANION_BINARY = $oldCompanion }
        if ($null -eq $oldManifest) { Remove-Item Env:MCDEV_NATIVE_MANIFEST -ErrorAction SilentlyContinue }
        else { $env:MCDEV_NATIVE_MANIFEST = $oldManifest }
    }
    $smokeResult | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'native-release-stdio-smoke.result.json')
    $summary.Add(('stdio-smoke: exit={0} timeout={1} seconds={2} pid={3} deadline={4}s' -f
        $smokeResult.ExitCode, $smokeResult.TimedOut, $smokeResult.Seconds, $smokeResult.Pid, $smokeTimeout))
    if ($smokeResult.ExitCode -ne 0) {
        Get-Content -LiteralPath (Join-Path $logs 'native-release-stdio-smoke.stderr.log') -Tail 40
        throw 'native release --stdio-completion smoke failed; see complete logs'
    }

    $archiveInfo = Get-Item -LiteralPath $archive
    $archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $summary.Add(('version: release={0}; product={1}; companion-cargo={2}; source=existing-tag-or-input' -f
        $Version, $productVersion, $companionPackageVersion))
    $summary.Add(('runtime directories: {0}' -f (@($runtimeDirectories) -join ', ')))
    $summary.Add(('archive: {0} bytes={1} sha256={2}' -f $archive, $archiveInfo.Length, $archiveHash))
    $summary.Add('scope: semantic-v1 required base; optional inputs require host operation negotiation and provider commands require configured allowlists; no bundled host/JVM/tests/corpora/Kotlin/JDT jar or complete MC project/SDK authority')
    Write-Output ('artifact: {0}' -f $archive)
    Write-Output ('artifact bytes: {0}' -f $archiveInfo.Length)
    Write-Output ('artifact sha256: {0}' -f $archiveHash)
    $exitCode = 0
} catch {
    $summary.Add(('error: {0}' -f $_.Exception.Message))
    Write-Output $_.Exception.Message
} finally {
    if ($null -ne $stage -and (Test-Path -LiteralPath $stage)) {
        Assert-PathUnderRoot $logs $stage
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    $summary.Add(('exit code: {0}' -f $exitCode))
    $summary.Add(('logs: {0}' -f $logs))
    $summary | Set-Content -Encoding utf8NoBOM -LiteralPath (Join-Path $logs 'summary.txt')
    Write-Output ('logs: {0}' -f $logs)
}
exit $exitCode
