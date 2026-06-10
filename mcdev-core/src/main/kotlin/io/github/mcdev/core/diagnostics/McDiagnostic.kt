package io.github.mcdev.core.diagnostics

enum class McSeverity {
    ERROR,
    WARNING,
    INFO,
}

data class McTextPosition(
    val line: Int,
    val character: Int,
)

data class McTextRange(
    val start: McTextPosition,
    val end: McTextPosition,
)

data class McDiagnostic(
    val code: String,
    val severity: McSeverity,
    val message: String,
    val range: McTextRange,
    val metadata: Map<String, String> = emptyMap(),
)
