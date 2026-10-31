#requires -Version 7.0
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1')
$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo ('build/rust-test-logs/runner-check-' + (Get-Date -Format 'yyyyMMdd-HHmmssfff'))
$null = New-Item -ItemType Directory -Force -Path $logs
$pwsh = (Get-Process -Id $PID).Path
$failure = Invoke-RustCheck 'failure' $pwsh @('-NoProfile', '-Command', '[Console]::Out.WriteLine("retained stdout"); [Console]::Error.WriteLine("retained stderr"); exit 17') $repo $logs 10
if ($failure.ExitCode -ne 17 -or $failure.TimedOut) { throw 'Nonzero exit was not preserved' }
if ((Get-Content (Join-Path $logs 'failure.stdout.log') -Raw) -notmatch 'retained stdout' -or
    (Get-Content (Join-Path $logs 'failure.stderr.log') -Raw) -notmatch 'retained stderr') { throw 'Full streams were not retained' }
$timeoutScript = Join-Path $logs 'timeout-child.ps1'
@'
param($ChildPidFile)
$info = [System.Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
foreach ($argument in @('-NoProfile', '-Command', 'Start-Sleep -Seconds 60')) { $info.ArgumentList.Add($argument) }
$child = [System.Diagnostics.Process]::Start($info)
[System.IO.File]::WriteAllText($ChildPidFile, [string]$child.Id)
Start-Sleep -Seconds 60
'@ | Set-Content -LiteralPath $timeoutScript
$childPidFile = Join-Path $logs 'child.pid'
$timeout = Invoke-RustCheck 'timeout' $pwsh @('-NoProfile', '-File', $timeoutScript, $childPidFile) $repo $logs 3
if ($timeout.ExitCode -ne 124 -or -not $timeout.TimedOut -or $timeout.Seconds -gt 10) { throw 'Timeout was not bounded' }
if (Get-Process -Id $timeout.Pid -ErrorAction SilentlyContinue) { throw 'Owned timed-out process survived' }
$childPid = [int](Get-Content -LiteralPath $childPidFile -Raw)
if (Get-Process -Id $childPid -ErrorAction SilentlyContinue) { throw 'Owned descendant survived timeout' }
foreach ($name in @('MCDEV_FACADE_CASE', 'MCDEV_TWO_OPS_TEST_PORT')) {
    $previous = [Environment]::GetEnvironmentVariable($name, 'Process')
    try {
        Set-Item -LiteralPath "Env:$name" -Value 'runner-guard-check'
        $guard = Invoke-RustCheck "guard-$name" $pwsh @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'run-rust-tests-compact.ps1'), '-SkipEditor') $repo $logs 10
        if ($guard.ExitCode -ne 1 -or $guard.TimedOut) { throw "$name did not reject an incomplete full-suite run" }
        if ((Get-Content -LiteralPath (Join-Path $logs "guard-$name.stdout.log") -Raw) -notlike "*requires $name to be absent*") {
            throw "$name did not fail at the full-suite environment gate"
        }
    } finally {
        if ($null -eq $previous) { Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue }
        else { Set-Item -LiteralPath "Env:$name" -Value $previous }
    }
}
$nativeRepo = Join-Path $logs 'native launcher repo'
$nativeScripts = Join-Path $nativeRepo 'scripts'
$null = New-Item -ItemType Directory -Force -Path $nativeScripts
$nativeLauncher = Join-Path $nativeScripts 'run-osgi-e2e.ps1'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'run-osgi-e2e.ps1') -Destination $nativeLauncher
foreach ($script in @('e2e-workspace-common.ps1', 'run-rust-tests-compact.ps1',
    'run-osgi-e2e-matrix.ps1', 'run-osgi-loom-e2e.ps1', 'run-drill-definition-diagnostics-e2e.ps1')) {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $script) -Destination (Join-Path $nativeScripts $script)
}
$fakeEditor = Join-Path $nativeRepo 'fake editor.ps1'
$fakeGradle = Join-Path $nativeRepo 'fake gradle.ps1'
$fakeHost = Join-Path $nativeRepo 'fake host.ps1'
$fakeCompanion = Join-Path $nativeRepo 'fake companion.ps1'
@'
param([Parameter(ValueFromRemainingArguments = $true)][string[]] $EditorArgs)
[ordered]@{
    host = $env:MCDEV_E2E_KTLSP; companion = $env:MCDEV_E2E_COMPANION
    editor = $env:NVIM_CMD; gradle = $env:GRADLE_CMD
    workspace = $env:MCDEV_E2E_WORKSPACE; fixture = $env:MCDEV_E2E_FIXTURE
    inputsReceipt = $env:MCDEV_E2E_INPUTS_RECEIPT
    arguments = @($EditorArgs); cwd = (Get-Location).Path
} | ConvertTo-Json -Compress | Add-Content -LiteralPath $env:MCDEV_NATIVE_LAUNCHER_PROOF
exit 0
'@ | Set-Content -LiteralPath $fakeEditor
@'
throw 'Fixture preparation must not invoke the project Gradle build'
'@ | Set-Content -LiteralPath $fakeGradle
"throw 'The launcher must not build or directly run the selected native binaries'" |
    Set-Content -LiteralPath $fakeHost
Copy-Item -LiteralPath $fakeHost -Destination $fakeCompanion
@'
param([string] $Fixture)
$null = New-Item -ItemType Directory -Force -Path (Join-Path (Split-Path -Parent $PSScriptRoot) 'build/e2e-workspace')
'@ | Set-Content -LiteralPath (Join-Path $nativeScripts 'prepare-e2e-workspace.ps1')
@'
param([switch] $RunGenSources)
$null = New-Item -ItemType Directory -Force -Path (Join-Path (Split-Path -Parent $PSScriptRoot) 'build/e2e-loom-workspace')
if (-not $RunGenSources) { throw 'RunGenSources was not forwarded' }
'@ | Set-Content -LiteralPath (Join-Path $nativeScripts 'prepare-loom-e2e-workspace.ps1')
$environmentNames = @('MCDEV_E2E_KTLSP', 'MCDEV_E2E_COMPANION', 'NVIM_CMD', 'GRADLE_CMD',
    'MCDEV_E2E_WORKSPACE', 'MCDEV_E2E_FIXTURE', 'MCDEV_NATIVE_LAUNCHER_PROOF', 'MCDEV_BUNDLE_JAR', 'JDTLS_CMD',
    'MCDEV_E2E_BLINK_RTP', 'MCDEV_E2E_BLINK_LIB_RTP', 'NVIM_APPNAME', 'MCDEV_LOOM_RUN_GEN_SOURCES',
    'MCDEV_E2E_INPUTS_RECEIPT')
$previousEnvironment = @{}
foreach ($name in $environmentNames) { $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:MCDEV_E2E_KTLSP = $fakeHost
    $env:MCDEV_E2E_COMPANION = $fakeCompanion
    $env:NVIM_CMD = $fakeEditor
    $env:GRADLE_CMD = $fakeGradle
    Remove-Item Env:MCDEV_LOOM_RUN_GEN_SOURCES -ErrorAction SilentlyContinue
    Remove-Item Env:MCDEV_E2E_INPUTS_RECEIPT -ErrorAction SilentlyContinue
    $env:MCDEV_BUNDLE_JAR = Join-Path $nativeRepo 'missing bundle.jar'
    $env:JDTLS_CMD = Join-Path $nativeRepo 'missing jdtls'
    foreach ($fixture in @('fabric-basic', 'real-cem', 'real-sodium', 'create-drill-definition-diagnostics')) {
        $caseLogs = Join-Path $logs ('native-' + $fixture)
        $null = New-Item -ItemType Directory -Force -Path $caseLogs
        $env:MCDEV_E2E_FIXTURE = $fixture
        $env:MCDEV_E2E_WORKSPACE = Join-Path $nativeRepo ('external workspace ' + $fixture)
        $null = New-Item -ItemType Directory -Force -Path $env:MCDEV_E2E_WORKSPACE
        $expectedWorkspace = if ($fixture -eq 'fabric-basic') { Join-Path $nativeRepo 'build/e2e-workspace' } else { $env:MCDEV_E2E_WORKSPACE }
        $env:MCDEV_NATIVE_LAUNCHER_PROOF = Join-Path $caseLogs 'editor.json'
        $result = Invoke-RustCheck "native-$fixture" $pwsh @('-NoProfile', '-File',
            (Join-Path $PSScriptRoot 'run-osgi-e2e-compact.ps1'), '-LauncherPath', $nativeLauncher,
            '-LogDirectory', $caseLogs, '-TimeoutSeconds', '10') $repo $logs 20
        if ($result.ExitCode -ne 0 -or $result.TimedOut) { throw "Native $fixture launcher failed; see $caseLogs" }
        $proof = Get-Content -LiteralPath $env:MCDEV_NATIVE_LAUNCHER_PROOF -Raw | ConvertFrom-Json
        if ($proof.host -ne $fakeHost -or $proof.companion -ne $fakeCompanion -or
            $proof.editor -ne $fakeEditor -or $proof.gradle -ne $fakeGradle -or
            $proof.workspace -ne $expectedWorkspace -or $proof.fixture -ne $fixture -or $proof.cwd -ne $nativeRepo) {
            throw "Native $fixture launcher did not preserve explicit inputs"
        }
        $expectedArguments = @('--headless', '-u', (Join-Path $nativeRepo 'mcdev-nvim/tests/e2e/minimal_init.lua'),
            '-c', 'luafile mcdev-nvim/tests/e2e/run_bundle_e2e.lua')
        if (($proof.arguments | ConvertTo-Json -Compress) -ne ($expectedArguments | ConvertTo-Json -Compress)) {
            throw "Native $fixture launcher changed the original editor entry point"
        }
    }
    foreach ($launcher in @('run-osgi-e2e-matrix.ps1', 'run-osgi-loom-e2e.ps1')) {
        $caseLogs = Join-Path $logs $launcher
        $null = New-Item -ItemType Directory -Force -Path $caseLogs
        $env:MCDEV_NATIVE_LAUNCHER_PROOF = Join-Path $caseLogs 'editor.jsonl'
        $arguments = @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'run-osgi-e2e-compact.ps1'),
            '-LauncherPath', (Join-Path $nativeScripts $launcher), '-LogDirectory', $caseLogs, '-TimeoutSeconds', '10')
        if ($launcher -eq 'run-osgi-loom-e2e.ps1') { $arguments += '-RunGenSources' }
        $result = Invoke-RustCheck $launcher $pwsh $arguments $repo $logs 20
        if ($result.ExitCode -ne 0 -or $result.TimedOut) { throw "Native routing failed: $launcher" }
        $proof = @(Get-Content -LiteralPath $env:MCDEV_NATIVE_LAUNCHER_PROOF | ForEach-Object { $_ | ConvertFrom-Json })
        $expectedFixtures = if ($launcher -eq 'run-osgi-e2e-matrix.ps1') {
            @('fabric-basic', 'fabric-mixinextras', 'fabric-aw-at', 'multi-source-set', 'forge-basic')
        } else { @('fabric-loom-e2e') }
        if (($proof.fixture | ConvertTo-Json -Compress) -ne ($expectedFixtures | ConvertTo-Json -Compress)) {
            throw "Native fixture routing changed: $launcher"
        }
        foreach ($entry in $proof) {
            if ($entry.host -ne $fakeHost -or $entry.companion -ne $fakeCompanion -or
                $entry.editor -ne $fakeEditor -or $entry.gradle -ne $fakeGradle) {
                throw "Explicit native binaries were lost: $launcher"
            }
        }
    }
    foreach ($launcher in @('run-osgi-e2e.ps1', 'run-osgi-e2e-matrix.ps1', 'run-osgi-loom-e2e.ps1',
        'run-drill-definition-diagnostics-e2e.ps1')) {
        $missingLogs = Join-Path $logs ('missing-host-' + $launcher)
        $null = New-Item -ItemType Directory -Force -Path $missingLogs
        Remove-Item Env:MCDEV_E2E_KTLSP -ErrorAction SilentlyContinue
        $env:MCDEV_NATIVE_LAUNCHER_PROOF = Join-Path $missingLogs 'editor.json'
        $missing = Invoke-RustCheck ('missing-host-' + $launcher) $pwsh @('-NoProfile', '-File',
            (Join-Path $PSScriptRoot 'run-osgi-e2e-compact.ps1'), '-LauncherPath', (Join-Path $nativeScripts $launcher),
            '-LogDirectory', $missingLogs, '-TimeoutSeconds', '10') $repo $logs 20
        $failureLog = Get-ChildItem -LiteralPath $missingLogs -Filter 'osgi-e2e-*.log' | Select-Object -First 1
        if ($missing.ExitCode -eq 0 -or $missing.TimedOut -or
            (Test-Path -LiteralPath $env:MCDEV_NATIVE_LAUNCHER_PROOF) -or
            (Get-Content -LiteralPath $failureLog.FullName -Raw) -notlike '*MCDEV_E2E_KTLSP must explicitly name an existing executable*') {
            throw "Missing native host must fail before launching: $launcher"
        }
    }

    # Substitute only fixed input paths in this disposable launcher copy; no MC semantics are tested here.
    $drillLauncher = Join-Path $nativeScripts 'run-drill-definition-diagnostics-e2e.ps1'
    $drillText = [IO.File]::ReadAllText($drillLauncher)
    $drillInputs = Join-Path $nativeRepo 'stub drill inputs'
    $null = New-Item -ItemType Directory -Force -Path $drillInputs
    $drillPaths = @{}
    foreach ($name in @('createJar', 'createSources', 'mixinJar', 'ponderJar', 'mixinextrasJar', 'cemSource', 'placementOffsetSource')) {
        $assignment = [regex]::Match($drillText, ('(?m)^    \$' + $name + ' = "([^"]+)"'))
        if (-not $assignment.Success) { throw "Original Drill fixed input assignment missing: $name" }
        $filename = ($assignment.Groups[1].Value.Replace('\', '/') -split '/')[-1]
        $path = Join-Path $drillInputs $filename
        [IO.File]::WriteAllText($path, ('stub launcher input: ' + $name))
        $drillPaths[$name] = $path
        $replacement = '    $' + $name + " = '" + $path.Replace("'", "''") + "'"
        $drillText = $drillText.Remove($assignment.Index, $assignment.Length).Insert($assignment.Index, $replacement)
    }
    [IO.File]::WriteAllText($drillLauncher, $drillText)
    $env:MCDEV_E2E_KTLSP = $fakeHost
    $env:MCDEV_E2E_BLINK_RTP = Join-Path $drillInputs 'blink.cmp'
    $env:MCDEV_E2E_BLINK_LIB_RTP = Join-Path $drillInputs 'blink.lib'
    $null = New-Item -ItemType Directory -Force -Path $env:MCDEV_E2E_BLINK_RTP, $env:MCDEV_E2E_BLINK_LIB_RTP
    $drillLogs = Join-Path $logs 'native-drill-model'
    $null = New-Item -ItemType Directory -Force -Path $drillLogs
    $env:MCDEV_NATIVE_LAUNCHER_PROOF = Join-Path $drillLogs 'editor.jsonl'
    $drill = Invoke-RustCheck 'native-drill-model' $pwsh @('-NoProfile', '-File',
        (Join-Path $PSScriptRoot 'run-osgi-e2e-compact.ps1'), '-LauncherPath', $drillLauncher,
        '-LogDirectory', $drillLogs, '-TimeoutSeconds', '10') $repo $logs 20
    if ($drill.ExitCode -ne 0 -or $drill.TimedOut) { throw 'Native Drill launcher model/routing failed' }
    $proof = Get-Content -LiteralPath $env:MCDEV_NATIVE_LAUNCHER_PROOF -Raw | ConvertFrom-Json
    $copies = @(Get-Content -LiteralPath (Join-Path $proof.workspace 'inputs.json') -Raw | ConvertFrom-Json)
    if ($copies.Count -ne 7 -or $proof.fixture -ne 'create-drill-definition-diagnostics' -or
        $proof.host -ne $fakeHost -or $proof.companion -ne $fakeCompanion) {
        throw 'Native Drill must preserve all seven selected inputs and native routing'
    }
    foreach ($copy in $copies) {
        if ((Get-FileHash -LiteralPath $copy.copy -Algorithm SHA256).Hash.ToLowerInvariant() -ne $copy.sha256 -or
            (Get-FileHash -LiteralPath $copy.source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $copy.sha256) {
            throw 'Drill input copies must match their original selected bytes'
        }
    }
    $drillBuild = [IO.File]::ReadAllText((Join-Path $proof.workspace 'build.gradle'))
    $expectedDependencies = @('createJar', 'mixinJar', 'ponderJar', 'mixinextrasJar') | ForEach-Object {
        "'libs/" + (Split-Path -Leaf $drillPaths[$_]) + "'"
    }
    if (-not $drillBuild.Contains('implementation files(' + ($expectedDependencies -join ', ') + ')') -or
        $drillBuild.Contains((Split-Path -Leaf $drillPaths.createSources)) -or
        -not (Test-Path -LiteralPath (Join-Path $proof.workspace ('libs/' + (Split-Path -Leaf $drillPaths.createSources))))) {
        throw 'Drill must retain binary order and attach Create sources outside the compiler classpath'
    }

    $resourceRoot = Join-Path $nativeRepo 'mcdev-test-fixtures/src/main/resources'
    $javaRoot = Join-Path $nativeRepo 'mcdev-test-fixtures/src/fixture-java/com/example/target'
    $null = New-Item -ItemType Directory -Force -Path $resourceRoot, $javaRoot
    Copy-Item -LiteralPath (Join-Path $repo 'mcdev-test-fixtures/src/main/resources/fixtures') -Destination $resourceRoot -Recurse
    Copy-Item -LiteralPath (Join-Path $repo 'mcdev-test-fixtures/src/fixture-java/com/example/target/SimpleTarget.java') -Destination $javaRoot
    foreach ($script in @('prepare-e2e-workspace.ps1', 'prepare-loom-e2e-workspace.ps1')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $script) -Destination (Join-Path $nativeScripts $script) -Force
    }
    $classSha = $null
    foreach ($fixture in @('fabric-basic', 'fabric-mixinextras', 'fabric-aw-at', 'multi-source-set', 'forge-basic', 'broken-diagnostics')) {
        $result = Invoke-RustCheck ('prepare-' + $fixture) $pwsh @('-NoProfile', '-File',
            (Join-Path $nativeScripts 'prepare-e2e-workspace.ps1'), '-Fixture', $fixture) $nativeRepo $logs 45
        if ($result.ExitCode -ne 0 -or $result.TimedOut) { throw "Original fixture preparation failed: $fixture; see $logs" }
        $workspace = Join-Path $nativeRepo 'build/e2e-workspace'
        $sourceRoot = Join-Path $resourceRoot ('fixtures/' + $fixture)
        $changed = @('build.gradle')
        if ($fixture -eq 'fabric-basic') { $changed += 'fabric.mod.json' }
        if ($fixture -eq 'fabric-mixinextras') { $changed += 'mixins.json' }
        foreach ($file in Get-ChildItem -LiteralPath $sourceRoot -Recurse -Force -File) {
            $relative = [IO.Path]::GetRelativePath($sourceRoot, $file.FullName)
            if ($relative -in $changed) { continue }
            $copy = Join-Path $workspace $relative
            if ((Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne
                (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash) {
                throw "Original fixture bytes changed: $fixture/$relative"
            }
        }
        $class = Join-Path $workspace 'classpath/com/example/target/SimpleTarget.class'
        $bytes = [IO.File]::ReadAllBytes($class)
        if ($bytes.Length -lt 8 -or $bytes[0] -ne 0xCA -or $bytes[1] -ne 0xFE -or
            $bytes[2] -ne 0xBA -or $bytes[3] -ne 0xBE -or (($bytes[6] -shl 8) + $bytes[7]) -ne 65) {
            throw "Original target must be a JDK 21 class: $fixture"
        }
        $sha = (Get-FileHash -LiteralPath $class -Algorithm SHA256).Hash
        if ($null -ne $classSha -and $sha -ne $classSha) { throw 'Fixtures must share the same exact debug target class' }
        $classSha = $sha
        foreach ($wrapper in @('gradlew', 'gradlew.bat')) {
            if (-not ([IO.File]::ReadAllText((Join-Path $workspace $wrapper))).Contains($fakeGradle)) {
                throw "Explicit Gradle executable was not written into $wrapper"
            }
        }
    }
    $javap = if ($env:JAVA_HOME_21_X64) {
        Join-Path $env:JAVA_HOME_21_X64 ('bin/javap' + $(if ($IsWindows) { '.exe' } else { '' }))
    } else { (Get-Command javap -CommandType Application | Select-Object -First 1).Source }
    $debug = Invoke-RustCheck 'fixture-javap' $javap @('-v', '-p', $class) $nativeRepo $logs 15
    $debugText = Get-Content -LiteralPath (Join-Path $logs 'fixture-javap.stdout.log') -Raw
    if ($debug.ExitCode -ne 0 -or $debugText -notmatch 'LocalVariableTable' -or
        $debugText -notmatch 'descriptor: \(Ljava/lang/String;FF\)V' -or
        $debugText -notmatch '\btext\s+Ljava/lang/String;' -or $debugText -notmatch '\bx\s+F' -or $debugText -notmatch '\by\s+F') {
        throw 'Original debug target must retain the draw descriptor and text/x/y locals'
    }
    $preparedLoom = Invoke-RustCheck 'prepare-loom' $pwsh @('-NoProfile', '-File',
        (Join-Path $nativeScripts 'prepare-loom-e2e-workspace.ps1')) $nativeRepo $logs 45
    if ($preparedLoom.ExitCode -ne 0 -or $preparedLoom.TimedOut) { throw 'Original Loom fixture preparation failed' }
    $loomWorkspace = Join-Path $nativeRepo 'build/e2e-loom-workspace'
    $loomSource = Join-Path $loomWorkspace 'mapped-sources/com/example/target/SimpleTarget.java'
    $loomArchiveRoot = Join-Path $loomWorkspace '.gradle/loom-cache/remapped_working'
    foreach ($archiveName in @('minecraft-client-mapped.jar', 'minecraft-client-mapped-sources.jar')) {
        $archive = [IO.Compression.ZipFile]::OpenRead((Join-Path $loomArchiveRoot $archiveName))
        try {
            $entryName = if ($archiveName -eq 'minecraft-client-mapped.jar') {
                'com/example/target/SimpleTarget.class'
            } else { 'com/example/target/SimpleTarget.java' }
            $entry = $archive.GetEntry($entryName)
            if ($null -eq $entry) { throw "Original Loom archive entry missing: $entryName" }
            $stream = $entry.Open()
            $memory = [IO.MemoryStream]::new()
            try { $stream.CopyTo($memory); $archiveBytes = $memory.ToArray() } finally { $stream.Dispose(); $memory.Dispose() }
            $expected = if ($archiveName -eq 'minecraft-client-mapped.jar') { [IO.File]::ReadAllBytes($class) }
                else { [IO.File]::ReadAllBytes($loomSource) }
            if ([Convert]::ToBase64String($archiveBytes) -ne [Convert]::ToBase64String($expected)) {
                throw "Original Loom archive bytes changed: $entryName"
            }
        } finally { $archive.Dispose() }
    }
    $loomBuild = [IO.File]::ReadAllText((Join-Path $loomWorkspace 'build.gradle'))
    if ($loomBuild -notmatch "implementation files\('\.gradle/loom-cache/remapped_working/minecraft-client-mapped.jar'\)" -or
        $loomBuild -match 'srcDir.*mapped-sources' -or
        (Test-Path -LiteralPath (Join-Path $loomWorkspace 'src/main/java/com/example/target/SimpleTarget.java'))) {
        throw 'Loom mapped sources must remain an attached dependency, outside mod compiler sources'
    }

    $fixedRoot = Join-Path $nativeRepo 'fixed input receipt'
    $null = New-Item -ItemType Directory -Force -Path $fixedRoot
    $artifacts = [ordered]@{}
    foreach ($name in @('mixin-0.8.7.jar', 'mixinextras-fabric-0.5.5.jar')) {
        $path = Join-Path $fixedRoot $name
        [IO.File]::WriteAllText($path, ('stub dependency bytes: ' + $name))
        $artifacts[$name] = [ordered]@{ path = $path; size = (Get-Item -LiteralPath $path).Length; readOnly = $true
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    $fixedReceipt = Join-Path $fixedRoot 'receipt.json'
    [ordered]@{ artifacts = $artifacts; failures = @() } | ConvertTo-Json -Depth 5 |
        Set-Content -LiteralPath $fixedReceipt -Encoding utf8
    $env:MCDEV_E2E_INPUTS_RECEIPT = $fixedReceipt
    foreach ($fixture in @('multi-source-set', 'fabric-mixinextras')) {
        $caseLogs = Join-Path $logs ('fixed-' + $fixture)
        $null = New-Item -ItemType Directory -Force -Path $caseLogs
        $env:MCDEV_E2E_FIXTURE = $fixture
        $env:MCDEV_NATIVE_LAUNCHER_PROOF = Join-Path $caseLogs 'editor.jsonl'
        $result = Invoke-RustCheck ('fixed-' + $fixture) $pwsh @('-NoProfile', '-File',
            (Join-Path $PSScriptRoot 'run-osgi-e2e-compact.ps1'), '-LauncherPath', $nativeLauncher,
            '-LogDirectory', $caseLogs, '-TimeoutSeconds', '15') $repo $logs 25
        if ($result.ExitCode -ne 0 -or $result.TimedOut) { throw "Fixed receipt fixture preparation failed: $fixture" }
        $proof = Get-Content -LiteralPath $env:MCDEV_NATIVE_LAUNCHER_PROOF -Raw | ConvertFrom-Json
        if ($proof.inputsReceipt -ne $fixedReceipt) { throw 'Selected fixed receipt did not reach the editor process' }
        $dependencies = Get-Content -LiteralPath (Join-Path $proof.workspace 'fixed-dependencies.json') -Raw | ConvertFrom-Json
        if ($dependencies.receipt -ne $fixedReceipt -or @($dependencies.artifacts.PSObject.Properties).Count -ne 2 -or
            $dependencies.receiptSha256 -ne (Get-FileHash -LiteralPath $fixedReceipt -Algorithm SHA256).Hash.ToLowerInvariant()) {
            throw 'Fixed fixture dependency selection/provenance changed'
        }
        foreach ($name in $artifacts.Keys) {
            $copy = $dependencies.artifacts.$name
            if ($copy.sha256 -ne $artifacts[$name].sha256 -or
                (Get-Item -LiteralPath $copy.copy).Length -ne $artifacts[$name].size -or
                (Get-FileHash -LiteralPath $copy.copy -Algorithm SHA256).Hash.ToLowerInvariant() -ne $copy.sha256) {
                throw "Fixed dependency copy changed: $name"
            }
        }
        $build = [IO.File]::ReadAllText((Join-Path $proof.workspace 'build.gradle'))
        if (-not $build.Contains('    compileOnly files("libs/mixin-0.8.7.jar")') -or
            $build -match 'org\.spongepowered:mixin:|io\.github\.llamalad7:mixinextras-fabric:|mavenCentral\(|maven\s*\{') {
            throw 'Selected fixed fixture dependencies must use local files without remote coordinates/repositories'
        }
        if ($fixture -eq 'multi-source-set' -and -not $build.Contains('clientCompileOnly files("libs/mixin-0.8.7.jar")')) {
            throw 'Selected fixed Mixin must reach the client classpath'
        }
        if ($fixture -eq 'fabric-mixinextras' -and -not $build.Contains('compileOnly files("libs/mixinextras-fabric-0.5.5.jar")')) {
            throw 'Selected fixed MixinExtras fabric 0.5.5 must reach the main classpath'
        }
    }
    foreach ($defect in @('hash', 'size')) {
        $bad = Get-Content -LiteralPath $fixedReceipt -Raw | ConvertFrom-Json
        if ($defect -eq 'hash') { $bad.artifacts.'mixin-0.8.7.jar'.sha256 = '0' * 64 }
        else { $bad.artifacts.'mixin-0.8.7.jar'.size++ }
        $env:MCDEV_E2E_INPUTS_RECEIPT = Join-Path $fixedRoot ('bad-' + $defect + '.json')
        $bad | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $env:MCDEV_E2E_INPUTS_RECEIPT -Encoding utf8
        $name = 'fixed-bad-' + $defect
        $result = Invoke-RustCheck $name $pwsh @('-NoProfile', '-File',
            (Join-Path $nativeScripts 'prepare-e2e-workspace.ps1'), '-Fixture', 'multi-source-set') $nativeRepo $logs 15
        if ($result.ExitCode -eq 0 -or $result.TimedOut -or
            (Get-Content -LiteralPath (Join-Path $logs ($name + '.stderr.log')) -Raw) -notlike '*checksum/size mismatch*') {
            throw "Selected fixed receipt must reject a broken $defect without fallback"
        }
    }
    Remove-Item Env:MCDEV_E2E_INPUTS_RECEIPT
    . (Join-Path $nativeScripts 'e2e-workspace-common.ps1')
    $escaped = $false
    try { $null = Assert-E2eBuildPath (Join-Path $nativeRepo 'outside-build') } catch { $escaped = $true }
    if (-not $escaped) { throw 'E2E mutation outside repo/build was not rejected' }
} finally {
    foreach ($name in $environmentNames) {
        if ($null -eq $previousEnvironment[$name]) { Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue }
        else { Set-Item -LiteralPath "Env:$name" -Value $previousEnvironment[$name] }
    }
}
Write-Output "runner checks: PASS (failure, retained streams, timeout, owned tree exit, suite gate, native launcher routes/inputs, Drill model/copy ordering, exact original fixtures, JDK21 debug class, attached Loom sources, pinned main/client dependencies, rejected bad hashes/sizes, build path guard); logs: $logs"
