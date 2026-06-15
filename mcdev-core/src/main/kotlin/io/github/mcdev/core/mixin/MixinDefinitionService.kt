package io.github.mcdev.core.mixin

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.model.MappingNamespace

class MixinDefinitionService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val accessorService: AccessorService = AccessorService(classIndex),
    private val invokerService: InvokerService = InvokerService(classIndex),
) {
    fun definitionsAt(source: String, line: Int, character: Int): List<McDefinitionTarget> {
        val offset = AnnotationContextExtractor.toOffset(source, line, character) ?: return emptyList()
        return definitionsAtOffset(source, offset)
    }

    fun definitionsAtOffset(source: String, offset: Int): List<McDefinitionTarget> {
        if (offset < 0 || offset > source.length) return emptyList()

        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        if (context != null) {
            val fromContext = resolveFromAnnotationContext(source, context)
            if (fromContext.isNotEmpty()) return fromContext
        }

        resolveAtTargetStringAtOffset(source, offset)?.let { return it }

        return resolveFromMemberDeclaration(source, offset)
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
    ): List<McDefinitionTarget> =
        when (context.annotation) {
            MixinAnnotation.MIXIN -> when (context.slot) {
                AnnotationSlot.CLASS,
                AnnotationSlot.TARGETS,
                    -> resolveMixinClassTarget(source, context)
                else -> emptyList()
            }
            MixinAnnotation.SHADOW -> when (context.slot) {
                AnnotationSlot.SHADOW_MEMBER -> resolveShadowMemberAtOffset(source, offsetFromContext(context))
                else -> emptyList()
            }
            MixinAnnotation.OVERWRITE -> when (context.slot) {
                AnnotationSlot.OVERWRITE_METHOD -> resolveOverwriteMethodAtOffset(source, offsetFromContext(context))
                else -> emptyList()
            }
            MixinAnnotation.ACCESSOR -> when (context.slot) {
                AnnotationSlot.ACCESSOR_VALUE -> resolveAccessorTarget(source, context)
                else -> emptyList()
            }
            MixinAnnotation.INVOKER -> when (context.slot) {
                AnnotationSlot.INVOKER_VALUE -> resolveInvokerTarget(source, context)
                else -> emptyList()
            }
            MixinAnnotation.AT -> when (context.slot) {
                AnnotationSlot.TARGET -> resolveAtTarget(source, context)
                else -> emptyList()
            }
            else -> emptyList()
        }

    private fun offsetFromContext(context: AnnotationContext): Int =
        context.valueEndOffset.coerceAtLeast(context.valueStartOffset)

    private fun resolveMixinClassTarget(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        val raw = context.partialValue.trim().trim('"').removeSuffix(".class")
        if (raw.isEmpty()) return emptyList()
        val entry = resolveClassEntry(raw) ?: return emptyList()
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

    private fun resolveFromMemberDeclaration(source: String, offset: Int): List<McDefinitionTarget> {
        findShadowMemberAtOffset(source, offset)?.let { declaration ->
            return resolveShadowMemberAtOffset(source, offset)
        }

        findOverwriteMethodAtOffset(source, offset)?.let { declaration ->
            return resolveOverwriteMethodAtOffset(source, offset)
        }

        MixinMemberDeclarationParser.parseAccessorDeclarations(source).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.methodName) ?: return@forEach
            if (offset in nameRange.first until nameRange.last) {
                val mixinTargets = MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
                val fieldName = accessorService.inferFieldName(declaration) ?: return emptyList()
                return resolveFieldInTargets(mixinTargets, fieldName, declaration.range)
            }
        }

        MixinMemberDeclarationParser.parseInvokerDeclarations(source).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.methodName) ?: return@forEach
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
    ): List<McDefinitionTarget> {
        val fieldName = context.partialValue.trim('"').ifEmpty {
            findAccessorDeclarationNear(source, context)?.let { accessorService.inferFieldName(it) }
        } ?: return emptyList()
        val mixinTargets = resolveMixinTargets(source, context)
        val range = offsetRange(source, context.valueStartOffset, context.valueEndOffset)
        return resolveFieldInTargets(mixinTargets, fieldName, range)
    }

    private fun resolveInvokerTarget(
        source: String,
        context: AnnotationContext,
    ): List<McDefinitionTarget> {
        val methodName = context.partialValue.trim('"').ifEmpty {
            findInvokerDeclarationNear(source, context)?.let { invokerService.inferTargetName(it) }
        } ?: return emptyList()
        val mixinTargets = resolveMixinTargets(source, context)
        val range = offsetRange(source, context.valueStartOffset, context.valueEndOffset)
        val descriptor = findInvokerDeclarationNear(source, context)?.let(::descriptorFromInvokerDeclaration)
        return resolveMethodInTargets(mixinTargets, methodName, range, descriptor)
    }

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
        descriptor: String,
        sourceRange: McTextRange,
    ): List<McDefinitionTarget> =
        if (isMethod) {
            val owner = mixinTargets.firstOrNull { candidate ->
                classIndex.getMethods(candidate).any { it.name == name }
            } ?: return emptyList()
            val methods = classIndex.getMethods(owner).filter { it.name == name }
            val method = methods.find { it.descriptor == descriptor } ?: return emptyList()
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

    private fun resolveMixinTargets(source: String, context: AnnotationContext): List<String> {
        val rawTargets = context.mixinTargetInternalNames.ifEmpty {
            AnnotationContextExtractor.resolveRawMixinTargets(source, context.valueStartOffset)
        }
        return MixinTargetResolver.resolveTargets(rawTargets, classIndex, JavaTypeDescriptorResolver.importsFor(source))
    }

    private fun findShadowMemberAtOffset(source: String, offset: Int): ShadowMemberDeclaration? {
        MixinMemberDeclarationParser.parseShadowDeclarations(source).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.name) ?: return@forEach
            if (offset in nameRange.first until nameRange.last) return declaration
        }
        return null
    }

    private fun findOverwriteMethodAtOffset(source: String, offset: Int): OverwriteMethodDeclaration? {
        MixinMemberDeclarationParser.parseOverwriteDeclarations(source).forEach { declaration ->
            val nameRange = memberNameRange(source, declaration.range, declaration.name) ?: return@forEach
            if (offset in nameRange.first until nameRange.last) return declaration
            val bodyStart = source.indexOf('{', nameRange.last)
            if (bodyStart >= 0 && offset >= bodyStart) {
                val bodyEnd = findMatchingBrace(source, bodyStart) ?: source.length
                if (offset <= bodyEnd) return declaration
            }
        }
        return null
    }

    private fun findMatchingBrace(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '{') return null
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
                source[i] == '{' -> depth++
                source[i] == '}' -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return null
    }

    private fun findAccessorDeclarationNear(
        source: String,
        context: AnnotationContext,
    ): AccessorMethodDeclaration? =
        MixinMemberDeclarationParser.parseAccessorDeclarations(source)
            .firstOrNull { declaration ->
                declaration.range.start.line >= lineAtOffset(source, context.annotationStartOffset)
            }

    private fun findInvokerDeclarationNear(
        source: String,
        context: AnnotationContext,
    ): InvokerMethodDeclaration? =
        MixinMemberDeclarationParser.parseInvokerDeclarations(source)
            .firstOrNull { declaration ->
                declaration.range.start.line >= lineAtOffset(source, context.annotationStartOffset)
            }

    private fun memberNameRange(
        source: String,
        declarationRange: McTextRange,
        memberName: String,
    ): IntRange? {
        val startOffset = positionToOffset(source, declarationRange.start)
        val endOffset = positionToOffset(source, declarationRange.end)
        val sliceStart = startOffset.coerceAtLeast(0)
        val sliceEnd = endOffset.coerceAtMost(source.length)
        val slice = source.substring(sliceStart, sliceEnd)
        val localIndex = slice.indexOf(memberName)
        if (localIndex < 0) return null
        val absoluteStart = sliceStart + localIndex
        return absoluteStart until (absoluteStart + memberName.length)
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

    private fun resolveClassEntry(raw: String): ClassIndexEntry? {
        val trimmed = raw.trim().trim('"').removeSuffix(".class")
        if (trimmed.isEmpty()) return null
        classIndex.findClassByFqn(trimmed)?.let { return it }
        classIndex.findClasses(trimmed, limit = 5).firstOrNull()?.let { return it }
        MixinTargetResolver.resolveTarget(trimmed, classIndex)?.let { internalName ->
            classIndex.findClass(internalName)?.let { return it }
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
