package io.github.mcdev.jdtls.definition

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.jdtls.java.JdtReflectionBridge
import io.github.mcdev.protocol.McdevDefinitionResolution

fun interface DefinitionBackend {
    fun resolve(target: McDefinitionTarget, projectContext: ProjectContext, workspaceRootUri: String): ResolvedDefinition?
}

class SourceDefinitionBackend(
    private val sourceIndexFactory: (ProjectContext) -> SourceIndex = SourceIndex::fromProject,
) : DefinitionBackend {
    override fun resolve(
        target: McDefinitionTarget,
        projectContext: ProjectContext,
        workspaceRootUri: String,
    ): ResolvedDefinition? {
        val resolved = sourceIndexFactory(projectContext).resolve(target)
        return resolved.takeIf { it.resolution == McdevDefinitionResolution.SOURCE }
    }
}

class JdtDefinitionBackend(
    private val bridge: JdtReflectionBridge? = JdtReflectionBridge.instance,
) : DefinitionBackend {
    override fun resolve(
        target: McDefinitionTarget,
        projectContext: ProjectContext,
        workspaceRootUri: String,
    ): ResolvedDefinition? {
        val jdtLocation = bridge?.resolveDefinition(target, workspaceRootUri) ?: return null
        if (jdtLocation.documentUri.isBlank() &&
            jdtLocation.resolution != McdevDefinitionResolution.BYTECODE_ONLY
        ) {
            return null
        }
        return ResolvedDefinition(
            target = target,
            documentUri = jdtLocation.documentUri,
            range = jdtLocation.range,
            resolution = jdtLocation.resolution,
            resolutionMessage = jdtLocation.resolutionMessage,
        )
    }
}

class DefinitionResolutionService(
    private val backends: List<DefinitionBackend> = listOf(
        SourceDefinitionBackend(),
        JdtDefinitionBackend(),
    ),
) {
    fun resolveAll(
        targets: List<McDefinitionTarget>,
        projectContext: ProjectContext,
        workspaceRootUri: String,
    ): List<ResolvedDefinition> {
        if (targets.isEmpty()) return emptyList()
        return targets.map { target ->
            backends.firstNotNullOfOrNull { backend ->
                backend.resolve(target, projectContext, workspaceRootUri)
            } ?: unresolved(target, "no navigable definition found")
        }
    }

    private fun unresolved(target: McDefinitionTarget, message: String): ResolvedDefinition =
        ResolvedDefinition(
            target = target,
            documentUri = "",
            range = target.sourceRange ?: McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            resolution = McdevDefinitionResolution.UNRESOLVED,
            resolutionMessage = message,
        )
}
