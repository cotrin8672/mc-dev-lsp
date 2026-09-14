package io.github.mcdev.jdtls.mixin

typealias UnresolvedErasureNameResolver = (erasureName: String) -> String?

object JdtTypeSignatureConverter {
    private const val OBJECT_DESCRIPTOR = "Ljava/lang/Object;"

    fun toJvmDescriptorOrNull(
        jdtTypeSignature: String,
        resolveErasureName: UnresolvedErasureNameResolver = { null },
    ): String? {
        if (jdtTypeSignature.isEmpty()) {
            return null
        }
        return Parser(jdtTypeSignature, resolveErasureName).parseType()
    }

    private class Parser(
        private val signature: String,
        private val resolveErasureName: UnresolvedErasureNameResolver,
    ) {
        private var index = 0

        fun parseType(): String? {
            skipLeadingFormalTypeParameters()
            if (index >= signature.length) {
                return null
            }
            return when (signature[index]) {
                in PRIMITIVE_DESCRIPTORS -> primitiveDescriptor()
                '[' -> arrayDescriptor()
                'L' -> classDescriptor(resolved = true)
                'Q' -> classDescriptor(resolved = false)
                'T' -> typeVariableDescriptor()
                '*' -> wildcardDescriptor()
                '+', '-' -> boundedWildcardDescriptor()
                '!' -> captureDescriptor()
                '|' -> intersectionDescriptor()
                else -> null
            }
        }

        private fun primitiveDescriptor(): String {
            val descriptor = signature[index].toString()
            index++
            return descriptor
        }

        private fun arrayDescriptor(): String? {
            index++
            val element = parseType() ?: return null
            return "[$element"
        }

        private fun classDescriptor(resolved: Boolean): String? {
            index++
            val erasureName = readErasureName() ?: return null
            if (index >= signature.length || signature[index] != ';') {
                return null
            }
            index++
            val internalName = if (resolved) {
                qualifiedNameToInternal(erasureName)
            } else {
                resolveErasureName(erasureName) ?: return null
            }
            return "L$internalName;"
        }

        private fun readErasureName(): String? {
            val first = readIdentifier() ?: return null
            val segments = mutableListOf(first)
            while (index < signature.length && signature[index] != ';') {
                when (signature[index]) {
                    '.', '/' -> {
                        index++
                        val segment = readIdentifier() ?: return null
                        segments += segment
                    }
                    '<' -> {
                        if (!skipBalanced('<', '>')) {
                            return null
                        }
                    }
                    else -> return null
                }
            }
            return segments.joinToString(".")
        }

        private fun typeVariableDescriptor(): String? {
            index++
            while (index < signature.length && signature[index] != ';') {
                index++
            }
            if (index >= signature.length || signature[index] != ';') {
                return null
            }
            index++
            return OBJECT_DESCRIPTOR
        }

        private fun wildcardDescriptor(): String {
            index++
            return OBJECT_DESCRIPTOR
        }

        private fun boundedWildcardDescriptor(): String? {
            val boundKind = signature[index++]
            val erased = parseType() ?: return null
            return if (boundKind == '+') erased else OBJECT_DESCRIPTOR
        }

        private fun captureDescriptor(): String? {
            index++
            return parseType()
        }

        private fun intersectionDescriptor(): String? {
            index++
            val first = parseType() ?: return null
            while (index < signature.length && signature[index] == ':') {
                index++
                if (parseType() == null) {
                    return null
                }
            }
            return first
        }

        private fun skipLeadingFormalTypeParameters() {
            if (index >= signature.length || signature[index] != '<') {
                return
            }
            if (!looksLikeFormalTypeParameters()) {
                return
            }
            skipBalanced('<', '>')
        }

        private fun looksLikeFormalTypeParameters(): Boolean {
            var cursor = index + 1
            while (cursor < signature.length && signature[cursor] != '>' && signature[cursor] != ':') {
                cursor++
            }
            return cursor < signature.length && signature[cursor] == ':'
        }

        private fun readIdentifier(): String? {
            val start = index
            while (index < signature.length && isIdentifierPart(signature[index])) {
                index++
            }
            if (start >= index) {
                return null
            }
            return signature.substring(start, index)
        }

        private fun skipBalanced(open: Char, close: Char): Boolean {
            if (index >= signature.length || signature[index] != open) {
                return false
            }
            index++
            var depth = 1
            while (index < signature.length && depth > 0) {
                when (signature[index]) {
                    open -> depth++
                    close -> depth--
                }
                index++
            }
            return depth == 0
        }
    }

    private fun qualifiedNameToInternal(qualifiedName: String): String =
        qualifiedName.replace('.', '/')

    private fun isIdentifierPart(character: Char): Boolean =
        character.isLetterOrDigit() || character == '_' || character == '$'

    private val PRIMITIVE_DESCRIPTORS = setOf('B', 'C', 'D', 'F', 'I', 'J', 'S', 'V', 'Z')
}
