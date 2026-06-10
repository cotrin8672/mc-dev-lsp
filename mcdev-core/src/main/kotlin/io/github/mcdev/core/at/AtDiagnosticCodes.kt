package io.github.mcdev.core.at

import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex

object AtDiagnosticCodes {
    const val INVALID_MODIFIER = "at.invalidModifier"
    const val UNRESOLVED_CLASS = "at.unresolvedClass"
    const val UNRESOLVED_MEMBER = "at.unresolvedMember"
    const val MISSING_METHOD_DESCRIPTOR = "at.missingMethodDescriptor"
    const val INVALID_DESCRIPTOR = "at.invalidDescriptor"
    const val DUPLICATE_ENTRY = "at.duplicateEntry"
    const val WRONG_NAMESPACE = "at.wrongNamespace"
    const val SRG_MAPPING_NOT_FOUND = "at.srgMappingNotFound"
    const val PARSE_ERROR = "at.parseError"
}

data class AtDiagnosticRequest(
    val source: String,
    val documentUri: String = "",
    val mappingContext: ProjectMappingContext? = null,
    val classIndex: ClassIndex? = null,
)
