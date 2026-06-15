package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mixinextras.ExpressionSupport
import io.github.mcdev.core.mixinextras.MixinExtrasCodeActionService
import io.github.mcdev.core.mixinextras.MixinExtrasCompletionService
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticRequest
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticsService

data class MixinFacadeRequest(
    val bufferText: String,
    val line: Int,
    val character: Int,
    val documentUri: String = "file:///Mixin.java",
    val mixinClassName: String? = null,
    val mixinPackage: String? = null,
    val mixinConfigContent: String? = null,
    val mixinConfigPath: String? = null,
)

class MixinServiceFacade(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val mixinTargetCompletion: MixinTargetCompletionService = MixinTargetCompletionService(classIndex),
    private val injectMethodCompletion: InjectMethodCompletionService = InjectMethodCompletionService(classIndex),
    private val atValueCompletion: AtValueCompletionService = AtValueCompletionService(),
    private val atTargetCompletion: AtTargetCompletionService = AtTargetCompletionService(),
    private val shadowValidation: ShadowValidationService = ShadowValidationService(classIndex),
    private val accessorService: AccessorService = AccessorService(classIndex),
    private val invokerService: InvokerService = InvokerService(classIndex),
    private val diagnosticsService: MixinDiagnosticsService = MixinDiagnosticsService(classIndex, bytecodeIndex),
    private val codeActionService: MixinCodeActionService = MixinCodeActionService(
        accessorService = accessorService,
        invokerService = invokerService,
    ),
    private val mixinExtrasCompletion: MixinExtrasCompletionService = MixinExtrasCompletionService(
        classIndex,
        bytecodeIndex,
    ),
    private val mixinExtrasDiagnostics: MixinExtrasDiagnosticsService = MixinExtrasDiagnosticsService(classIndex, bytecodeIndex),
    private val mixinExtrasCodeActions: MixinExtrasCodeActionService = MixinExtrasCodeActionService(classIndex),
    private val expressionSupport: ExpressionSupport = ExpressionSupport(),
) {
    private val mixinExtrasMethodAnnotations = setOf(
        MixinAnnotation.MODIFY_EXPRESSION_VALUE,
        MixinAnnotation.MODIFY_RETURN_VALUE,
        MixinAnnotation.MODIFY_RECEIVER,
        MixinAnnotation.WRAP_OPERATION,
        MixinAnnotation.WRAP_WITH_CONDITION,
        MixinAnnotation.WRAP_METHOD,
    )
    fun complete(
        request: MixinFacadeRequest,
        options: MixinCompletionOptions = MixinCompletionOptions(),
    ): List<McCompletionItem> {
        val offset = AnnotationContextExtractor.toOffset(request.bufferText, request.line, request.character)
            ?: return emptyList()
        val context = AnnotationContextExtractor.extractAtOffset(request.bufferText, offset) ?: return emptyList()
        return routeCompletion(request.bufferText, context, options)
    }

    fun diagnose(request: MixinFacadeRequest): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        diagnostics += diagnosticsService.analyze(
            MixinDiagnosticRequest(
                source = request.bufferText,
                documentUri = request.documentUri,
                mixinClassName = request.mixinClassName,
                mixinPackage = request.mixinPackage,
                mixinConfigContent = request.mixinConfigContent,
                mixinConfigPath = request.mixinConfigPath,
            ),
        )
        diagnostics += analyzeMemberDeclarations(request.bufferText)
        diagnostics += mixinExtrasDiagnostics.analyze(
            MixinExtrasDiagnosticRequest(
                source = request.bufferText,
                documentUri = request.documentUri,
            ),
        )
        return diagnostics
    }

    fun codeActions(
        request: MixinFacadeRequest,
        diagnosticCode: String? = null,
    ): List<McFix> {
        val diagnostics = diagnose(request).filter { diagnosticCode == null || it.code == diagnosticCode }
        val mixinFixes = codeActionService.fixesForDiagnostics(
            diagnostics = diagnostics,
            documentUri = request.documentUri,
            source = request.bufferText,
            mixinConfigContent = request.mixinConfigContent,
            mixinConfigPath = request.mixinConfigPath,
            mixinPackage = request.mixinPackage,
            classIndex = classIndex,
        )
        val extrasFixes = mixinExtrasCodeActions.fixesForDiagnostics(
            diagnostics = diagnostics,
            documentUri = request.documentUri,
            source = request.bufferText,
        )
        return (mixinFixes + extrasFixes).distinctBy { it.title }
    }

    private fun routeCompletion(
        source: String,
        context: AnnotationContext,
        options: MixinCompletionOptions,
    ): List<McCompletionItem> =
        when (context.annotation) {
            MixinAnnotation.MIXIN -> mixinTargetCompletion.complete(context, options)
            MixinAnnotation.INJECT,
            MixinAnnotation.REDIRECT,
            MixinAnnotation.MODIFY_ARG,
            MixinAnnotation.MODIFY_ARGS,
            MixinAnnotation.MODIFY_VARIABLE,
            MixinAnnotation.MODIFY_CONSTANT,
            -> injectMethodCompletion.complete(context.withResolvedMixinTargets(source), options)
            in mixinExtrasMethodAnnotations -> completeMixinExtrasMethod(source, context, options)
            MixinAnnotation.AT -> when (context.slot) {
                AnnotationSlot.VALUE -> {
                    val expressionItems = expressionSupport.completeAtValue(context)
                    if (expressionItems.isNotEmpty()) expressionItems else atValueCompletion.complete(context)
                }
                AnnotationSlot.TARGET -> {
                    val extrasItems = mixinExtrasCompletion.complete(context.withResolvedMixinTargets(source), options)
                    if (extrasItems.isNotEmpty()) extrasItems else completeAtTarget(source, context)
                }
                else -> emptyList()
            }
            MixinAnnotation.ACCESSOR -> if (context.slot == AnnotationSlot.ACCESSOR_VALUE) {
                accessorService.completeFields(
                    mixinTargets = resolveMixinTargets(source, context),
                    prefix = context.partialValue.trim('"'),
                )
            } else {
                emptyList()
            }
            MixinAnnotation.INVOKER -> if (context.slot == AnnotationSlot.INVOKER_VALUE) {
                invokerService.completeMethods(
                    mixinTargets = resolveMixinTargets(source, context),
                    prefix = context.partialValue.trim('"'),
                )
            } else {
                emptyList()
            }
            MixinAnnotation.SHADOW -> completeShadow(source, context)
            else -> emptyList()
        }

    private fun completeMixinExtrasMethod(
        source: String,
        context: AnnotationContext,
        options: MixinCompletionOptions,
    ): List<McCompletionItem> {
        val resolvedContext = context.withResolvedMixinTargets(source)
        val extrasItems = mixinExtrasCompletion.complete(resolvedContext, options)
        if (extrasItems.isNotEmpty() || context.annotation != MixinAnnotation.MODIFY_RECEIVER) {
            return extrasItems
        }
        return injectMethodCompletion.complete(resolvedContext, options)
    }

    private fun completeAtTarget(source: String, context: AnnotationContext): List<McCompletionItem> {
        val owners = resolveMixinTargets(source, context)
        val owner = owners.firstOrNull() ?: return emptyList()
        val methodTarget = parseMethodTarget(
            context.injectMethodName
            ?: findEnclosingInjectorMethod(source, context.valueStartOffset)
            ?: return emptyList(),
        )
        val atValue = context.atValue
            ?: findAtValueInAnnotationBody(source, context.annotationStartOffset, context.annotationEndOffset)
            ?: return emptyList()
        val candidates = bytecodeIndex.getAtTargetCandidates(
            owner,
            methodTarget.name,
            methodTarget.descriptor,
            atValue,
        )
        return atTargetCompletion.complete(context, candidates)
    }

    private data class MethodTarget(
        val name: String,
        val descriptor: String?,
    )

    private fun parseMethodTarget(value: String): MethodTarget {
        val paren = value.indexOf('(')
        return if (paren > 0) {
            MethodTarget(value.substring(0, paren), value.substring(paren))
        } else {
            MethodTarget(value, null)
        }
    }

    private fun findAtValueInAnnotationBody(source: String, annotationStart: Int, annotationEnd: Int): String? {
        val bodyStart = source.indexOf('(', annotationStart).takeIf { it in annotationStart until annotationEnd } ?: return null
        val body = source.substring(bodyStart, annotationEnd.coerceAtMost(source.length))
        return Regex("""value\s*=\s*"([^"]*)"""").find(body)?.groupValues?.get(1)
    }

    private fun resolveMixinTargets(source: String, context: AnnotationContext): List<String> {
        val rawTargets = context.mixinTargetInternalNames.ifEmpty {
            AnnotationContextExtractor.resolveRawMixinTargets(source, context.valueStartOffset)
        }
        return MixinTargetResolver.resolveTargets(rawTargets, classIndex, JavaTypeDescriptorResolver.importsFor(source))
    }

    private fun findEnclosingInjectorMethod(source: String, cursorOffset: Int): String? {
        val before = source.substring(0, cursorOffset.coerceIn(0, source.length))
        val pattern = Regex(
            """@(Inject|Redirect|ModifyArg|ModifyArgs|ModifyVariable|ModifyConstant|ModifyExpressionValue|ModifyReturnValue|ModifyReceiver|WrapOperation|WrapWithCondition|WrapMethod)\s*\(""",
        )
        val match = pattern.findAll(before).lastOrNull() ?: return null
        val bodyStart = match.range.last
        val bodyEnd = findMatchingParen(source, bodyStart) ?: return null
        val body = source.substring(bodyStart, bodyEnd + 1)
        val methodMatch = Regex("method\\s*=\\s*\"([^\"]+)\"").find(body) ?: return null
        return methodMatch.groupValues[1]
    }

    private fun findMatchingParen(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '(') return null
        var depth = 0
        var inString = false
        var i = openIndex
        while (i < source.length) {
            when {
                inString -> {
                    if (source[i] == '\\') {
                        i += 2
                        continue
                    }
                    if (source[i] == '"') inString = false
                }
                source[i] == '"' -> inString = true
                source[i] == '(' -> depth++
                source[i] == ')' -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return null
    }

    private fun AnnotationContext.withResolvedMixinTargets(source: String): AnnotationContext =
        copy(mixinTargetInternalNames = resolveMixinTargets(source, this))

    private fun completeShadow(source: String, context: AnnotationContext): List<McCompletionItem> {
        if (context.slot != AnnotationSlot.SHADOW_MEMBER) return emptyList()
        val prefix = context.partialValue
        val targets = resolveMixinTargets(source, context)
        val fields = shadowValidation.completeFields(targets, prefix, context.shadowPrefix)
        val methods = shadowValidation.completeMethods(targets, prefix, context.shadowPrefix)
        val fieldItems = fields.map { field ->
            McCompletionItem(
                label = field.name,
                detail = field.readableType,
                documentation = field.descriptor,
                filterText = "${field.name} ${field.readableType}",
                insertText = field.name,
                kind = McCompletionKind.FIELD,
                sortKey = "0700_${field.name}",
                metadata = McCompletionMetadata(source = "mixin.shadow", name = field.name, descriptor = field.descriptor),
            )
        }
        val methodItems = methods.map { method ->
            McCompletionItem(
                label = method.readableSignature,
                detail = method.descriptor,
                documentation = method.readableSignature,
                filterText = "${method.name} ${method.readableSignature}",
                insertText = method.name,
                kind = McCompletionKind.METHOD,
                sortKey = "0701_${method.name}",
                metadata = McCompletionMetadata(
                    source = "mixin.shadow",
                    name = method.name,
                    descriptor = method.descriptor,
                ),
            )
        }
        return fieldItems + methodItems
    }

    private fun analyzeMemberDeclarations(source: String): List<McDiagnostic> {
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        if (mixinTargets.isEmpty()) return emptyList()
        val diagnostics = mutableListOf<McDiagnostic>()
        MixinMemberDeclarationParser.parseShadowDeclarations(source, classIndex).forEach { declaration ->
            val prefix = MixinMemberDeclarationParser.findShadowPrefix(source)
            val remap = MixinMemberDeclarationParser.findShadowRemap(source)
            diagnostics += shadowValidation.validate(mixinTargets, declaration, prefix, remap)
        }
        MixinMemberDeclarationParser.parseAccessorDeclarations(source, classIndex).forEach { declaration ->
            diagnostics += accessorService.validate(mixinTargets, declaration)
        }
        MixinMemberDeclarationParser.parseInvokerDeclarations(source, classIndex).forEach { declaration ->
            diagnostics += invokerService.validate(mixinTargets, declaration)
        }
        return diagnostics
    }
}
