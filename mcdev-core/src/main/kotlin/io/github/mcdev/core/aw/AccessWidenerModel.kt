package io.github.mcdev.core.aw

import io.github.mcdev.core.model.MappingNamespace

enum class AccessWidenerDirective {
    ACCESSIBLE,
    EXTENDABLE,
    MUTABLE,
    NATURAL,
}

enum class AccessWidenerKind {
    CLASS,
    METHOD,
    FIELD,
}

data class AccessWidenerFile(
    val namespace: MappingNamespace,
    val entries: List<AccessWidenerEntry>,
)

data class AccessWidenerEntry(
    val directive: AccessWidenerDirective,
    val kind: AccessWidenerKind,
    val owner: String,
    val name: String? = null,
    val descriptor: String? = null,
    val line: Int,
)

sealed interface AccessWidenerParseResult {
    data class Success(val file: AccessWidenerFile) : AccessWidenerParseResult
    data class Failure(val line: Int, val message: String) : AccessWidenerParseResult
}
