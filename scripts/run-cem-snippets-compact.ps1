#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $HostBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $HostSha256,
    [Parameter(Mandatory = $true)][string] $CompanionBinary,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string] $CompanionSha256,
    [Parameter(Mandatory = $true)][string] $NvimBinary,
    [string] $CemMixinSource = 'C:/Users/gummy/IdeaProjects/CreateEnchantableMachinery/src/main/java/io/github/cotrin8672/cem/mixin/ItemMixin.java',
    [ValidateRange(30, 600)][int] $EditorTimeoutSeconds = 240
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$logs = Join-Path $repo ('build/rust-test-logs/cem-snippets-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$project = Join-Path $temporaryRoot ('mcdev-cem-snippets-' + [guid]::NewGuid())
$exitCode = 1
$receipt = [ordered]@{
    scope = 'original CEM Inject, Implements, Descriptors, Group snippets'
    mode = 'real-host'; scopePassed = $false; fullCemWorkflowVerified = $false
    unverified = @('Item/BlockItem target members', 'Shadow fields and methods', 'Blink cache and aggregate routing')
    hostSha256 = $HostSha256.ToLowerInvariant(); companionSha256 = $CompanionSha256.ToLowerInvariant()
    deadlineSeconds = $EditorTimeoutSeconds; project = $project
    fixture = 'One isolated copy of the original CEM ItemMixin.java; no Minecraft jars, builds, or user-project writes'
}
$environment = @('MCDEV_SNIPPET_HOST', 'MCDEV_SNIPPET_COMPANION', 'MCDEV_SNIPPET_PROJECT', 'MCDEV_SNIPPET_ARTIFACTS',
    'KTLSP_CACHE_DIR', 'KTLSP_TRACE', 'KTLSP_JDK_SRC', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_CACHE_HOME', 'NVIM_LOG_FILE')
$savedEnvironment = @{}
foreach ($name in $environment) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    foreach ($path in @($HostBinary, $CompanionBinary, $NvimBinary, $CemMixinSource)) {
        if (-not [IO.Path]::IsPathFullyQualified($path)) { throw "An explicit absolute input path is required: $path" }
    }
    $hostPath = (Resolve-Path -LiteralPath $HostBinary).Path
    $companionPath = (Resolve-Path -LiteralPath $CompanionBinary).Path
    $editorPath = (Resolve-Path -LiteralPath $NvimBinary).Path
    $sourcePath = (Resolve-Path -LiteralPath $CemMixinSource).Path
    if ((Get-FileHash -LiteralPath $hostPath -Algorithm SHA256).Hash -ne $HostSha256) { throw 'Stock host checksum mismatch' }
    if ((Get-FileHash -LiteralPath $companionPath -Algorithm SHA256).Hash -ne $CompanionSha256) { throw 'Companion checksum mismatch' }
    $fixturePath = Join-Path $project 'src/main/java/io/github/cotrin8672/cem/mixin/ItemMixin.java'
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $fixturePath)
    Copy-Item -LiteralPath $sourcePath -Destination $fixturePath
    $archive = Join-Path $logs 'fixture-inputs'
    $null = New-Item -ItemType Directory -Path $archive
    Copy-Item -LiteralPath $fixturePath -Destination (Join-Path $archive 'ItemMixin.java')
    $env:MCDEV_SNIPPET_HOST = $hostPath
    $env:MCDEV_SNIPPET_COMPANION = $companionPath
    $env:MCDEV_SNIPPET_PROJECT = $project
    $env:MCDEV_SNIPPET_ARTIFACTS = $logs
    $env:KTLSP_CACHE_DIR = Join-Path $logs 'host-cache'
    $env:KTLSP_TRACE = Join-Path $logs 'host.trace.jsonl'
    $env:KTLSP_JDK_SRC = Join-Path $logs 'jdk-src.zip'
    $env:XDG_CONFIG_HOME = Join-Path $logs 'editor-config'
    $env:XDG_DATA_HOME = Join-Path $logs 'editor-data'
    $env:XDG_STATE_HOME = Join-Path $logs 'editor-state'
    $env:XDG_CACHE_HOME = Join-Path $logs 'editor-cache'
    $env:NVIM_LOG_FILE = Join-Path $logs 'nvim.log'
    $jdk = [IO.Compression.ZipFile]::Open($env:KTLSP_JDK_SRC, [IO.Compression.ZipArchiveMode]::Create)
    try {
        $entry = $jdk.CreateEntry('java.base/java/lang/Object.java')
        $writer = [IO.StreamWriter]::new($entry.Open())
        try { $writer.Write('package java.lang; public class Object {}') } finally { $writer.Dispose() }
    } finally { $jdk.Dispose() }
    $editor = Join-Path $repo 'mcdev-nvim/tests/e2e/cem_snippets_host.lua'
    $startup = Join-Path $repo 'mcdev-nvim/tests/e2e/minimal_init.lua'
    $sources = @('mcdev-core/src', 'mcdev-protocol/src', 'mcdev-companion/src', 'mcdev-nvim/lua') |
        ForEach-Object { Get-ChildItem -LiteralPath (Join-Path $repo $_) -Recurse -File } |
        Where-Object { $_.Extension -in @('.rs', '.lua', '.json') } | ForEach-Object FullName
    $inputs = @($sources) + @($hostPath, $companionPath, $editorPath, $sourcePath, $fixturePath,
        $env:KTLSP_JDK_SRC, $editor, $startup, $PSCommandPath,
        (Join-Path $repo 'mcdev-nvim/tests/test_helpers.lua'),
        (Join-Path $repo 'mcdev-nvim/tests/e2e/cem_project_e2e.lua'),
        (Join-Path $repo 'mcdev-nvim/tests/e2e/cem_feature_snippets.lua'),
        (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1'))
    $before = @(Get-FileHash -LiteralPath $inputs -Algorithm SHA256)
    $before | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.before.json')
    $receipt.nvimPath = $editorPath
    $receipt.nvimSha256 = ($before | Where-Object { $_.Path -eq $editorPath }).Hash.ToLowerInvariant()
    $receipt.cemSource = $sourcePath
    $receipt.cemSourceSha256 = ($before | Where-Object { $_.Path -eq $sourcePath }).Hash.ToLowerInvariant()
    $editorRun = Invoke-RustCheck -Name editor -Command $editorPath -Arguments @(
        '--headless', '-u', $startup, '-i', 'NONE', '-l', $editor
    ) -Directory $repo -LogDirectory $logs -TimeoutSeconds $EditorTimeoutSeconds
    $receipt.editor = $editorRun
    $editorRun | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'summary.json')
    $after = @(Get-FileHash -LiteralPath $inputs -Algorithm SHA256)
    $after | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $logs 'proof-inputs.after.json')
    $receipt.inputHashesStable = @(Compare-Object $before $after -Property Path,Hash).Count -eq 0
    $proofPath = Join-Path $logs 'editor-proof.json'
    $proof = if (Test-Path -LiteralPath $proofPath) { Get-Content -LiteralPath $proofPath -Raw | ConvertFrom-Json } else { $null }
    $receipt.proof = $proof
    if (-not $receipt.inputHashesStable) { throw 'Snippet E2E inputs or original CEM disk content changed during execution' }
    if ($editorRun.ExitCode -ne 0 -or $editorRun.TimedOut -or -not $proof -or $proof.mode -ne 'real-host' -or
        $null -ne $proof.error -or -not $proof.scopePassed -or @($proof.snippets).Count -ne 4 -or
        @($proof.frames).Count -ne 4 -or -not $proof.shutdown.clean -or -not $proof.diskUnchanged) {
        throw 'Original CEM snippet scope failed or was incomplete; see full editor proof'
    }
    $receipt.scopePassed = $true
    $exitCode = 0
    $editorRun | ConvertTo-Json -Compress
} catch {
    $receipt.error = $_.Exception.Message
    Write-Output $_.Exception.Message
    $stderr = Join-Path $logs 'editor.stderr.log'
    if (Test-Path -LiteralPath $stderr) { Get-Content -LiteralPath $stderr -Tail 24 }
} finally {
    try {
        if (Test-Path -LiteralPath $project) {
            $resolved = [IO.Path]::GetFullPath($project)
            if ((Split-Path -Parent $resolved) -ne $temporaryRoot -or
                (Split-Path -Leaf $resolved) -notlike 'mcdev-cem-snippets-*') {
                throw "Refusing unexpected fixture cleanup path: $resolved"
            }
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    } catch {
        $receipt.cleanupError = $_.Exception.Message
        $receipt.scopePassed = $false
        $exitCode = 1
    }
    foreach ($name in $environment) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
    $receipt.exitCode = $exitCode
    $receipt | ConvertTo-Json -Depth 100 | Set-Content -Encoding utf8 (Join-Path $logs 'receipt.json')
    Write-Output "logs: $logs"
}
exit $exitCode
