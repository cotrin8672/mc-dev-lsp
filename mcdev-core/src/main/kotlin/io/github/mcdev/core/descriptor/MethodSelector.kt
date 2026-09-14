package io.github.mcdev.core.descriptor

sealed interface Pattern<out T> {
    data object Any : Pattern<Nothing>

    data class Exact<T>(val value: T) : Pattern<T>
}

data class MethodSelector(
    val owner: Pattern<String>,
    val name: Pattern<String>,
    val descriptor: Pattern<MethodDescriptor>,
)

fun parseMethodSelector(input: String): DescriptorParseResult<MethodSelector> =
    MethodSelectorParser.parse(input.replace(" ", ""))

private object MethodSelectorParser {
    fun parse(input: String): DescriptorParseResult<MethodSelector> {
        if (input.isEmpty()) {
            return failure("empty method selector", 0)
        }
        if (input == "*") {
            return success(
                MethodSelector(
                    owner = Pattern.Any,
                    name = Pattern.Any,
                    descriptor = Pattern.Any,
                ),
            )
        }
        return if (usesInternalOwnerForm(input, kindDelimiter = '(')) {
            parseInternalOwnerForm(input)
        } else {
            parseQualifiedOrBareForm(input)
        }
    }

    private fun usesInternalOwnerForm(input: String, kindDelimiter: Char): Boolean {
        if (!input.startsWith("L")) {
            return false
        }
        val semicolon = input.indexOf(';')
        if (semicolon <= 0) {
            return false
        }
        val kindStart = input.indexOf(kindDelimiter)
        return kindStart < 0 || semicolon < kindStart
    }

    private fun parseInternalOwnerForm(input: String): DescriptorParseResult<MethodSelector> {
        val ownerEnd = input.indexOf(';')
        if (ownerEnd <= 1) {
            return failure("missing owner internal name", 1)
        }
        val owner = input.substring(1, ownerEnd)
        validateInternalOwner(owner, offset = 1)?.let { return it }

        val memberStart = ownerEnd + 1
        if (memberStart >= input.length) {
            return success(
                MethodSelector(
                    owner = Pattern.Exact(owner),
                    name = Pattern.Any,
                    descriptor = Pattern.Any,
                ),
            )
        }
        return parseMember(
            owner = Pattern.Exact(owner),
            member = input.substring(memberStart),
            memberOffset = memberStart,
        )
    }

    private fun parseQualifiedOrBareForm(input: String): DescriptorParseResult<MethodSelector> {
        val descriptorStart = input.indexOf('(')
        val ownerDot = findOwnerMemberDelimiter(input, descriptorStart)
        if (ownerDot < 0) {
            return parseMember(
                owner = Pattern.Any,
                member = input,
                memberOffset = 0,
            )
        }
        val ownerText = input.substring(0, ownerDot)
        validateQualifiedOwner(ownerText, offset = 0)?.let { return it }
        val owner = normalizeQualifiedOwner(ownerText)
        return parseMember(
            owner = Pattern.Exact(owner),
            member = input.substring(ownerDot + 1),
            memberOffset = ownerDot + 1,
        )
    }

    private fun findOwnerMemberDelimiter(input: String, descriptorStart: Int): Int {
        val searchBefore = if (descriptorStart >= 0) descriptorStart else input.length
        val lastDot = input.lastIndexOf('.', searchBefore)
        return if (lastDot >= 0) lastDot else -1
    }

    private fun parseMember(
        owner: Pattern<String>,
        member: String,
        memberOffset: Int,
    ): DescriptorParseResult<MethodSelector> {
        if (member.contains(':')) {
            return failure("field selector syntax is not supported for methods", memberOffset + member.indexOf(':'))
        }

        val descriptorStart = member.indexOf('(')
        return if (descriptorStart >= 0) {
            parseMemberWithDescriptor(owner, member, memberOffset, descriptorStart)
        } else {
            parseMemberWithoutDescriptor(owner, member, memberOffset)
        }
    }

    private fun parseMemberWithDescriptor(
        owner: Pattern<String>,
        member: String,
        memberOffset: Int,
        descriptorStart: Int,
    ): DescriptorParseResult<MethodSelector> {
        val nameText = member.substring(0, descriptorStart)
        if (nameText.isEmpty()) {
            return failure("missing method name", memberOffset)
        }
        if (nameText.endsWith('*') && nameText != "*") {
            return failure(
                "malformed method name wildcard",
                memberOffset + nameText.lastIndexOf('*'),
            )
        }
        validateMethodName(nameText, memberOffset)?.let { return it }

        val descriptorText = member.substring(descriptorStart)
        val descriptorOffset = memberOffset + descriptorStart
        val parsedDescriptor = when (val parsed = parseMethodDescriptor(descriptorText)) {
            is DescriptorParseResult.Success -> parsed.value
            is DescriptorParseResult.Failure -> return parsed
        }
        validateSpecialMethodDescriptor(nameText, parsedDescriptor, descriptorOffset)?.let { return it }

        return success(
            MethodSelector(
                owner = owner,
                name = namePattern(nameText),
                descriptor = Pattern.Exact(parsedDescriptor),
            ),
        )
    }

    private fun parseMemberWithoutDescriptor(
        owner: Pattern<String>,
        member: String,
        memberOffset: Int,
    ): DescriptorParseResult<MethodSelector> {
        if (member.isEmpty()) {
            return failure("missing method name", memberOffset)
        }
        validateMethodName(member, memberOffset)?.let { return it }
        return success(
            MethodSelector(
                owner = owner,
                name = namePattern(member),
                descriptor = Pattern.Any,
            ),
        )
    }

    private fun namePattern(nameText: String): Pattern<String> = when {
        nameText == "*" -> Pattern.Any
        nameText.endsWith('*') -> Pattern.Exact(nameText.dropLast(1))
        else -> Pattern.Exact(nameText)
    }

    private fun validateMethodName(name: String, offset: Int): DescriptorParseResult.Failure? {
        if (name == "<init>" || name == "<clinit>") {
            return null
        }
        if (name == "*") {
            return null
        }
        if (name.endsWith('*')) {
            if (name.length == 1) {
                return failure("malformed method name wildcard", offset)
            }
            if (name.indexOf('*') != name.lastIndex) {
                return failure("malformed method name wildcard", offset + name.indexOf('*'))
            }
            val prefix = name.dropLast(1)
            if (prefix == "<init>" || prefix == "<clinit>") {
                return null
            }
            if (prefix.isEmpty() || !isValidMethodName(prefix)) {
                return failure("invalid method name prefix", offset)
            }
            return null
        }
        if (name.contains('*')) {
            return failure("malformed method name wildcard", offset + name.indexOf('*'))
        }
        if (!isValidMethodName(name)) {
            return failure("invalid method name", offset)
        }
        return null
    }

    private fun validateSpecialMethodDescriptor(
        name: String,
        descriptor: MethodDescriptor,
        offset: Int,
    ): DescriptorParseResult.Failure? {
        if (name == "<init>" && descriptor.returnType != JvmType.VoidType) {
            return failure("constructor descriptor must return void", offset)
        }
        if (name == "<clinit>" && descriptor != MethodDescriptor(emptyList(), JvmType.VoidType)) {
            return failure("class initializer descriptor must be ()V", offset)
        }
        return null
    }

    private fun validateInternalOwner(owner: String, offset: Int): DescriptorParseResult.Failure? {
        if (owner.isEmpty()) {
            return failure("empty owner internal name", offset)
        }
        if (owner.contains('.')) {
            return failure("owner internal name must use slash separators", offset)
        }
        if (owner.contains('*')) {
            return failure("wildcards are not supported in owner", offset + owner.indexOf('*'))
        }
        return validateOwnerSegments(owner.split('/'), offset)
    }

    private fun validateQualifiedOwner(owner: String, offset: Int): DescriptorParseResult.Failure? {
        if (owner.isEmpty()) {
            return failure("empty owner name", offset)
        }
        if (owner.contains('*')) {
            return failure("wildcards are not supported in owner", offset + owner.indexOf('*'))
        }
        val hasDot = owner.contains('.')
        val hasSlash = owner.contains('/')
        if (hasDot && hasSlash) {
            return failure("owner name must not mix dot and slash separators", offset)
        }
        val segments = if (hasSlash) owner.split('/') else owner.split('.')
        return validateOwnerSegments(segments, offset)
    }

    private fun normalizeQualifiedOwner(owner: String): String =
        if (owner.contains('/')) owner else owner.replace('.', '/')

    private fun validateOwnerSegments(segments: List<String>, offset: Int): DescriptorParseResult.Failure? {
        for (segment in segments) {
            if (segment.isEmpty()) {
                return failure("empty owner segment", offset)
            }
            if (!isValidOwnerSegment(segment)) {
                return failure("invalid owner segment", offset)
            }
        }
        return null
    }

    private fun isValidOwnerSegment(segment: String): Boolean {
        if (segment.isEmpty()) return false
        if (!(segment[0].isLetter() || segment[0] == '_' || segment[0] == '$')) {
            return false
        }
        return segment.all { it.isLetterOrDigit() || it == '_' || it == '$' }
    }

    private fun isMethodNameChar(ch: Char): Boolean =
        ch.isLetterOrDigit() || ch == '_' || ch == '$'

    private fun isValidMethodName(name: String): Boolean =
        name.isNotEmpty() &&
            isValidMethodNameStart(name[0]) &&
            name.all(::isMethodNameChar)

    private fun isValidMethodNameStart(ch: Char): Boolean =
        ch.isLetter() || ch == '_' || ch == '$'

    private fun success(value: MethodSelector): DescriptorParseResult<MethodSelector> =
        DescriptorParseResult.Success(value)

    private fun failure(message: String, offset: Int): DescriptorParseResult.Failure =
        DescriptorParseResult.Failure(
            DescriptorParseError(message, offset.coerceAtMost(Int.MAX_VALUE).coerceAtLeast(0)),
        )
}
