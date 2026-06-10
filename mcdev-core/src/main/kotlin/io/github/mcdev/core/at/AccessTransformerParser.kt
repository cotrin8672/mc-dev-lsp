package io.github.mcdev.core.at

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseMethodDescriptor

object AccessTransformerParser {
    fun parse(text: String): AccessTransformerParseResult {
        val entries = mutableListOf<AccessTransformerEntry>()
        text.lineSequence().forEachIndexed { index, rawLine ->
            val lineNumber = index + 1
            val line = rawLine.substringBefore('#').trim()
            if (line.isEmpty()) return@forEachIndexed
            val parts = line.split(Regex("\\s+"))
            val modifier = AccessTransformerModifier.entries.firstOrNull { it.token == parts.getOrNull(0) }
                ?: return AccessTransformerParseResult.Failure(lineNumber, "invalid access transformer modifier")
            if (parts.size !in 2..3) {
                return AccessTransformerParseResult.Failure(lineNumber, "entry must contain modifier, owner, and optional member")
            }
            val owner = parts[1]
            val member = parts.getOrNull(2)
            if (member == null) {
                entries += AccessTransformerEntry(modifier, owner, line = lineNumber)
            } else {
                val methodStart = member.indexOf('(')
                if (methodStart >= 0) {
                    val name = member.substring(0, methodStart)
                    val descriptor = member.substring(methodStart)
                    if (parseMethodDescriptor(descriptor) is DescriptorParseResult.Failure) {
                        return AccessTransformerParseResult.Failure(lineNumber, "invalid method descriptor")
                    }
                    entries += AccessTransformerEntry(modifier, owner, name, descriptor, lineNumber)
                } else {
                    entries += AccessTransformerEntry(modifier, owner, member, null, lineNumber)
                }
            }
        }
        return AccessTransformerParseResult.Success(AccessTransformerFile(entries))
    }
}
