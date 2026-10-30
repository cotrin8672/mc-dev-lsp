#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $HostBinary,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $HostSha256,
    [Parameter(Mandatory)][string] $ProbeBinary,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $ProbeSha256,
    [string] $NvimBinary = '',
    [string] $GradleBinary = ''
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$logs = Join-Path $repo ('build/rust-test-logs/runtime-bytes-probe-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$project = Join-Path $temporaryRoot ('mcdev-runtime-bytes-' + [guid]::NewGuid())
$watch = [Diagnostics.Stopwatch]::StartNew()
$exitCode, $before, $inputs = 1, $null, @()
$receipt = [ordered]@{
    testOnly = $true; scope = 'real host producer + test provider + production Rust typed reader Code/LVT proof'
    productLocalCompletionVerified = $false; fullSugarParityVerified = $false
    hostSha256 = $HostSha256.ToLowerInvariant(); probeSha256 = $ProbeSha256.ToLowerInvariant()
    deadlineSeconds = 600; project = $project; refreshRequested = $false
}
function Budget([int] $Maximum) {
    # Reserve time for owned-tree termination, log closure and the receipt.
    $remaining = 585 - [int][Math]::Ceiling($watch.Elapsed.TotalSeconds)
    if ($remaining -le 0) { throw 'Whole runtime byte proof exceeded its 600 second deadline' }
    [Math]::Min($Maximum, $remaining)
}
function Write-FixtureFile([string] $Relative, [string] $Text) {
    $path = Join-Path $project $Relative
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path)
    [IO.File]::WriteAllText($path, $Text.Replace("`r`n", "`n"), [Text.UTF8Encoding]::new($false))
}
try {
    if (-not $IsWindows) { throw 'This pinned Windows host fixture requires Windows' }
    if (-not [IO.Path]::IsPathRooted($HostBinary) -or -not [IO.Path]::IsPathRooted($ProbeBinary)) {
        throw 'HostBinary and ProbeBinary must be absolute paths'
    }
    $hostPath, $probePath = (Resolve-Path -LiteralPath $HostBinary).Path, (Resolve-Path -LiteralPath $ProbeBinary).Path
    if ((Get-FileHash -LiteralPath $hostPath -Algorithm SHA256).Hash -ne $HostSha256) { throw 'Stock host checksum mismatch' }
    if ((Get-FileHash -LiteralPath $probePath -Algorithm SHA256).Hash -ne $ProbeSha256) { throw 'Test provider checksum mismatch' }
    if (-not $NvimBinary) { $NvimBinary = Join-Path $repo 'build/tools/nvim-v0.11.5/extracted/nvim-win64/bin/nvim.exe' }
    if (-not $GradleBinary) {
        $GradleBinary = Join-Path $repo 'build/tools/gradle-9.6.1/extracted/gradle-9.6.1/bin/gradle.bat'
        if (-not (Test-Path -LiteralPath $GradleBinary -PathType Leaf)) {
            throw 'Workspace Gradle 9.6.1 is missing; pass an explicit -GradleBinary recognized by the pinned host SDK adapter'
        }
    }
    $editorPath, $gradlePath = (Get-Command $NvimBinary).Source, (Get-Command $GradleBinary).Source
    $receipt.gradleBinary = [ordered]@{
        path = $gradlePath; sha256 = (Get-FileHash -LiteralPath $gradlePath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    $source = Join-Path $repo 'mcdev-test-fixtures/src/fixture-java/com/example/target/SimpleTarget.java'
    $editor = Join-Path $repo 'mcdev-companion/tests/runtime_bytes_editor_smoke.lua'
    $startup = Join-Path $repo 'mcdev-nvim/tests/e2e/minimal_init.lua'
    $sourceFiles = @('mcdev-core/src', 'mcdev-protocol/src', 'mcdev-companion/src') |
        ForEach-Object { Get-ChildItem -LiteralPath (Join-Path $repo $_) -Recurse -File -Filter '*.rs' } |
        ForEach-Object FullName
    $inputs = @($sourceFiles) + @($source, $editor, $startup, $hostPath, $probePath, $editorPath, $gradlePath,
        $PSCommandPath, (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1'),
        (Join-Path $repo 'mcdev-companion/examples/runtime_bytes_probe.rs'),
        (Join-Path $repo 'Cargo.toml'), (Join-Path $repo 'Cargo.lock'), (Join-Path $repo 'mcdev-companion/Cargo.toml'))
    $before = @(Get-FileHash -LiteralPath $inputs -Algorithm SHA256)
    $before | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.before.json')
    Write-FixtureFile 'settings.gradle' "rootProject.name = 'mcdev-runtime-bytes-proof'`n"
    Write-FixtureFile 'build.gradle' @'
plugins { id 'java' }
java { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
tasks.withType(JavaCompile).configureEach {
    options.encoding = 'UTF-8'
    options.debug = true
    options.debugOptions.debugLevel = 'source,lines,vars'
}
'@
    Write-FixtureFile 'gradle.properties' "org.gradle.daemon=false`norg.gradle.parallel=false`norg.gradle.workers.max=2`n"
    Write-FixtureFile 'gradlew.bat' ("@echo off`ncall `"" + $gradlePath + "`" %*`nexit /b %ERRORLEVEL%`n")
    Write-FixtureFile 'src/main/java/fixture/Owner.java' "package fixture; class Owner { com.example.target.SimpleTarget target; }`n"
    $targetSource = Join-Path $project 'src/main/java/com/example/target/SimpleTarget.java'
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $targetSource)
    Copy-Item -LiteralPath $source -Destination $targetSource
    $env:MCDEV_RUNTIME_HOST = $hostPath
    $env:MCDEV_RUNTIME_PROBE = $probePath
    $env:MCDEV_RUNTIME_PROJECT = $project
    $env:MCDEV_RUNTIME_ARTIFACTS = $logs
    foreach ($name in @('XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_CACHE_HOME', 'GRADLE_USER_HOME', 'KTLSP_CACHE_DIR')) {
        Set-Item -LiteralPath "Env:$name" -Value (Join-Path $logs $name.ToLowerInvariant())
    }
    $env:NVIM_LOG_FILE = Join-Path $logs 'nvim.log'
    $env:KTLSP_TRACE = Join-Path $logs 'host.trace.jsonl'
    $env:KTLSP_JDK_SRC = Join-Path $logs 'jdk-src.zip'
    $jdk = [IO.Compression.ZipFile]::Open($env:KTLSP_JDK_SRC, [IO.Compression.ZipArchiveMode]::Create)
    try {
        $writer = [IO.StreamWriter]::new($jdk.CreateEntry('java.base/java/lang/Object.java').Open())
        try { $writer.Write('package java.lang; public class Object {}') } finally { $writer.Dispose() }
    } finally { $jdk.Dispose() }
    $receipt.sdkSourceFixture = 'reduced Object.java; no SDK or source semantic authority claimed'
    $receipt.gradle = Invoke-RustCheck -Name gradle-build -Command (Join-Path ([Environment]::SystemDirectory) 'cmd.exe') -Arguments @(
        '/d', '/c', $gradlePath, '--offline', '--no-daemon', '--max-workers=2', '--console=plain', 'classes'
    ) -Directory $project -LogDirectory $logs -TimeoutSeconds (Budget 180)
    $receipt.gradle | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'gradle-build.result.json')
    if ($receipt.gradle.ExitCode -ne 0 -or $receipt.gradle.TimedOut) { throw 'Isolated Gradle debug compilation failed' }
    $fixture = Join-Path $logs 'fixture-inputs'
    $null = New-Item -ItemType Directory -Path $fixture
    Copy-Item -LiteralPath (Join-Path $project 'src') -Destination (Join-Path $fixture 'src') -Recurse
    foreach ($file in @('settings.gradle', 'build.gradle', 'gradle.properties', 'gradlew.bat')) {
        Copy-Item -LiteralPath (Join-Path $project $file) -Destination $fixture
    }
    $classOutput = Join-Path $project 'build/classes/java/main/com/example/target/SimpleTarget.class'
    $classCopy = Join-Path $fixture 'SimpleTarget.class'
    Copy-Item -LiteralPath $classOutput -Destination $classCopy
    $receipt.editor = Invoke-RustCheck -Name editor -Command $editorPath -Arguments @(
        '--headless', '-u', $startup, '-i', 'NONE', '-l', $editor
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds (Budget 360)
    $receipt.editor | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'editor.result.json')
    $proofPath = Join-Path $logs 'editor-proof.json'
    $receipt.proof = if (Test-Path -LiteralPath $proofPath) { Get-Content -LiteralPath $proofPath -Raw | ConvertFrom-Json } else { $null }
    if ($receipt.editor.ExitCode -ne 0 -or $receipt.editor.TimedOut -or -not $receipt.proof.scopePassed) {
        throw 'Real host/test-provider Code and LVT proof failed'
    }
    $result = Get-Content -LiteralPath (Join-Path $logs 'probe-result.json') -Raw | ConvertFrom-Json
    if (-not $result.testOnly -or $result.probe -ne 'runtime-bytes-code-lvt' -or $result.error) { throw 'Missing test-only runtime proof' }
    $actual = [IO.File]::ReadAllBytes($classOutput)
    $returned = [byte[]]$result.bytes
    if ([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($returned)) { throw 'Host bytes differ from actual Gradle class output' }
    $classSha = (Get-FileHash -LiteralPath $classOutput -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($classSha -ne $result.sha256 -or $classSha -ne $result.runtime.contentSha256 -or
        $actual.Length -ne $result.byteLength -or $actual.Length -ne $result.runtime.byteLength -or
        (Get-FileHash -LiteralPath $classCopy -Algorithm SHA256).Hash.ToLowerInvariant() -ne $classSha) {
        throw 'Class output, streamed bytes or typed descriptor digest/length differs'
    }
    $receipt.byteComparison = [ordered]@{
        rawBytesEqual = $true; sha256 = $classSha; bytes = $actual.Length; classOutput = $classOutput
        preservedClass = $classCopy; runtime = $result.runtime; method = $result.method
    }
    $null = Budget 600
    $exitCode = 0
} catch {
    $receipt.error = $_.Exception.Message
    Write-Output $receipt.error
    foreach ($name in @('gradle-build', 'editor')) {
        $stderr = Join-Path $logs ($name + '.stderr.log')
        if (Test-Path -LiteralPath $stderr) { Get-Content -LiteralPath $stderr -Tail 20 }
    }
} finally {
    if ($null -ne $before) {
        $after = @(Get-FileHash -LiteralPath $inputs -Algorithm SHA256)
        $after | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.after.json')
        $receipt.inputHashesStable = @(Compare-Object $before $after -Property Path,Hash).Count -eq 0
        if (-not $receipt.inputHashesStable) { $exitCode = 1; $receipt.error = 'Proof input hashes changed during execution' }
    }
    $receipt.exitCode, $receipt.seconds = $exitCode, [Math]::Round($watch.Elapsed.TotalSeconds, 1)
    $receipt | ConvertTo-Json -Depth 100 | Set-Content -Encoding utf8 (Join-Path $logs 'receipt.json')
    if (Test-Path -LiteralPath $project) {
        $resolved = [IO.Path]::GetFullPath($project)
        if ((Split-Path -Parent $resolved) -ne $temporaryRoot -or (Split-Path -Leaf $resolved) -notlike 'mcdev-runtime-bytes-*') {
            throw "Refusing unexpected fixture cleanup path: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    $summary = "runtime-bytes-probe: exit=$exitCode testOnly=True seconds=$($receipt.seconds) deadline=600s; logs: $logs"
    $summary | Set-Content -Encoding utf8 (Join-Path $logs 'summary.txt')
    Write-Output $summary
}
exit $exitCode
