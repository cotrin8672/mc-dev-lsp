package io.github.mcdev.jdtls.support

import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.project.UriPathSupport
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.createDirectories

object JdtlsFixtureSupport {
    fun copyFixture(fixtureRoot: String, targetDir: Path) {
        val resources = FixtureResourceLoader.listResources(fixtureRoot)
        resources.forEach { resourcePath ->
            val relative = resourcePath.removePrefix("$fixtureRoot/")
            val destination = targetDir.resolve(relative)
            destination.parent?.createDirectories()
            Files.copy(
                FixtureResourceLoader.openStream(resourcePath),
                destination,
            )
        }
    }

    fun installClasspathClasses(targetDir: Path) {
        val classpathRoot = targetDir.resolve("classpath").createDirectories()
        val classResource = FixturePaths.SIMPLE_TARGET_CLASS
        val relativeClassPath = classResource.removePrefix("${FixturePaths.SHARED_CLASSES}/")
        val destination = classpathRoot.resolve(relativeClassPath)
        destination.parent?.createDirectories()
        Files.copy(
            FixtureResourceLoader.openStream(classResource),
            destination,
        )
    }

    fun workspaceUri(root: Path): String = UriPathSupport.pathToUri(root)

    fun mixinCursorPosition(source: String, needle: String): Pair<Int, Int> {
        val lines = source.lineSequence().toList()
        val lineIndex = lines.indexOfFirst { it.contains("@Mixin") && it.contains(needle) }
        require(lineIndex >= 0) { "@Mixin context with '$needle' not found in source" }
        val lineText = lines[lineIndex]
        val character = lineText.indexOf(needle) + needle.length
        require(character > 0) { "needle not found on @Mixin line: $needle" }
        return lineIndex to character
    }

    fun memberCursorPosition(source: String, memberName: String, offsetInName: Int = 0): Pair<Int, Int> {
        val index = source.indexOf(memberName)
        require(index >= 0) { "member '$memberName' not found in source" }
        val safeOffset = (index + offsetInName.coerceIn(0, memberName.length - 1))
        return offsetToPosition(source, safeOffset)
    }

    fun offsetToPosition(source: String, offset: Int): Pair<Int, Int> {
        val safeOffset = offset.coerceIn(0, source.length)
        val before = source.substring(0, safeOffset)
        val line = before.count { it == '\n' }
        val lineStart = before.lastIndexOf('\n') + 1
        val character = safeOffset - lineStart
        return line to character
    }
}
