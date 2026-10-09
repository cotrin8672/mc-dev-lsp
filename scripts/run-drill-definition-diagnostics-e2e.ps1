$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $runId = "drill-definition-diagnostics-{0}-{1}" -f (Get-Date).ToString("yyyyMMdd-HHmmssfff"), $PID
    $workspace = Join-Path $repoRoot (Join-Path "build" $runId)
    New-Item -ItemType Directory -Force -Path $workspace | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $workspace "src/main/java/io/github/cotrin8672/cem/mixin") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $workspace "bin") | Out-Null

    $createJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\com.simibubi.create\create-1.21.1\6.0.10-280\62c085fd48fc84b62f88d7bc98b7c4206492c080\create-1.21.1-6.0.10-280.jar"
    $createSources = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\com.simibubi.create\create-1.21.1\6.0.10-280\23e1219501c0debfa0bb56c30ef8e0193341aae5\create-1.21.1-6.0.10-280-sources.jar"
    $mixinJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\org.spongepowered\mixin\0.8.7\8ab114ac385e6dbdad5efafe28aba4df8120915f\mixin-0.8.7.jar"
    $ponderJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\net.createmod.ponder\ponder-neoforge\1.0.87+mc1.21.1\7783eeff0389f8c6003457133b9dd98e903266c6\ponder-neoforge-1.0.87+mc1.21.1.jar"
    $mixinextrasJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\io.github.llamalad7\mixinextras-fabric\0.4.1\8d1a9e96afb990367fa1f904d17580d164da72e3\mixinextras-fabric-0.4.1.jar"
    $cemSource = "C:\Users\gummy\IdeaProjects\CreateEnchantableMachinery\src\main\java\io\github\cotrin8672\cem\mixin\DrillBlockMixin.java"
    $placementOffsetSource = "C:\Users\gummy\IdeaProjects\CreateEnchantableMachinery\src\main\java\io\github\cotrin8672\cem\mixin\PlacementOffsetMixin.java"
    foreach ($path in @($createJar, $createSources, $mixinJar, $ponderJar, $mixinextrasJar, $cemSource, $placementOffsetSource)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "required E2E input not found: $path" }
    }

    $bundleJar = $env:MCDEV_BUNDLE_JAR
    if (-not $bundleJar) {
        $bundleJar = Get-ChildItem -LiteralPath (Join-Path $repoRoot "mcdev-jdtls-extension/build/libs") `
            -Filter "io.github.mcdev.jdtls-*.jar" |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $bundleJar -or -not (Test-Path -LiteralPath $bundleJar -PathType Leaf)) {
        throw "MCDEV bundle jar not found"
    }
    $bundleSnapshot = Join-Path $workspace "mcdev-bundle-snapshot.jar"
    Copy-Item -LiteralPath $bundleJar -Destination $bundleSnapshot

    $mixinDestination = Join-Path $workspace "src/main/java/io/github/cotrin8672/cem/mixin/DrillBlockMixin.java"
    Copy-Item -LiteralPath $cemSource -Destination $mixinDestination
    (Get-Item -LiteralPath $mixinDestination).IsReadOnly = $true
    $placementOffsetDestination = Join-Path $workspace "src/main/java/io/github/cotrin8672/cem/mixin/PlacementOffsetMixin.java"
    Copy-Item -LiteralPath $placementOffsetSource -Destination $placementOffsetDestination
    (Get-Item -LiteralPath $placementOffsetDestination).IsReadOnly = $true

    $project = @"
<?xml version="1.0" encoding="UTF-8"?>
<projectDescription>
  <name>mcdev-drill-definition-diagnostics</name>
  <comment></comment>
  <projects></projects>
  <buildSpec>
    <buildCommand>
      <name>org.eclipse.jdt.core.javabuilder</name>
      <arguments></arguments>
    </buildCommand>
  </buildSpec>
  <natures>
    <nature>org.eclipse.jdt.core.javanature</nature>
  </natures>
</projectDescription>
"@
    $classpath = @"
<?xml version="1.0" encoding="UTF-8"?>
<classpath>
  <classpathentry kind="src" path="src/main/java"/>
  <classpathentry kind="lib" path="$createJar" sourcepath="$createSources"/>
  <classpathentry kind="lib" path="$mixinJar"/>
  <classpathentry kind="lib" path="$ponderJar"/>
  <classpathentry kind="lib" path="$mixinextrasJar"/>
  <classpathentry kind="con" path="org.eclipse.jdt.launching.JRE_CONTAINER/org.eclipse.jdt.internal.debug.ui.launcher.StandardVMType/JavaSE-21"/>
  <classpathentry kind="output" path="bin"/>
</classpath>
"@
    [IO.File]::WriteAllText((Join-Path $workspace ".project"), $project, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $workspace ".classpath"), $classpath, [Text.UTF8Encoding]::new($false))

    $jdtlsCmd = $env:JDTLS_CMD
    if (-not $jdtlsCmd) {
        $jdtlsCmd = Join-Path $env:LOCALAPPDATA "nvim-data/mason/bin/jdtls.cmd"
    }
    if (-not (Test-Path -LiteralPath $jdtlsCmd -PathType Leaf) -and $jdtlsCmd -notmatch '^[^\\/]+$') {
        throw "installed JDTLS command not found: $jdtlsCmd"
    }

    $nvimCmd = $env:NVIM_CMD
    if (-not $nvimCmd) {
        $nvimCmd = "nvim"
    }
    $blinkRtp = $env:MCDEV_E2E_BLINK_RTP
    if (-not $blinkRtp) {
        $blinkRtp = Join-Path $env:LOCALAPPDATA "nvim-data/lazy/blink.cmp"
    }
    if (-not (Test-Path -LiteralPath $blinkRtp -PathType Container)) {
        throw "installed blink.cmp checkout not found: $blinkRtp"
    }
    $blinkLibRtp = $env:MCDEV_E2E_BLINK_LIB_RTP
    if (-not $blinkLibRtp) {
        $blinkLibRtp = Join-Path (Split-Path -Parent $blinkRtp) "blink.lib"
    }
    if (-not (Test-Path -LiteralPath $blinkLibRtp -PathType Container)) {
        throw "installed blink.lib checkout not found: $blinkLibRtp"
    }
    $env:MCDEV_BUNDLE_JAR = $bundleSnapshot
    $env:MCDEV_E2E_WORKSPACE = $workspace
    $env:MCDEV_E2E_FIXTURE = "create-drill-definition-diagnostics"
    $env:JDTLS_CMD = $jdtlsCmd
    $env:MCDEV_E2E_BLINK_RTP = $blinkRtp
    $env:MCDEV_E2E_BLINK_LIB_RTP = $blinkLibRtp
    $env:NVIM_APPNAME = "mcdev-$runId"

    & $nvimCmd --headless `
        -u (Join-Path $repoRoot "mcdev-nvim/tests/e2e/minimal_init.lua") `
        -c "luafile mcdev-nvim/tests/e2e/run_bundle_e2e.lua"
    if ($LASTEXITCODE -ne 0) { throw "drill definition/diagnostics E2E failed" }
} finally {
    Pop-Location
}
