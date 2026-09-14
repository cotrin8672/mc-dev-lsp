package io.github.mcdev.core.mixinextras

enum class OfficialExpressionMemberKind {
    FIELD,
    METHOD,
}

sealed class OfficialExpressionMemberCandidate {
    abstract val kind: OfficialExpressionMemberKind
    abstract val ownerInternalName: String
    abstract val name: String
    abstract val descriptor: String
    abstract val originalInstructionOpcode: Int
    abstract val originalInstructionIndex: Int
    abstract val decorations: Map<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue>

    data class FieldAccess(
        override val ownerInternalName: String,
        override val name: String,
        override val descriptor: String,
        override val originalInstructionOpcode: Int,
        override val originalInstructionIndex: Int,
        override val decorations: Map<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue> = emptyMap(),
    ) : OfficialExpressionMemberCandidate() {
        override val kind: OfficialExpressionMemberKind = OfficialExpressionMemberKind.FIELD
    }

    data class MethodInvocation(
        override val ownerInternalName: String,
        override val name: String,
        override val descriptor: String,
        val isInterface: Boolean,
        override val originalInstructionOpcode: Int,
        override val originalInstructionIndex: Int,
        override val decorations: Map<OfficialExpressionMatchDecorationKey, OfficialExpressionMatchDecorationValue> = emptyMap(),
    ) : OfficialExpressionMemberCandidate() {
        override val kind: OfficialExpressionMemberKind = OfficialExpressionMemberKind.METHOD
    }
}

sealed interface OfficialExpressionMemberCompletionResult {
    data class Available(val candidates: List<OfficialExpressionMemberCandidate>) : OfficialExpressionMemberCompletionResult

    data class Unavailable(
        val reason: String,
        val cause: Throwable? = null,
    ) : OfficialExpressionMemberCompletionResult
}

internal data class OfficialExpressionMemberCompletionProbe(
    val kind: OfficialExpressionMemberKind,
    val source: String,
)

internal object OfficialExpressionMemberCompletionProbes {
    private val preservedExpressionKeywords = setOf(
        "new", "true", "false", "null", "this", "super", "instanceof", "return", "throw",
    )

    fun build(
        receiverExpression: String,
        identifierPool: OfficialExpressionIdentifierPool = OfficialExpressionIdentifierPool.EMPTY,
    ): List<OfficialExpressionMemberCompletionProbe>? {
        val receiver = receiverExpression.trim()
        if (!ExpressionReceiverParser.isValidReceiverExpression(receiver)) {
            return null
        }
        val probeReceiver = replaceUnresolvedNamesWithWildcards(receiver, identifierPool)
        if (receiver == "super") {
            return listOf(
                OfficialExpressionMemberCompletionProbe(
                    kind = OfficialExpressionMemberKind.METHOD,
                    source = "@(super.?())",
                ),
            )
        }
        val groupedReceiver = "($probeReceiver)"
        return listOf(
            OfficialExpressionMemberCompletionProbe(
                kind = OfficialExpressionMemberKind.FIELD,
                source = "@($groupedReceiver.?)",
            ),
            OfficialExpressionMemberCompletionProbe(
                kind = OfficialExpressionMemberKind.METHOD,
                source = "@($groupedReceiver.?())",
            ),
        )
    }

    /**
     * Keep the receiver's operators and grouping intact, but use MixinExtras' wildcard for
     * names that are not declared in the identifier pool. The official matcher then binds
     * each wildcard to an actual bytecode value or member, rather than inventing a member.
     */
    private fun replaceUnresolvedNamesWithWildcards(
        receiverExpression: String,
        identifierPool: OfficialExpressionIdentifierPool,
    ): String {
        val source = receiverExpression.trim()
        val result = StringBuilder(source.length)
        var index = 0
        while (index < source.length) {
            when {
                source[index] == '\'' || source[index] == '"' -> {
                    val end = quotedLiteralEnd(source, index)
                    if (end == null) {
                        result.append(source[index++])
                    } else {
                        result.append(source, index, end)
                        index = end
                    }
                }
                source[index] == '/' && index + 1 < source.length && source[index + 1] == '/' -> {
                    val end = source.indexOf('\n', index + 2).let { if (it < 0) source.length else it }
                    result.append(source, index, end)
                    index = end
                }
                source[index] == '/' && index + 1 < source.length && source[index + 1] == '*' -> {
                    val close = source.indexOf("*/", index + 2)
                    val end = if (close < 0) source.length else close + 2
                    result.append(source, index, end)
                    index = end
                }
                source[index].isJavaIdentifierStart() -> {
                    var end = index + 1
                    while (end < source.length && source[end].isJavaIdentifierPart()) end++
                    val name = source.substring(index, end)
                    if (name in preservedExpressionKeywords ||
                        identifierPool.identifierExists(name)
                    ) {
                        result.append(name)
                    } else {
                        result.append('?')
                    }
                    index = end
                }
                else -> {
                    result.append(source[index])
                    index++
                }
            }
        }
        return result.toString()
    }

    private fun OfficialExpressionIdentifierPool.identifierExists(name: String): Boolean =
        delegate.memberExists(name) || delegate.typeExists(name)

    private fun quotedLiteralEnd(source: String, start: Int): Int? {
        val quote = source[start]
        var index = start + 1
        while (index < source.length) {
            when {
                source[index] == '\\' -> index += 2
                source[index] == quote -> return index + 1
                else -> index++
            }
        }
        return null
    }

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'
}
