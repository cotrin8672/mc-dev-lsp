package io.github.mcdev.core.at

enum class AtSlot {
    MODIFIER,
    OWNER,
    MEMBER_NAME,
    MEMBER_DESCRIPTOR,
}

data class AtContext(
    val slot: AtSlot,
    val partialValue: String,
    val lineNumber: Int,
    val lineStartOffset: Int,
    val lineEndOffset: Int,
    val valueStartOffset: Int,
    val valueEndOffset: Int,
    val modifier: AccessTransformerModifier?,
    val owner: String?,
    val memberName: String?,
    val memberDescriptor: String?,
)

object AtContextExtractor {
    fun extract(source: String, line: Int, character: Int): AtContext? {
        val offset = toOffset(source, line, character) ?: return null
        return extractAtOffset(source, offset)
    }

    fun toOffset(source: String, line: Int, character: Int): Int? =
        AtTextPositions.toOffset(source, line, character)

    fun extractAtOffset(source: String, offset: Int): AtContext? {
        if (offset < 0 || offset > source.length) return null
        val lineContext = findLineAtOffset(source, offset) ?: return null
        if (lineContext.content.isEmpty()) {
            return buildContext(
                lineContext = lineContext,
                slot = AtSlot.MODIFIER,
                partialValue = "",
                valueStart = lineContext.lineStartOffset,
                valueEnd = offset.coerceAtMost(lineContext.lineEndOffset),
                modifier = null,
                owner = null,
                memberName = null,
                memberDescriptor = null,
            )
        }

        val slot = detectSlot(lineContext, offset)
        val memberParts = lineContext.member?.let { splitMemberToken(it) }
        return buildContext(
            lineContext = lineContext,
            slot = slot,
            partialValue = partialValueForSlot(lineContext, slot, offset, memberParts),
            valueStart = valueStartForSlot(lineContext, slot, memberParts) ?: lineContext.lineStartOffset,
            valueEnd = valueEndForSlot(lineContext, slot, offset, memberParts),
            modifier = lineContext.modifier?.text?.let { token ->
                AccessTransformerModifier.entries.firstOrNull { it.token == token }
            },
            owner = lineContext.owner?.text,
            memberName = memberParts?.name,
            memberDescriptor = memberParts?.descriptor,
        )
    }

    fun parseLine(source: String, lineNumber: Int): AtLineContext? {
        var currentLine = 1
        var index = 0
        while (index <= source.length) {
            if (currentLine == lineNumber) {
                val rawEnd = source.indexOf('\n', index).let { if (it < 0) source.length else it }
                val rawLine = source.substring(index, rawEnd)
                val content = rawLine.substringBefore('#').trimEnd()
                val contentEnd = index + rawLine.substringBefore('#').length
                return buildLineContext(lineNumber, index, contentEnd, content)
            }
            val nextNewline = source.indexOf('\n', index)
            if (nextNewline < 0) return null
            index = nextNewline + 1
            currentLine++
        }
        return null
    }

    private fun findLineAtOffset(source: String, cursorOffset: Int): AtLineContext? {
        var lineNumber = 1
        var index = 0
        while (index <= source.length) {
            val rawEnd = source.indexOf('\n', index).let { if (it < 0) source.length else it }
            if (cursorOffset <= rawEnd) {
                val rawLine = source.substring(index, rawEnd)
                val content = rawLine.substringBefore('#').trimEnd()
                val contentEnd = index + rawLine.substringBefore('#').length
                return buildLineContext(lineNumber, index, contentEnd, content)
            }
            index = rawEnd + 1
            lineNumber++
        }
        return null
    }

    private fun buildLineContext(
        lineNumber: Int,
        lineStartOffset: Int,
        lineEndOffset: Int,
        content: String,
    ): AtLineContext {
        val trimmed = content.trim()
        if (trimmed.isEmpty()) {
            return AtLineContext(lineNumber, lineStartOffset, lineEndOffset, "", null, null, null)
        }
        val leadingSpaces = content.indexOf(trimmed)
        val tokens = mutableListOf<AtLineToken>()
        var searchFrom = leadingSpaces
        val baseOffset = lineStartOffset
        while (searchFrom < content.length) {
            while (searchFrom < content.length && content[searchFrom].isWhitespace()) searchFrom++
            if (searchFrom >= content.length) break
            var end = searchFrom
            while (end < content.length && !content[end].isWhitespace()) end++
            val text = content.substring(searchFrom, end)
            tokens += AtLineToken(
                text = text,
                startOffset = baseOffset + searchFrom,
                endOffset = baseOffset + end,
            )
            searchFrom = end
        }
        return AtLineContext(
            lineNumber = lineNumber,
            lineStartOffset = lineStartOffset,
            lineEndOffset = lineEndOffset,
            content = content,
            modifier = tokens.getOrNull(0),
            owner = tokens.getOrNull(1),
            member = tokens.getOrNull(2),
        )
    }

    private fun detectSlot(lineContext: AtLineContext, cursorOffset: Int): AtSlot {
        val contentEnd = lineContext.lineStartOffset + lineContext.content.length
        if (cursorOffset > contentEnd) {
            return when {
                lineContext.modifier == null -> AtSlot.MODIFIER
                lineContext.owner == null -> AtSlot.OWNER
                lineContext.member == null -> AtSlot.MEMBER_NAME
                else -> {
                    val memberParts = splitMemberToken(lineContext.member!!)
                    if (memberParts.descriptorStart == null) AtSlot.MEMBER_DESCRIPTOR else AtSlot.MEMBER_NAME
                }
            }
        }

        lineContext.modifier?.let { modifier ->
            if (cursorOffset <= modifier.endOffset || cursorInToken(modifier, cursorOffset)) {
                return AtSlot.MODIFIER
            }
        } ?: return AtSlot.MODIFIER

        lineContext.owner?.let { owner ->
            if (cursorOffset <= owner.endOffset || cursorInToken(owner, cursorOffset)) {
                return AtSlot.OWNER
            }
        } ?: return AtSlot.OWNER

        lineContext.member?.let { member ->
            val parts = splitMemberToken(member)
            if (parts.descriptorStart != null && cursorOffset >= parts.descriptorStart) {
                return AtSlot.MEMBER_DESCRIPTOR
            }
            return AtSlot.MEMBER_NAME
        }
        return AtSlot.MEMBER_NAME
    }

    private fun cursorInToken(token: AtLineToken, cursorOffset: Int): Boolean =
        cursorOffset in token.startOffset until token.endOffset

    private data class MemberParts(
        val name: String,
        val descriptor: String?,
        val descriptorStart: Int?,
    )

    private fun splitMemberToken(token: AtLineToken): MemberParts {
        val openParen = token.text.indexOf('(')
        return if (openParen >= 0) {
            MemberParts(
                name = token.text.substring(0, openParen),
                descriptor = token.text.substring(openParen),
                descriptorStart = token.startOffset + openParen,
            )
        } else {
            MemberParts(name = token.text, descriptor = null, descriptorStart = null)
        }
    }

    private fun partialValueForSlot(
        lineContext: AtLineContext,
        slot: AtSlot,
        cursorOffset: Int,
        memberParts: MemberParts?,
    ): String {
        val token = tokenForSlot(lineContext, slot) ?: return ""
        return when (slot) {
            AtSlot.MODIFIER, AtSlot.OWNER ->
                token.text.substring(0, (cursorOffset - token.startOffset).coerceIn(0, token.text.length))
            AtSlot.MEMBER_NAME -> {
                val nameEnd = memberParts?.descriptorStart ?: token.endOffset
                val relative = (cursorOffset - token.startOffset).coerceIn(0, nameEnd - token.startOffset)
                token.text.substring(0, relative)
            }
            AtSlot.MEMBER_DESCRIPTOR -> {
                val descriptorStart = memberParts?.descriptorStart ?: return ""
                token.text.substring(0, (cursorOffset - descriptorStart).coerceIn(0, token.text.length - (descriptorStart - token.startOffset)))
            }
        }
    }

    private fun tokenForSlot(lineContext: AtLineContext, slot: AtSlot): AtLineToken? = when (slot) {
        AtSlot.MODIFIER -> lineContext.modifier
        AtSlot.OWNER -> lineContext.owner
        AtSlot.MEMBER_NAME, AtSlot.MEMBER_DESCRIPTOR -> lineContext.member
    }

    private fun valueStartForSlot(
        lineContext: AtLineContext,
        slot: AtSlot,
        memberParts: MemberParts?,
    ): Int? = when (slot) {
        AtSlot.MODIFIER -> lineContext.modifier?.startOffset
        AtSlot.OWNER -> lineContext.owner?.startOffset
        AtSlot.MEMBER_NAME -> lineContext.member?.startOffset
        AtSlot.MEMBER_DESCRIPTOR -> memberParts?.descriptorStart
    }

    private fun valueEndForSlot(
        lineContext: AtLineContext,
        slot: AtSlot,
        cursorOffset: Int,
        memberParts: MemberParts?,
    ): Int = when (slot) {
        AtSlot.MEMBER_DESCRIPTOR -> memberParts?.descriptorStart?.let { start ->
            val token = lineContext.member ?: return cursorOffset
            start + (cursorOffset - start).coerceAtMost(token.endOffset - start)
        } ?: cursorOffset
        else -> cursorOffset
    }

    private fun buildContext(
        lineContext: AtLineContext,
        slot: AtSlot,
        partialValue: String,
        valueStart: Int,
        valueEnd: Int,
        modifier: AccessTransformerModifier?,
        owner: String?,
        memberName: String?,
        memberDescriptor: String?,
    ): AtContext = AtContext(
        slot = slot,
        partialValue = partialValue,
        lineNumber = lineContext.lineNumber,
        lineStartOffset = lineContext.lineStartOffset,
        lineEndOffset = lineContext.lineEndOffset,
        valueStartOffset = valueStart,
        valueEndOffset = valueEnd,
        modifier = modifier,
        owner = owner,
        memberName = memberName,
        memberDescriptor = memberDescriptor,
    )
}

data class AtLineToken(
    val text: String,
    val startOffset: Int,
    val endOffset: Int,
)

data class AtLineContext(
    val lineNumber: Int,
    val lineStartOffset: Int,
    val lineEndOffset: Int,
    val content: String,
    val modifier: AtLineToken?,
    val owner: AtLineToken?,
    val member: AtLineToken?,
)
