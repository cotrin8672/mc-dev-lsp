package io.github.mcdev.core.at

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MemberKind

class AccessTransformerReferenceService(
    private val classIndex: ClassIndex,
) {
    fun findReferences(
        target: McDefinitionTarget,
        sources: List<SourceScanEntry>,
    ): List<McReferenceLocation> =
        sources.flatMap { entry -> findReferences(target, entry) }

    fun findReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> =
        when (target.kind) {
            MemberKind.CLASS -> findClassReferences(target, entry)
            MemberKind.FIELD -> findFieldReferences(target, entry)
            MemberKind.METHOD -> findMethodReferences(target, entry)
            else -> emptyList()
        }

    private fun findClassReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val ownerFqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        val results = mutableListOf<McReferenceLocation>()
        entry.text.lineSequence().forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val tokens = line.split(Regex("\\s+"))
            if (tokens.size != 2) return@forEachIndexed
            if (!ownerMatchesTarget(tokens[1], ownerFqn, target.ownerInternalName)) return@forEachIndexed
            val range = tokenRange(entry.text, lineNumber, 1)
                ?: AtTextPositions.lineRange(entry.text, lineNumber)
                ?: return@forEachIndexed
            results += McReferenceLocation(
                documentUri = entry.documentUri,
                range = range,
                metadata = mapOf("source" to "at.class"),
            )
        }
        return results.distinctBy { "${it.documentUri}:${it.range.start.line}:${it.range.start.character}" }
    }

    private fun findFieldReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val fieldName = target.name ?: return emptyList()
        val ownerFqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        val results = mutableListOf<McReferenceLocation>()
        entry.text.lineSequence().forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val tokens = line.split(Regex("\\s+"))
            if (tokens.size != 3) return@forEachIndexed
            if (!ownerMatchesTarget(tokens[1], ownerFqn, target.ownerInternalName)) return@forEachIndexed
            val member = tokens[2]
            if (member.contains('(')) return@forEachIndexed
            if (member != fieldName) return@forEachIndexed
            val range = tokenRange(entry.text, lineNumber, 2)
                ?: AtTextPositions.lineRange(entry.text, lineNumber)
                ?: return@forEachIndexed
            results += McReferenceLocation(
                documentUri = entry.documentUri,
                range = range,
                metadata = mapOf("source" to "at.field"),
            )
        }
        return results
    }

    private fun findMethodReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val methodName = target.name ?: return emptyList()
        val ownerFqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        val results = mutableListOf<McReferenceLocation>()
        entry.text.lineSequence().forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val tokens = line.split(Regex("\\s+"))
            if (tokens.size != 3) return@forEachIndexed
            if (!ownerMatchesTarget(tokens[1], ownerFqn, target.ownerInternalName)) return@forEachIndexed
            val member = tokens[2]
            if (!member.startsWith(methodName)) return@forEachIndexed
            if (target.descriptor != null && member != "$methodName${target.descriptor}") return@forEachIndexed
            val range = tokenRange(entry.text, lineNumber, 2)
                ?: AtTextPositions.lineRange(entry.text, lineNumber)
                ?: return@forEachIndexed
            results += McReferenceLocation(
                documentUri = entry.documentUri,
                range = range,
                metadata = mapOf("source" to "at.method"),
            )
        }
        return results
    }

    private fun ownerMatchesTarget(owner: String, targetFqn: String, targetInternal: String): Boolean {
        if (owner == targetFqn || owner == targetInternal || owner == targetInternal.replace('/', '.')) {
            return true
        }
        val entry = classIndex.findClassByFqn(owner) ?: classIndex.findClass(owner.replace('.', '/'))
        return entry?.internalName == targetInternal
    }

    private fun tokenRange(source: String, lineNumber: Int, tokenIndex: Int): McTextRange? {
        val rawLine = source.lineSequence().elementAtOrNull(lineNumber - 1) ?: return null
        val contentLine = rawLine.substringBefore('#').trim()
        val tokens = contentLine.split(Regex("\\s+"))
        if (tokenIndex !in tokens.indices) return null
        val lineStart = lineStartOffset(source, lineNumber)
        val contentOffset = rawLine.indexOf(contentLine).coerceAtLeast(0)
        val tokenStartInLine = tokens.take(tokenIndex).sumOf { it.length + 1 }
        val token = tokens[tokenIndex]
        val start = lineStart + contentOffset + tokenStartInLine
        val end = start + token.length
        return AtTextPositions.rangeForOffsets(source, start, end)
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
