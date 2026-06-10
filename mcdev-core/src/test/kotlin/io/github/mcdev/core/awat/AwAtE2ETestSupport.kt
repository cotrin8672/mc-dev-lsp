package io.github.mcdev.core.awat

import io.github.mcdev.core.bytecode.InMemoryClassBytesProvider
import io.github.mcdev.core.index.ProjectContextMixinIndex
import io.github.mcdev.core.mapping.MappingParseResult
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mapping.TinyV2Parser
import io.github.mcdev.core.mapping.asResolver
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.project.ProjectContextBuilder
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import java.nio.file.Path

internal object AwAtE2ETestSupport {
    const val SIMPLE_TARGET_INTERNAL = "com/example/target/SimpleTarget"

    fun loadFixtureText(path: String): String = FixtureResourceLoader.loadText(path)

    fun simpleTargetClassIndex(): ClassIndex =
        object : ClassIndex {
            private val entry = ClassIndexEntry(
                simpleName = "SimpleTarget",
                packageName = "com.example.target",
                internalName = SIMPLE_TARGET_INTERNAL,
            )

            override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
                if (
                    entry.simpleName.startsWith(prefix, ignoreCase = true) ||
                    entry.internalName.substringAfterLast('/').startsWith(prefix, ignoreCase = true) ||
                    prefix.isEmpty()
                ) {
                    listOf(entry)
                } else {
                    emptyList()
                }

            override fun findClass(internalName: String): ClassIndexEntry? =
                entry.takeIf { it.internalName == internalName }

            override fun findClassByFqn(fqn: String): ClassIndexEntry? =
                entry.takeIf { it.fqn == fqn }

            override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
                if (ownerInternalName == SIMPLE_TARGET_INTERNAL) {
                    listOf(
                        MethodIndexEntry("draw", "(Ljava/lang/String;FF)V", false, "draw(String, float, float): void"),
                        MethodIndexEntry("staticMethod", "()V", true, "staticMethod(): void"),
                    )
                } else {
                    emptyList()
                }

            override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
                if (ownerInternalName == SIMPLE_TARGET_INTERNAL) {
                    listOf(FieldIndexEntry("counter", "I", false, "int"))
                } else {
                    emptyList()
                }
        }

    fun fabricAwAtMappingContext(): ProjectMappingContext {
        val mappingsText = FixtureResourceLoader.loadText(FixturePaths.FABRIC_AW_AT_MAPPINGS)
        val mappings = (TinyV2Parser.parse(mappingsText) as MappingParseResult.Success).mappings
        return ProjectMappingContext(
            sourceNamespace = MappingNamespace.NAMED,
            runtimeNamespace = MappingNamespace.INTERMEDIARY,
            awNamespace = MappingNamespace.NAMED,
            atNamespace = MappingNamespace.INTERMEDIARY,
            availableNamespaces = setOf(MappingNamespace.NAMED, MappingNamespace.INTERMEDIARY),
            resolver = mappings.asResolver(),
        )
    }

    fun facade(): AwAtServiceFacade =
        AwAtServiceFacade(
            classIndex = simpleTargetClassIndex(),
            mappingContext = fabricAwAtMappingContext(),
        )

    fun indexedFacade(): AwAtServiceFacade {
        val bytes = FixtureResourceLoader.loadBytes(FixturePaths.SIMPLE_TARGET_CLASS)
        val provider = InMemoryClassBytesProvider(mapOf(SIMPLE_TARGET_INTERNAL to bytes))
        val index = ProjectContextMixinIndex()
        val context = ProjectContextBuilder.empty("e2e", Path.of("."))
        return AwAtServiceFacade(
            classIndex = index.buildClassIndex(context, provider),
            mappingContext = fabricAwAtMappingContext(),
        )
    }

    fun requestAt(
        source: String,
        needle: String,
        fileType: AwAtFileType,
        documentUri: String = defaultDocumentUri(fileType),
    ): AwAtFacadeRequest {
        val offset = source.lastIndexOf(needle)
        require(offset >= 0) { "needle not found: $needle" }
        return requestAtOffset(source, offset + needle.length, fileType, documentUri)
    }

    fun requestAtOffset(
        source: String,
        offset: Int,
        fileType: AwAtFileType,
        documentUri: String = defaultDocumentUri(fileType),
    ): AwAtFacadeRequest {
        val (line, character) = offsetToLineCharacter(source, offset.coerceIn(0, source.length))
        return AwAtFacadeRequest(
            bufferText = source,
            line = line,
            character = character,
            documentUri = documentUri,
            fileType = fileType,
        )
    }

    fun offsetToLineCharacter(source: String, offset: Int): Pair<Int, Int> {
        var line = 0
        var character = 0
        var i = 0
        while (i < offset && i < source.length) {
            if (source[i] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            i++
        }
        return line to character
    }

    private fun defaultDocumentUri(fileType: AwAtFileType): String =
        when (fileType) {
            AwAtFileType.ACCESS_WIDENER -> "file:///mod.accesswidener"
            AwAtFileType.ACCESS_TRANSFORMER -> "file:///mod_at.cfg"
        }
}
