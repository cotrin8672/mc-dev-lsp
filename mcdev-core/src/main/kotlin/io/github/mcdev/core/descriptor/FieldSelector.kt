package io.github.mcdev.core.descriptor

data class FieldSelector(
    val owner: Pattern<String>,
    val name: Pattern<String>,
    val descriptor: Pattern<JvmType>,
)

fun parseFieldSelector(input: String): DescriptorParseResult<FieldSelector> =
    FieldSelectorParser.parse(input.replace(" ", ""))

private object FieldSelectorParser {
    fun parse(input: String): DescriptorParseResult<FieldSelector> {
        if (input.isEmpty()) {
            return failure("empty field selector", 0)
        }
        if (input == "*") {
            return success(
                FieldSelector(
                    owner = Pattern.Any,
                    name = Pattern.Any,
                    descriptor = Pattern.Any,
                ),
            )
        }
        return if (usesInternalOwnerForm(input, kindDelimiter = ':')) {
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

    private fun parseInternalOwnerForm(input: String): DescriptorParseResult<FieldSelector> {
        val ownerEnd = input.indexOf(';')
        if (ownerEnd <= 1) {
            return failure("missing owner internal name", 1)
        }
        val owner = input.substring(1, ownerEnd)
        validateInternalOwner(owner, offset = 1)?.let { return it }

        val memberStart = ownerEnd + 1
        if (memberStart >= input.length) {
            return success(
                FieldSelector(
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

    private fun parseQualifiedOrBareForm(input: String): DescriptorParseResult<FieldSelector> {
        val descriptorStart = input.indexOf(':')
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
    ): DescriptorParseResult<FieldSelector> {
        if (member.contains('(')) {
            return failure(
                "method selector syntax is not supported for fields",
                memberOffset + member.indexOf('('),
            )
        }

        val descriptorStart = member.indexOf(':')
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
    ): DescriptorParseResult<FieldSelector> {
        val nameText = member.substring(0, descriptorStart)
        if (nameText.isEmpty()) {
            return failure("missing field name", memberOffset)
        }
        if (nameText.endsWith('*') && nameText != "*") {
            return failure(
                "malformed field name wildcard",
                memberOffset + nameText.lastIndexOf('*'),
            )
        }
        validateFieldName(nameText, memberOffset)?.let { return it }

        val descriptorText = member.substring(descriptorStart + 1)
        val descriptorOffset = memberOffset + descriptorStart + 1
        if (descriptorText.isEmpty()) {
            return failure("missing field descriptor", descriptorOffset)
        }
        val parsedDescriptor = when (val parsed = parseFieldDescriptor(descriptorText)) {
            is DescriptorParseResult.Success -> parsed.value
            is DescriptorParseResult.Failure -> return parsed
        }

        return success(
            FieldSelector(
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
    ): DescriptorParseResult<FieldSelector> {
        if (member.isEmpty()) {
            return failure("missing field name", memberOffset)
        }
        validateFieldName(member, memberOffset)?.let { return it }
        return success(
            FieldSelector(
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

    private fun validateFieldName(name: String, offset: Int): DescriptorParseResult.Failure? {
        if (name == "<init>" || name == "<clinit>") {
            return failure("invalid field name", offset)
        }
        if (name == "*") {
            return null
        }
        if (name.endsWith('*')) {
            if (name.length == 1) {
                return failure("malformed field name wildcard", offset)
            }
            if (name.indexOf('*') != name.lastIndex) {
                return failure("malformed field name wildcard", offset + name.indexOf('*'))
            }
            val prefix = name.dropLast(1)
            if (prefix.isEmpty() || !isValidFieldName(prefix)) {
                return failure("invalid field name prefix", offset)
            }
            return null
        }
        if (name.contains('*')) {
            return failure("malformed field name wildcard", offset + name.indexOf('*'))
        }
        if (!isValidFieldName(name)) {
            return failure("invalid field name", offset)
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

    private fun isFieldNameChar(ch: Char): Boolean =
        ch.isLetterOrDigit() || ch == '_' || ch == '$'

    private fun isValidFieldName(name: String): Boolean =
        name.isNotEmpty() &&
            isValidFieldNameStart(name[0]) &&
            name.all(::isFieldNameChar)

    private fun isValidFieldNameStart(ch: Char): Boolean =
        ch.isLetter() || ch == '_' || ch == '$'

    private fun success(value: FieldSelector): DescriptorParseResult<FieldSelector> =
        DescriptorParseResult.Success(value)

    private fun failure(message: String, offset: Int): DescriptorParseResult.Failure =
        DescriptorParseResult.Failure(
            DescriptorParseError(message, offset.coerceAtMost(Int.MAX_VALUE).coerceAtLeast(0)),
        )
}
