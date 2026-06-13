package io.github.mcdev.core.aw

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MemberKind

class AccessWidenerReferenceService(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
) {
    fun findReferences(
        target: McDefinitionTarget,
        sources: List<SourceScanEntry>,
    ): List<McReferenceLocation> =
        sources.flatMap { entry -> findReferences(target, entry) }

    fun findReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val fileNamespace = AwContextExtractor.parseFileNamespace(entry.text)
        return when (target.kind) {
            MemberKind.CLASS -> findClassReferences(target, entry, fileNamespace)
            MemberKind.FIELD -> findFieldReferences(target, entry, fileNamespace)
            MemberKind.METHOD -> findMethodReferences(target, entry, fileNamespace)
            else -> emptyList()
        }
    }

    private fun findClassReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        fileNamespace: io.github.mcdev.core.model.MappingNamespace?,
    ): List<McReferenceLocation> {
        val results = mutableListOf<McReferenceLocation>()
        entry.text.lineSequence().forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            if (lineNumber == 1) return@forEachIndexed
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val tokens = tokenizeLine(line)
            val kind = tokens.getOrNull(1) ?: return@forEachIndexed
            val owner = tokens.getOrNull(2) ?: return@forEachIndexed
            if (!ownerMatchesTarget(owner, target.ownerInternalName, fileNamespace)) return@forEachIndexed
            val range = when (kind) {
                "class" -> tokenRange(entry.text, lineNumber, 2)
                else -> tokenRange(entry.text, lineNumber, 2)
            } ?: lineRange(entry.text, lineNumber)
            results += McReferenceLocation(
                documentUri = entry.documentUri,
                range = range,
                metadata = mapOf("source" to if (kind == "class") "aw.class" else "aw.owner"),
            )
        }
        return results.distinctBy { "${it.documentUri}:${it.range.start.line}:${it.range.start.character}" }
    }

    private fun findFieldReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        fileNamespace: io.github.mcdev.core.model.MappingNamespace?,
    ): List<McReferenceLocation> {
        val fieldName = target.name ?: return emptyList()
        val results = mutableListOf<McReferenceLocation>()
        awFieldLinePattern.findAll(entry.text).forEach { match ->
            val owner = match.groupValues[1]
            val name = match.groupValues[2]
            val descriptor = match.groupValues[3]
            if (name != fieldName) return@forEach
            if (!ownerMatchesTarget(owner, target.ownerInternalName, fileNamespace)) return@forEach
            if (target.descriptor != null && descriptor != target.descriptor) return@forEach
            results += McReferenceLocation(
                documentUri = entry.documentUri,
                range = locationRange(entry.text, match.range),
                metadata = mapOf("source" to "aw.field"),
            )
        }
        return results
    }

    private fun findMethodReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        fileNamespace: io.github.mcdev.core.model.MappingNamespace?,
    ): List<McReferenceLocation> {
        val methodName = target.name ?: return emptyList()
        val results = mutableListOf<McReferenceLocation>()
        awMethodLinePattern.findAll(entry.text).forEach { match ->
            val owner = match.groupValues[1]
            val name = match.groupValues[2]
            val descriptor = match.groupValues[3]
            if (name != methodName) return@forEach
            if (!ownerMatchesTarget(owner, target.ownerInternalName, fileNamespace)) return@forEach
            if (target.descriptor != null && descriptor != target.descriptor) return@forEach
            results += McReferenceLocation(
                documentUri = entry.documentUri,
                range = locationRange(entry.text, match.range),
                metadata = mapOf("source" to "aw.method"),
            )
        }
        return results
    }

    private fun ownerMatchesTarget(
        owner: String,
        targetOwner: String,
        fileNamespace: io.github.mcdev.core.model.MappingNamespace?,
    ): Boolean {
        if (owner == targetOwner) return true
        val resolved = AwNamespaceHelper.resolveOwnerInIndex(
            owner = owner,
            fileNamespace = fileNamespace,
            classIndex = classIndex,
            mappingContext = mappingContext,
        )
        return resolved == targetOwner
    }

    private fun tokenizeLine(line: String): List<String> {
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

    private fun tokenRange(source: String, lineNumber: Int, tokenIndex: Int): McTextRange? {
        val lineStart = lineStartOffset(source, lineNumber)
        val rawLine = source.lineSequence().elementAtOrNull(lineNumber - 1) ?: return null
        val contentLine = rawLine.substringBefore('#').trim()
        val tokens = tokenizeLine(contentLine)
        if (tokenIndex !in tokens.indices) return null
        val tokenStartInLine = tokens.take(tokenIndex).sumOf { it.length + 1 }
        val token = tokens[tokenIndex]
        val contentOffset = rawLine.indexOf(contentLine).coerceAtLeast(0)
        val start = lineStart + contentOffset + tokenStartInLine
        val end = start + token.length
        return McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))
    }

    private fun lineRange(source: String, lineNumber: Int): McTextRange {
        val start = lineStartOffset(source, lineNumber)
        val end = source.indexOf('\n', start).let { if (it < 0) source.length else it }
        return McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))
    }

    private fun lineStartOffset(source: String, lineNumber: Int): Int {
        var offset = 0
        repeat((lineNumber - 1).coerceAtLeast(0)) {
            val next = source.indexOf('\n', offset)
            offset = if (next < 0) source.length else next + 1
        }
        return offset
    }

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        val (line, character) = AwContextExtractor.offsetToPosition(source, offset.coerceIn(0, source.length))
        return McTextPosition(line = line, character = character)
    }

    private fun locationRange(source: String, range: IntRange): McTextRange =
        McTextRange(
            start = offsetToPosition(source, range.first),
            end = offsetToPosition(source, range.last + 1),
        )

    companion object {
        private val awMethodLinePattern = Regex(
            """(?m)^\s*(?:accessible|extendable|mutable|natural)\s+method\s+(\S+)\s+(\w+)\s+(\S+)""",
        )
        private val awFieldLinePattern = Regex(
            """(?m)^\s*(?:accessible|extendable|mutable|natural)\s+field\s+(\S+)\s+(\w+)\s+(\S+)""",
        )
    }
}
