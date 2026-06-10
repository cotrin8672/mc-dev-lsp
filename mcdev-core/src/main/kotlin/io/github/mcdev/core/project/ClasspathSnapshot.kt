package io.github.mcdev.core.project

import java.nio.file.Path

data class ClasspathSnapshot(
    val projectOutputs: List<Path> = emptyList(),
    val dependencyJars: List<Path> = emptyList(),
    val minecraftJars: List<Path> = emptyList(),
    val generatedOutputs: List<Path> = emptyList(),
    val entryTimestamps: Map<Path, Long> = emptyMap(),
    val capturedAt: Long = System.currentTimeMillis(),
) {
    val allEntries: List<Path> =
        projectOutputs + dependencyJars + minecraftJars + generatedOutputs

    val entryCount: Int = allEntries.size

    companion object {
        val EMPTY = ClasspathSnapshot()
    }
}
