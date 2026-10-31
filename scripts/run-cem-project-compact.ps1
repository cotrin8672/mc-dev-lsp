#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $HostBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $HostSha256,
    [Parameter(Mandatory = $true)][string] $CompanionBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $CompanionSha256,
    [Parameter(Mandatory = $true)][string] $NvimBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $NvimSha256,
    [Parameter(Mandatory = $true)][string] $GradleBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $GradleSha256,
    [string] $JdkHome = 'C:/Program Files/Java/jdk-21',
    [string] $BlinkCmpPath = 'C:/Users/gummy/AppData/Local/nvim-data/lazy/blink.cmp',
    [string] $BlinkLibPath = 'C:/Users/gummy/AppData/Local/nvim-data/lazy/blink.lib',
    [string] $InputsReceipt = (Join-Path $PSScriptRoot '../build/live-fixture-inputs/receipt.json'),
    [string] $SourceProvenance = (Join-Path $PSScriptRoot '../build/live-fixture-inputs/source-provenance.json'),
    [string] $MemberReference = $env:MCDEV_E2E_MEMBER_REFERENCE,
    [ValidateRange(60, 1200)][int] $TimeoutSeconds = 600,
    [switch] $PrepareOnly
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$logs = Assert-E2eBuildPath (Join-Path $repo ('build/rust-test-logs/cem-project-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8)))
$null = New-Item -ItemType Directory -Path $logs
$project = Join-Path $logs 'workspace'
$exitCode = 1
$before = $null
$hashPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$receipt = [ordered]@{
    scope = 'unchanged original CEM member, Shadow, Blink aggregate and four snippet scenarios'
    mode = $(if ($PrepareOnly) { 'fixture-selfcheck' } else { 'real-host' })
    fixture = $project; deadlineSeconds = $TimeoutSeconds; scopePassed = $false
    originalSuiteExecuted = $false; completeReplacementVerified = $false
    userCemProjectCopiedOrWritten = $false; gradleBuildExecuted = $false
    fullMemberSetComparisonSkipped = [string]::IsNullOrWhiteSpace($MemberReference)
    memberReference = $(if ($MemberReference) { $MemberReference } else { $null })
    memberReferencePolicy = 'Only an existing external reference is accepted; native results never create the golden file'
}
$environment = @('MCDEV_E2E_KTLSP', 'MCDEV_E2E_COMPANION', 'MCDEV_E2E_WORKSPACE', 'MCDEV_E2E_FIXTURE',
    'MCDEV_E2E_BLINK_RTP', 'MCDEV_E2E_BLINK_LIB_RTP', 'MCDEV_E2E_MEMBER_REFERENCE', 'MCDEV_E2E_ACCEPTANCE_PROOF',
    'NVIM_CMD', 'GRADLE_CMD', 'JAVA_HOME', 'JAVA_HOME_21_X64', 'PATH', 'GRADLE_USER_HOME',
    'KTLSP_TRACE', 'KTLSP_CACHE_DIR', 'KTLSP_JDK_SRC', 'NVIM_LOG_FILE',
    'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_CACHE_HOME')
$savedEnvironment = @{}
foreach ($name in $environment) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }

function Select-PinnedExecutable([string] $Path, [string] $Sha256, [string] $Name) {
    if (-not [IO.Path]::IsPathFullyQualified($Path)) { throw "$Name requires an explicit absolute path" }
    $selected = (Resolve-Path -LiteralPath $Path).Path
    if (-not (Test-Path -LiteralPath $selected -PathType Leaf) -or
        (Get-FileHash -LiteralPath $selected -Algorithm SHA256).Hash -ne $Sha256) { throw "$Name checksum mismatch" }
    $null = $hashPaths.Add($selected)
    return $selected
}

function Write-CemFixture([string] $Relative, [string] $Text) {
    $path = Assert-E2eBuildPath (Join-Path $project $Relative)
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path)
    [IO.File]::WriteAllText($path, $Text, [Text.UTF8Encoding]::new($false))
}

try {
    $hostPath = Select-PinnedExecutable $HostBinary $HostSha256 'HostBinary'
    $companionPath = Select-PinnedExecutable $CompanionBinary $CompanionSha256 'CompanionBinary'
    $editorPath = Select-PinnedExecutable $NvimBinary $NvimSha256 'NvimBinary'
    $gradlePath = Select-PinnedExecutable $GradleBinary $GradleSha256 'GradleBinary'
    foreach ($path in @($JdkHome, $BlinkCmpPath, $BlinkLibPath, $InputsReceipt, $SourceProvenance)) {
        if (-not [IO.Path]::IsPathFullyQualified($path)) { throw "An absolute input path is required: $path" }
    }
    $jdk = (Resolve-Path -LiteralPath $JdkHome).Path
    $jdkRelease = Join-Path $jdk 'release'
    if ([IO.File]::ReadAllText($jdkRelease) -notmatch '(?m)^JAVA_VERSION="21(?:\.|\+|")') { throw 'An explicitly selected JDK 21 is required' }
    foreach ($relative in @('release', 'bin/java.exe', 'bin/javac.exe', 'lib/src.zip')) {
        $path = (Resolve-Path -LiteralPath (Join-Path $jdk $relative)).Path
        $null = $hashPaths.Add($path)
    }
    $gradleHome = Split-Path -Parent (Split-Path -Parent $gradlePath)
    $gradleCore = Join-Path $gradleHome 'lib/gradle-core-9.6.1.jar'
    if (-not (Test-Path -LiteralPath $gradleCore -PathType Leaf)) { throw 'The recognized Gradle 9.6.1 distribution is required' }
    $null = $hashPaths.Add((Resolve-Path -LiteralPath $gradleCore).Path)
    $blinkCmp = (Resolve-Path -LiteralPath $BlinkCmpPath).Path
    $blinkLib = (Resolve-Path -LiteralPath $BlinkLibPath).Path
    foreach ($runtime in @($blinkCmp, $blinkLib)) {
        if (-not (Test-Path -LiteralPath (Join-Path $runtime 'lua') -PathType Container)) { throw "Blink runtime has no Lua sources: $runtime" }
        foreach ($file in Get-ChildItem -LiteralPath $runtime -Recurse -File |
            Where-Object { $_.FullName -notmatch '[\\/]\.git[\\/]' -and $_.Extension -in @('.lua', '.dll', '.so', '.dylib', '.json') }) {
            $null = $hashPaths.Add($file.FullName)
        }
    }
    $receiptPath = (Resolve-Path -LiteralPath $InputsReceipt).Path
    $provenancePath = (Resolve-Path -LiteralPath $SourceProvenance).Path
    $inputManifest = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
    $provenance = Get-Content -LiteralPath $provenancePath -Raw | ConvertFrom-Json
    if (@($inputManifest.failures).Count -ne 0) { throw 'The fixed-input receipt contains failures' }
    $expectedInputs = @('mcdev-test-fixtures-0.7.7.jar', 'neoforge-21.1.248-merged.jar', 'ItemMixin.java',
        'createenchantablemachinery.mixins.json', 'mixinextras-neoforge-0.5.3.jar', 'mixin-0.8.7.jar', 'mixinextras-fabric-0.5.5.jar')
    if (@($inputManifest.artifacts.PSObject.Properties).Count -ne $expectedInputs.Count) { throw 'Expected the seven fixed live inputs' }
    $selectedInputs = [ordered]@{}
    foreach ($name in $expectedInputs) {
        $artifact = $inputManifest.artifacts.$name
        if (-not $artifact -or $artifact.sha256 -notmatch '^[0-9a-f]{64}$' -or -not $artifact.readOnly) { throw "Invalid fixed input: $name" }
        $expectedPath = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $receiptPath) $name))
        if (-not [IO.Path]::IsPathFullyQualified($artifact.path) -or [IO.Path]::GetFullPath($artifact.path) -ne $expectedPath) {
            throw "Input must be the exact fixed copy beside its receipt: $name"
        }
        $path = (Resolve-Path -LiteralPath $artifact.path).Path
        $file = Get-Item -LiteralPath $path
        if ($file.Length -ne $artifact.size -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $artifact.sha256) {
            throw "Fixed input checksum/size mismatch: $name"
        }
        if ($name.EndsWith('.jar')) {
            $archive = [IO.Compression.ZipFile]::OpenRead($path)
            try {
                foreach ($entry in @($artifact.requiredEntries)) {
                    if (-not $archive.GetEntry($entry)) { throw "Fixed input misses $entry in $name" }
                }
            } finally { $archive.Dispose() }
        }
        $selectedInputs[$name] = [ordered]@{ path = $path; sha256 = $artifact.sha256; size = $file.Length }
        $null = $hashPaths.Add($path)
    }
    if ($provenance.jarSha256 -ne $selectedInputs['mcdev-test-fixtures-0.7.7.jar'].sha256) { throw 'Source provenance fixture archive mismatch' }
    foreach ($name in @('ItemMixin.java', 'createenchantablemachinery.mixins.json')) {
        $claimed = @($provenance.cemOriginalsRaw | Where-Object { [IO.Path]::GetFileName($_.path) -eq $name })
        if ($claimed.Count -ne 1 -or $claimed[0].sha256 -ne $selectedInputs[$name].sha256) { throw "Original CEM provenance mismatch: $name" }
    }
    Copy-Item -LiteralPath $receiptPath -Destination (Join-Path $logs 'fixed-input-receipt.json')
    Copy-Item -LiteralPath $provenancePath -Destination (Join-Path $logs 'fixed-input-source-provenance.json')
    $null = $hashPaths.Add($receiptPath)
    $null = $hashPaths.Add($provenancePath)
    $null = New-Item -ItemType Directory -Path $project
    $mixin = Join-Path $project 'src/main/java/io/github/cotrin8672/cem/mixin/ItemMixin.java'
    $config = Join-Path $project 'src/main/resources/createenchantablemachinery.mixins.json'
    foreach ($destination in @($mixin, $config, (Join-Path $project 'libs'))) {
        $directory = if ($destination.EndsWith('libs')) { $destination } else { Split-Path -Parent $destination }
        $null = New-Item -ItemType Directory -Force -Path (Assert-E2eBuildPath $directory)
    }
    Copy-Item -LiteralPath $selectedInputs['ItemMixin.java'].path -Destination $mixin
    Copy-Item -LiteralPath $selectedInputs['createenchantablemachinery.mixins.json'].path -Destination $config
    $cemJars = @('neoforge-21.1.248-merged.jar', 'mixin-0.8.7.jar', 'mixinextras-neoforge-0.5.3.jar')
    foreach ($name in $cemJars) { Copy-Item -LiteralPath $selectedInputs[$name].path -Destination (Join-Path $project ('libs/' + $name)) }
    $lf = [string][char]10
    Write-CemFixture 'settings.gradle' ("rootProject.name = 'mcdev-cem-original-editor-proof'" + $lf)
    Write-CemFixture 'build.gradle' (@(
        "plugins { id 'java' }",
        'java { toolchain { languageVersion = JavaLanguageVersion.of(21) } }',
        "tasks.withType(JavaCompile).configureEach { options.encoding = 'UTF-8' }",
        "dependencies { implementation files('libs/neoforge-21.1.248-merged.jar', 'libs/mixin-0.8.7.jar', 'libs/mixinextras-neoforge-0.5.3.jar') }", ''
    ) -join $lf)
    Write-CemFixture 'gradle.properties' (@('org.gradle.daemon=false', 'org.gradle.parallel=false', 'org.gradle.workers.max=2',
        ('org.gradle.java.home=' + $jdk.Replace('\', '/')), ('org.gradle.java.installations.paths=' + $jdk.Replace('\', '/')),
        'org.gradle.java.installations.auto-detect=false', 'org.gradle.java.installations.auto-download=false', '') -join $lf)
    $env:GRADLE_CMD = $gradlePath
    Write-E2eGradleWrapper $project
    if ($MemberReference) {
        if (-not [IO.Path]::IsPathFullyQualified($MemberReference)) { throw 'MemberReference must be an absolute path to an existing golden file' }
        $reference = (Resolve-Path -LiteralPath $MemberReference).Path
        $null = $hashPaths.Add($reference)
        $memberCopy = Join-Path $logs 'member-reference.tsv'
        Copy-Item -LiteralPath $reference -Destination $memberCopy
        $null = $hashPaths.Add($memberCopy)
        $receipt.memberReference = $reference
        $env:MCDEV_E2E_MEMBER_REFERENCE = $memberCopy
    } else { [Environment]::SetEnvironmentVariable('MCDEV_E2E_MEMBER_REFERENCE', $null, 'Process') }
    $fixtureFiles = @(Get-ChildItem -LiteralPath $project -Recurse -File)
    $fixtureSnapshot = Join-Path $logs 'fixture.snapshot.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($project, $fixtureSnapshot)
    $null = $hashPaths.Add($fixtureSnapshot)
    foreach ($file in $fixtureFiles) { $null = $hashPaths.Add($file.FullName) }
    $testFiles = @('mcdev-nvim/tests/e2e/jdtls_bundle_e2e.lua', 'mcdev-nvim/tests/e2e/cem_project_e2e.lua',
        'mcdev-nvim/tests/e2e/cem_feature_snippets.lua', 'mcdev-nvim/tests/e2e/run_bundle_e2e.lua',
        'mcdev-nvim/tests/e2e/minimal_init.lua', 'mcdev-nvim/tests/test_helpers.lua',
        'mcdev-companion/tests/support/accepted_trace.lua', 'mcdev-companion/tests/support/accepted_command_readiness.lua',
        'scripts/run-osgi-e2e-compact.ps1', 'scripts/run-osgi-e2e.ps1', 'scripts/e2e-workspace-common.ps1',
        'scripts/run-rust-tests-compact.ps1', 'scripts/run-cem-project-compact.ps1')
    foreach ($relative in $testFiles) { $null = $hashPaths.Add((Join-Path $repo $relative)) }
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $repo 'mcdev-nvim/lua') -Recurse -File) { $null = $hashPaths.Add($file.FullName) }
    $before = @(Get-FileHash -LiteralPath @($hashPaths | Sort-Object) -Algorithm SHA256)
    $before | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $logs 'proof-inputs.before.json') -Encoding utf8
    $receipt.selectedInputs = $selectedInputs
    $receipt.selectedCemClasspath = $cemJars
    $receipt.binaries = [ordered]@{ host = $hostPath; hostSha256 = $HostSha256.ToLowerInvariant();
        companion = $companionPath; companionSha256 = $CompanionSha256.ToLowerInvariant();
        nvim = $editorPath; nvimSha256 = $NvimSha256.ToLowerInvariant();
        gradle = $gradlePath; gradleSha256 = $GradleSha256.ToLowerInvariant(); gradleDistribution = '9.6.1'; jdkHome = $jdk }
    $receipt.blink = @{ cmp = $blinkCmp; lib = $blinkLib }
    $actualTestPaths = @($testFiles | Where-Object { $_.EndsWith('.lua') } | ForEach-Object { Join-Path $repo $_ })
    $receipt.actualTests = @($before | Where-Object { $_.Path -in $actualTestPaths })
    $receipt.fixtureSnapshotSha256 = (Get-FileHash -LiteralPath $fixtureSnapshot -Algorithm SHA256).Hash.ToLowerInvariant()
    foreach ($name in @('ItemMixin.java', 'createenchantablemachinery.mixins.json') + $cemJars) {
        $copy = if ($name -eq 'ItemMixin.java') { $mixin } elseif ($name.EndsWith('.json')) { $config } else { Join-Path $project ('libs/' + $name) }
        if ((Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne $selectedInputs[$name].sha256) { throw "Fixture copy mismatch: $name" }
    }
    $receipt.fixtureSelfCheckPassed = $true
    if (-not $PrepareOnly) {
        $env:MCDEV_E2E_KTLSP, $env:MCDEV_E2E_COMPANION = $hostPath, $companionPath
        $env:MCDEV_E2E_WORKSPACE, $env:MCDEV_E2E_FIXTURE = $project, 'real-cem'
        $env:MCDEV_E2E_BLINK_RTP, $env:MCDEV_E2E_BLINK_LIB_RTP = $blinkCmp, $blinkLib
        $env:NVIM_CMD, $env:JAVA_HOME, $env:JAVA_HOME_21_X64 = $editorPath, $jdk, $jdk
        $env:PATH = (Join-Path $jdk 'bin') + [IO.Path]::PathSeparator + (Split-Path -Parent $gradlePath) + [IO.Path]::PathSeparator + $savedEnvironment.PATH
        $env:GRADLE_USER_HOME = Join-Path $logs 'gradle-user'
        $env:KTLSP_CACHE_DIR = Join-Path $logs 'host-cache'
        $env:KTLSP_TRACE = Join-Path $logs 'host.trace.jsonl'
        $env:KTLSP_JDK_SRC = Join-Path $jdk 'lib/src.zip'
        $env:MCDEV_E2E_ACCEPTANCE_PROOF = Join-Path $logs 'editor-proof.json'
        $env:NVIM_LOG_FILE = Join-Path $logs 'nvim.log'
        foreach ($entry in @{ XDG_CONFIG_HOME='editor-config'; XDG_DATA_HOME='editor-data'; XDG_STATE_HOME='editor-state'; XDG_CACHE_HOME='editor-cache' }.GetEnumerator()) {
            [Environment]::SetEnvironmentVariable($entry.Key, (Join-Path $logs $entry.Value), 'Process')
        }
        $receipt.originalSuiteExecuted = $true
        $run = Invoke-RustCheck -Name 'cem-native-e2e' -Command (Get-Process -Id $PID).Path -Arguments @(
            '-NoLogo', '-NoProfile', '-File', (Join-Path $PSScriptRoot 'run-osgi-e2e-compact.ps1'),
            '-LogDirectory', (Join-Path $logs 'e2e'), '-TimeoutSeconds', [string]$TimeoutSeconds
        ) -Directory $repo -LogDirectory $logs -TimeoutSeconds ($TimeoutSeconds + 30)
        $receipt.editor = $run
        $proofPath = Join-Path $logs 'editor-proof.json'
        $proof = if (Test-Path -LiteralPath $proofPath) { Get-Content -LiteralPath $proofPath -Raw | ConvertFrom-Json } else { $null }
        $receipt.editorProof = $proof
        if ($run.ExitCode -ne 0 -or $run.TimedOut -or -not $proof -or -not $proof.scopePassed -or $proof.error -or
            -not $proof.originalCemInvoked -or @($proof.completions).Count -eq 0 -or @($proof.originalAssertions).Count -eq 0 -or
            -not $proof.shutdown.exited -or $proof.shutdown.code -ne 0 -or $proof.shutdown.signal -ne 0) {
            throw 'Original CEM native E2E failed or was incomplete; see the finite compact run and editor proof'
        }
        $receipt.scopePassed = $true
    }
    $exitCode = 0
} catch {
    $receipt.error = $_.Exception.Message
    Write-Output $_.Exception.Message
} finally {
    if ($before) {
        try {
            $after = @(Get-FileHash -LiteralPath @($hashPaths | Sort-Object) -Algorithm SHA256)
            $after | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $logs 'proof-inputs.after.json') -Encoding utf8
            $receipt.inputHashesStable = @(Compare-Object $before $after -Property Path, Hash).Count -eq 0
            if (-not $receipt.inputHashesStable) { throw 'CEM inputs, fixture files, binaries or actual test sources changed during the run' }
        } catch { $receipt.inputHashError = $_.Exception.Message; $exitCode = 1; $receipt.scopePassed = $false }
    }
    foreach ($name in $environment) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
    $receipt.exitCode = $exitCode
    $receipt | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath (Join-Path $logs 'receipt.json') -Encoding utf8
    Write-Output ('CEM: exit=' + $exitCode + ' fixture=' + [bool]$receipt.fixtureSelfCheckPassed +
        ' live=' + [bool]$receipt.originalSuiteExecuted + ' passed=' + [bool]$receipt.scopePassed +
        ' memberGoldenSkipped=' + [bool]$receipt.fullMemberSetComparisonSkipped)
    Write-Output ('logs: ' + $logs)
}
exit $exitCode
