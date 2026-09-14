package io.github.mcdev.core.project

import java.nio.file.FileVisitResult
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.SimpleFileVisitor
import java.nio.file.attribute.BasicFileAttributes
import kotlin.io.path.exists

internal object ProjectTreeWalker {
    fun walkRegularFiles(root: Path): List<Path> {
        if (!root.exists()) {
            return emptyList()
        }
        val found = mutableListOf<Path>()
        Files.walkFileTree(
            root,
            object : SimpleFileVisitor<Path>() {
                override fun preVisitDirectory(
                    dir: Path,
                    attrs: BasicFileAttributes,
                ): FileVisitResult {
                    if (dir != root && ProjectPathFilters.isExcludedDirectorySegment(dir.fileName.toString())) {
                        return FileVisitResult.SKIP_SUBTREE
                    }
                    return FileVisitResult.CONTINUE
                }

                override fun visitFile(
                    file: Path,
                    attrs: BasicFileAttributes,
                ): FileVisitResult {
                    if (attrs.isRegularFile) {
                        found.add(file)
                    }
                    return FileVisitResult.CONTINUE
                }
            },
        )
        return found.sorted()
    }

    fun walkMappingDiscoveryRegularFiles(root: Path): List<Path> {
        if (!root.exists()) {
            return emptyList()
        }
        val normalizedRoot = root.toAbsolutePath().normalize()
        val found = mutableListOf<Path>()
        Files.walkFileTree(
            normalizedRoot,
            object : SimpleFileVisitor<Path>() {
                override fun preVisitDirectory(
                    dir: Path,
                    attrs: BasicFileAttributes,
                ): FileVisitResult {
                    if (dir != normalizedRoot) {
                        val relativePath = normalizedRoot.relativize(dir)
                        if (ProjectPathFilters.shouldPruneMappingDiscoveryDirectory(relativePath)) {
                            return FileVisitResult.SKIP_SUBTREE
                        }
                    }
                    return FileVisitResult.CONTINUE
                }

                override fun visitFile(
                    file: Path,
                    attrs: BasicFileAttributes,
                ): FileVisitResult {
                    if (attrs.isRegularFile) {
                        found.add(file)
                    }
                    return FileVisitResult.CONTINUE
                }
            },
        )
        return found.sorted()
    }
}
