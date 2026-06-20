package io.github.mcdev.jdtls.project

import io.github.mcdev.core.project.ClasspathSnapshot
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.core.project.ProjectContextInput
import io.github.mcdev.core.project.ProjectIndexState
import io.github.mcdev.core.project.SourceSetContext
import io.github.mcdev.jdtls.java.JdtClasspathBridge
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.extension
import kotlin.io.path.isRegularFile
import kotlin.io.path.name
import kotlin.io.path.readText

data class CachedProjectSession(
    val session: McdevProjectSession,
    val version: Long,
    val cacheHit: Boolean,
)

class FileBasedProjectContextService {
    private data class SessionEntry(
        val session: McdevProjectSession,
        val version: Long,
    )

    private val sessionCache: MutableMap<String, SessionEntry> = mutableMapOf()
    private val versionCounters: MutableMap<String, Long> = mutableMapOf()

    fun loadSession(workspaceRootUri: String): McdevProjectSession =
        loadCachedSession(workspaceRootUri).session

    fun loadCachedSession(workspaceRootUri: String): CachedProjectSession {
        val cacheKey = workspaceRootUri.trim()
        sessionCache[cacheKey]?.let { entry ->
            return CachedProjectSession(entry.session, entry.version, cacheHit = true)
        }
        val root = UriPathSupport.uriToPath(workspaceRootUri)
        val context = buildProjectContext(root)
        val session = McdevProjectSession.create(context)
        val version = versionCounters.getOrPut(cacheKey) { 1L }
        sessionCache[cacheKey] = SessionEntry(session, version)
        return CachedProjectSession(session, version, cacheHit = false)
    }

    fun reindex(workspaceRootUri: String): McdevProjectSession {
        val cacheKey = workspaceRootUri.trim()
        val root = UriPathSupport.uriToPath(workspaceRootUri)
        val context = buildProjectContext(root)
        val session = McdevProjectSession.create(context).reindex()
        val version = (versionCounters[cacheKey] ?: 0L) + 1L
        versionCounters[cacheKey] = version
        sessionCache[cacheKey] = SessionEntry(session, version)
        return session
    }

    fun clearCache() {
        sessionCache.clear()
    }

    fun buildProjectContext(root: Path): ProjectContext {
        val gradleContents = readGradleContents(root)
        val classpath = discoverEnhancedClasspath(root)
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

    private fun discoverSourceSets(root: Path): List<SourceSetContext> =
        discoverGradleSourceSetNames(root).map { name ->
            val sourceSetRoot = root.resolve("src").resolve(name)
            val javaDir = sourceSetRoot.resolve("java")
            val resourcesDir = sourceSetRoot.resolve("resources")
            val sourceDirectories = buildList {
                if (javaDir.exists()) {
                    add(javaDir)
                }
                if (name == "main") {
                    val mappedSourcesDir = root.resolve("mapped-sources")
                    if (mappedSourcesDir.exists()) {
                        add(mappedSourcesDir)
                    }
                }
            }
            SourceSetContext(
                name = name,
                sourceDirectories = sourceDirectories,
                resourceDirectories = listOfNotNull(resourcesDir.takeIf { it.exists() }),
                outputDirectory = root.resolve("build/classes/java/$name"),
            )
        }

    private fun discoverGradleSourceSetNames(root: Path): List<String> {
        val srcRoot = root.resolve("src")
        if (!srcRoot.exists() || !Files.isDirectory(srcRoot)) {
            return emptyList()
        }
        return Files.list(srcRoot).use { stream ->
            stream
                .filter { Files.isDirectory(it) }
                .map { it.fileName.toString() }
                .filter { name ->
                    val sourceSetRoot = srcRoot.resolve(name)
                    sourceSetRoot.resolve("java").exists() || sourceSetRoot.resolve("resources").exists()
                }
                .sorted()
                .toList()
        }
    }

    private fun discoverEnhancedClasspath(root: Path): ClasspathSnapshot {
        val base = discoverClasspath(root)
        val withLoom = JdtClasspathBridge.mergeClasspath(
            base,
            JdtClasspathBridge.discoverLoomRemappedJars(root),
        )
        return JdtClasspathBridge.mergeWithJdtClasspath(
            withLoom,
            UriPathSupport.pathToUri(root),
        )
    }

    private fun discoverClasspath(root: Path): ClasspathSnapshot {
        val projectOutputs = linkedSetOf<Path>()
        val generatedOutputs = linkedSetOf<Path>()
        val dependencyJars = linkedSetOf<Path>()
        val minecraftJars = linkedSetOf<Path>()
        val timestamps = linkedMapOf<Path, Long>()

        discoverGradleSourceSetNames(root).forEach { name ->
            addClasspathDirectory(root.resolve("build/classes/java/$name"), projectOutputs, timestamps)
        }
        if (projectOutputs.isEmpty()) {
            addClasspathDirectory(root.resolve("build/classes"), projectOutputs, timestamps)
        }
        listOf(
            root.resolve("bin/main"),
            root.resolve("out/production/classes"),
            root.resolve("classpath"),
        ).forEach { path ->
            addClasspathDirectory(path, projectOutputs, timestamps)
        }
        discoverGeneratedOutputDirs(root).forEach { path ->
            addClasspathDirectory(path, generatedOutputs, timestamps)
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
            projectOutputs = projectOutputs.toList().sortedBy { it.toString() },
            dependencyJars = dependencyJars.toList().sortedBy { it.toString() },
            minecraftJars = minecraftJars.toList().sortedBy { it.toString() },
            generatedOutputs = generatedOutputs.toList().sortedBy { it.toString() },
            entryTimestamps = timestamps,
        )
    }

    private fun discoverGeneratedOutputDirs(root: Path): List<Path> {
        val generated = linkedSetOf<Path>()
        listOf(
            root.resolve("build/generated/sources"),
            root.resolve("build/generated/resources"),
        ).forEach { base ->
            if (!base.exists() || !Files.isDirectory(base)) {
                return@forEach
            }
            Files.walk(base).use { stream ->
                stream
                    .filter { Files.isDirectory(it) && it != base }
                    .forEach { dir ->
                        val hasChildDirectory = Files.list(dir).use { children ->
                            children.anyMatch(Files::isDirectory)
                        }
                        if (!hasChildDirectory) {
                            generated.add(dir)
                        }
                    }
            }
        }
        return generated.toList().sortedBy { it.toString() }
    }

    private fun addClasspathDirectory(
        path: Path,
        target: MutableSet<Path>,
        timestamps: MutableMap<Path, Long>,
    ) {
        if (path.exists() && Files.isDirectory(path)) {
            target.add(path)
            timestamps[path] = Files.getLastModifiedTime(path).toMillis()
        }
    }
}
