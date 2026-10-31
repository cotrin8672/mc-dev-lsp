param([switch] $RunGenSources)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'e2e-workspace-common.ps1')
$repoRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Assert-E2eBuildPath (Join-Path $repoRoot 'build/e2e-loom-workspace')
$gradleCmd = Resolve-E2eExecutable 'GRADLE_CMD'
if (Test-Path -LiteralPath $workspaceRoot) { Remove-Item -LiteralPath $workspaceRoot -Recurse -Force }
$null = New-Item -ItemType Directory -Force -Path $workspaceRoot
Copy-FixtureTree 'fixtures/fabric-loom-e2e' $workspaceRoot

$staging = Assert-E2eBuildPath (Join-Path $workspaceRoot 'classpath-staging')
Copy-FixtureTargetClass $staging
$loomJarDir = Join-Path $workspaceRoot '.gradle/loom-cache/remapped_working'
$null = New-Item -ItemType Directory -Force -Path $loomJarDir
$loomJar = Join-Path $loomJarDir 'minecraft-client-mapped.jar'
[IO.Compression.ZipFile]::CreateFromDirectory($staging, $loomJar)
# Keep attached dependency sources separate from the mod compiler's source roots.
[IO.Compression.ZipFile]::CreateFromDirectory((Join-Path $workspaceRoot 'mapped-sources'),
    (Join-Path $loomJarDir 'minecraft-client-mapped-sources.jar'))
$staging = Assert-E2eBuildPath $staging
Remove-Item -LiteralPath $staging -Recurse -Force

$liveGenSources = $RunGenSources -or $env:MCDEV_LOOM_RUN_GEN_SOURCES -eq '1'
if (-not $liveGenSources) {
    $build = @'
plugins { id 'java'; id 'maven-publish' }
// fabric-loom marker preserves the original simulated Loom fixture.
java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}
tasks.withType(JavaCompile).configureEach {
    options.encoding = 'UTF-8'
    options.compilerArgs = ['-proc:none']
}
repositories {
    mavenCentral()
    maven { url 'https://repo.spongepowered.org/repository/maven-public/' }
}
dependencies {
    implementation files('.gradle/loom-cache/remapped_working/minecraft-client-mapped.jar')
    compileOnly 'org.spongepowered:mixin:0.8.7'
}
'@
    [IO.File]::WriteAllText((Join-Path $workspaceRoot 'build.gradle'), $build, [Text.UTF8Encoding]::new($false))
}
[IO.File]::WriteAllText((Join-Path $workspaceRoot 'settings.gradle'),
    "rootProject.name = 'mcdev-loom-e2e-workspace'`n", [Text.UTF8Encoding]::new($false))
Write-E2eGradleWrapper $workspaceRoot

if ($liveGenSources) {
    Write-Host 'Attempting the original live Loom genSources task (requires network)'
    Push-Location $workspaceRoot
    try {
        & $gradleCmd --no-daemon genSources
        if ($LASTEXITCODE -ne 0) { throw 'Gradle genSources failed' }
    } finally { Pop-Location }
}
Write-Host "loom e2e workspace prepared at $workspaceRoot"
