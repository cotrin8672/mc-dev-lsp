package io.github.mcdev.core.mixin

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.FieldSelector
import io.github.mcdev.core.descriptor.MethodSelector
import io.github.mcdev.core.descriptor.Pattern
import io.github.mcdev.core.descriptor.parseFieldSelector
import io.github.mcdev.core.descriptor.parseMethodSelector
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition
import io.github.mcdev.core.mixinextras.ExpressionContextResolver
import io.github.mcdev.core.mixinextras.MixinExtrasAnnotation
import io.github.mcdev.core.mixinextras.ClassLiteralTypeNameResolver
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.model.MappingNamespace
import org.objectweb.asm.Type

class MixinDefinitionService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val accessorService: AccessorService = AccessorService(classIndex),
    private val invokerService: InvokerService = InvokerService(classIndex),
) {
    fun definitionsAt(
        source: String,
        line: Int,
        character: Int,
        semanticModel: MixinClassModel? = null,
        documentUri: String? = null,
    ): List<McDefinitionTarget> {
        val offset = AnnotationContextExtractor.toOffset(source, line, character) ?: return emptyList()
        return definitionsAtOffset(source, offset, semanticModel, documentUri)
    }

    fun definitionsAtOffset(
        source: String,
        offset: Int,
        semanticModel: MixinClassModel? = null,
        documentUri: String? = null,
    ): List<McDefinitionTarget> {
        if (offset < 0 || offset > source.length) return emptyList()

        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        if (context != null) {
            val fromContext = resolveFromAnnotationContext(source, context, semanticModel, offset, documentUri)
            if (fromContext.isNotEmpty()) return fromContext
        }

        resolveAtTargetStringAtOffset(source, offset)?.let { return it }

        return resolveFromMemberDeclaration(source, offset, semanticModel)
    }

    private fun resolveAtTargetStringAtOffset(source: String, offset: Int): List<McDefinitionTarget>? {
        val pattern = Regex("""target\s*=\s*"([^"]*)"""")
        pattern.findAll(source).forEach { match ->
            val valueStart = match.range.first + match.value.indexOf('"') + 1
            val valueEnd = valueStart + match.groupValues[1].length
            if (offset in valueStart..valueEnd) {
                val parsed = AtTargetParser.parse(match.groupValues[1]) ?: return null
                return listOf(
                    McDefinitionTarget(
                        kind = parsed.kind,
                        ownerInternalName = parsed.ownerInternalName,
                        ownerFqn = classIndex.findClass(parsed.ownerInternalName)?.fqn
                            ?: AnnotationContextExtractor.internalToFqn(parsed.ownerInternalName),
                        name = parsed.name,
                        descriptor = parsed.descriptor,
                        sourceRange = offsetRange(source, valueStart, valueEnd),
                    ),
                )
            }
        }
        return null
    }

    private fun resolveFromAnnotationContext(
        source: String,
        context: AnnotationContext,
        semanticModel: MixinClassModel?,
        offset: Int,
        documentUri: String?,
    ): List<McDefinitionTarget> {
        if (context.slot == AnnotationSlot.METHOD && MixinExtrasAnnotation.fromMixinAnnotation(context.annotation) != null) {
            return resolveInjectorMethodTarget(source, context, semanticModel)
        }
        return when (context.annotation) {
            MixinAnnotation.MIXIN -> when (context.slot) {
                AnnotationSlot.CLASS,
                AnnotationSlot.TARGETS,
                    -> resolveMixinClassTarget(source, context, semanticModel, offset)
                else -> emptyList()
            }
            MixinAnnotation.SHADOW -> when (context.slot) {
                AnnotationSlot.SHADOW_MEMBER -> resolveShadowMemberAtOffset(source, offset)
                else -> emptyList()
            }
            MixinAnnotation.OVERWRITE -> when (context.slot) {
                AnnotationSlot.OVERWRITE_METHOD -> resolveOverwriteMethodAtOffset(source, offset)
                else -> emptyList()
            }
            MixinAnnotation.ACCESSOR -> when (context.slot) {
                AnnotationSlot.ACCESSOR_VALUE -> resolveAccessorTarget(source, context, semanticModel)
                else -> emptyList()
            }
            MixinAnnotation.INVOKER -> when (context.slot) {
                AnnotationSlot.INVOKER_VALUE -> resolveInvokerTarget(source, context, semanticModel)
                else -> emptyList()
            }
            MixinAnnotation.AT -> when (context.slot) {
                AnnotationSlot.TARGET -> resolveAtTarget(source, context)
                else -> emptyList()
            }
            MixinAnnotation.DEFINITION -> when (context.slot) {
                AnnotationSlot.VALUE -> resolveDefinitionMemberReference(source, context)
                else -> emptyList()
            }
            MixinAnnotation.LOCAL -> resolveDefinitionLocalType(source, context)
            MixinAnnotation.EXPRESSION,
            MixinAnnotation.EXPRESSIONS,
            -> when (context.slot) {
                AnnotationSlot.VALUE -> resolveExpressionDefinitionIdReference(source, context, offset, documentUri)
                else -> emptyList()
            }
            else -> emptyList()
        }
    }

    private fun offsetFromContext(context: AnnotationContext): Int =
        context.valueEndOffset.coerceAtLeast(context.valueStartOffset)

    private fun resolveMixinClassTarget(
        source: String,
        context: AnnotationContext,
        semanticModel: MixinClassModel?,
        cursorOffset: Int,
    ): List<McDefinitionTarget> {
        val raw = context.partialValue.trim().trim('"').removeSuffix(".class")
        if (raw.isEmpty()) return emptyList()
        val semanticTarget = semanticModel?.targets?.firstOrNull { target ->
            val start = positionToOffset(source, target.range.start)
            val end = positionToOffset(source, target.range.end).coerceAtLeast(start)
            cursorOffset in start..end
        }
        val entry = if (semanticTarget != null) {
            classIndex.findClass(semanticTarget.internalName)
                ?: MixinTargetResolver.resolveTarget(
                    semanticTarget.internalName,
                    classIndex,
                    JavaTypeDescriptorResolver.importsFor(source),
                )
                    ?.let(classIndex::findClass)
                ?: return emptyList()
        } else {
            val completeRaw = completeMixinTargetValue(source, context, cursorOffset)
            resolveClassEntry(
                raw = completeRaw ?: raw,
                imports = JavaTypeDescriptorResolver.importsFor(source),
                allowPrefix = completeRaw == null,
            ) ?: return emptyList()
        }
        val internalName = entry.internalName
        return listOf(
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = internalName,
                ownerFqn = entry?.fqn ?: AnnotationContextExtractor.internalToFqn(internalName),
                sourceRange = offsetRange(source, context.valueStartOffset, context.valueEndOffset),
            ),
        )
    }

    private fun completeMixinTargetValue(
        source: String,
        context: AnnotationContext,
        cursorOffset: Int,
    ): String? {
        val start = context.valueStartOffset.coerceIn(0, source.length)
        val end = context.valueEndOffset.coerceIn(start, source.length)
        if (start >= end || cursorOffset !in start..end) return null
        val candidate = source.substring(start, end).trim()
        if (candidate.isEmpty()) return null
        if (candidate.endsWith(".class")) return candidate.removeSuffix(".class").trim()
        return null
    }

    private fun resolveOverwriteMethodAtOffset(source: String, offset: Int): List<McDefinitionTarget> {
        val declaration = findOverwriteMethodAtOffset(source, offset) ?: return emptyList()
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        if (mixinTargets.isEmpty()) return emptyList()
        return resolveMemberInTargets(
            mixinTargets = mixinTargets,
            name = declaration.name,
            isMethod = true,
            descriptor = declaration.descriptor,
            sourceRange = declaration.range,
        )
    }

    private fun resolveShadowMemberAtOffset(source: String, offset: Int): List<McDefinitionTarget> {
        val declaration = findShadowMemberAtOffset(source, offset) ?: return emptyList()
        val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        if (mixinTargets.isEmpty()) return emptyList()
        val prefix = MixinMemberDeclarationParser.findShadowPrefix(source)
        val targetName = applyShadowPrefix(declaration.name, prefix)
        return resolveMemberInTargets(
            mixinTargets = mixinTargets,
            name = targetName,
            isMethod = declaration.isMethod,
            descriptor = declaration.descriptor,
            sourceRange = declaration.range,
        )
    }

    private fun resolveFromMemberDeclaration(
        source: String,
        offset: Int,
        semanticModel: MixinClassModel?,
    ): List<McDefinitionTarget> {
        semanticModel?.members
            ?.asSequence()
            ?.mapNotNull { member ->
                semanticMemberNameRange(source, member)?.let { nameRange -> member to nameRange }
            }
            ?.firstOrNull { (_, nameRange) ->
                val start = positionToOffset(source, nameRange.start)
                val end = positionToOffset(source, nameRange.end)
                offset in start..end
            }
            ?.let { (member, nameRange) ->
            val mixinTargets = resolveMixinTargets(source, null, semanticModel)
            return when (member.annotationKind) {
                MixinMemberAnnotationKind.SHADOW,
                MixinMemberAnnotationKind.OVERWRITE,
                -> resolveMemberInTargets(
                    mixinTargets = mixinTargets,
                    name = member.explicitTargetName ?: member.javaName,
                    isMethod = member.isMethod ?: (member.methodDescriptor != null),
                    descriptor = if (member.isMethod ?: (member.methodDescriptor != null)) {
                        member.methodDescriptor
                    } else {
                        member.returnDescriptor.orEmpty()
                    },
                    sourceRange = nameRange,
                )
                MixinMemberAnnotationKind.ACCESSOR -> {
                    val declaration = AccessorMethodDeclaration(
                        methodName = member.javaName,
                        returnTypeDescriptor = member.returnDescriptor,
                        parameterDescriptors = member.parameterDescriptors,
                        explicitFieldName = member.explicitTargetName,
                        range = member.range,
                        parseSource = member.parseSource,
                        confidence = member.confidence,
                        warnings = member.warnings,
                    )
                    accessorService.inferFieldName(declaration)?.let {
                        resolveFieldInTargets(mixinTargets, it, nameRange)
                    }.orEmpty()
                }
                MixinMemberAnnotationKind.INVOKER -> {
                    val declaration = InvokerMethodDeclaration(
                        methodName = member.javaName,
                        parameterDescriptors = member.parameterDescriptors,
                        returnTypeDescriptor = member.returnDescriptor,
                        explicitTargetName = member.explicitTargetName,
                        range = member.range,
                        parseSource = member.parseSource,
                        confidence = member.confidence,
                        warnings = member.warnings,
                    )
                    val targetName = declaration.explicitTargetName ?: invokerService.inferTargetName(declaration)
                    targetName?.let {
                        resolveMethodInTargets(mixinTargets, it, nameRange, member.methodDescriptor)
                    }.orEmpty()
                }
            }
        }
        findShadowMemberAtOffset(source, offset)?.let { declaration ->
            return resolveShadowMemberAtOffset(source, offset)
        }

        findOverwriteMethodAtOffset(source, offset)?.let { declaration ->
            return resolveOverwriteMethodAtOffset(source, offset)
        }

        MixinMemberDeclarationParser.parseAccessorDeclarations(source, classIndex).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.methodName, "Accessor", isMethod = true)
                ?: return@forEach
            if (offset in nameRange.first until nameRange.last) {
                val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
                val fieldName = accessorService.inferFieldName(declaration) ?: return emptyList()
                return resolveFieldInTargets(mixinTargets, fieldName, declaration.range)
            }
        }

        MixinMemberDeclarationParser.parseInvokerDeclarations(source, classIndex).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.methodName, "Invoker", isMethod = true)
                ?: return@forEach
            if (offset in nameRange.first until nameRange.last) {
                val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
                val methodName = declaration.explicitTargetName
                    ?: invokerService.inferTargetName(declaration)
                    ?: return emptyList()
                return resolveMethodInTargets(
                    mixinTargets,
                    methodName,
                    declaration.range,
                    descriptor = descriptorFromInvokerDeclaration(declaration),
                )
            }
        }

        return emptyList()
    }

    private fun resolveAccessorTarget(
        source: String,
        context: AnnotationContext,
        semanticModel: MixinClassModel?,
    ): List<McDefinitionTarget> {
        val declaration = findAccessorDeclarationNear(source, context)
        if (declaration == null && hasMemberParseFailureNear(source, context)) return emptyList()
        val fieldName = context.partialValue.trim('"').ifEmpty {
            declaration?.let { accessorService.inferFieldName(it) }
        } ?: return emptyList()
        val mixinTargets = resolveMixinTargets(source, context, semanticModel)
        val range = offsetRange(source, context.valueStartOffset, context.valueEndOffset)
        return resolveFieldInTargets(mixinTargets, fieldName, range)
    }

    private fun resolveInvokerTarget(
        source: String,
        context: AnnotationContext,
        semanticModel: MixinClassModel?,
    ): List<McDefinitionTarget> {
        val declaration = findInvokerDeclarationNear(source, context)
        if (declaration == null && hasMemberParseFailureNear(source, context)) return emptyList()
        val methodName = context.partialValue.trim('"').ifEmpty {
            declaration?.let { invokerService.inferTargetName(it) }
        } ?: return emptyList()
        val mixinTargets = resolveMixinTargets(source, context, semanticModel)
        val range = offsetRange(source, context.valueStartOffset, context.valueEndOffset)
        val descriptor = declaration?.let(::descriptorFromInvokerDeclaration)
        return resolveMethodInTargets(mixinTargets, methodName, range, descriptor)
    }

    private fun resolveInjectorMethodTarget(
        source: String,
        context: AnnotationContext,
        semanticModel: MixinClassModel?,
    ): List<McDefinitionTarget> {
        val rawValue = definitionAttributeRawValue(source, context) ?: return emptyList()
        if (rawValue.contains('*')) return emptyList()
        val selector = when (val parsed = parseMethodSelector(rawValue)) {
            is DescriptorParseResult.Success -> parsed.value
            is DescriptorParseResult.Failure -> return emptyList()
        }
        val methodName = when (val namePattern = selector.name) {
            is Pattern.Exact -> namePattern.value
            is Pattern.Any -> return emptyList()
        }
        val descriptor = when (val descriptorPattern = selector.descriptor) {
            is Pattern.Exact -> DescriptorRenderer.toDescriptor(descriptorPattern.value)
            is Pattern.Any -> null
        }
        val mixinTargets = resolveMixinTargets(source, context, semanticModel)
        if (mixinTargets.isEmpty()) return emptyList()
        val targetOwners = when (val ownerPattern = selector.owner) {
            is Pattern.Exact -> {
                if (ownerPattern.value !in mixinTargets) return emptyList()
                listOf(ownerPattern.value)
            }
            is Pattern.Any -> mixinTargets
        }
        return resolveMethodInTargets(
            mixinTargets = targetOwners,
            methodName = methodName,
            sourceRange = offsetRange(source, context.valueStartOffset, context.valueEndOffset),
            descriptor = descriptor,
        )
    }

    private fun resolveExpressionDefinitionIdReference(
        source: String,
        context: AnnotationContext,
        offset: Int,
        documentUri: String?,
    ): List<McDefinitionTarget> {
        val position = context.expressionCompletionPosition ?: return emptyList()
        if (position != ExpressionCompletionPosition.STATEMENT_START &&
            position != ExpressionCompletionPosition.VALUE_START &&
            position != ExpressionCompletionPosition.AFTER_DOT &&
            position != ExpressionCompletionPosition.AFTER_METHOD_REFERENCE
        ) {
            return emptyList()
        }
        if (offset !in context.valueStartOffset until context.valueEndOffset) return emptyList()

        val identifier = context.decodedPartialValue
            ?: source.substring(context.valueStartOffset, context.valueEndOffset)
        if (!isExpressionDefinitionIdentifier(identifier)) return emptyList()

        val site = ExpressionContextResolver.findEnclosingSite(source, context.valueStartOffset) ?: return emptyList()
        val expressionContext = ExpressionContextResolver.resolveExpressionContext(source, site)
        val matches = expressionContext.definitionIndex.definitionsWithId(identifier)
        return matches.mapNotNull { definition ->
            if (position == ExpressionCompletionPosition.AFTER_DOT &&
                definition.rawFieldReferences.isEmpty() && definition.rawMethodReferences.isEmpty()
            ) return@mapNotNull null
            if (position == ExpressionCompletionPosition.AFTER_METHOD_REFERENCE &&
                definition.rawMethodReferences.isEmpty()
            ) return@mapNotNull null
            val idRange = definition.idSourceRange ?: return@mapNotNull null
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = "",
                ownerFqn = null,
                name = identifier,
                sourceRange = definitionIdSourceRange(source, idRange),
                directSourceDocumentUri = documentUri,
            )
        }
    }

    private fun definitionIdSourceRange(source: String, idRange: IntRange): McTextRange =
        offsetRange(source, idRange.first, idRange.last + 1)

    private fun isExpressionDefinitionIdentifier(identifier: String): Boolean {
        if (identifier.isEmpty()) return false
        if (identifier in EXPRESSION_DEFINITION_IDENTIFIER_KEYWORDS) return false
        if (!identifier[0].isJavaIdentifierStart()) return false
        if (identifier.drop(1).any { !it.isJavaIdentifierPart() }) return false
        return true
    }

    private fun Char.isJavaIdentifierStart(): Boolean = isLetter() || this == '_' || this == '$'

    private fun Char.isJavaIdentifierPart(): Boolean = isLetterOrDigit() || this == '_' || this == '$'

    private fun resolveDefinitionMemberReference(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        when (context.attributeName) {
            "method" -> return resolveExactDefinitionMethod(source, context)
            "field" -> return resolveExactDefinitionField(source, context)
            "type" -> return resolveDefinitionClassLiteralType(source, context)
            else -> return emptyList()
        }
    }

    private fun resolveDefinitionLocalType(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        if (context.slot != AnnotationSlot.VALUE || context.attributeName != "type") return emptyList()
        val localStart = context.annotationStartOffset
        val insideDefinition = AnnotationContextExtractor.findAnnotationOffsets(source, MixinAnnotation.DEFINITION)
            .any { definition ->
                val definitionEnd = AnnotationContextExtractor.annotationEndOffset(source, definition)
                localStart in (definition + 1) until definitionEnd
            }
        if (!insideDefinition) return emptyList()
        return resolveDefinitionClassLiteralType(source, context)
    }

    private fun resolveDefinitionClassLiteralType(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        if (context.slot != AnnotationSlot.VALUE || context.attributeName != "type") return emptyList()
        val start = context.valueStartOffset.coerceIn(0, source.length)
        val end = context.valueEndOffset.coerceIn(start, source.length)
        val rawLiteral = source.substring(start, end)
        val literal = rawLiteral.trim()
        val suffix = literal.indexOf(".class")
        if (suffix < 0 || literal.substring(suffix + ".class".length).isNotBlank()) return emptyList()
        val typeName = literal.substring(0, suffix).trim()
        val resolvedType = ClassLiteralTypeNameResolver.forSource(source, classIndex).resolve(typeName)
            ?.let { if (it.sort == Type.ARRAY) it.elementType else it }
            ?.takeIf { it.sort == Type.OBJECT }
            ?: return emptyList()
        val entry = classIndex.findClass(resolvedType.internalName) ?: return emptyList()
        val literalStart = start + rawLiteral.indexOf(literal)
        val literalEnd = literalStart + suffix + ".class".length
        return listOf(
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = entry.internalName,
                ownerFqn = entry.fqn,
                sourceRange = offsetRange(source, literalStart, literalEnd),
            ),
        )
    }

    private fun resolveExactDefinitionMethod(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        val rawValue = definitionAttributeRawValue(source, context) ?: return emptyList()
        val selector = when (val parsed = parseMethodSelector(rawValue)) {
            is DescriptorParseResult.Success -> parsed.value
            is DescriptorParseResult.Failure -> return emptyList()
        }
        if (!isExactMethodSelector(selector, rawValue)) return emptyList()
        val owner = (selector.owner as Pattern.Exact).value
        return listOf(
            McDefinitionTarget(
                kind = MemberKind.METHOD,
                ownerInternalName = owner,
                ownerFqn = classIndex.findClass(owner)?.fqn
                    ?: AnnotationContextExtractor.internalToFqn(owner),
                name = (selector.name as Pattern.Exact).value,
                descriptor = DescriptorRenderer.toDescriptor((selector.descriptor as Pattern.Exact).value),
                sourceRange = offsetRange(source, context.valueStartOffset, context.valueEndOffset),
            ),
        )
    }

    private fun resolveExactDefinitionField(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        val rawValue = definitionAttributeRawValue(source, context) ?: return emptyList()
        val selector = when (val parsed = parseFieldSelector(rawValue)) {
            is DescriptorParseResult.Success -> parsed.value
            is DescriptorParseResult.Failure -> return emptyList()
        }
        if (!isExactFieldSelector(selector, rawValue)) return emptyList()
        val owner = (selector.owner as Pattern.Exact).value
        return listOf(
            McDefinitionTarget(
                kind = MemberKind.FIELD,
                ownerInternalName = owner,
                ownerFqn = classIndex.findClass(owner)?.fqn
                    ?: AnnotationContextExtractor.internalToFqn(owner),
                name = (selector.name as Pattern.Exact).value,
                descriptor = DescriptorRenderer.toDescriptor((selector.descriptor as Pattern.Exact).value),
                sourceRange = offsetRange(source, context.valueStartOffset, context.valueEndOffset),
            ),
        )
    }

    private fun definitionAttributeRawValue(source: String, context: AnnotationContext): String? {
        val start = context.valueStartOffset.coerceIn(0, source.length)
        val end = context.valueEndOffset.coerceIn(start, source.length)
        if (start >= end) return null
        return source.substring(start, end).trim('"').takeIf { it.isNotEmpty() }
    }

    private fun isExactMethodSelector(selector: MethodSelector, rawValue: String): Boolean =
        !rawValue.contains('*') &&
            selector.owner is Pattern.Exact &&
            selector.name is Pattern.Exact &&
            selector.descriptor is Pattern.Exact

    private fun isExactFieldSelector(selector: FieldSelector, rawValue: String): Boolean =
        !rawValue.contains('*') &&
            selector.owner is Pattern.Exact &&
            selector.name is Pattern.Exact &&
            selector.descriptor is Pattern.Exact

    private fun resolveAtTarget(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        val targetValue = context.partialValue.trim('"')
        if (targetValue.isEmpty()) return emptyList()
        val parsed = AtTargetParser.parse(targetValue) ?: return emptyList()
        val range = offsetRange(source, context.valueStartOffset, context.valueEndOffset)
        return listOf(
            McDefinitionTarget(
                kind = parsed.kind,
                ownerInternalName = parsed.ownerInternalName,
                ownerFqn = classIndex.findClass(parsed.ownerInternalName)?.fqn
                    ?: AnnotationContextExtractor.internalToFqn(parsed.ownerInternalName),
                name = parsed.name,
                descriptor = parsed.descriptor,
                sourceRange = range,
            ),
        )
    }

    private fun resolveFieldInTargets(
        mixinTargets: List<String>,
        fieldName: String,
        sourceRange: McTextRange,
    ): List<McDefinitionTarget> {
        for (owner in mixinTargets) {
            val field = classIndex.getFields(owner).find { it.name == fieldName } ?: continue
            return listOf(
                McDefinitionTarget(
                    kind = MemberKind.FIELD,
                    ownerInternalName = owner,
                    ownerFqn = classIndex.findClass(owner)?.fqn
                        ?: AnnotationContextExtractor.internalToFqn(owner),
                    name = field.name,
                    descriptor = field.descriptor,
                    sourceRange = sourceRange,
                ),
            )
        }
        return emptyList()
    }

    private fun resolveMethodInTargets(
        mixinTargets: List<String>,
        methodName: String,
        sourceRange: McTextRange,
        descriptor: String? = null,
    ): List<McDefinitionTarget> {
        for (owner in mixinTargets) {
            val methods = classIndex.getMethods(owner).filter { it.name == methodName }
            val method = if (descriptor != null) {
                methods.find { it.descriptor == descriptor }
            } else {
                methods.singleOrNull()
            } ?: continue
            return listOf(
                McDefinitionTarget(
                    kind = MemberKind.METHOD,
                    ownerInternalName = owner,
                    ownerFqn = classIndex.findClass(owner)?.fqn
                        ?: AnnotationContextExtractor.internalToFqn(owner),
                    name = method.name,
                    descriptor = method.descriptor,
                    sourceRange = sourceRange,
                ),
            )
        }
        return emptyList()
    }

    private fun resolveMemberInTargets(
        mixinTargets: List<String>,
        name: String,
        isMethod: Boolean,
        descriptor: String?,
        sourceRange: McTextRange,
    ): List<McDefinitionTarget> =
        if (isMethod) {
            val resolvedDescriptor = descriptor ?: return emptyList()
            val owner = mixinTargets.firstOrNull { candidate ->
                classIndex.getMethods(candidate).any { it.name == name }
            } ?: return emptyList()
            val methods = classIndex.getMethods(owner).filter { it.name == name }
            val method = methods.find { it.descriptor == resolvedDescriptor } ?: return emptyList()
            listOf(
                McDefinitionTarget(
                    kind = MemberKind.METHOD,
                    ownerInternalName = owner,
                    ownerFqn = classIndex.findClass(owner)?.fqn
                        ?: AnnotationContextExtractor.internalToFqn(owner),
                    name = method.name,
                    descriptor = method.descriptor,
                    sourceRange = sourceRange,
                ),
            )
        } else {
            resolveFieldInTargets(mixinTargets, name, sourceRange)
        }

    private fun resolveMixinTargets(
        source: String,
        context: AnnotationContext?,
        semanticModel: MixinClassModel? = null,
    ): List<String> {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        semanticModel?.targets
            ?.mapNotNull { target -> MixinTargetResolver.resolveTarget(target.internalName, classIndex, imports) }
            ?.takeIf { it.isNotEmpty() }
            ?.let { return it }
        val rawTargets = context?.mixinTargetInternalNames?.ifEmpty {
            AnnotationContextExtractor.resolveRawMixinTargets(source, context.valueStartOffset)
        } ?: AnnotationContextExtractor.resolveRawMixinTargets(source, 0)
        return MixinTargetResolver.resolveTargets(rawTargets, classIndex, imports)
    }

    private fun findShadowMemberAtOffset(source: String, offset: Int): ShadowMemberDeclaration? {
        MixinMemberDeclarationParser.parseShadowDeclarations(source, classIndex).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.name, "Shadow", declaration.isMethod)
                ?: return@forEach
            if (offset in nameRange.first..(nameRange.last + 1)) return declaration
        }
        return null
    }

    private fun findOverwriteMethodAtOffset(source: String, offset: Int): OverwriteMethodDeclaration? {
        MixinMemberDeclarationParser.parseOverwriteDeclarations(source, classIndex).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.name, "Overwrite", isMethod = true)
                ?: return@forEach
            if (offset in nameRange.first..(nameRange.last + 1)) return declaration
        }
        return null
    }

    private fun findAccessorDeclarationNear(
        source: String,
        context: AnnotationContext,
    ): AccessorMethodDeclaration? =
        MixinMemberDeclarationParser.parseAccessorDeclarations(source, classIndex)
            .firstOrNull { declaration ->
                declaration.range.start.line >= lineAtOffset(source, context.annotationStartOffset)
            }

    private fun findInvokerDeclarationNear(
        source: String,
        context: AnnotationContext,
    ): InvokerMethodDeclaration? =
        MixinMemberDeclarationParser.parseInvokerDeclarations(source, classIndex)
            .firstOrNull { declaration ->
                declaration.range.start.line >= lineAtOffset(source, context.annotationStartOffset)
            }

    private fun hasMemberParseFailureNear(source: String, context: AnnotationContext): Boolean {
        val annotationLine = lineAtOffset(source, context.annotationStartOffset)
        return MixinMemberDeclarationParser.parseDeclarationDiagnostics(source, classIndex)
            .any { diagnostic -> diagnostic.range.start.line >= annotationLine }
    }

    private fun memberNameRange(
        source: String,
        declarationRange: McTextRange,
        memberName: String,
        annotationName: String,
        isMethod: Boolean,
    ): IntRange? {
        return MixinMemberDeclarationParser.findMemberNameRange(
            source = source,
            declarationRange = declarationRange,
            memberName = memberName,
            annotationName = annotationName,
            isMethod = isMethod,
        )?.let { it.start until it.end }
    }

    private fun semanticMemberNameRange(source: String, member: MixinMemberModel): McTextRange? {
        val semanticRange = member.nameRange
        val semanticStart = positionToOffset(source, semanticRange.start)
        val semanticEnd = positionToOffset(source, semanticRange.end).coerceAtLeast(semanticStart)
        if (semanticEnd > semanticStart && source.substring(semanticStart, semanticEnd) == member.javaName) {
            return semanticRange
        }
        val annotationName = when (member.annotationKind) {
            MixinMemberAnnotationKind.ACCESSOR -> "Accessor"
            MixinMemberAnnotationKind.INVOKER -> "Invoker"
            MixinMemberAnnotationKind.SHADOW -> "Shadow"
            MixinMemberAnnotationKind.OVERWRITE -> "Overwrite"
        }
        val isMethod = member.isMethod ?: (member.methodDescriptor != null)
        return MixinMemberDeclarationParser.findMemberNameTextRange(
            source = source,
            declarationRange = member.range,
            memberName = member.javaName,
            annotationName = annotationName,
            isMethod = isMethod,
        )
    }

    private fun applyShadowPrefix(name: String, prefix: String?): String {
        if (prefix.isNullOrEmpty()) return name
        return if (name.startsWith(prefix)) name.removePrefix(prefix) else name
    }

    private fun descriptorFromInvokerDeclaration(declaration: InvokerMethodDeclaration): String? {
        val returnType = declaration.returnTypeDescriptor ?: return null
        return "(${declaration.parameterDescriptors.joinToString("")})$returnType"
    }

    private fun offsetRange(source: String, startOffset: Int, endOffset: Int): McTextRange =
        McTextRange(
            start = offsetToPosition(source, startOffset),
            end = offsetToPosition(source, endOffset.coerceAtLeast(startOffset)),
        )

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        val safeOffset = offset.coerceIn(0, source.length)
        var line = 0
        var lastLineStart = 0
        var index = 0
        while (index < safeOffset) {
            if (source[index] == '\n') {
                line++
                lastLineStart = index + 1
            }
            index++
        }
        return McTextPosition(line = line, character = safeOffset - lastLineStart)
    }

    private fun positionToOffset(source: String, position: McTextPosition): Int {
        var line = 0
        var offset = 0
        while (offset < source.length && line < position.line) {
            if (source[offset] == '\n') line++
            offset++
        }
        return (offset + position.character).coerceAtMost(source.length)
    }

    private fun lineAtOffset(source: String, offset: Int): Int =
        offsetToPosition(source, offset.coerceIn(0, source.length)).line

    private fun resolveClassEntry(
        raw: String,
        imports: JavaSourceImports? = null,
        allowPrefix: Boolean = true,
    ): ClassIndexEntry? {
        val trimmed = raw.trim().trim('"').removeSuffix(".class")
        if (trimmed.isEmpty()) return null
        classIndex.findClassByFqn(trimmed)?.let { return it }
        MixinTargetResolver.resolveTarget(trimmed, classIndex, imports)?.let { internalName ->
            classIndex.findClass(internalName)?.let { return it }
        }
        if (allowPrefix) {
            classIndex.findClasses(trimmed, limit = 5).singleOrNull()?.let { return it }
        }
        return null
    }
}

internal object AtTargetParser {
    fun parse(target: String): ParsedAtTarget? {
        val trimmed = target.trim()
        if (!trimmed.startsWith("L")) return null
        val semicolon = trimmed.indexOf(';')
        if (semicolon <= 1) return null
        val owner = trimmed.substring(1, semicolon)
        val remainder = trimmed.substring(semicolon + 1)
        if (remainder.isEmpty()) return null

        val colon = remainder.indexOf(':')
        val hasParen = remainder.contains('(')
        if (colon >= 0 && !hasParen) {
            val name = remainder.substring(0, colon)
            val descriptor = remainder.substring(colon + 1)
            if (name.isEmpty() || descriptor.isEmpty()) return null
            return ParsedAtTarget(owner, name, descriptor, MemberKind.FIELD)
        }

        val paren = remainder.indexOf('(')
        if (paren <= 0) return null
        val name = remainder.substring(0, paren)
        val descriptor = remainder.substring(paren)
        if (name.isEmpty() || descriptor.isEmpty()) return null
        return ParsedAtTarget(owner, name, descriptor, MemberKind.METHOD)
    }
}

internal data class ParsedAtTarget(
    val ownerInternalName: String,
    val name: String,
    val descriptor: String,
    val kind: MemberKind,
)

private val EXPRESSION_DEFINITION_IDENTIFIER_KEYWORDS = setOf(
    "return",
    "throw",
    "this",
    "super",
    "true",
    "false",
    "null",
    "new",
)
