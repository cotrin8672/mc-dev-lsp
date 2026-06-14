package io.github.mcdev.core.project

import io.github.mcdev.core.mapping.ProjectMappingContext
import java.nio.file.Path

data class ProjectContext(
    val projectId: String,
    val root: Path,
    val platform: ModPlatform,
    val classpath: ClasspathSnapshot,
    val mappings: ProjectMappingContext,
    val mixinConfigs: List<MixinConfigRef>,
    val accessWideners: List<AccessWidenerRef>,
    val accessTransformers: List<AccessTransformerRef>,
    val minecraftJars: List<Path>,
    val sourceSets: List<SourceSetContext>,
    val indexState: ProjectIndexState,
    val mappingFiles: List<Path> = emptyList(),
)
