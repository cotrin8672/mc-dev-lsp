package io.github.mcdev.jdtls.convert

import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.protocol.McdevDiagnosticDto
import io.github.mcdev.protocol.McdevPosition
import io.github.mcdev.protocol.McdevRange

object DiagnosticConverter {
    fun toDto(diagnostic: McDiagnostic): McdevDiagnosticDto =
        McdevDiagnosticDto(
            code = diagnostic.code,
            severity = diagnostic.severity.toProtocolSeverity(),
            message = diagnostic.message,
            range = McdevRange(
                start = McdevPosition(
                    line = diagnostic.range.start.line,
                    character = diagnostic.range.start.character,
                ),
                end = McdevPosition(
                    line = diagnostic.range.end.line,
                    character = diagnostic.range.end.character,
                ),
            ),
            metadata = diagnostic.metadata,
        )

    fun toDtos(diagnostics: List<McDiagnostic>): List<McdevDiagnosticDto> =
        diagnostics.map(::toDto)

    private fun McSeverity.toProtocolSeverity(): String = when (this) {
        McSeverity.ERROR -> "error"
        McSeverity.WARNING -> "warning"
        McSeverity.INFO -> "information"
    }
}
