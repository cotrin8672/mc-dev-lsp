package io.github.mcdev.core.descriptor

object MemberTargetParser {
    fun parse(input: String): DescriptorParseResult<MemberTarget> {
        if (!input.startsWith("L")) {
            return DescriptorParseResult.Failure(DescriptorParseError("target must start with owner marker 'L'", 0))
        }
        val ownerEnd = input.indexOf(';')
        if (ownerEnd <= 1) {
            return DescriptorParseResult.Failure(DescriptorParseError("missing owner internal name", 1))
        }
        val owner = input.substring(1, ownerEnd)
        val memberStart = ownerEnd + 1
        if (memberStart >= input.length) {
            return DescriptorParseResult.Failure(DescriptorParseError("missing member name", memberStart))
        }
        val methodStart = input.indexOf('(', startIndex = memberStart)
        val fieldStart = input.indexOf(':', startIndex = memberStart)
        return when {
            methodStart > memberStart -> parseMethod(owner, input.substring(memberStart, methodStart), input.substring(methodStart))
            fieldStart > memberStart -> parseField(owner, input.substring(memberStart, fieldStart), input.substring(fieldStart + 1))
            else -> DescriptorParseResult.Failure(DescriptorParseError("missing member descriptor", memberStart))
        }
    }

    private fun parseMethod(owner: String, name: String, descriptorText: String): DescriptorParseResult<MemberTarget> =
        when (val parsed = parseMethodDescriptor(descriptorText)) {
            is DescriptorParseResult.Success -> DescriptorParseResult.Success(MemberTarget.Method(owner, name, parsed.value))
            is DescriptorParseResult.Failure -> parsed
        }

    private fun parseField(owner: String, name: String, descriptorText: String): DescriptorParseResult<MemberTarget> =
        when (val parsed = parseFieldDescriptor(descriptorText)) {
            is DescriptorParseResult.Success -> DescriptorParseResult.Success(MemberTarget.Field(owner, name, parsed.value))
            is DescriptorParseResult.Failure -> parsed
        }
}
