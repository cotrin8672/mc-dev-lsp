$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $env:MCDEV_E2E_KTLSP = Resolve-E2eExecutable 'MCDEV_E2E_KTLSP'
    $env:MCDEV_E2E_COMPANION = Resolve-E2eExecutable 'MCDEV_E2E_COMPANION'
    $env:NVIM_CMD = Resolve-E2eExecutable 'NVIM_CMD'
    $env:GRADLE_CMD = Resolve-E2eExecutable 'GRADLE_CMD'
    $runId = "drill-definition-diagnostics-{0}-{1}" -f (Get-Date).ToString("yyyyMMdd-HHmmssfff"), $PID
    $workspace = Assert-E2eBuildPath (Join-Path $repoRoot (Join-Path 'build' $runId))
    $createJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\com.simibubi.create\create-1.21.1\6.0.10-280\62c085fd48fc84b62f88d7bc98b7c4206492c080\create-1.21.1-6.0.10-280.jar"
    $createSources = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\com.simibubi.create\create-1.21.1\6.0.10-280\23e1219501c0debfa0bb56c30ef8e0193341aae5\create-1.21.1-6.0.10-280-sources.jar"
    $mixinJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\org.spongepowered\mixin\0.8.7\8ab114ac385e6dbdad5efafe28aba4df8120915f\mixin-0.8.7.jar"
    $ponderJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\net.createmod.ponder\ponder-neoforge\1.0.87+mc1.21.1\7783eeff0389f8c6003457133b9dd98e903266c6\ponder-neoforge-1.0.87+mc1.21.1.jar"
    $mixinextrasJar = "C:\Users\gummy\.gradle\caches\modules-2\files-2.1\io.github.llamalad7\mixinextras-fabric\0.4.1\8d1a9e96afb990367fa1f904d17580d164da72e3\mixinextras-fabric-0.4.1.jar"
    $cemSource = "C:\Users\gummy\IdeaProjects\CreateEnchantableMachinery\src\main\java\io\github\cotrin8672\cem\mixin\DrillBlockMixin.java"
    $placementOffsetSource = "C:\Users\gummy\IdeaProjects\CreateEnchantableMachinery\src\main\java\io\github\cotrin8672\cem\mixin\PlacementOffsetMixin.java"
    $inputs = @($createJar, $createSources, $mixinJar, $ponderJar, $mixinextrasJar, $cemSource, $placementOffsetSource)
    $missing = @($inputs | Where-Object { -not (Test-Path -LiteralPath $_ -PathType Leaf) })
    if ($missing.Count -gt 0) { throw ("required original E2E inputs not found: " + ($missing -join ', ')) }

    $null = New-Item -ItemType Directory -Force -Path (Join-Path $workspace 'src/main/java/io/github/cotrin8672/cem/mixin')
    $null = New-Item -ItemType Directory -Force -Path (Join-Path $workspace 'libs')
    $mixinDestination = Join-Path $workspace 'src/main/java/io/github/cotrin8672/cem/mixin/DrillBlockMixin.java'
    Copy-Item -LiteralPath $cemSource -Destination $mixinDestination
    (Get-Item -LiteralPath $mixinDestination).IsReadOnly = $true
    $placementOffsetDestination = Join-Path $workspace 'src/main/java/io/github/cotrin8672/cem/mixin/PlacementOffsetMixin.java'
    Copy-Item -LiteralPath $placementOffsetSource -Destination $placementOffsetDestination
    (Get-Item -LiteralPath $placementOffsetDestination).IsReadOnly = $true

    $inputProof = foreach ($path in $inputs) {
        $destination = if ($path -eq $cemSource) { $mixinDestination }
            elseif ($path -eq $placementOffsetSource) { $placementOffsetDestination }
            else { Join-Path $workspace ('libs/' + (Split-Path -Leaf $path)) }
        if ($path -ne $cemSource -and $path -ne $placementOffsetSource) {
            Copy-Item -LiteralPath $path -Destination $destination
        }
        $sha = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $sha) {
            throw "Original E2E input changed while copying: $path"
        }
        [ordered]@{ source = $path; copy = $destination; sha256 = $sha }
    }
    $inputProof | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $workspace 'inputs.json') -Encoding utf8
    $dependencies = @($createJar, $mixinJar, $ponderJar, $mixinextrasJar) | ForEach-Object {
        "'libs/" + (Split-Path -Leaf $_) + "'"
    }
    $build = @"
plugins { id 'java' }
java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}
tasks.withType(JavaCompile).configureEach {
    options.encoding = 'UTF-8'
    options.compilerArgs = ['-proc:none']
}
dependencies { implementation files($($dependencies -join ', ')) }
"@
    [IO.File]::WriteAllText((Join-Path $workspace 'build.gradle'), $build, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $workspace 'settings.gradle'),
        "rootProject.name = 'mcdev-drill-definition-diagnostics'`n", [Text.UTF8Encoding]::new($false))
    Write-E2eGradleWrapper $workspace

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
    $env:MCDEV_E2E_WORKSPACE = $workspace
    $env:MCDEV_E2E_FIXTURE = 'create-drill-definition-diagnostics'
    $env:MCDEV_E2E_BLINK_RTP = $blinkRtp
    $env:MCDEV_E2E_BLINK_LIB_RTP = $blinkLibRtp
    $env:NVIM_APPNAME = "mcdev-$runId"
    & (Join-Path $PSScriptRoot 'run-osgi-e2e.ps1')
} finally { Pop-Location }
