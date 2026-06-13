package io.github.mcdev.jdtls.definition

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.protocol.McdevDefinitionResolution

data class ResolvedDefinition(
    val target: McDefinitionTarget,
    val documentUri: String,
    val range: McTextRange,
    val resolution: McdevDefinitionResolution,
    val resolutionMessage: String? = null,
)
