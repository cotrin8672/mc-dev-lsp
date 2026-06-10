package io.github.mcdev.core.aw

import io.github.mcdev.core.model.MappingNamespace

enum class AwSyntaxSlot {
    HEADER_NAMESPACE,
    DIRECTIVE,
    KIND,
    OWNER,
    NAME,
    DESCRIPTOR,
}

data class AwAnnotationContext(
    val slot: AwSyntaxSlot,
    val partialValue: String,
    val valueStartOffset: Int,
    val valueEndOffset: Int,
    val lineStartOffset: Int,
    val lineEndOffset: Int,
    val lineNumber: Int,
    val fileNamespace: MappingNamespace?,
    val directive: AccessWidenerDirective?,
    val kind: AccessWidenerKind?,
    val owner: String?,
    val name: String?,
    val descriptor: String?,
    val isHeaderLine: Boolean,
)
