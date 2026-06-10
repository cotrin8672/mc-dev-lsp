package io.github.mcdev.core.definition

import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.model.MemberKind

data class McDefinitionTarget(
    val kind: MemberKind,
    val ownerInternalName: String,
    val ownerFqn: String?,
    val name: String? = null,
    val descriptor: String? = null,
    val namespace: MappingNamespace = MappingNamespace.NAMED,
    val sourceRange: McTextRange? = null,
)

data class McReferenceLocation(
    val documentUri: String,
    val range: McTextRange,
    val metadata: Map<String, String> = emptyMap(),
)

data class SourceScanEntry(
    val documentUri: String,
    val text: String,
)
