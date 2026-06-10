package io.github.mcdev.core.mixin

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MemberKind

class MixinReferenceService {
    fun findReferences(
        target: McDefinitionTarget,
        sources: List<SourceScanEntry>,
    ): List<McReferenceLocation> =
        sources.flatMap { entry ->
            when (target.kind) {
                MemberKind.CLASS -> findClassReferences(target, entry)
                MemberKind.FIELD -> findFieldReferences(target, entry)
                MemberKind.METHOD -> findMethodReferences(target, entry)
                else -> emptyList()
            }
        }

    private fun findClassReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val results = mutableListOf<McReferenceLocation>()
        val simpleName = target.ownerFqn?.substringAfterLast('.') ?: target.ownerInternalName.substringAfterLast('/')
        val fqn = target.ownerFqn
        val internal = target.ownerInternalName

        Regex("""@Mixin\s*\(([^)]*)\)""").findAll(entry.text).forEach { match ->
            val body = match.groupValues[1]
            if (body.contains("$simpleName.class") ||
                (fqn != null && (body.contains(fqn) || body.contains("\"$fqn\""))) ||
                body.contains("\"$internal\"") ||
                body.contains(internal.replace('/', '.'))
            ) {
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.class")
            }
        }

        Regex("""@Mixin\s*\(\s*targets\s*=\s*("([^"]*)"|(\{[^}]*\}))""").findAll(entry.text).forEach { match ->
            val value = match.groupValues[1]
            if (value.contains(simpleName) ||
                (fqn != null && value.contains(fqn)) ||
                value.contains(internal) ||
                value.contains(internal.replace('/', '.'))
            ) {
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.targets")
            }
        }

        return results.distinctBy { "${it.documentUri}:${it.range.start.line}:${it.range.start.character}" }
    }

    private fun findFieldReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val fieldName = target.name ?: return emptyList()
        val results = mutableListOf<McReferenceLocation>()

        Regex("""@Shadow(?:\s*\([^)]*\))?\s+(?:private|protected|public)?\s*(?:static\s+)?[\w.<>\[\]]+\s+(\w+)\s*;""")
            .findAll(entry.text)
            .filter { it.groupValues[1] == fieldName }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.shadow")
            }

        Regex("""@Accessor\s*\(\s*"([^"]*)"\s*\)""")
            .findAll(entry.text)
            .filter { it.groupValues[1] == fieldName }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.accessor")
            }

        Regex("""target\s*=\s*"([^"]*)"""")
            .findAll(entry.text)
            .filter { it.groupValues[1].contains(";$fieldName:") }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.atTarget")
            }

        return results
    }

    private fun findMethodReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val methodName = target.name ?: return emptyList()
        val results = mutableListOf<McReferenceLocation>()

        Regex("""@Shadow(?:\s*\([^)]*\))?\s+(?:private|protected|public)?\s*(?:static\s+)?(?:abstract\s+)?[\w.<>\[\]]+\s+(\w+)\s*\([^)]*\)\s*;""")
            .findAll(entry.text)
            .filter { it.groupValues[1] == methodName }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.shadow")
            }

        Regex("""@Invoker\s*\(\s*"([^"]*)"\s*\)""")
            .findAll(entry.text)
            .filter { it.groupValues[1] == methodName }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.invoker")
            }

        Regex("""method\s*=\s*"([^"]*)"""")
            .findAll(entry.text)
            .filter { it.groupValues[1].startsWith(methodName) }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.inject")
            }

        val descriptor = target.descriptor
        if (descriptor != null) {
            Regex("""target\s*=\s*"([^"]*)"""")
                .findAll(entry.text)
                .filter { it.groupValues[1].contains(";$methodName$descriptor") }
                .forEach { match ->
                    results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.atTarget")
                }
        }

        return results
    }

    private fun locationForMatch(
        documentUri: String,
        text: String,
        range: IntRange,
        source: String,
    ): McReferenceLocation =
        McReferenceLocation(
            documentUri = documentUri,
            range = offsetRange(text, range.first, range.last + 1),
            metadata = mapOf("source" to source),
        )

    private fun offsetRange(source: String, start: Int, end: Int): McTextRange =
        McTextRange(
            start = offsetToPosition(source, start),
            end = offsetToPosition(source, end.coerceAtLeast(start)),
        )

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        val safeOffset = offset.coerceIn(0, source.length)
        var line = 0
        var lastLineStart = 0
        var index = 0
        while (index < safeOffset) {
            if (source[index] == '\n') {
                line++
                lastLineStart = index + 1
            }
            index++
        }
        return McTextPosition(line = line, character = safeOffset - lastLineStart)
    }
}
