package io.github.mcdev.core.at

enum class AccessTransformerModifier(val token: String) {
    PUBLIC("public"),
    PROTECTED("protected"),
    DEFAULT("default"),
    PRIVATE("private"),
    PUBLIC_REMOVE_FINAL("public-f"),
    PROTECTED_REMOVE_FINAL("protected-f"),
    PRIVATE_REMOVE_FINAL("private-f"),
    PUBLIC_ADD_FINAL("public+f"),
    PROTECTED_ADD_FINAL("protected+f"),
    PRIVATE_ADD_FINAL("private+f"),
}

data class AccessTransformerFile(
    val entries: List<AccessTransformerEntry>,
)

data class AccessTransformerEntry(
    val modifier: AccessTransformerModifier,
    val owner: String,
    val name: String? = null,
    val descriptor: String? = null,
    val line: Int,
)

sealed interface AccessTransformerParseResult {
    data class Success(val file: AccessTransformerFile) : AccessTransformerParseResult
    data class Failure(val line: Int, val message: String) : AccessTransformerParseResult
}
