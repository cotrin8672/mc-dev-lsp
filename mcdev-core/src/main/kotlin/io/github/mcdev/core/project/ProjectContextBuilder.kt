package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.ProjectMappingContext
import java.nio.file.Path

data class ProjectContextInput(
    val projectId: String,
    val root: Path,
    val platform: ModPlatform? = null,
    val gradleContents: List<String> = emptyList(),
    val classpath: ClasspathSnapshot = ClasspathSnapshot.EMPTY,
    val mappings: ProjectMappingContext? = null,
    val mixinConfigs: List<MixinConfigRef>? = null,
    val accessWideners: List<AccessWidenerRef>? = null,
    val accessTransformers: List<AccessTransformerRef>? = null,
    val minecraftJars: List<Path>? = null,
    val sourceSets: List<SourceSetContext> = emptyList(),
    val indexState: ProjectIndexState = ProjectIndexState.NOT_READY,
)

object ProjectContextBuilder {
    fun build(input: ProjectContextInput): ProjectContext {
        val platform = input.platform
            ?: if (input.gradleContents.isNotEmpty()) {
                PlatformDetector.detect(input.gradleContents)
            } else {
                PlatformDetector.detectFromRoot(input.root)
            }

        val discoveredMappingFiles = if (input.mappings == null) {
            MappingDiscoveryService.discoverMappingFiles(input.root)
        } else {
            emptyList()
        }
        val mappings = input.mappings ?: MappingDiscoveryService.buildMappingContext(discoveredMappingFiles, platform)
        val mixinConfigs = input.mixinConfigs ?: MixinConfigDiscoveryService.discover(input.root)
        val accessWideners = input.accessWideners ?: AwAtDiscoveryService.discoverAccessWideners(input.root)
        val accessTransformers = input.accessTransformers ?: AwAtDiscoveryService.discoverAccessTransformers(input.root)
        val minecraftJars = input.minecraftJars ?: input.classpath.minecraftJars

        return ProjectContext(
            projectId = input.projectId,
            root = input.root,
            platform = platform,
            classpath = input.classpath,
            mappings = mappings,
            mixinConfigs = mixinConfigs,
            accessWideners = accessWideners,
            accessTransformers = accessTransformers,
            minecraftJars = minecraftJars,
            sourceSets = input.sourceSets,
            indexState = input.indexState,
            mappingFiles = discoveredMappingFiles,
        )
    }

    fun empty(projectId: String, root: Path): ProjectContext =
        build(
            ProjectContextInput(
                projectId = projectId,
                root = root,
                platform = ModPlatform.UNKNOWN,
                mappings = MappingDiscoveryService.buildMappingContext(emptyList(), ModPlatform.UNKNOWN),
                mixinConfigs = emptyList(),
                accessWideners = emptyList(),
                accessTransformers = emptyList(),
                minecraftJars = emptyList(),
            ),
        )
}
