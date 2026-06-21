package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

object SemanticCompletionContextExtractor {
    fun extract(
        source: String,
        line: Int,
        character: Int,
        model: MixinClassModel?,
    ): MixinCompletionContext? {
        val semanticModel = model ?: return null
        val offset = toOffset(source, line, character) ?: return null

        semanticModel.targets
            .firstOrNull { offset in sourceRange(source, it.range).expandForOpenString(source) }
            ?.let {
                return MixinCompletionContext.MixinTarget(
                    annotationRange = it.range,
                    partialValue = partialValue(source, offset),
                )
            }

        semanticModel.injectors
            .firstOrNull { offset in sourceRange(source, it.range) }
            ?.let { injector ->
                injector.methodSelectors
                    .firstOrNull { offset in sourceRange(source, it.range).expandForOpenString(source) }
                    ?.let {
                        return MixinCompletionContext.InjectMethod(
                            injector = injector,
                            partialValue = partialValue(source, offset),
                        )
                    }
                injector.atSelectors
                    .firstOrNull { offset in sourceRange(source, it.range) }
                    ?.let { atSelector ->
                        if (atSelector.targetRange != null && offset in sourceRange(source, atSelector.targetRange).expandForOpenString(source)) {
                            val method = injector.methodSelectors.firstOrNull()
                            val parsed = method?.let { parseMethodTarget(it.value) }
                            return MixinCompletionContext.AtTarget(
                                injector = injector,
                                atSelector = atSelector,
                                owner = semanticModel.targets.firstOrNull()?.internalName,
                                methodName = parsed?.name,
                                methodDescriptor = parsed?.descriptor,
                                partialValue = partialValue(source, offset),
                            )
                        }
                        return MixinCompletionContext.AtValue(
                            injector = injector,
                            atSelector = atSelector,
                            partialValue = partialValue(source, offset),
                        )
                    }
            }

        semanticModel.members
            .firstOrNull { offset in sourceRange(source, it.annotationRange).expandForOpenString(source) }
            ?.let { member ->
                val partial = partialValue(source, offset)
                return when (member.annotationKind) {
                    MixinMemberAnnotationKind.ACCESSOR -> MixinCompletionContext.AccessorValue(member, partial)
                    MixinMemberAnnotationKind.INVOKER -> MixinCompletionContext.InvokerValue(member, partial)
                    MixinMemberAnnotationKind.SHADOW -> MixinCompletionContext.ShadowMember(partial)
                    MixinMemberAnnotationKind.OVERWRITE -> null
                }
            }

        return incompleteInjectorContext(source, offset, semanticModel)
    }

    fun toAnnotationContext(
        source: String,
        line: Int,
        character: Int,
        model: MixinClassModel?,
        context: MixinCompletionContext,
    ): AnnotationContext? {
        val offset = toOffset(source, line, character) ?: return null
        val targets = model?.targets?.map { it.internalName }.orEmpty()
        return when (context) {
            is MixinCompletionContext.MixinTarget -> AnnotationContext(
                annotation = MixinAnnotation.MIXIN,
                slot = if (isMixinTargetsStringAttribute(source, offset)) AnnotationSlot.TARGETS else AnnotationSlot.CLASS,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offsetOf(source, context.annotationRange.start),
                annotationEndOffset = offsetOf(source, context.annotationRange.end),
                mixinTargetInternalNames = targets,
            )
            is MixinCompletionContext.InjectMethod -> AnnotationContext(
                annotation = context.injector.annotation,
                slot = AnnotationSlot.METHOD,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offsetOf(source, context.injector.range.start),
                annotationEndOffset = offsetOf(source, context.injector.range.end),
                mixinTargetInternalNames = targets,
                injectMethodName = context.partialValue.trim('"'),
            )
            is MixinCompletionContext.AtValue -> AnnotationContext(
                annotation = MixinAnnotation.AT,
                slot = AnnotationSlot.VALUE,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offsetOf(source, context.atSelector.range.start),
                annotationEndOffset = offsetOf(source, context.atSelector.range.end),
                mixinTargetInternalNames = targets,
                injectMethodName = context.injector.methodSelectors.firstOrNull()?.value,
                atValue = context.partialValue.trim('"'),
            )
            is MixinCompletionContext.AtTarget -> AnnotationContext(
                annotation = MixinAnnotation.AT,
                slot = AnnotationSlot.TARGET,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offsetOf(source, context.atSelector.range.start),
                annotationEndOffset = offsetOf(source, context.atSelector.range.end),
                mixinTargetInternalNames = targets,
                injectMethodName = context.methodName?.let { name -> name + (context.methodDescriptor ?: "") }
                    ?: context.injector.methodSelectors.firstOrNull()?.value,
                atValue = context.atSelector.value,
            )
            is MixinCompletionContext.AccessorValue -> AnnotationContext(
                annotation = MixinAnnotation.ACCESSOR,
                slot = AnnotationSlot.ACCESSOR_VALUE,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offsetOf(source, context.member.annotationRange.start),
                annotationEndOffset = offsetOf(source, context.member.annotationRange.end),
                mixinTargetInternalNames = targets,
            )
            is MixinCompletionContext.InvokerValue -> AnnotationContext(
                annotation = MixinAnnotation.INVOKER,
                slot = AnnotationSlot.INVOKER_VALUE,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offsetOf(source, context.member.annotationRange.start),
                annotationEndOffset = offsetOf(source, context.member.annotationRange.end),
                mixinTargetInternalNames = targets,
            )
            is MixinCompletionContext.ShadowMember -> AnnotationContext(
                annotation = MixinAnnotation.SHADOW,
                slot = AnnotationSlot.SHADOW_MEMBER,
                partialValue = context.partialValue,
                valueStartOffset = valueStart(source, offset),
                valueEndOffset = offset,
                annotationStartOffset = offset,
                annotationEndOffset = offset,
                mixinTargetInternalNames = targets,
            )
        }
    }

    fun toOffset(source: String, line: Int, character: Int): Int? {
        if (line < 0 || character < 0) return null
        var currentLine = 0
        var offset = 0
        while (offset < source.length && currentLine < line) {
            if (source[offset] == '\n') currentLine++
            offset++
        }
        val result = offset + character
        return if (result <= source.length) result else null
    }

    private data class OffsetRange(val start: Int, val end: Int) {
        operator fun contains(offset: Int): Boolean = offset in start..end

        fun expandForOpenString(source: String): OffsetRange {
            var endOffset = end
            while (endOffset < source.length && source[endOffset] != '\n' && source[endOffset] != ',' && source[endOffset] != ')') {
                endOffset++
            }
            return copy(end = endOffset)
        }
    }

    private data class ParsedMethodTarget(val name: String, val descriptor: String?)

    private fun parseMethodTarget(value: String): ParsedMethodTarget {
        val paren = value.indexOf('(')
        return if (paren > 0) ParsedMethodTarget(value.substring(0, paren), value.substring(paren)) else ParsedMethodTarget(value, null)
    }

    private fun sourceRange(source: String, range: McTextRange): OffsetRange =
        OffsetRange(offsetOf(source, range.start), offsetOf(source, range.end))

    private fun offsetOf(source: String, position: McTextPosition): Int {
        var currentLine = 0
        var offset = 0
        while (offset < source.length && currentLine < position.line) {
            if (source[offset] == '\n') currentLine++
            offset++
        }
        return (offset + position.character).coerceIn(0, source.length)
    }

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        var line = 0
        var character = 0
        var i = 0
        val safeOffset = offset.coerceIn(0, source.length)
        while (i < safeOffset) {
            if (source[i] == '\n') {
                line++
                character = 0
            } else {
                character++
            }
            i++
        }
        return McTextPosition(line, character)
    }

    private fun valueStart(source: String, cursorOffset: Int): Int {
        var index = cursorOffset.coerceIn(0, source.length)
        while (index > 0) {
            val previous = source[index - 1]
            if (previous == '"' || previous == '{' || previous == '=' || previous == ',' || previous == '(') break
            index--
        }
        return index
    }

    private fun partialValue(source: String, cursorOffset: Int): String {
        val start = valueStart(source, cursorOffset)
        return source.substring(start.coerceIn(0, source.length), cursorOffset.coerceIn(0, source.length)).trimStart('"')
    }

    private fun incompleteInjectorContext(
        source: String,
        cursorOffset: Int,
        model: MixinClassModel,
    ): MixinCompletionContext? {
        if (model.targets.isEmpty()) return null
        val before = source.substring(0, cursorOffset.coerceIn(0, source.length))
        val match = INJECTOR_ANNOTATION_PATTERN.findAll(before).lastOrNull() ?: return null
        val annotation = MixinAnnotation.fromSimpleName(match.groupValues[1]) ?: return null
        val methodAttribute = Regex("""method\s*=\s*"?[^",)]*$""").find(before.substring(match.range.first)) ?: return null
        val injector = InjectorModel(
            annotation = annotation,
            methodSelectors = emptyList(),
            atSelectors = emptyList(),
            range = McTextRange(offsetToPosition(source, match.range.first), offsetToPosition(source, cursorOffset)),
        )
        return MixinCompletionContext.InjectMethod(
            injector = injector,
            partialValue = methodAttribute.value.substringAfter('=').trim().trimStart('"'),
        )
    }

    private fun isMixinTargetsStringAttribute(source: String, cursorOffset: Int): Boolean {
        val before = source.substring(0, cursorOffset.coerceIn(0, source.length))
        val attrStart = maxOf(before.lastIndexOf("targets"), before.lastIndexOf("target"))
        if (attrStart < 0) return false
        val equals = source.indexOf('=', attrStart).takeIf { it in attrStart until cursorOffset } ?: return false
        val quote = source.indexOf('"', equals).takeIf { it in equals until cursorOffset } ?: return false
        return source.lastIndexOf('@', cursorOffset).let { at -> at >= 0 && at < attrStart } && quote >= 0
    }

    private val INJECTOR_ANNOTATION_PATTERN = Regex(
        """@(Inject|Redirect|ModifyArg|ModifyArgs|ModifyVariable|ModifyConstant|ModifyExpressionValue|ModifyReturnValue|ModifyReceiver|WrapOperation|WrapWithCondition|WrapMethod)\s*\(""",
    )
}
