package io.github.mcdev.core.aw

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex

data class AccessWidenerDiagnosticRequest(
    val source: String,
    val documentUri: String,
    val mappingContext: ProjectMappingContext? = null,
)

class AccessWidenerDiagnosticsService(
    private val classIndex: ClassIndex,
) {
    fun analyze(request: AccessWidenerDiagnosticRequest): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        when (val parsed = AccessWidenerParser.parse(request.source)) {
            is AccessWidenerParseResult.Failure -> {
                diagnostics += McDiagnostic(
                    code = when {
                        parsed.message.contains("directive") -> AwDiagnosticCodes.INVALID_DIRECTIVE
                        parsed.message.contains("kind") -> AwDiagnosticCodes.INVALID_KIND
                        parsed.message.contains("descriptor") -> AwDiagnosticCodes.INVALID_DESCRIPTOR
                        else -> AwDiagnosticCodes.INVALID_DIRECTIVE
                    },
                    severity = McSeverity.ERROR,
                    message = parsed.message,
                    range = lineRange(request.source, parsed.line),
                    metadata = mapOf("line" to parsed.line.toString()),
                )
            }
            is AccessWidenerParseResult.Success -> {
                diagnostics += analyzeEntries(request, parsed.file)
                diagnostics += analyzeDuplicates(request, parsed.file)
            }
        }
        diagnostics += analyzePartialLines(request)
        return diagnostics
    }

    private fun analyzeEntries(
        request: AccessWidenerDiagnosticRequest,
        file: AccessWidenerFile,
    ): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        for (entry in file.entries) {
            if (entry.directive == AccessWidenerDirective.MUTABLE && entry.kind != AccessWidenerKind.FIELD) {
                diagnostics += lineDiagnostic(
                    AwDiagnosticCodes.MUTABLE_ON_NON_FIELD,
                    McSeverity.ERROR,
                    "mutable directive can only be applied to fields",
                    request.source,
                    entry.line,
                )
            }
            if (entry.directive == AccessWidenerDirective.EXTENDABLE && entry.kind != AccessWidenerKind.CLASS) {
                diagnostics += lineDiagnostic(
                    AwDiagnosticCodes.EXTENDABLE_INVALID_TARGET,
                    McSeverity.ERROR,
                    "extendable directive can only be applied to classes",
                    request.source,
                    entry.line,
                )
            }
            if (AwNamespaceHelper.hasNamespaceMismatch(entry.owner, file.namespace, classIndex, request.mappingContext)) {
                diagnostics += tokenDiagnostic(
                    AwDiagnosticCodes.NAMESPACE_MISMATCH,
                    McSeverity.ERROR,
                    "Owner '${entry.owner}' does not match access widener namespace '${file.namespace.name.lowercase()}'",
                    request.source,
                    entry.line,
                    2,
                    mapOf("owner" to entry.owner, "line" to entry.line.toString()),
                )
            }
            val resolvedOwner = AwNamespaceHelper.resolveOwnerInIndex(
                entry.owner,
                file.namespace,
                classIndex,
                request.mappingContext,
            )
            if (resolvedOwner == null) {
                diagnostics += tokenDiagnostic(
                    AwDiagnosticCodes.UNRESOLVED_CLASS,
                    McSeverity.ERROR,
                    "Unresolved class: ${entry.owner}",
                    request.source,
                    entry.line,
                    2,
                    mapOf("owner" to entry.owner, "line" to entry.line.toString()),
                )
            } else when (entry.kind) {
                AccessWidenerKind.CLASS -> Unit
                AccessWidenerKind.METHOD -> diagnostics += analyzeMethodEntry(request, entry, resolvedOwner)
                AccessWidenerKind.FIELD -> diagnostics += analyzeFieldEntry(request, entry, resolvedOwner)
            }
        }
        return diagnostics
    }

    private fun analyzeMethodEntry(
        request: AccessWidenerDiagnosticRequest,
        entry: AccessWidenerEntry,
        resolvedOwner: String,
    ): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        val name = entry.name.orEmpty()
        val descriptor = entry.descriptor.orEmpty()
        if (parseMethodDescriptor(descriptor) is DescriptorParseResult.Failure) {
            diagnostics += tokenDiagnostic(
                AwDiagnosticCodes.INVALID_DESCRIPTOR,
                McSeverity.ERROR,
                "Invalid method descriptor: $descriptor",
                request.source,
                entry.line,
                4,
                mapOf("descriptor" to descriptor, "line" to entry.line.toString()),
            )
        }
        val methods = classIndex.getMethods(resolvedOwner).filter { it.name == name }
        when {
            methods.isEmpty() -> diagnostics += tokenDiagnostic(
                AwDiagnosticCodes.UNRESOLVED_MEMBER,
                McSeverity.ERROR,
                "Unresolved method: $name",
                request.source,
                entry.line,
                3,
                mapOf("name" to name, "line" to entry.line.toString()),
            )
            descriptor.isNotEmpty() && methods.none { it.descriptor == descriptor } -> diagnostics += tokenDiagnostic(
                AwDiagnosticCodes.INVALID_DESCRIPTOR,
                McSeverity.ERROR,
                "Method descriptor mismatch for '$name'",
                request.source,
                entry.line,
                4,
                mapOf("name" to name, "descriptor" to descriptor, "line" to entry.line.toString()),
            )
        }
        return diagnostics
    }

    private fun analyzeFieldEntry(
        request: AccessWidenerDiagnosticRequest,
        entry: AccessWidenerEntry,
        resolvedOwner: String,
    ): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        val name = entry.name.orEmpty()
        val descriptor = entry.descriptor.orEmpty()
        if (parseFieldDescriptor(descriptor) is DescriptorParseResult.Failure) {
            diagnostics += tokenDiagnostic(
                AwDiagnosticCodes.INVALID_DESCRIPTOR,
                McSeverity.ERROR,
                "Invalid field descriptor: $descriptor",
                request.source,
                entry.line,
                4,
                mapOf("descriptor" to descriptor, "line" to entry.line.toString()),
            )
        }
        val fields = classIndex.getFields(resolvedOwner).filter { it.name == name }
        when {
            fields.isEmpty() -> diagnostics += tokenDiagnostic(
                AwDiagnosticCodes.UNRESOLVED_MEMBER,
                McSeverity.ERROR,
                "Unresolved field: $name",
                request.source,
                entry.line,
                3,
                mapOf("name" to name, "line" to entry.line.toString()),
            )
            descriptor.isNotEmpty() && fields.none { it.descriptor == descriptor } -> diagnostics += tokenDiagnostic(
                AwDiagnosticCodes.INVALID_DESCRIPTOR,
                McSeverity.ERROR,
                "Field descriptor mismatch for '$name'",
                request.source,
                entry.line,
                4,
                mapOf("name" to name, "descriptor" to descriptor, "line" to entry.line.toString()),
            )
        }
        return diagnostics
    }

    private fun analyzeDuplicates(
        request: AccessWidenerDiagnosticRequest,
        file: AccessWidenerFile,
    ): List<McDiagnostic> =
        file.entries
            .groupBy { AccessWidenerEditor.entryKey(it) }
            .filter { it.value.size > 1 }
            .flatMap { (_, entries) ->
                entries.map { entry ->
                    lineDiagnostic(
                        AwDiagnosticCodes.DUPLICATE_ENTRY,
                        McSeverity.WARNING,
                        "Duplicate access widener entry",
                        request.source,
                        entry.line,
                        mapOf("line" to entry.line.toString(), "key" to AccessWidenerEditor.entryKey(entry)),
                    )
                }
            }

    private fun analyzePartialLines(request: AccessWidenerDiagnosticRequest): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        request.source.lineSequence().forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            if (lineNumber == 1) return@forEachIndexed
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val parts = line.split(Regex("\\s+"))
            parts.getOrNull(0)?.let { directive ->
                if (directive !in setOf("accessible", "extendable", "mutable", "natural")) {
                    diagnostics += tokenDiagnostic(
                        AwDiagnosticCodes.INVALID_DIRECTIVE,
                        McSeverity.ERROR,
                        "Invalid access widener directive: $directive",
                        request.source,
                        lineNumber,
                        0,
                        mapOf("directive" to directive, "line" to lineNumber.toString()),
                    )
                }
            }
            parts.getOrNull(1)?.let { kind ->
                if (kind !in setOf("class", "method", "field")) {
                    diagnostics += tokenDiagnostic(
                        AwDiagnosticCodes.INVALID_KIND,
                        McSeverity.ERROR,
                        "Invalid access widener kind: $kind",
                        request.source,
                        lineNumber,
                        1,
                        mapOf("kind" to kind, "line" to lineNumber.toString()),
                    )
                }
            }
        }
        return diagnostics
    }

    private fun lineDiagnostic(
        code: String,
        severity: McSeverity,
        message: String,
        source: String,
        lineNumber: Int,
        metadata: Map<String, String> = emptyMap(),
    ): McDiagnostic = McDiagnostic(
        code = code,
        severity = severity,
        message = message,
        range = lineRange(source, lineNumber),
        metadata = metadata,
    )

    private fun tokenDiagnostic(
        code: String,
        severity: McSeverity,
        message: String,
        source: String,
        lineNumber: Int,
        tokenIndex: Int,
        metadata: Map<String, String> = emptyMap(),
    ): McDiagnostic = McDiagnostic(
        code = code,
        severity = severity,
        message = message,
        range = tokenRange(source, lineNumber, tokenIndex),
        metadata = metadata,
    )

    private fun lineRange(source: String, lineNumber: Int): McTextRange {
        val start = lineStartOffset(source, lineNumber)
        val end = source.indexOf('\n', start).let { if (it < 0) source.length else it }
        val (startLine, startChar) = AwContextExtractor.offsetToPosition(source, start)
        val (endLine, endChar) = AwContextExtractor.offsetToPosition(source, end)
        return McTextRange(McTextPosition(startLine, startChar), McTextPosition(endLine, endChar))
    }

    private fun tokenRange(source: String, lineNumber: Int, tokenIndex: Int): McTextRange {
        val lineStart = lineStartOffset(source, lineNumber)
        val rawLine = source.lineSequence().elementAt(lineNumber - 1)
        val contentLine = rawLine.substringBefore('#').trim()
        val tokens = tokenizeForRange(contentLine)
        if (tokenIndex !in tokens.indices) return lineRange(source, lineNumber)
        val tokenStartInLine = tokens.take(tokenIndex).sumOf { it.length + 1 }
        val token = tokens[tokenIndex]
        val contentOffset = rawLine.indexOf(contentLine).coerceAtLeast(0)
        val start = lineStart + contentOffset + tokenStartInLine
        val end = start + token.length
        val (startLine, startChar) = AwContextExtractor.offsetToPosition(source, start)
        val (endLine, endChar) = AwContextExtractor.offsetToPosition(source, end)
        return McTextRange(McTextPosition(startLine, startChar), McTextPosition(endLine, endChar))
    }

    private fun tokenizeForRange(line: String): List<String> {
        val tokens = mutableListOf<String>()
        var index = 0
        while (index < line.length) {
            while (index < line.length && line[index].isWhitespace()) index++
            if (index >= line.length) break
            val start = index
            if (line[index] == '(') {
                var depth = 0
                while (index < line.length) {
                    when (line[index]) {
                        '(' -> depth++
                        ')' -> if (--depth == 0) {
                            index++
                            break
                        }
                    }
                    index++
                }
            } else {
                while (index < line.length && !line[index].isWhitespace()) index++
            }
            tokens += line.substring(start, index)
        }
        return tokens
    }

    private fun lineStartOffset(source: String, lineNumber: Int): Int {
        var offset = 0
        repeat((lineNumber - 1).coerceAtLeast(0)) {
            val next = source.indexOf('\n', offset)
            offset = if (next < 0) source.length else next + 1
        }
        return offset
    }
}
