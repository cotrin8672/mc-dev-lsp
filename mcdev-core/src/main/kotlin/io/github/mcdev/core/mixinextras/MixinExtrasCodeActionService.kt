package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.JavaSourceImports
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinImportEditBuilder
import io.github.mcdev.core.mixin.MixinTargetResolver

private const val CANCELLABLE_PARAMETER_TYPE_MESSAGE_PREFIX = "Cancellable parameter type should be "
private const val CALLBACK_INFO_DESCRIPTOR =
    "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;"
private const val CALLBACK_INFO_RETURNABLE_DESCRIPTOR =
    "Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;"
private const val CALLBACK_INFO_FQN = "org.spongepowered.asm.mixin.injection.callback.CallbackInfo"
private const val CALLBACK_INFO_RETURNABLE_FQN =
    "org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable"
private const val OPERATION_INTERNAL_NAME =
    "com/llamalad7/mixinextras/injector/wrapoperation/Operation"

class MixinExtrasCodeActionService(
    private val classIndex: ClassIndex,
    private val signatureService: HandlerSignatureService = HandlerSignatureService(classIndex),
) {
    fun fixesForDiagnostics(
        diagnostics: List<McDiagnostic>,
        documentUri: String,
        source: String,
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
    ): List<McFix> {
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        val sites = HandlerSignatureService.findAnnotationSites(source)
        val sugarSites = HandlerSignatureService.findSugarHandlerAnnotationSites(source)
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        val fixes = mutableListOf<McFix>()
        for (diagnostic in diagnostics) {
            if (diagnostic.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                diagnostic.message.startsWith(CANCELLABLE_PARAMETER_TYPE_MESSAGE_PREFIX)
            ) {
                fixCancellableParameterTypeFix(
                    documentUri = documentUri,
                    source = source,
                    diagnostic = diagnostic,
                    sites = sugarSites,
                    imports = imports,
                )?.let { fixes += it }
                continue
            }
            if (diagnostic.code == MixinExtrasDiagnosticCodes.WRONG_OPERATION_CALL_ARGUMENTS) {
                fixOperationCallArgumentsFix(
                    documentUri = documentUri,
                    source = source,
                    diagnostic = diagnostic,
                    mixinTargets = mixinTargets,
                    resolvedContexts = resolvedContexts,
                )?.let { fixes += it }
                continue
            }
            val site = siteForDiagnostic(sites, diagnostic)
            if (site == null) continue
            when (diagnostic.code) {
                MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH,
                MixinExtrasDiagnosticCodes.MISSING_OPERATION_PARAMETER,
                MixinExtrasDiagnosticCodes.WRONG_OPERATION_GENERIC,
                MixinExtrasDiagnosticCodes.WRONG_ORIGINAL_VALUE_TYPE,
                MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
                    -> {
                    val fix = fixHandlerSignatureFix(
                        documentUri,
                        source,
                        site,
                        sites,
                        mixinTargets,
                        resolvedContexts,
                    )
                    if (fix != null) fixes += fix
                }
            }
            if (site.handlerMethod == null) {
                val fix = generateHandlerFix(
                    documentUri,
                    source,
                    site,
                    sites,
                    mixinTargets,
                    resolvedContexts,
                )
                if (fix != null) fixes += fix
            }
        }
        return deduplicateFixes(fixes)
    }

    private fun fixCancellableParameterTypeFix(
        documentUri: String,
        source: String,
        diagnostic: McDiagnostic,
        sites: List<MixinExtrasAnnotationSite>,
        imports: JavaSourceImports,
    ): WorkspaceEditFix? {
        val matches = sites.mapNotNull { site ->
            val handler = site.handlerMethod ?: return@mapNotNull null
            val parameterIndices = handler.parameters.mapIndexedNotNull { index, parameter ->
                index.takeIf {
                    parameter.sugarSpec is HandlerParameterSugarSpec.Cancellable &&
                        parameter.sugarAnnotationRange == diagnostic.range
                }
            }
            if (parameterIndices.isEmpty()) null else site to (handler to parameterIndices)
        }
        if (matches.isEmpty()) return null

        val resolved = matches.mapNotNull { match ->
            val targetMethod = targetMethodForSite(source, match.first, imports) ?: return@mapNotNull null
            Triple(match.first, match.second, targetMethod to expectedCancellableType(targetMethod))
        }
        if (resolved.size != matches.size || resolved.map { it.third.second }.distinct().size != 1) return null

        val candidates = resolved.flatMap { (site, handlerAndIndices, targetAndExpected) ->
            val (handler, parameterIndices) = handlerAndIndices
            val (targetMethod, expectedType) = targetAndExpected
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            parameterIndices.mapNotNull { index ->
                val parameter = handler.parameters[index]
                if (signatureService.validateCommonSugarConstraints(enriched, targetMethod, site.annotation).none { issue ->
                        issue.code == MixinExtrasDiagnosticCodes.HANDLER_SIGNATURE_MISMATCH &&
                            issue.range == diagnostic.range &&
                            issue.message == diagnostic.message
                    }
                ) return@mapNotNull null
                val actual = enriched.parameters.getOrNull(index) ?: return@mapNotNull null
                if (actual.typeDescriptor == expectedType.first) return@mapNotNull null
                val typeRange = parameterTypeRange(source, parameter) ?: return@mapNotNull null
                McTextEdit(typeRange.first, typeRange.last + 1, expectedType.second)
            }
        }.distinct()
        if (candidates.size != 1) return null
        return WorkspaceEditFix(
            title = "Fix @Cancellable parameter type",
            kind = "quickfix.mixinextras.fixCancellableParameterType",
            documentUri = documentUri,
            edits = listOf(candidates.single()),
            metadata = mapOf("annotation" to "Cancellable"),
        )
    }

    private fun targetMethodForSite(
        source: String,
        site: MixinExtrasAnnotationSite,
        imports: JavaSourceImports,
    ): MethodIndexEntry? {
        val annotationOffset = rangeStart(source, site.annotationRange) ?: return null
        val scope = AnnotationContextExtractor.resolveMixinClassScope(source, annotationOffset) ?: return null
        val targets = MixinTargetResolver.resolveTargets(scope.rawTargets, classIndex, imports)
        return signatureService.resolveTargetMethod(targets, site.methodAttribute)
    }

    private fun expectedCancellableType(targetMethod: MethodIndexEntry): Pair<String, String> {
        val returnDescriptor = targetMethod.descriptor.substringAfterLast(')')
        return if (returnDescriptor == "V") {
            CALLBACK_INFO_DESCRIPTOR to CALLBACK_INFO_FQN
        } else {
            CALLBACK_INFO_RETURNABLE_DESCRIPTOR to CALLBACK_INFO_RETURNABLE_FQN
        }
    }

    private fun parameterTypeRange(source: String, parameter: HandlerParameterDeclaration): IntRange? {
        val parameterRange = parameter.range ?: return null
        val parameterStart = rangeStart(source, parameterRange) ?: return null
        val parameterEnd = rangeEnd(source, parameterRange) ?: return null
        val searchStart = parameter.sugarAnnotationRange?.let { rangeEnd(source, it) } ?: parameterStart
        val maskedSource = AnnotationContextExtractor.maskNonCode(source)
        val nameStart = maskedSource.lastIndexOf(parameter.name, parameterEnd - parameter.name.length)
        if (nameStart < searchStart) return null
        val typeStart = maskedSource.lastIndexOf(parameter.typeName, nameStart - 1)
        if (typeStart < searchStart) return null
        val typeEnd = typeStart + parameter.typeName.length
        if (typeEnd > nameStart) return null
        val leadingFinalLength = Regex("""^final\s+""").find(parameter.typeName)?.value?.length ?: 0
        return (typeStart + leadingFinalLength) until typeEnd
    }

    private fun siteForDiagnostic(
        sites: List<MixinExtrasAnnotationSite>,
        diagnostic: McDiagnostic,
    ): MixinExtrasAnnotationSite? {
        val exactMatches = sites.filter { site ->
            site.handlerMethod?.range == diagnostic.range || site.annotationRange == diagnostic.range
        }
        if (exactMatches.isNotEmpty()) {
            return exactMatches.groupBy(::logicalHandlerGroup).values.singleOrNull()?.firstOrNull()
        }

        val method = diagnostic.metadata["method"]
        val annotation = diagnostic.metadata["annotation"]
        if (method != null) {
            val methodMatches = sites.filter { it.methodAttribute == method }
            if (methodMatches.isNotEmpty()) {
                return methodMatches.groupBy(::logicalHandlerGroup).values.singleOrNull()?.firstOrNull()
            }
        }
        if (annotation == null) return null
        return sites.filter { it.annotation.simpleName == annotation }
            .groupBy(::logicalHandlerGroup)
            .values
            .singleOrNull()
            ?.firstOrNull()
    }

    private fun logicalHandlerGroup(site: MixinExtrasAnnotationSite): Pair<Boolean, McTextRange> =
        site.handlerMethod?.range?.let { true to it } ?: (false to site.annotationRange)

    fun generateHandlerFixes(
        documentUri: String,
        source: String,
        site: MixinExtrasAnnotationSite,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
    ): List<McFix> = listOfNotNull(
        generateHandlerFix(
            documentUri,
            source,
            site,
            HandlerSignatureService.findAnnotationSites(source),
            mixinTargets,
            resolvedContexts,
        ),
    )

    fun generateHandlerFixesAtPosition(
        documentUri: String,
        source: String,
        line: Int,
        character: Int,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
    ): List<McFix> {
        val offset = AnnotationContextExtractor.toOffset(source, line, character) ?: return emptyList()
        val sites = HandlerSignatureService.findAnnotationSites(source)
        val selected = sites.filter { site ->
            if (site.handlerMethod != null) return@filter false
            val start = rangeStart(source, site.annotationRange) ?: return@filter false
            val end = rangeEnd(source, site.annotationRange) ?: return@filter false
            offset in start..end
        }
        return deduplicateFixes(selected.flatMap { site ->
            generateHandlerFixes(documentUri, source, site, mixinTargets, resolvedContexts)
        })
    }

    fun completeHandler(
        source: String,
        annotationStartOffset: Int,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext> = emptyList(),
        cursorOffset: Int? = null,
    ): List<McCompletionItem> {
        val sites = HandlerSignatureService.findSugarHandlerAnnotationSites(source)
        val selected = sites.filter { site ->
            site.handlerMethod == null &&
                rangeStart(source, site.annotationRange) == annotationStartOffset
        }
        if (selected.isEmpty()) return emptyList()

        val plans = selected.mapNotNull { site ->
            agreedHandlerPlan(
                source = source,
                site = site,
                allSites = sites,
                mixinTargets = mixinTargets,
                resolvedContexts = resolvedContexts,
                handlerName = HANDLER_NAME_MARKER,
            )
        }
        val plan = plans.distinctBy { it.text }.singleOrNull() ?: return emptyList()
        val annotation = selected.first().annotation
        val annotationEndOffset = AnnotationContextExtractor.annotationEndOffset(source, annotationStartOffset)
        val insertionOffset = cursorOffset ?: annotationEndOffset
        return listOf(
            McCompletionItem(
                label = "Generate ${annotation.simpleName} handler",
                detail = "Insert the inferred handler signature",
                documentation = "Generate a ${annotation.simpleName} handler and edit its method name.",
                filterText = "${annotation.simpleName} handler",
                insertText = handlerSnippet(
                    text = plan.text,
                    source = source,
                    annotationEndOffset = annotationEndOffset,
                    cursorOffset = insertionOffset,
                ),
                kind = McCompletionKind.METHOD,
                sortKey = "0100_handler_${annotation.simpleName}",
                metadata = McCompletionMetadata(
                    source = "mixin.handler",
                    name = annotation.simpleName,
                ),
                additionalEdits = buildImportEdits(source, plan.importFqns),
                insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
            ),
        )
    }

    fun applyHandlerFix(
        fix: WorkspaceEditFix,
        currentSource: String,
    ): WorkspaceEditFix = fix

    private fun resolvedContextForSite(
        site: MixinExtrasAnnotationSite,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
    ): ExpressionContext? {
        if (resolvedContexts.isEmpty()) return null
        return selectResolvedMixinExtrasContext(site, resolvedContexts)?.context
    }

    private fun relatedSitesForHandlerGroup(
        site: MixinExtrasAnnotationSite,
        allSites: List<MixinExtrasAnnotationSite>,
    ): List<MixinExtrasAnnotationSite> {
        val handlerRange = site.handlerMethod?.range
        return if (handlerRange != null) {
            allSites.filter { it.handlerMethod?.range == handlerRange }
        } else {
            allSites.filter { it.handlerMethod == null && it.annotationRange == site.annotationRange }
        }
    }

    private data class RenderedHandlerPlan(
        val text: String,
        val importFqns: Set<String>,
    )

    private data class RenderedType(
        val text: String,
        val importFqns: Set<String> = emptySet(),
    )

    private data class RenderedSignature(
        val text: String,
        val importFqns: Set<String>,
    )

    private data class RenderedExpectedSignature(
        val returnType: String,
        val parameterTypes: List<String>,
        val importFqns: Set<String>,
    )

    private fun agreedHandlerPlan(
        source: String,
        site: MixinExtrasAnnotationSite,
        allSites: List<MixinExtrasAnnotationSite>,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
        handlerName: String? = null,
    ): RenderedHandlerPlan? {
        val relatedSites = relatedSitesForHandlerGroup(site, allSites)
        val methodName = handlerName ?: site.handlerMethod?.methodName ?: "mcdevHandler"
        val candidates = relatedSites.map { relatedSite ->
            val context = resolvedContextForSite(relatedSite, resolvedContexts)
            val expected = signatureService.expectedSignature(source, relatedSite, mixinTargets, context)
                ?: return null
            val rendered = renderExpectedSignature(source, expected)
            val renderedSpec = expected.copy(
                readableReturnType = rendered.returnType,
                parameters = expected.parameters.mapIndexed { index, parameter ->
                    parameter.copy(readableType = rendered.parameterTypes[index])
                },
            )
            val stub = signatureService.generateHandlerStub(
                source,
                relatedSite,
                mixinTargets,
                methodName = methodName,
                resolvedContext = context,
                signatureSpec = renderedSpec,
            ) ?: return null
            RenderedHandlerPlan(stub, rendered.importFqns)
        }
        val text = candidates.map { it.text }.distinct().singleOrNull() ?: return null
        return RenderedHandlerPlan(text, candidates.flatMap { it.importFqns }.toSet())
    }

    private fun renderExpectedSignature(
        source: String,
        expected: HandlerSignatureSpec,
    ): RenderedExpectedSignature {
        val candidateInternalNames = buildSet {
            descriptorInternalName(expected.returnTypeDescriptor)?.let(::add)
            expected.parameters.forEach { parameter ->
                if (parameter.isOperation) add(OPERATION_INTERNAL_NAME)
                descriptorInternalName(parameter.typeDescriptor)?.let(::add)
                descriptorInternalName(parameter.genericTypeDescriptor)?.let(::add)
                if (parameter.isOperation) {
                    descriptorInternalName(parameter.operationGenericDescriptor)?.let(::add)
                }
            }
        }
        val parameters = expected.parameters.map { parameter ->
            if (parameter.isOperation) {
                val operation = MixinImportEditBuilder.referenceForInternalName(
                    source,
                    OPERATION_INTERNAL_NAME,
                    candidateInternalNames,
                )
                val generic = parameter.operationGenericDescriptor?.let {
                    descriptorTypeReference(
                        source,
                        it,
                        parameter.readableType.substringAfter('<').substringBeforeLast('>'),
                        candidateInternalNames,
                    )
                }
                val genericText = generic?.text
                    ?: parameter.readableType.substringAfter('<').substringBeforeLast('>')
                RenderedType(
                    text = "${operation.text}<$genericText>",
                    importFqns = setOfNotNull(operation.importFqn) + generic?.importFqns.orEmpty(),
                )
            } else {
                descriptorTypeReference(
                    source,
                    parameter.typeDescriptor,
                    parameter.readableType,
                    candidateInternalNames,
                    parameter.genericTypeDescriptor,
                )
            }
        }
        val returnType = descriptorTypeReference(
            source,
            expected.returnTypeDescriptor,
            expected.readableReturnType,
            candidateInternalNames,
        )
        return RenderedExpectedSignature(
            returnType = returnType.text,
            parameterTypes = parameters.map { it.text },
            importFqns = parameters.flatMap { it.importFqns }.toSet() + returnType.importFqns,
        )
    }

    private fun descriptorTypeReference(
        source: String,
        descriptor: String?,
        fallback: String,
        candidateInternalNames: Set<String>,
        genericDescriptor: String? = null,
    ): RenderedType {
        val internalName = descriptorInternalName(descriptor) ?: return RenderedType(fallback)
        val depth = descriptor.orEmpty().takeWhile { it == '[' }.length
        val reference = MixinImportEditBuilder.referenceForInternalName(
            source,
            internalName,
            candidateInternalNames,
        )
        val generic = genericDescriptor?.let {
            descriptorTypeReference(
                source = source,
                descriptor = it,
                fallback = boxedReadableType(it),
                candidateInternalNames = candidateInternalNames,
            )
        }
        return RenderedType(
            text = buildString {
                append(reference.text)
                generic?.let { append('<').append(it.text).append('>') }
                append("[]".repeat(depth))
            },
            importFqns = setOfNotNull(reference.importFqn) + generic?.importFqns.orEmpty(),
        )
    }

    private fun boxedReadableType(descriptor: String): String = when (descriptor) {
        "Z" -> "Boolean"
        "B" -> "Byte"
        "C" -> "Character"
        "S" -> "Short"
        "I" -> "Integer"
        "J" -> "Long"
        "F" -> "Float"
        "D" -> "Double"
        else -> OperationSignatureRenderer.readableType(descriptor)
    }

    private fun descriptorInternalName(descriptor: String?): String? {
        var value = descriptor ?: return null
        while (value.startsWith("[")) value = value.substring(1)
        if (!value.startsWith("L") || !value.endsWith(';')) return null
        return value.substring(1, value.length - 1)
    }

    private fun buildImportEdits(source: String, fqns: Set<String>): List<McTextEdit> {
        val edits = fqns.sorted().mapNotNull { MixinImportEditBuilder.buildImportEdit(source, it) }
        if (edits.isEmpty()) return emptyList()
        val first = edits.first()
        val leadingNewline = first.newText.takeIf { it.startsWith("\n") }?.let { "\n" }.orEmpty()
        val imports = edits.joinToString(separator = "") { it.newText.trimStart('\n') }
        return listOf(McTextEdit(first.startOffset, first.endOffset, leadingNewline + imports))
    }

    private fun agreedHandlerStub(
        source: String,
        site: MixinExtrasAnnotationSite,
        allSites: List<MixinExtrasAnnotationSite>,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
        handlerName: String? = null,
    ): String? = agreedHandlerPlan(
        source,
        site,
        allSites,
        mixinTargets,
        resolvedContexts,
        handlerName,
    )?.text

    private fun generateHandlerFix(
        documentUri: String,
        source: String,
        site: MixinExtrasAnnotationSite,
        allSites: List<MixinExtrasAnnotationSite>,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
    ): WorkspaceEditFix? {
        val plan = agreedHandlerPlan(source, site, allSites, mixinTargets, resolvedContexts) ?: return null
        val insertOffset = findInsertOffset(source, site) ?: return null
        val title = when (site.annotation) {
            MixinExtrasAnnotation.WRAP_OPERATION -> "Generate WrapOperation handler"
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE -> "Generate ModifyExpressionValue handler"
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> "Generate ModifyReturnValue handler"
            MixinExtrasAnnotation.MODIFY_RECEIVER -> "Generate ModifyReceiver handler"
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> "Generate WrapWithCondition handler"
            MixinExtrasAnnotation.WRAP_METHOD -> "Generate WrapMethod handler"
            else -> "Generate MixinExtras handler"
        }
        return WorkspaceEditFix(
            title = title,
            kind = "quickfix.mixinextras.generateHandler",
            documentUri = documentUri,
            edits = listOf(McTextEdit(insertOffset, insertOffset, "\n${plan.text}")) +
                buildImportEdits(source, plan.importFqns),
            metadata = mapOf("annotation" to site.annotation.simpleName),
        )
    }

    private fun fixOperationCallArgumentsFix(
        documentUri: String,
        source: String,
        diagnostic: McDiagnostic,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
    ): WorkspaceEditFix? {
        val sites = HandlerSignatureService.findAnnotationSites(source)
        val matchingIssues = mutableListOf<OperationCallIssue>()
        val matchingSites = mutableListOf<MixinExtrasAnnotationSite>()
        for (site in sites) {
            if (site.annotation != MixinExtrasAnnotation.WRAP_OPERATION &&
                site.annotation != MixinExtrasAnnotation.WRAP_METHOD
            ) {
                continue
            }
            val handler = site.handlerMethod ?: continue
            val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
            val resolvedContext = resolvedContextForSite(site, resolvedContexts)
            val expectedSignature = signatureService.expectedSignature(
                source,
                site,
                mixinTargets,
                resolvedContext,
            )
            val layout = OperationCallValidator.resolveOperationCallLayout(site, enriched, expectedSignature)
            val issues = OperationCallValidator.validate(source, site, layout)
            val siteIssues = issues.filter { it.argumentRange == diagnostic.range }
            if (siteIssues.isNotEmpty()) matchingSites += site
            matchingIssues += siteIssues
        }
        val distinctMatchingIssues = matchingIssues.distinct()
        if (distinctMatchingIssues.size != 1) return null

        val matchingSite = matchingSites.firstOrNull() ?: return null
        if (agreedHandlerStub(source, matchingSite, sites, mixinTargets, resolvedContexts) == null) return null

        val issue = distinctMatchingIssues.single()
        val start = AnnotationContextExtractor.toOffset(
            source,
            issue.argumentRange.start.line,
            issue.argumentRange.start.character,
        ) ?: return null
        val end = AnnotationContextExtractor.toOffset(
            source,
            issue.argumentRange.end.line,
            issue.argumentRange.end.character,
        ) ?: return null
        if (start > end || end > source.length) return null

        return WorkspaceEditFix(
            title = "Fix Operation.call arguments",
            kind = "quickfix.mixinextras.fixOperationCallArguments",
            documentUri = documentUri,
            edits = listOf(McTextEdit(start, end, issue.expectedNames.joinToString(", "))),
        )
    }

    private fun fixHandlerSignatureFix(
        documentUri: String,
        source: String,
        site: MixinExtrasAnnotationSite,
        allSites: List<MixinExtrasAnnotationSite>,
        mixinTargets: List<String>,
        resolvedContexts: List<ResolvedMixinExtrasContext>,
    ): WorkspaceEditFix? {
        val handler = site.handlerMethod ?: return null
        if (agreedHandlerStub(
            source,
            site,
            allSites,
            mixinTargets,
            resolvedContexts,
            handlerName = handler.methodName,
        ) == null) return null
        val title = when (site.annotation) {
            MixinExtrasAnnotation.WRAP_OPERATION -> "Fix WrapOperation handler signature"
            MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE -> "Fix ModifyExpressionValue handler signature"
            MixinExtrasAnnotation.MODIFY_RETURN_VALUE -> "Fix ModifyReturnValue handler signature"
            MixinExtrasAnnotation.MODIFY_RECEIVER -> "Fix ModifyReceiver handler signature"
            MixinExtrasAnnotation.WRAP_WITH_CONDITION -> "Fix WrapWithCondition handler signature"
            MixinExtrasAnnotation.WRAP_METHOD -> "Fix WrapMethod handler signature"
            else -> "Fix MixinExtras handler signature"
        }
        val handlerRangeStart = rangeStart(source, handler.range) ?: return null
        val handlerRangeEnd = rangeEnd(source, handler.range) ?: return null
        val returnTypeStart = source.indexOf(handler.returnTypeName, handlerRangeStart)
        if (returnTypeStart < 0 || returnTypeStart >= handlerRangeEnd) return null
        val signatures = relatedSitesForHandlerGroup(site, allSites).map { relatedSite ->
            val relatedHandler = relatedSite.handlerMethod ?: return null
            val expected = signatureService.expectedSignature(
                source,
                relatedSite,
                mixinTargets,
                resolvedContextForSite(relatedSite, resolvedContexts),
            ) ?: return null
            signatureWithExistingParameters(
                source = source,
                handler = relatedHandler,
                expected = expected,
            ) ?: return null
        }
        val signature = signatures.distinctBy { it.text }.singleOrNull() ?: return null
        return WorkspaceEditFix(
            title = title,
            kind = "quickfix.mixinextras.fixHandlerSignature",
            documentUri = documentUri,
            edits = listOf(
                McTextEdit(
                    returnTypeStart,
                    handlerRangeEnd,
                    signature.text,
                ),
            ) + buildImportEdits(source, signatures.flatMap { it.importFqns }.toSet()),
            metadata = mapOf("annotation" to site.annotation.simpleName),
        )
    }

    private fun signatureWithExistingParameters(
        source: String,
        handler: HandlerMethodDeclaration,
        expected: HandlerSignatureSpec,
    ): RenderedSignature? {
        val actualHandler = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
        val renderExpected = signatureService.specializeIntLikeSignature(expected, actualHandler)
        val renderedExpected = renderExpectedSignature(source, renderExpected)
        val actualParameters = actualHandler.parameters
        val firstSugar = actualParameters.indexOfFirst { it.isSugar }
        if (firstSugar >= 0 && actualParameters.drop(firstSugar).any { !it.isSugar }) return null

        val required = renderExpected.parameters
        val actualBase = actualParameters.filterNot { it.isSugar }
        val actualRequired = actualBase.take(required.size)
        val prefixLayout = actualRequired.zip(required).all { (actual, expectedParameter) ->
            actual.isOperation == expectedParameter.isOperation
        }
        if (!prefixLayout) return null
        val captured = if (actualBase.size >= required.size) actualBase.drop(required.size) else emptyList()
        val trailingSugar = actualParameters.filter { it.isSugar }
        if (captured.size > expected.optionalCapturedTargetParameters.size) return null
        if (captured.indices.any { index ->
                !signatureService.parameterTypeMatches(
                    expected.optionalCapturedTargetParameters[index],
                    captured[index],
                )
            }
        ) return null
        val requiredNames = required.mapIndexed { index, expectedParameter ->
            actualRequired.getOrNull(index)?.name ?: expectedParameter.name
        }
        val allNames = requiredNames + captured.map { it.name } + trailingSugar.map { it.name }
        if (allNames.size != allNames.toSet().size) return null

        val preservedTail = (captured + trailingSugar).map { parameter ->
            val range = parameter.range ?: return null
            val start = rangeStart(source, range) ?: return null
            val end = rangeEnd(source, range) ?: return null
            if (start < 0 || start > end || end > source.length) return null
            source.substring(start, end)
        }

        val requiredDeclarations = required.mapIndexed { index, expectedParameter ->
            val actualParameter = actualRequired.getOrNull(index)
            val preserved = if (
                actualParameter != null &&
                !expectedParameter.isOperation &&
                actualParameter.hasCoerce &&
                signatureService.parameterTypeMatches(expectedParameter, actualParameter)
            ) {
                parameterSource(source, actualParameter)
            } else {
                null
            }
            preserved ?: "${renderedExpected.parameterTypes[index]} ${requiredNames[index]}"
        }
        val parameters = (requiredDeclarations + preservedTail).joinToString(", ")
        val preserveReturnType = if (
            actualHandler.returnTypeDescriptor != null &&
            actualHandler.returnTypeDescriptor != renderExpected.returnTypeDescriptor &&
            signatureService.returnTypeMatches(renderExpected, actualHandler)
        ) {
            true
        } else {
            false
        }
        val returnTypeName = if (preserveReturnType) handler.returnTypeName else renderedExpected.returnType
        return RenderedSignature(
            text = "$returnTypeName ${handler.methodName}($parameters)",
            importFqns = renderedExpected.importFqns,
        )
    }

    private fun parameterSource(source: String, parameter: HandlerParameterDeclaration): String? {
        val range = parameter.range ?: return null
        val start = rangeStart(source, range) ?: return null
        val end = rangeEnd(source, range) ?: return null
        if (start < 0 || start > end || end > source.length) return null
        return source.substring(start, end)
    }

    private fun findInsertOffset(source: String, site: MixinExtrasAnnotationSite): Int? =
        rangeEnd(source, site.annotationRange)

    private fun handlerSnippet(
        text: String,
        source: String,
        annotationEndOffset: Int,
        cursorOffset: Int,
    ): String {
        val escaped = text
            .replace("$", "\\$")
            .replace(HANDLER_NAME_MARKER, "\${1}")
        val indentLength = escaped.indexOfFirst { !it.isWhitespace() }.takeIf { it >= 0 } ?: 0
        val declaration = escaped.substring(indentLength)
        val withAccess = if (declaration.startsWith("private ")) {
            escaped
        } else {
            escaped.substring(0, indentLength) + "private " + declaration
        }
        val closing = withAccess.lastIndexOf("\n    }\n")
        val withStop = if (closing < 0) {
            "$withAccess\$0"
        } else {
            val methodIndent = withAccess.takeWhile { it == ' ' || it == '\t' }
            val bodyStart = withAccess.indexOf('\n').takeIf { it >= 0 }?.plus(1) ?: closing
            val body = withAccess.substring(bodyStart, closing)
            val indentedBody = body
                .lineSequence()
                .joinToString("\n") { line -> if (line.isBlank()) line else methodIndent + line }
            val bodyCursor = "${methodIndent}    \$0"
            val bodyContent = if (indentedBody.isBlank()) bodyCursor else "$bodyCursor\n$indentedBody"
            withAccess.substring(0, bodyStart) + bodyContent + withAccess.substring(closing)
        }
        val safeCursor = cursorOffset.coerceIn(0, source.length)
        val safeAnnotationEnd = annotationEndOffset.coerceIn(0, safeCursor)
        val afterAnnotation = source.substring(safeAnnotationEnd, safeCursor)
        val sameLineAsAnnotation = !afterAnnotation.any { it == '\n' || it == '\r' }
        if (sameLineAsAnnotation) return "\n$withStop"

        val lineStart = source.lastIndexOf('\n', safeCursor - 1).let { if (it < 0) 0 else it + 1 }
        val existingIndent = source.substring(lineStart, safeCursor)
        if (existingIndent.isNotEmpty() && existingIndent.all { it == ' ' || it == '\t' }) {
            val generatedIndent = withStop.takeWhile { it == ' ' || it == '\t' }
            val consumedIndent = existingIndent.takeIf { generatedIndent.startsWith(it) }.orEmpty()
            if (consumedIndent.isNotEmpty()) return withStop.removePrefix(consumedIndent)
        }
        return withStop
    }

    private fun rangeStart(source: String, range: io.github.mcdev.core.diagnostics.McTextRange): Int? =
        AnnotationContextExtractor.toOffset(source, range.start.line, range.start.character)

    private fun rangeEnd(source: String, range: io.github.mcdev.core.diagnostics.McTextRange): Int? =
        AnnotationContextExtractor.toOffset(source, range.end.line, range.end.character)

    companion object {
        private const val HANDLER_NAME_MARKER = "__MCDEV_HANDLER_NAME__"

        fun deduplicateFixes(fixes: List<McFix>): List<McFix> =
            fixes.distinctBy { fix ->
                when (fix) {
                    is WorkspaceEditFix -> fix.documentUri to fix.edits.map { edit ->
                        "${edit.startOffset}\u0000${edit.endOffset}\u0000${edit.newText}"
                    }
                    else -> fix.title to fix.kind
                }
            }
    }
}
