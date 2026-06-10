package io.github.mcdev.core.aw

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.mapping.parseNamespace

object AccessWidenerParser {
    fun parse(text: String): AccessWidenerParseResult {
        val lines = text.lineSequence().toList()
        if (lines.isEmpty()) return AccessWidenerParseResult.Failure(1, "empty access widener")
        val header = lines.first().trim().split(Regex("\\s+"))
        if (header.size != 3 || header[0] != "accessWidener" || header[1] != "v2") {
            return AccessWidenerParseResult.Failure(1, "expected accessWidener v2 header")
        }
        val namespace = parseNamespace(header[2])
            ?: return AccessWidenerParseResult.Failure(1, "unknown access widener namespace '${header[2]}'")
        val entries = mutableListOf<AccessWidenerEntry>()

        lines.drop(1).forEachIndexed { index, rawLine ->
            val lineNumber = index + 2
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val parts = line.split(Regex("\\s+"))
            val directive = parseDirective(parts.getOrNull(0))
                ?: return AccessWidenerParseResult.Failure(lineNumber, "invalid access widener directive")
            val kind = parseKind(parts.getOrNull(1))
                ?: return AccessWidenerParseResult.Failure(lineNumber, "invalid access widener kind")
            entries += when (kind) {
                AccessWidenerKind.CLASS -> {
                    if (parts.size != 3) return AccessWidenerParseResult.Failure(lineNumber, "class entry must contain owner")
                    AccessWidenerEntry(directive, kind, parts[2], line = lineNumber)
                }
                AccessWidenerKind.METHOD -> {
                    if (parts.size != 5) return AccessWidenerParseResult.Failure(lineNumber, "method entry must contain owner, name, and descriptor")
                    if (parseMethodDescriptor(parts[4]) is DescriptorParseResult.Failure) {
                        return AccessWidenerParseResult.Failure(lineNumber, "invalid method descriptor")
                    }
                    AccessWidenerEntry(directive, kind, parts[2], parts[3], parts[4], lineNumber)
                }
                AccessWidenerKind.FIELD -> {
                    if (parts.size != 5) return AccessWidenerParseResult.Failure(lineNumber, "field entry must contain owner, name, and descriptor")
                    if (parseFieldDescriptor(parts[4]) is DescriptorParseResult.Failure) {
                        return AccessWidenerParseResult.Failure(lineNumber, "invalid field descriptor")
                    }
                    AccessWidenerEntry(directive, kind, parts[2], parts[3], parts[4], lineNumber)
                }
            }
        }

        return AccessWidenerParseResult.Success(AccessWidenerFile(namespace, entries))
    }

    private fun parseDirective(value: String?): AccessWidenerDirective? = when (value) {
        "accessible" -> AccessWidenerDirective.ACCESSIBLE
        "extendable" -> AccessWidenerDirective.EXTENDABLE
        "mutable" -> AccessWidenerDirective.MUTABLE
        "natural" -> AccessWidenerDirective.NATURAL
        else -> null
    }

    private fun parseKind(value: String?): AccessWidenerKind? = when (value) {
        "class" -> AccessWidenerKind.CLASS
        "method" -> AccessWidenerKind.METHOD
        "field" -> AccessWidenerKind.FIELD
        else -> null
    }
}
