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

        findAwClassReferences(target, entry, results)
        findAtClassReferences(target, entry, results)

        return results.distinctBy { "${it.documentUri}:${it.range.start.line}:${it.range.start.character}" }
    }

    private fun findFieldReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val fieldName = target.name ?: return emptyList()
        val results = mutableListOf<McReferenceLocation>()

        MixinMemberDeclarationParser.parseShadowDeclarations(entry.text)
            .filter { !it.isMethod && it.name == fieldName }
            .forEach { declaration ->
                results += locationForRange(entry.documentUri, declaration.range, "mixin.shadow")
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

        findAwFieldReferences(target, entry, results)
        findAtFieldReferences(target, entry, results)

        return results
    }

    private fun findMethodReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
    ): List<McReferenceLocation> {
        val methodName = target.name ?: return emptyList()
        val results = mutableListOf<McReferenceLocation>()

        MixinMemberDeclarationParser.parseShadowDeclarations(entry.text)
            .filter { it.isMethod && it.name == methodName }
            .forEach { declaration ->
                results += locationForRange(entry.documentUri, declaration.range, "mixin.shadow")
            }

        Regex("""@Invoker\s*\(\s*"([^"]*)"\s*\)""")
            .findAll(entry.text)
            .filter { it.groupValues[1] == methodName }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.invoker")
            }

        Regex("""method\s*=\s*"([^"]*)"""")
            .findAll(entry.text)
            .filter { methodAnnotationMatches(it.groupValues[1], methodName, target.descriptor) }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "mixin.inject")
            }

        MixinMemberDeclarationParser.parseOverwriteDeclarations(entry.text)
            .filter { it.name == methodName }
            .forEach { declaration ->
                results += locationForRange(entry.documentUri, declaration.range, "mixin.overwrite")
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

        findAwMethodReferences(target, entry, results)
        findAtMethodReferences(target, entry, results)

        return results
    }

    private fun methodAnnotationMatches(value: String, methodName: String, descriptor: String?): Boolean {
        if (value == methodName) return true
        if (!descriptor.isNullOrBlank() && value == "$methodName$descriptor") return true
        if (descriptor.isNullOrBlank() && value.startsWith("$methodName(")) return true
        return false
    }

    private fun findAwClassReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        results: MutableList<McReferenceLocation>,
    ) {
        val owner = target.ownerInternalName
        val ownerDot = owner.replace('/', '.')
        Regex("""(accessible|extendable|mutable|natural)\s+class\s+(\S+)""")
            .findAll(entry.text)
            .filter { it.groupValues[2] == owner || it.groupValues[2] == ownerDot }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "aw.class")
            }
        Regex("""(accessible|extendable|mutable|natural)\s+(method|field)\s+(\S+)\s+""")
            .findAll(entry.text)
            .filter { it.groupValues[3] == owner || it.groupValues[3] == ownerDot }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "aw.owner")
            }
    }

    private fun findAtClassReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        results: MutableList<McReferenceLocation>,
    ) {
        val fqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        Regex("""(public|protected|private|default)\s+([\w.]+)(?:\s+\S+)?\s*$""", RegexOption.MULTILINE)
            .findAll(entry.text)
            .filter { it.groupValues[2] == fqn }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "at.class")
            }
    }

    private fun findAwFieldReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        results: MutableList<McReferenceLocation>,
    ) {
        val fieldName = target.name ?: return
        val owner = target.ownerInternalName
        val ownerDot = owner.replace('/', '.')
        val descriptor = target.descriptor
        Regex("""(accessible|extendable|mutable|natural)\s+field\s+(\S+)\s+(\w+)\s+(\S+)""")
            .findAll(entry.text)
            .filter {
                it.groupValues[3] == fieldName &&
                    (it.groupValues[2] == owner || it.groupValues[2] == ownerDot) &&
                    (descriptor == null || it.groupValues[4] == descriptor)
            }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "aw.field")
            }
    }

    private fun findAtFieldReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        results: MutableList<McReferenceLocation>,
    ) {
        val fieldName = target.name ?: return
        val fqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        Regex("""(public|protected|private|default)\s+([\w.]+)\s+(\w+)\s*$""", RegexOption.MULTILINE)
            .findAll(entry.text)
            .filter { it.groupValues[2] == fqn && it.groupValues[3] == fieldName }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "at.field")
            }
    }

    private fun findAwMethodReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        results: MutableList<McReferenceLocation>,
    ) {
        val methodName = target.name ?: return
        val owner = target.ownerInternalName
        val ownerDot = owner.replace('/', '.')
        val descriptor = target.descriptor
        Regex("""(accessible|extendable|mutable|natural)\s+method\s+(\S+)\s+(\w+)\s+(\S+)""")
            .findAll(entry.text)
            .filter {
                it.groupValues[3] == methodName &&
                    (it.groupValues[2] == owner || it.groupValues[2] == ownerDot) &&
                    (descriptor == null || it.groupValues[4] == descriptor)
            }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "aw.method")
            }
    }

    private fun findAtMethodReferences(
        target: McDefinitionTarget,
        entry: SourceScanEntry,
        results: MutableList<McReferenceLocation>,
    ) {
        val methodName = target.name ?: return
        val fqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        val descriptor = target.descriptor
        Regex("""(public|protected|private|default)\s+([\w.]+)\s+(\S+)\s*$""", RegexOption.MULTILINE)
            .findAll(entry.text)
            .filter {
                it.groupValues[2] == fqn &&
                    it.groupValues[3].startsWith(methodName) &&
                    (descriptor == null || it.groupValues[3] == "$methodName$descriptor")
            }
            .forEach { match ->
                results += locationForMatch(entry.documentUri, entry.text, match.range, "at.method")
            }
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

    private fun locationForRange(
        documentUri: String,
        range: McTextRange,
        source: String,
    ): McReferenceLocation =
        McReferenceLocation(
            documentUri = documentUri,
            range = range,
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
