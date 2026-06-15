package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.mixin.InjectMethodDescriptorMode
import io.github.mcdev.core.mixin.MixinClassInsertMode
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinDefinitionService
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinReferenceService
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.mixin.MixinServiceFacade as CoreMixinServiceFacade
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.project.MixinConfigDiscoveryService
import io.github.mcdev.core.project.MixinConfigRef
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.jdtls.project.McdevProjectSession
import io.github.mcdev.jdtls.project.UriPathSupport

class MixinServiceFacade(
    private val facadeFactory: (ClassIndex, BytecodeIndex) -> CoreMixinServiceFacade = { classIndex, bytecodeIndex ->
        CoreMixinServiceFacade(classIndex, bytecodeIndex)
    },
    private val referenceService: MixinReferenceService = MixinReferenceService(),
    private val completeOverride: ((
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
    ) -> List<McCompletionItem>)? = null,
) {
    fun complete(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
        documentUri: String = "file:///Mixin.java",
    ): List<McCompletionItem> =
        completeOverride?.invoke(session, source, line, character, options)
            ?: facade(session, source = source, documentUri = documentUri).complete(
                MixinFacadeRequest(
                    bufferText = source,
                    line = line,
                    character = character,
                    documentUri = documentUri,
                ),
                options,
            )

    fun analyzeDiagnostics(
        session: McdevProjectSession,
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
    ): List<McDiagnostic> =
        facade(session).diagnose(buildFacadeRequest(projectContext, source, documentUri))

    fun selectedMixinConfig(
        projectContext: ProjectContext,
        source: String,
    ): MixinConfigRef? =
        MixinConfigDiscoveryService.selectForMixin(
            configs = projectContext.mixinConfigs,
            mixinClassName = extractMixinClassName(source),
            mixinPackage = extractPackageName(source),
        )

    fun selectedMixinConfigContent(
        projectContext: ProjectContext,
        source: String,
    ): String? =
        selectedMixinConfig(projectContext, source)?.path?.let(MixinConfigDiscoveryService::readContent)

    fun codeActions(
        session: McdevProjectSession,
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
        diagnosticCode: String? = null,
    ): List<McFix> {
        val request = buildFacadeRequest(
            projectContext = projectContext,
            source = source,
            documentUri = documentUri,
        )
        return facade(session).codeActions(request, diagnosticCode)
    }

    fun definitions(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
    ): List<McDefinitionTarget> =
        MixinDefinitionService(session.classIndex, session.bytecodeIndex)
            .definitionsAt(source, line, character)

    fun references(
        session: McdevProjectSession,
        target: McDefinitionTarget,
        sources: List<SourceScanEntry>,
    ): List<McReferenceLocation> =
        referenceService.findReferences(target, sources)

    fun collectSourceEntries(
        projectContext: ProjectContext,
        currentDocumentUri: String,
        currentBufferText: String,
    ): List<SourceScanEntry> {
        val entries = linkedMapOf<String, String>()
        entries[currentDocumentUri] = currentBufferText
        projectContext.sourceSets.forEach { sourceSet ->
            sourceSet.sourceDirectories.forEach { sourceDir ->
                if (!sourceDir.toFile().isDirectory) return@forEach
                sourceDir.toFile().walkTopDown()
                    .filter { it.isFile && it.extension == "java" }
                    .forEach { file ->
                        val uri = UriPathSupport.pathToUri(file.toPath())
                        if (uri !in entries) {
                            entries[uri] = runCatching { file.readText() }.getOrDefault("")
                        }
                    }
            }
        }
        return entries.map { (uri, text) -> SourceScanEntry(uri, text) }
    }

    fun toCompletionOptions(
        mixinClassInsert: String,
        injectMethodDescriptor: String,
        preferredAtTarget: String = "descriptor",
    ): MixinCompletionOptions =
        MixinCompletionOptions(
            classInsertMode = when (mixinClassInsert.lowercase()) {
                "fqn" -> MixinClassInsertMode.FQN
                else -> MixinClassInsertMode.IMPORT
            },
            injectMethodDescriptorMode = when (injectMethodDescriptor.lowercase()) {
                "always" -> InjectMethodDescriptorMode.ALWAYS
                "never" -> InjectMethodDescriptorMode.NEVER
                else -> InjectMethodDescriptorMode.AUTO
            },
            preferredAtTarget = preferredAtTarget,
        )

    private fun facade(session: McdevProjectSession): CoreMixinServiceFacade =
        facadeFactory(session.classIndex, session.bytecodeIndex)

    private fun facade(session: McdevProjectSession, source: String, documentUri: String): CoreMixinServiceFacade =
        facadeFactory(
            SourceBackedClassIndex(
                delegate = session.classIndex,
                session = session,
                currentDocumentUri = documentUri,
                currentBufferText = source,
            ),
            session.bytecodeIndex,
        )

    private fun buildFacadeRequest(
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
    ): MixinFacadeRequest {
        val mixinConfig = selectedMixinConfig(projectContext, source)
        val mixinPackage = extractPackageName(source) ?: mixinConfig?.packageName
        return MixinFacadeRequest(
            bufferText = source,
            line = 0,
            character = 0,
            documentUri = documentUri,
            mixinClassName = extractMixinClassName(source),
            mixinPackage = mixinPackage,
            mixinConfigContent = mixinConfig?.path?.let(MixinConfigDiscoveryService::readContent),
            mixinConfigPath = mixinConfig?.path?.toString(),
        )
    }

    private fun extractMixinClassName(source: String): String? =
        Regex("""\bclass\s+(\w+)""").find(source)?.groupValues?.get(1)

    private fun extractPackageName(source: String): String? =
        Regex("""^\s*package\s+([\w.]+)\s*;""", RegexOption.MULTILINE).find(source)?.groupValues?.get(1)
}
