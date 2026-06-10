package io.github.mcdev.core.project

import java.nio.file.Path

data class SourceSetContext(
    val name: String,
    val sourceDirectories: List<Path> = emptyList(),
    val resourceDirectories: List<Path> = emptyList(),
    val outputDirectory: Path? = null,
)
