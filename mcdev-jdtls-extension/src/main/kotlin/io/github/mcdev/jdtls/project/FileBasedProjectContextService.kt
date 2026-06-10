package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ClasspathSnapshot
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.project.ProjectContextInput
import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.core.project.SourceSetContext
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.extension
import kotlin.io.path.isRegularFile
import kotlin.io.path.name
import kotlin.io.path.readText

class FileBasedProjectContextService(
    private val sessionCache: MutableMap<String, McdevProjectSession> = mutableMapOf(),
) {
    fun loadSession(workspaceRootUri: String): McdevProjectSession {
        val cacheKey = workspaceRootUri.trim()
        return sessionCache.getOrPut(cacheKey) {
            val root = UriPathSupport.uriToPath(workspaceRootUri)
            val context = buildProjectContext(root)
            McdevProjectSession.create(context)
        }
    }

    fun reindex(workspaceRootUri: String): McdevProjectSession {
        val cacheKey = workspaceRootUri.trim()
        val root = UriPathSupport.uriToPath(workspaceRootUri)
        val context = buildProjectContext(root)
        val session = McdevProjectSession.create(context).reindex()
        sessionCache[cacheKey] = session
        return session
    }

    fun clearCache() {
        sessionCache.clear()
    }

    fun buildProjectContext(root: Path): ProjectContext {
        val gradleContents = readGradleContents(root)
        val classpath = discoverClasspath(root)
        val sourceSets = discoverSourceSets(root)
        val indexState = if (classpath.entryCount > 0) ProjectIndexState.READY else ProjectIndexState.NOT_READY
        return ProjectContextBuilder.build(
            ProjectContextInput(
                projectId = root.fileName?.toString() ?: root.toString(),
                root = root,
                gradleContents = gradleContents,
                classpath = classpath,
                sourceSets = sourceSets,
                indexState = indexState,
            ),
        )
    }

    private fun readGradleContents(root: Path): List<String> {
        val files = listOf(
            root.resolve("build.gradle.kts"),
            root.resolve("build.gradle"),
            root.resolve("settings.gradle.kts"),
            root.resolve("settings.gradle"),
        )
        return files.filter { it.isRegularFile() }.map { it.readText() }
    }

    private fun discoverSourceSets(root: Path): List<SourceSetContext> {
        val javaDir = root.resolve("src/main/java")
        val resourcesDir = root.resolve("src/main/resources")
        if (!javaDir.exists() && !resourcesDir.exists()) return emptyList()
        return listOf(
            SourceSetContext(
                name = "main",
                sourceDirectories = listOfNotNull(javaDir.takeIf { it.exists() }),
                resourceDirectories = listOfNotNull(resourcesDir.takeIf { it.exists() }),
                outputDirectory = root.resolve("build/classes/java/main"),
            ),
        )
    }

    private fun discoverClasspath(root: Path): ClasspathSnapshot {
        val projectOutputs = linkedSetOf<Path>()
        val dependencyJars = linkedSetOf<Path>()
        val minecraftJars = linkedSetOf<Path>()
        val timestamps = linkedMapOf<Path, Long>()

        val outputCandidates = listOf(
            root.resolve("build/classes/java/main"),
            root.resolve("build/classes"),
            root.resolve("bin/main"),
            root.resolve("out/production/classes"),
            root.resolve("classpath"),
        )
        outputCandidates.forEach { path ->
            if (path.exists() && Files.isDirectory(path)) {
                projectOutputs.add(path)
                timestamps[path] = Files.getLastModifiedTime(path).toMillis()
            }
        }

        val jarRoots = listOf(
            root.resolve("libs"),
            root.resolve("build/libs"),
            root.resolve("run/mods"),
        )
        jarRoots.forEach { jarRoot ->
            if (!jarRoot.exists()) return@forEach
            Files.walk(jarRoot).use { stream ->
                stream
                    .filter { it.isRegularFile() && it.extension.equals("jar", ignoreCase = true) }
                    .forEach { jar ->
                        val name = jar.name.lowercase()
                        if (name.contains("minecraft") || name.contains("client") && name.contains("mapped")) {
                            minecraftJars.add(jar)
                        } else {
                            dependencyJars.add(jar)
                        }
                        timestamps[jar] = Files.getLastModifiedTime(jar).toMillis()
                    }
            }
        }

        return ClasspathSnapshot(
            projectOutputs = projectOutputs.toList(),
            dependencyJars = dependencyJars.toList(),
            minecraftJars = minecraftJars.toList(),
            entryTimestamps = timestamps,
        )
    }
}
