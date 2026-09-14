package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.SemanticCompletionContextExtractor
import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition

class ExpressionSupport(
    private val memberCompletionService: ExpressionMemberCompletionService? = null,
) {
    private val expressionAtValues = listOf("MIXINEXTRAS:EXPRESSION")

    private val featureSnippets = listOf(
        FeatureSnippet("modifyexpressionvalue", "ModifyExpressionValue", "ModifyExpressionValue(method = \"${'$'}{1}\", at = @At(\"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("modifyreturnvalue", "ModifyReturnValue", "ModifyReturnValue(method = \"${'$'}{1}\", at = @At(\"RETURN\"))${'$'}0"),
        FeatureSnippet("modifyreceiver", "ModifyReceiver", "ModifyReceiver(method = \"${'$'}{1}\", at = @At(value = \"INVOKE\", target = \"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("wrapoperation", "WrapOperation", "WrapOperation(method = \"${'$'}{1}\", at = @At(value = \"INVOKE\", target = \"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("wrapwithcondition", "WrapWithCondition", "WrapWithCondition(method = \"${'$'}{1}\", at = @At(value = \"INVOKE\", target = \"${'$'}{2}\"))${'$'}0"),
        FeatureSnippet("wrapmethod", "WrapMethod", "WrapMethod(method = \"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("definition", "Definition", "Definition(id = \"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("definitions", "Definitions", "Definitions({ ${'$'}{1} })${'$'}0"),
        FeatureSnippet("expression", "Expression", "Expression(\"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("expressions", "Expressions", "Expressions({ ${'$'}{1} })${'$'}0"),
        FeatureSnippet("share", "Share", "Share(\"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("sharenamespace", "Share namespace", "Share(value = \"${'$'}{1}\", namespace = \"${'$'}{2}\")${'$'}0"),
        FeatureSnippet("local", "Local", "Local${'$'}0"),
        FeatureSnippet("localordinal", "Local ordinal", "Local(ordinal = ${'$'}{1:0})${'$'}0"),
        FeatureSnippet("localindex", "Local index", "Local(index = ${'$'}{1:0})${'$'}0"),
        FeatureSnippet("localname", "Local name", "Local(name = \"${'$'}{1}\")${'$'}0"),
        FeatureSnippet("localargsonly", "Local argsOnly", "Local(argsOnly = ${'$'}{1|true,false|})${'$'}0"),
        FeatureSnippet("localtype", "Local type", "Local(type = ${'$'}{1:void}.class)${'$'}0"),
        FeatureSnippet("cancellable", "Cancellable", "Cancellable${'$'}0"),
    )

    fun completeAtValue(context: AnnotationContext): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.AT || context.slot != AnnotationSlot.VALUE) return emptyList()
        if (context.parentInjectorAnnotation == MixinAnnotation.MODIFY_RETURN_VALUE) return emptyList()
        val partial = context.partialValue.trim('"')
        return expressionAtValues
            .filter { it.startsWith(partial, ignoreCase = true) }
            .map { value ->
                McCompletionItem(
                    label = value,
                    detail = "MixinExtras expression injection point",
                    documentation = "Use @Definition and @Expression annotations to define expression slices",
                    filterText = value,
                    insertText = value,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0310_$value",
                    metadata = McCompletionMetadata(source = "mixinextras.expressionAtValue", name = value),
                )
            }
    }

    fun completeExpressionAnnotations(context: AnnotationContext): List<McCompletionItem> {
        val partial = context.partialValue.trim()
        return completeFeatureSnippets(partial)
    }

    fun completeExpressionValue(
        context: AnnotationContext,
        source: String = "",
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
        mixinTargetOwners: List<String> = emptyList(),
        cancellationChecker: OfficialExpressionCancellationChecker = OfficialExpressionCancellationContext.current(),
    ): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.EXPRESSION && context.annotation != MixinAnnotation.EXPRESSIONS) {
            return emptyList()
        }
        if (context.slot != AnnotationSlot.VALUE) return emptyList()
        val position = context.expressionCompletionPosition ?: return emptyList()
        val prefix = context.partialValue
        val keywordItems = keywordsForPosition(position)
            .filter { keyword -> prefix.isEmpty() || keyword.startsWith(prefix, ignoreCase = true) }
            .mapIndexed { index, keyword ->
                McCompletionItem(
                    label = keyword,
                    detail = "Expression keyword",
                    documentation = keyword,
                    filterText = keyword,
                    insertText = keyword,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0320_${index.toString().padStart(2, '0')}_$keyword",
                    metadata = McCompletionMetadata(source = "mixinextras.expressionValue", name = keyword),
                )
            }
        val definitionIdItems = if (position == ExpressionCompletionPosition.STATEMENT_START ||
            position == ExpressionCompletionPosition.VALUE_START
        ) {
            completeDefinitionIds(context, source, resolvedContexts, prefix, keywordItems.size)
        } else {
            emptyList()
        }
        val memberItems = if (position == ExpressionCompletionPosition.AFTER_DOT) {
            completeAfterDotMembers(
                context = context,
                source = source,
                resolvedContexts = resolvedContexts,
                mixinTargetOwners = mixinTargetOwners,
                cancellationChecker = cancellationChecker,
            )
        } else {
            emptyList()
        }
        return keywordItems + definitionIdItems + memberItems
    }

    fun completeFeatureAnnotations(source: String, line: Int, character: Int): List<McCompletionItem> {
        val offset = SemanticCompletionContextExtractor.toOffset(source, line, character) ?: return emptyList()
        val lineStart = source.lastIndexOf('\n', (offset - 1).coerceAtLeast(0)).let { if (it < 0) 0 else it + 1 }
        val beforeCursor = source.substring(lineStart, offset.coerceIn(0, source.length))
        val marker = beforeCursor.lastIndexOf('@')
        if (marker < 0) return emptyList()
        val partial = beforeCursor.substring(marker + 1)
        if (partial.any { it.isWhitespace() || it == '(' || it == ')' || it == '.' }) return emptyList()
        return completeFeatureSnippets(partial)
    }

    fun isExpressionAtValue(atValue: String?): Boolean =
        atValue?.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true) == true

    private fun keywordsForPosition(position: ExpressionCompletionPosition): List<String> =
        when (position) {
            ExpressionCompletionPosition.STATEMENT_START -> statementKeywords + valueKeywords
            ExpressionCompletionPosition.VALUE_START -> valueKeywords
            ExpressionCompletionPosition.AFTER_METHOD_REFERENCE -> listOf("new")
            ExpressionCompletionPosition.AFTER_DOT,
            ExpressionCompletionPosition.NONE,
            -> emptyList()
        }

    private val statementKeywords = listOf("return", "throw")
    private val valueKeywords = listOf("this", "super", "true", "false", "null", "new")

    private fun completeAfterDotMembers(
        context: AnnotationContext,
        source: String,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
        mixinTargetOwners: List<String>,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): List<McCompletionItem> {
        val service = memberCompletionService ?: return emptyList()
        val result = service.completeMembers(
            source = source,
            context = context,
            mixinTargetOwners = mixinTargetOwners,
            resolvedContexts = resolvedContexts,
            cancellationChecker = cancellationChecker,
        )
        val candidates = when (result) {
            is ExpressionMemberCompletionServiceResult.Available -> result.candidates
            is ExpressionMemberCompletionServiceResult.Empty,
            is ExpressionMemberCompletionServiceResult.Unavailable,
            -> return emptyList()
        }
        if (source.isEmpty()) return emptyList()
        val site = ExpressionContextResolver.findEnclosingSite(source, context.valueStartOffset) ?: return emptyList()
        val expressionContext = ExpressionContextResolver.resolveExpressionContextForSite(
            source = source,
            site = site,
            resolvedContexts = resolvedContexts,
        ) ?: return emptyList()
        val definitionIndex = expressionContext.definitionIndex
        val seen = linkedSetOf<MemberCompletionItemKey>()
        val items = mutableListOf<McCompletionItem>()
        for (candidate in candidates) {
            val key = MemberCompletionItemKey(
                kind = candidate.kind,
                ownerInternalName = candidate.ownerInternalName,
                name = candidate.name,
                descriptor = candidate.descriptor,
            )
            if (!seen.add(key)) continue
            when (
                val plan = ExpressionDefinitionEditPlanner.plan(
                    context = context,
                    expressionAnnotationOffset = context.annotationStartOffset,
                    source = source,
                    definitionIndex = definitionIndex,
                    candidate = candidate,
                )
            ) {
                is ExpressionDefinitionEditPlanResult.Unavailable -> continue
                is ExpressionDefinitionEditPlanResult.Available ->
                    items += toMemberCompletionItem(candidate, plan)
            }
        }
        return items.sortedBy { it.sortKey }
    }

    private fun toMemberCompletionItem(
        candidate: OfficialExpressionMemberCandidate,
        plan: ExpressionDefinitionEditPlanResult.Available,
    ): McCompletionItem {
        val completionKind = when (candidate.kind) {
            OfficialExpressionMemberKind.FIELD -> McCompletionKind.FIELD
            OfficialExpressionMemberKind.METHOD -> McCompletionKind.METHOD
        }
        val label = when (candidate.kind) {
            OfficialExpressionMemberKind.FIELD -> "${candidate.name}:${candidate.descriptor}"
            OfficialExpressionMemberKind.METHOD -> "${candidate.name}${candidate.descriptor}"
        }
        return McCompletionItem(
            label = label,
            detail = candidate.ownerInternalName,
            documentation = candidate.descriptor,
            filterText = "${candidate.name} ${candidate.descriptor}",
            insertText = plan.insertText,
            kind = completionKind,
            sortKey = "0322_${candidate.kind.ordinal}_${candidate.name}_${candidate.descriptor}",
            metadata = McCompletionMetadata(
                source = "mixinextras.expressionMember",
                owner = candidate.ownerInternalName,
                name = candidate.name,
                descriptor = candidate.descriptor,
            ),
            additionalEdits = plan.additionalEdits,
        )
    }

    private data class MemberCompletionItemKey(
        val kind: OfficialExpressionMemberKind,
        val ownerInternalName: String,
        val name: String,
        val descriptor: String,
    )

    private fun completeDefinitionIds(
        context: AnnotationContext,
        source: String,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
        prefix: String,
        keywordCount: Int,
    ): List<McCompletionItem> {
        if (source.isEmpty()) return emptyList()
        val site = ExpressionContextResolver.findEnclosingSite(source, context.valueStartOffset) ?: return emptyList()
        val expressionContext = ExpressionContextResolver.resolveExpressionContextForSite(
            source = source,
            site = site,
            resolvedContexts = resolvedContexts,
        ) ?: return emptyList()
        return expressionContext.definitionIndex.definitions
            .mapNotNull { it.id }
            .distinct()
            .filter { id -> prefix.isEmpty() || id.startsWith(prefix, ignoreCase = true) }
            .mapIndexed { index, id ->
                McCompletionItem(
                    label = id,
                    detail = "Definition ID",
                    documentation = id,
                    filterText = id,
                    insertText = id,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0321_${(keywordCount + index).toString().padStart(2, '0')}_$id",
                    metadata = McCompletionMetadata(source = "mixinextras.definitionId", name = id),
                )
            }
    }

    private fun completeFeatureSnippets(partial: String): List<McCompletionItem> =
        featureSnippets
            .filter { snippet ->
                partial.isEmpty() ||
                    snippet.trigger.startsWith(partial, ignoreCase = true) ||
                    snippet.label.startsWith(partial, ignoreCase = true)
            }
            .map { snippet ->
                McCompletionItem(
                    label = snippet.label,
                    detail = snippet.detail,
                    documentation = snippet.insertText,
                    filterText = "${snippet.trigger} ${snippet.label}",
                    insertText = snippet.insertText,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0311_${snippet.trigger}",
                    metadata = McCompletionMetadata(source = "mixinextras.feature", name = snippet.trigger),
                    insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
                )
            }

    private data class FeatureSnippet(
        val trigger: String,
        val label: String,
        val insertText: String,
        val detail: String = "MixinExtras feature",
    )
}
