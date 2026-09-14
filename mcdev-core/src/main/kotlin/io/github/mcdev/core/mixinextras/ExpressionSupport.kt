package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.completion.McCompletionReplacementRange
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinImportEditBuilder
import io.github.mcdev.core.mixin.SemanticCompletionContextExtractor
import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition

class ExpressionSupport(
    private val memberCompletionService: ExpressionMemberCompletionService? = null,
    private val classIndex: ClassIndex? = null,
) {
    private val expressionAtValues = listOf("MIXINEXTRAS:EXPRESSION")

    private val coreAnnotationSnippets = listOf(
        FeatureSnippet("mixin", "Mixin", "Mixin(${ '$' }{1:Target}.class)${ '$' }0", MixinAnnotation.MIXIN.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet("shadow", "Shadow", "Shadow${ '$' }0", MixinAnnotation.SHADOW.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet("accessor", "Accessor", "Accessor(\"${ '$' }{1}\")${ '$' }0", MixinAnnotation.ACCESSOR.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet("invoker", "Invoker", "Invoker(\"${ '$' }{1}\")${ '$' }0", MixinAnnotation.INVOKER.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet(
            "inject",
            "Inject",
            "Inject(method = \"${ '$' }{1}\", at = @At(\"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.INJECT.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "redirect",
            "Redirect",
            "Redirect(method = \"${ '$' }{1}\", at = @At(value = \"INVOKE\", target = \"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.REDIRECT.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "modifyarg",
            "ModifyArg",
            "ModifyArg(method = \"${ '$' }{1}\", at = @At(value = \"INVOKE\", target = \"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.MODIFY_ARG.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "modifyargs",
            "ModifyArgs",
            "ModifyArgs(method = \"${ '$' }{1}\", at = @At(value = \"INVOKE\", target = \"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.MODIFY_ARGS.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "modifyvariable",
            "ModifyVariable",
            "ModifyVariable(method = \"${ '$' }{1}\", at = @At(\"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.MODIFY_VARIABLE.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "modifyconstant",
            "ModifyConstant",
            "ModifyConstant(method = \"${ '$' }{1}\", constant = @Constant(intValue = ${ '$' }{2:0}))${ '$' }0",
            (MixinAnnotation.MODIFY_CONSTANT.officialFqns + MixinAnnotation.CONSTANT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet("overwrite", "Overwrite", "Overwrite${ '$' }0", MixinAnnotation.OVERWRITE.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet("at", "At", "At(\"${ '$' }{1}\")${ '$' }0", MixinAnnotation.AT.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet("constant", "Constant", "Constant(intValue = ${ '$' }{1:0})${ '$' }0", MixinAnnotation.CONSTANT.officialFqns.toList(), source = "mixin.annotation"),
        FeatureSnippet(
            "slice",
            "Slice",
            "Slice(from = @At(\"${ '$' }{1}\"), to = @At(\"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.SLICE.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "group",
            "Group",
            "Group(name = \"${ '$' }{1}\", min = ${ '$' }{2:-1}, max = ${ '$' }{3:-1})${ '$' }0",
            listOf("org.spongepowered.asm.mixin.injection.Group"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "debug",
            "Debug",
            "Debug(export = ${ '$' }{1:false}, print = ${ '$' }{2:false})${ '$' }0",
            listOf("org.spongepowered.asm.mixin.Debug"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "dynamic",
            "Dynamic",
            "Dynamic(value = \"${ '$' }{1}\", mixin = ${ '$' }{2:Target}.class)${ '$' }0",
            listOf("org.spongepowered.asm.mixin.Dynamic"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "unique",
            "Unique",
            "Unique(silent = ${ '$' }{1:false})${ '$' }0",
            listOf("org.spongepowered.asm.mixin.Unique"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "intrinsic",
            "Intrinsic",
            "Intrinsic(displace = ${ '$' }{1:false})${ '$' }0",
            listOf("org.spongepowered.asm.mixin.Intrinsic"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "implements",
            "Implements",
            "Implements({ @Interface(iface = ${ '$' }{1:Target}.class, prefix = \"${ '$' }{2:prefix\\${ '$' }}\") })${ '$' }0",
            listOf("org.spongepowered.asm.mixin.Implements", "org.spongepowered.asm.mixin.Interface"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "interface",
            "Interface",
            "Interface(iface = ${ '$' }{1:Target}.class, prefix = \"${ '$' }{2:prefix\\${ '$' }}\")${ '$' }0",
            listOf("org.spongepowered.asm.mixin.Interface"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "desc",
            "Desc",
            "Desc(value = \"${ '$' }{1}\", owner = ${ '$' }{2:Target}.class, ret = ${ '$' }{3:void}.class, args = { ${ '$' }{4} })${ '$' }0",
            listOf("org.spongepowered.asm.mixin.injection.Desc"),
            source = "mixin.annotation",
        ),
        FeatureSnippet(
            "descriptors",
            "Descriptors",
            "Descriptors({ @Desc(value = \"${ '$' }{1}\", owner = ${ '$' }{2:Target}.class, ret = ${ '$' }{3:void}.class, args = { ${ '$' }{4} }) })${ '$' }0",
            listOf("org.spongepowered.asm.mixin.injection.Descriptors", "org.spongepowered.asm.mixin.injection.Desc"),
            source = "mixin.annotation",
        ),
    )

    private val mixinExtrasSnippets = listOf(
        FeatureSnippet(
            "modifyexpressionvalue",
            "ModifyExpressionValue",
            "ModifyExpressionValue(method = \"${ '$' }{1}\", at = @At(\"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.MODIFY_EXPRESSION_VALUE.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
        ),
        FeatureSnippet(
            "modifyreturnvalue",
            "ModifyReturnValue",
            "ModifyReturnValue(method = \"${ '$' }{1}\", at = @At(\"RETURN\"))${ '$' }0",
            (MixinAnnotation.MODIFY_RETURN_VALUE.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
        ),
        FeatureSnippet(
            "modifyreceiver",
            "ModifyReceiver",
            "ModifyReceiver(method = \"${ '$' }{1}\", at = @At(value = \"INVOKE\", target = \"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.MODIFY_RECEIVER.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
        ),
        FeatureSnippet(
            "wrapoperation",
            "WrapOperation",
            "WrapOperation(method = \"${ '$' }{1}\", at = @At(value = \"INVOKE\", target = \"${ '$' }{2}\"))${ '$' }0",
            (MixinAnnotation.WRAP_OPERATION.officialFqns + MixinAnnotation.AT.officialFqns).toList(),
        ),
        FeatureSnippet(
            "wrapmethod",
            "WrapMethod",
            "WrapMethod(method = \"${ '$' }{1}\")${ '$' }0",
            MixinAnnotation.WRAP_METHOD.officialFqns.toList(),
        ),
        FeatureSnippet(
            "definition",
            "Definition",
            "Definition(id = \"${ '$' }{1}\")${ '$' }0",
            MixinAnnotation.DEFINITION.officialFqns.toList(),
        ),
        FeatureSnippet(
            "definitions",
            "Definitions",
            "Definitions({ ${ '$' }{1} })${ '$' }0",
            MixinAnnotation.DEFINITIONS.officialFqns.toList(),
        ),
        FeatureSnippet(
            "expression",
            "Expression",
            "Expression(\"${ '$' }{1}\")${ '$' }0",
            MixinAnnotation.EXPRESSION.officialFqns.toList(),
        ),
        FeatureSnippet(
            "expressions",
            "Expressions",
            "Expressions({ ${ '$' }{1} })${ '$' }0",
            MixinAnnotation.EXPRESSIONS.officialFqns.toList(),
        ),
        FeatureSnippet(
            "share",
            "Share",
            "Share(\"${ '$' }{1}\")${ '$' }0",
            MixinAnnotation.SHARE.officialFqns.toList(),
        ),
        FeatureSnippet(
            "sharenamespace",
            "Share namespace",
            "Share(value = \"${ '$' }{1}\", namespace = \"${ '$' }{2}\")${ '$' }0",
            MixinAnnotation.SHARE.officialFqns.toList(),
        ),
        FeatureSnippet("local", "Local", "Local${ '$' }0", MixinAnnotation.LOCAL.officialFqns.toList()),
        FeatureSnippet("localordinal", "Local ordinal", "Local(ordinal = ${ '$' }{1:0})${ '$' }0", MixinAnnotation.LOCAL.officialFqns.toList()),
        FeatureSnippet("localindex", "Local index", "Local(index = ${ '$' }{1:0})${ '$' }0", MixinAnnotation.LOCAL.officialFqns.toList()),
        FeatureSnippet("localname", "Local name", "Local(name = \"${ '$' }{1}\")${ '$' }0", MixinAnnotation.LOCAL.officialFqns.toList()),
        FeatureSnippet("localargsonly", "Local argsOnly", "Local(argsOnly = ${ '$' }{1|true,false|})${ '$' }0", MixinAnnotation.LOCAL.officialFqns.toList()),
        FeatureSnippet("localtype", "Local type", "Local(type = ${ '$' }{1:void}.class)${ '$' }0", MixinAnnotation.LOCAL.officialFqns.toList()),
        FeatureSnippet(
            "cancellable",
            "Cancellable",
            "Cancellable${ '$' }0",
            listOf("com.llamalad7.mixinextras.sugar.Cancellable"),
        ),
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

    fun completeFeatureAnnotations(source: String, line: Int, character: Int): List<McCompletionItem> =
        completeFeatureAnnotationsWithRange(source, line, character).items

    fun completeFeatureAnnotationsWithRange(
        source: String,
        line: Int,
        character: Int,
    ): FeatureAnnotationCompletion {
        val offset = SemanticCompletionContextExtractor.toOffset(source, line, character)
            ?: return FeatureAnnotationCompletion.EMPTY
        val lineStart = source.lastIndexOf('\n', (offset - 1).coerceAtLeast(0)).let { if (it < 0) 0 else it + 1 }
        val maskedSource = AnnotationContextExtractor.maskNonCode(source)
        val beforeCursor = maskedSource.substring(lineStart, offset.coerceIn(0, source.length))
        val marker = beforeCursor.lastIndexOf('@')
        if (marker < 0) return FeatureAnnotationCompletion.EMPTY
        val partial = beforeCursor.substring(marker + 1)
        if (partial.any { it.isWhitespace() || it == '(' || it == ')' || it == '.' }) {
            return FeatureAnnotationCompletion.EMPTY
        }
        var nameEnd = offset.coerceIn(0, source.length)
        while (nameEnd < maskedSource.length && isFeatureAnnotationNameChar(maskedSource[nameEnd])) {
            nameEnd++
        }
        var afterName = nameEnd
        while (afterName < maskedSource.length && maskedSource[afterName].isWhitespace()) {
            afterName++
        }
        // A template contains its own argument list. If an annotation body
        // already follows the name, suppress these templates rather than
        // producing `@Inject(...)(existingArgs)` or discarding the body.
        if (maskedSource.getOrNull(afterName) == '(') {
            return FeatureAnnotationCompletion.EMPTY
        }
        val items = completeFeatureSnippets(partial, source)
        if (items.isEmpty()) return FeatureAnnotationCompletion.EMPTY
        return FeatureAnnotationCompletion(
            items = items,
            replacementRange = McCompletionReplacementRange(
                startOffset = lineStart + marker + 1,
                endOffset = nameEnd,
            ),
        )
    }

    private fun isFeatureAnnotationNameChar(ch: Char): Boolean =
        ch.isLetterOrDigit() || ch == '_' || ch == '$'

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

    private fun completeFeatureSnippets(
        partial: String,
        source: String = "",
    ): List<McCompletionItem> =
        allFeatureSnippets()
            .filter { snippet ->
                partial.isEmpty() ||
                    snippet.trigger.startsWith(partial, ignoreCase = true) ||
                    snippet.label.startsWith(partial, ignoreCase = true)
            }
            .map { snippet ->
                val rendered = renderFeatureSnippet(source, snippet)
                McCompletionItem(
                    label = snippet.label,
                    detail = snippet.detail,
                    documentation = rendered.insertText,
                    filterText = "${snippet.trigger} ${snippet.label}",
                    insertText = rendered.insertText,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0311_${snippet.trigger}",
                    metadata = McCompletionMetadata(
                        source = snippet.source,
                        name = snippet.trigger,
                        owner = snippet.importFqns.firstOrNull()?.replace('.', '/'),
                    ),
                    additionalEdits = rendered.additionalEdits,
                    insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
                )
            }

    private fun renderFeatureSnippet(source: String, snippet: FeatureSnippet): RenderedFeatureSnippet {
        val additionalInternalNames = snippet.importFqns
            .map { it.replace('.', '/') }
            .toSet()
        val references = snippet.importFqns.map { fqn ->
            fqn to MixinImportEditBuilder.referenceForInternalName(
                source = source,
                internalName = fqn.replace('.', '/'),
                additionalInternalNames = additionalInternalNames,
            )
        }
        val insertText = references
            .sortedByDescending { (fqn, _) -> fqn.substringAfterLast('.').length }
            .fold(snippet.insertText) { text, (fqn, reference) ->
                replaceJavaIdentifier(text, fqn.substringAfterLast('.'), reference.text)
            }
        return RenderedFeatureSnippet(
            insertText = insertText,
            additionalEdits = buildImportEdits(
                source = source,
                fqns = references.mapNotNull { (_, reference) -> reference.importFqn },
            ),
        )
    }

    private fun replaceJavaIdentifier(text: String, name: String, replacement: String): String {
        if (name == replacement) return text
        // A bare annotation snippet ends in the snippet cursor marker (for
        // example `Local$0`), which is not part of the Java identifier but is
        // adjacent to it in the rendered text.
        return Regex("""(?<![A-Za-z0-9_$])${Regex.escape(name)}(?![A-Za-z0-9_])""")
            .replace(text, replacement)
    }

    private fun allFeatureSnippets(): List<FeatureSnippet> =
        coreAnnotationSnippets + mixinExtrasSnippets + wrapWithConditionSnippets()

    private fun wrapWithConditionSnippets(): List<FeatureSnippet> {
        val officialFqns = MixinAnnotation.WRAP_WITH_CONDITION.officialFqns.toList()
        val indexedFqns = classIndex
            ?.let { index -> officialFqns.filter { index.findClassByFqn(it) != null } }
            .orEmpty()
        return indexedFqns.map { fqn ->
            val version = if (fqn.endsWith(".v2.WrapWithCondition")) " (v2)" else " (v1)"
            FeatureSnippet(
                trigger = "wrapwithcondition",
                label = "WrapWithCondition$version",
                insertText = "WrapWithCondition(method = \"${ '$' }{1}\", at = @At(value = \"INVOKE\", target = \"${ '$' }{2}\"))${ '$' }0",
                importFqns = listOf(fqn) + MixinAnnotation.AT.officialFqns,
            )
        }
    }

    private fun buildImportEdits(source: String, fqns: List<String>): List<McTextEdit> {
        if (source.isEmpty() || fqns.isEmpty()) return emptyList()
        val edits = fqns
            .distinct()
            .mapNotNull { fqn -> MixinImportEditBuilder.buildImportEdit(source, fqn) }
        if (edits.isEmpty()) return emptyList()
        return edits
            .groupBy { it.startOffset to it.endOffset }
            .map { (_, grouped) ->
                val first = grouped.first()
                first.copy(
                    newText = grouped.mapIndexed { index, edit ->
                        if (index == 0) edit.newText else edit.newText.removePrefix("\n")
                    }.joinToString(separator = ""),
                )
            }
    }

    data class FeatureAnnotationCompletion(
        val items: List<McCompletionItem>,
        val replacementRange: McCompletionReplacementRange?,
    ) {
        companion object {
            val EMPTY = FeatureAnnotationCompletion(emptyList(), null)
        }
    }

    private data class FeatureSnippet(
        val trigger: String,
        val label: String,
        val insertText: String,
        val importFqns: List<String> = emptyList(),
        val detail: String = "MixinExtras feature",
        val source: String = "mixinextras.feature",
    )

    private data class RenderedFeatureSnippet(
        val insertText: String,
        val additionalEdits: List<McTextEdit>,
    )
}
