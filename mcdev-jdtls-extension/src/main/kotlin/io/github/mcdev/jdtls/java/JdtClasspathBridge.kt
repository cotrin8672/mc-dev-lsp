package io.github.mcdev.jdtls.java

import io.github.mcdev.core.project.ClasspathSnapshot
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.extension
import kotlin.io.path.isRegularFile

object JdtClasspathBridge {
    fun mergeWithJdtClasspath(
        base: ClasspathSnapshot,
        workspaceRootUri: String,
    ): ClasspathSnapshot {
        val jdtEntries = JdtReflectionBridge.instance?.discoverClasspathEntries(workspaceRootUri).orEmpty()
        if (jdtEntries.isEmpty()) return base
        return mergeClasspath(base, jdtEntries)
    }

    fun discoverLoomRemappedJars(root: Path): List<Path> {
        val jars = linkedSetOf<Path>()
        val loomDirs = listOf(
            root.resolve(".gradle/loom-cache/remapped_working"),
            root.resolve("build/loom-cache/remapped_working"),
        )
        loomDirs.forEach { dir ->
            if (!dir.toFile().isDirectory) return@forEach
            Files.walk(dir).use { stream ->
                stream
                    .filter { it.isRegularFile() && it.extension.equals("jar", ignoreCase = true) }
                    .forEach { jars.add(it) }
            }
        }
        return jars.toList()
    }

    fun mergeClasspath(base: ClasspathSnapshot, extraEntries: List<Path>): ClasspathSnapshot {
        if (extraEntries.isEmpty()) return base
        val projectOutputs = base.projectOutputs.toMutableList()
        val dependencyJars = base.dependencyJars.toMutableList()
        val minecraftJars = base.minecraftJars.toMutableList()
        val timestamps = base.entryTimestamps.toMutableMap()
        extraEntries.forEach { path ->
            if (!path.toFile().exists()) return@forEach
            val name = path.fileName.toString().lowercase()
            val targetList = when {
                name.contains("minecraft") || (name.contains("client") && name.contains("mapped")) -> minecraftJars
                path.toString().contains("classes") -> projectOutputs
                else -> dependencyJars
            }
            if (path !in targetList) {
                targetList.add(path)
            }
            timestamps.putIfAbsent(path, Files.getLastModifiedTime(path).toMillis())
        }
        return ClasspathSnapshot(
            projectOutputs = projectOutputs,
            dependencyJars = dependencyJars,
            minecraftJars = minecraftJars,
            generatedOutputs = base.generatedOutputs,
            entryTimestamps = timestamps,
        )
    }
}
