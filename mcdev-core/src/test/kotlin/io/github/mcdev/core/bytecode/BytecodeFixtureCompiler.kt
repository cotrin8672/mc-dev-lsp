package io.github.mcdev.core.bytecode

import java.nio.file.Files
import java.nio.file.Path
import javax.tools.ToolProvider

object BytecodeFixtureCompiler {
    private const val FIXTURE_INTERNAL_PREFIX = "io/github/mcdev/core/bytecode/fixtures"

    private var compiledClasses: Map<String, ByteArray>? = null

    fun compileFixtures(): Map<String, ByteArray> {
        compiledClasses?.let { return it }

        val compiler = ToolProvider.getSystemJavaCompiler()
            ?: error("Java compiler not available; cannot compile bytecode fixtures")

        val sourceDir = locateFixtureSourceDir()
        val outputDir = Files.createTempDirectory("mcdev-bytecode-fixtures")
        val sourceFiles = Files.list(sourceDir)
            .filter { it.toString().endsWith(".java") }
            .toList()

        val diagnostics = mutableListOf<String>()
        val fileManager = compiler.getStandardFileManager(null, null, null)
        val compilationUnits = fileManager.getJavaFileObjectsFromFiles(sourceFiles.map { it.toFile() })
        val task = compiler.getTask(
            null,
            fileManager,
            { diagnostic ->
                diagnostics += diagnostic.toString()
                true
            },
            listOf("-d", outputDir.toString(), "--release", "21"),
            null,
            compilationUnits,
        )
        check(task.call()) {
            "failed to compile bytecode fixture sources:${diagnostics.joinToString(prefix = System.lineSeparator())}"
        }
        fileManager.close()

        val classes = mutableMapOf<String, ByteArray>()
        Files.walk(outputDir)
            .filter { it.toString().endsWith(".class") }
            .forEach { classFile ->
                val relative = outputDir.relativize(classFile).toString().replace('\\', '/').removeSuffix(".class")
                classes[relative] = Files.readAllBytes(classFile)
            }

        compiledClasses = classes
        return classes
    }

    fun provider(entryHashes: Map<String, Long> = emptyMap()): InMemoryClassBytesProvider =
        InMemoryClassBytesProvider(compileFixtures(), entryHashes)

    fun classBytes(simpleName: String): ByteArray =
        compileFixtures()["$FIXTURE_INTERNAL_PREFIX/$simpleName"]
            ?: error("fixture class not found: $simpleName")

    fun internalName(simpleName: String): String = "$FIXTURE_INTERNAL_PREFIX/$simpleName"

    private fun locateFixtureSourceDir(): Path {
        val url = BytecodeFixtureCompiler::class.java.classLoader.getResource("bytecode/fixtures/InvokeSamples.java")
            ?: error("fixture sources not found on classpath")
        val uri = url.toURI()
        return if (uri.scheme == "jar") {
            val jarUri = java.net.URI(uri.schemeSpecificPart.substringBefore("!"))
            val fs = java.nio.file.FileSystems.newFileSystem(jarUri, emptyMap<String, Any>())
            fs.getPath("bytecode/fixtures")
        } else {
            Path.of(uri).parent ?: error("fixture source directory not found for $uri")
        }
    }
}
