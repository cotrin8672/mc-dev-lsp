package io.github.mcdev.core.project

import java.nio.file.Path

object SourceSetResolver {
    fun containingSourceSet(sourceSets: List<SourceSetContext>, documentPath: Path): SourceSetContext? {
        val normalizedDocument = documentPath.toAbsolutePath().normalize()
        val matches = sourceSets.flatMap { sourceSet ->
            sourceSet.sourceDirectories.mapNotNull { sourceDirectory ->
                val normalizedSource = sourceDirectory.toAbsolutePath().normalize()
                if (!isUnderSourceDirectory(normalizedDocument, normalizedSource)) {
                    null
                } else {
                    Match(sourceSet, normalizedSource.nameCount)
                }
            }
        }
        if (matches.isEmpty()) {
            return null
        }

        val bestDepth = matches.maxOf { it.depth }
        return matches
            .filter { it.depth == bestDepth }
            .map { it.sourceSet }
            .distinct()
            .singleOrNull()
    }

    private data class Match(val sourceSet: SourceSetContext, val depth: Int)

    private fun isUnderSourceDirectory(document: Path, sourceDirectory: Path): Boolean {
        if (document == sourceDirectory) {
            return true
        }
        if (document.nameCount <= sourceDirectory.nameCount) {
            return false
        }
        return document.startsWith(sourceDirectory)
    }
}
