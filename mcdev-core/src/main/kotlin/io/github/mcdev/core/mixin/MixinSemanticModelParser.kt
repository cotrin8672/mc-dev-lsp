package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

data class MixinClassModel(
    val sourceUri: String = "",
    val packageName: String = "",
    val qualifiedName: String = "",
    val typeKind: JavaTypeKind = JavaTypeKind.CLASS,
    val targets: List<MixinTargetRef>,
    val members: List<MixinMemberModel> = emptyList(),
    val injectors: List<InjectorModel>,
    val parseSource: ParseSource = ParseSource.HAND_WRITTEN_FALLBACK,
    val confidence: ParseConfidence = ParseConfidence.MEDIUM,
    val warnings: List<String> = emptyList(),
)

enum class JavaTypeKind {
    CLASS,
    INTERFACE,
    ENUM,
    ANNOTATION,
    RECORD,
    UNKNOWN,
}

enum class JavaModifier {
    PUBLIC,
    PROTECTED,
    PRIVATE,
    ABSTRACT,
    STATIC,
    FINAL,
    SYNCHRONIZED,
    NATIVE,
    STRICTFP,
    TRANSIENT,
    VOLATILE,
}

enum class MixinMemberAnnotationKind {
    ACCESSOR,
    INVOKER,
    SHADOW,
    OVERWRITE,
}

enum class InjectorKind {
    INJECT,
    REDIRECT,
    MODIFY_ARG,
    MODIFY_ARGS,
    MODIFY_VARIABLE,
    MODIFY_CONSTANT,
    MODIFY_EXPRESSION_VALUE,
    MODIFY_RETURN_VALUE,
    MODIFY_RECEIVER,
    WRAP_OPERATION,
    WRAP_WITH_CONDITION,
    WRAP_METHOD,
}

data class MixinTargetRef(
    val internalName: String,
    val range: McTextRange,
)

data class MixinMemberModel(
    val annotationKind: MixinMemberAnnotationKind,
    val javaName: String,
    val explicitTargetName: String?,
    val returnDescriptor: String?,
    val parameterDescriptors: List<String>,
    val methodDescriptor: String?,
    val modifiers: Set<JavaModifier>,
    val range: McTextRange,
    val annotationRange: McTextRange,
    val nameRange: McTextRange,
    val parseSource: ParseSource,
    val confidence: ParseConfidence,
    val warnings: List<String>,
)

data class InjectorModel(
    val annotation: MixinAnnotation,
    val annotationKind: InjectorKind = annotation.toInjectorKind(),
    val methodSelectors: List<MethodSelector>,
    val atSelectors: List<AtSelectorModel>,
    val handlerMethodDescriptor: String? = null,
    val range: McTextRange,
)

data class MethodSelector(
    val value: String,
    val name: String,
    val descriptor: String?,
    val range: McTextRange,
)

data class AtSelectorModel(
    val value: String,
    val target: String?,
    val ordinal: Int?,
    val range: McTextRange,
    val targetRange: McTextRange?,
    val ordinalRange: McTextRange?,
)

object MixinSemanticModelParser {
    private val injectorAnnotations = setOf(
        MixinAnnotation.INJECT,
        MixinAnnotation.REDIRECT,
        MixinAnnotation.MODIFY_ARG,
        MixinAnnotation.MODIFY_ARGS,
        MixinAnnotation.MODIFY_VARIABLE,
        MixinAnnotation.MODIFY_CONSTANT,
        MixinAnnotation.MODIFY_EXPRESSION_VALUE,
        MixinAnnotation.MODIFY_RETURN_VALUE,
        MixinAnnotation.MODIFY_RECEIVER,
        MixinAnnotation.WRAP_OPERATION,
        MixinAnnotation.WRAP_WITH_CONDITION,
        MixinAnnotation.WRAP_METHOD,
    )

    fun parse(
        source: String,
        sourceUri: String = "",
        parseSource: ParseSource = ParseSource.HAND_WRITTEN_FALLBACK,
        confidence: ParseConfidence = ParseConfidence.MEDIUM,
        warnings: List<String> = emptyList(),
    ): MixinClassModel =
        MixinClassModel(
            sourceUri = sourceUri,
            packageName = parsePackageName(source).orEmpty(),
            qualifiedName = parseQualifiedName(source).orEmpty(),
            targets = parseMixinTargets(source),
            members = parseMixinMembers(source, parseSource, confidence),
            injectors = parseInjectors(source),
            parseSource = parseSource,
            confidence = confidence,
            warnings = warnings,
        )

    private fun parsePackageName(source: String): String? =
        Regex("""^\s*package\s+([\w.]+)\s*;""", RegexOption.MULTILINE).find(source)?.groupValues?.get(1)

    private fun parseQualifiedName(source: String): String? {
        val typeName = Regex("""\b(class|interface|enum|record)\s+(\w+)""").find(source)?.groupValues?.get(2)
            ?: return null
        val pkg = parsePackageName(source)
        return if (pkg.isNullOrBlank()) typeName else "$pkg.$typeName"
    }

    private fun parseMixinMembers(
        source: String,
        parseSource: ParseSource,
        confidence: ParseConfidence,
    ): List<MixinMemberModel> {
        val members = mutableListOf<MixinMemberModel>()
        MixinMemberDeclarationParser.parseShadowDeclarations(source).forEach { declaration ->
            members += declaration.toModel(MixinMemberAnnotationKind.SHADOW, parseSource, confidence)
        }
        MixinMemberDeclarationParser.parseAccessorDeclarations(source).forEach { declaration ->
            val descriptor = "(${declaration.parameterDescriptors.joinToString("")})${declaration.returnTypeDescriptor ?: "V"}"
            members += MixinMemberModel(
                annotationKind = MixinMemberAnnotationKind.ACCESSOR,
                javaName = declaration.methodName,
                explicitTargetName = declaration.explicitFieldName,
                returnDescriptor = declaration.returnTypeDescriptor,
                parameterDescriptors = declaration.parameterDescriptors,
                methodDescriptor = descriptor,
                modifiers = emptySet(),
                range = declaration.range,
                annotationRange = declaration.range,
                nameRange = declaration.range,
                parseSource = parseSource,
                confidence = confidence,
                warnings = declaration.warnings,
            )
        }
        MixinMemberDeclarationParser.parseInvokerDeclarations(source).forEach { declaration ->
            val descriptor = "(${declaration.parameterDescriptors.joinToString("")})${declaration.returnTypeDescriptor ?: "V"}"
            members += MixinMemberModel(
                annotationKind = MixinMemberAnnotationKind.INVOKER,
                javaName = declaration.methodName,
                explicitTargetName = declaration.explicitTargetName,
                returnDescriptor = declaration.returnTypeDescriptor,
                parameterDescriptors = declaration.parameterDescriptors,
                methodDescriptor = descriptor,
                modifiers = emptySet(),
                range = declaration.range,
                annotationRange = declaration.range,
                nameRange = declaration.range,
                parseSource = parseSource,
                confidence = confidence,
                warnings = declaration.warnings,
            )
        }
        MixinMemberDeclarationParser.parseOverwriteDeclarations(source).forEach { declaration ->
            members += MixinMemberModel(
                annotationKind = MixinMemberAnnotationKind.OVERWRITE,
                javaName = declaration.name,
                explicitTargetName = declaration.name,
                returnDescriptor = declaration.descriptor.substringAfter(')', ""),
                parameterDescriptors = emptyList(),
                methodDescriptor = declaration.descriptor,
                modifiers = if (declaration.isStatic) setOf(JavaModifier.STATIC) else emptySet(),
                range = declaration.range,
                annotationRange = declaration.range,
                nameRange = declaration.range,
                parseSource = parseSource,
                confidence = confidence,
                warnings = declaration.warnings,
            )
        }
        return members
    }

    private fun ShadowMemberDeclaration.toModel(
        annotationKind: MixinMemberAnnotationKind,
        parseSource: ParseSource,
        confidence: ParseConfidence,
    ): MixinMemberModel =
        MixinMemberModel(
            annotationKind = annotationKind,
            javaName = name,
            explicitTargetName = name,
            returnDescriptor = if (isMethod) descriptor.substringAfter(')', "") else descriptor,
            parameterDescriptors = emptyList(),
            methodDescriptor = descriptor.takeIf { isMethod },
            modifiers = if (isStatic) setOf(JavaModifier.STATIC) else emptySet(),
            range = range,
            annotationRange = range,
            nameRange = range,
            parseSource = parseSource,
            confidence = confidence,
            warnings = warnings,
        )

    private fun parseMixinTargets(source: String): List<MixinTargetRef> {
        val results = mutableListOf<MixinTargetRef>()
        findAnnotations(source, MixinAnnotation.MIXIN).forEach { annotation ->
            parseClassTargets(source, annotation.bodyStart, annotation.bodyEnd).forEach { target ->
                results += MixinTargetRef(target.value, offsetRange(source, target.start, target.end))
            }
        }
        return results
    }

    private fun parseInjectors(source: String): List<InjectorModel> {
        val results = mutableListOf<InjectorModel>()
        findAnnotations(source).filter { it.annotation in injectorAnnotations }.forEach { annotation ->
            val methodSelectors = parseMethodSelectors(source, annotation.bodyStart, annotation.bodyEnd)
            val atSelectors = findAtSelectors(source, annotation.bodyStart, annotation.bodyEnd)
            results += InjectorModel(
                annotation = annotation.annotation,
                methodSelectors = methodSelectors,
                atSelectors = atSelectors,
                range = offsetRange(source, annotation.start, annotation.end),
            )
        }
        return results
    }

    private data class AnnotationSpan(
        val annotation: MixinAnnotation,
        val start: Int,
        val end: Int,
        val bodyStart: Int,
        val bodyEnd: Int,
    )

    private data class RawValue(
        val value: String,
        val start: Int,
        val end: Int,
        val attributeStart: Int? = null,
        val attributeEnd: Int? = null,
    )

    private data class RawInt(
        val value: Int,
        val start: Int,
        val end: Int,
    )

    private fun findAnnotations(source: String, only: MixinAnnotation? = null): List<AnnotationSpan> {
        val results = mutableListOf<AnnotationSpan>()
        var search = 0
        while (search < source.length) {
            val at = source.indexOf('@', search)
            if (at < 0) break
            val nameEnd = readQualifiedNameEnd(source, at + 1)
            val annotation = MixinAnnotation.fromSimpleName(source.substring(at + 1, nameEnd).substringAfterLast('.'))
            if (annotation == null || only != null && annotation != only) {
                search = at + 1
                continue
            }
            val paren = skipWhitespace(source, nameEnd)
            val close = if (source.getOrNull(paren) == '(') findMatching(source, paren, '(', ')') else null
            val end = close?.plus(1) ?: nameEnd
            val bodyStart = close?.let { paren + 1 } ?: end
            val bodyEnd = close ?: end
            results += AnnotationSpan(annotation, at, end, bodyStart, bodyEnd)
            search = end
        }
        return results
    }

    private fun parseClassTargets(source: String, start: Int, end: Int): List<RawValue> =
        parseAttributeValues(source, start, end, setOf("value", "targets", "target"), allowShorthand = true)
            .map { value ->
                val cleaned = value.value.removeSuffix(".class").trim()
                val internal = if ('.' in cleaned) cleaned.replace('.', '/') else cleaned
                value.copy(value = internal)
            }

    private fun parseMethodSelectors(source: String, start: Int, end: Int): List<MethodSelector> =
        parseAttributeValues(source, start, end, setOf("method"), allowShorthand = false).map { raw ->
            val paren = raw.value.indexOf('(')
            MethodSelector(
                value = raw.value,
                name = if (paren >= 0) raw.value.substring(0, paren) else raw.value,
                descriptor = if (paren >= 0) raw.value.substring(paren) else null,
                range = offsetRange(source, raw.attributeStart ?: raw.start, raw.attributeEnd ?: raw.end),
            )
        }

    private fun findAtSelectors(source: String, start: Int, end: Int): List<AtSelectorModel> {
        val results = mutableListOf<AtSelectorModel>()
        findAnnotations(source.substring(start, end), MixinAnnotation.AT).forEach { relative ->
            val atStart = start + relative.start
            val bodyStart = start + relative.bodyStart
            val bodyEnd = start + relative.bodyEnd
            val values = parseAttributeValues(source, bodyStart, bodyEnd, setOf("value"), allowShorthand = true)
            val value = values.firstOrNull()?.value ?: return@forEach
            val target = parseAttributeValues(source, bodyStart, bodyEnd, setOf("target"), allowShorthand = false)
                .firstOrNull()
            val ordinal = parseIntAttribute(source, bodyStart, bodyEnd, "ordinal")
            results += AtSelectorModel(
                value = value,
                target = target?.value,
                ordinal = ordinal?.value,
                range = offsetRange(source, atStart, start + relative.end),
                targetRange = target?.let { offsetRange(source, it.start, it.end) },
                ordinalRange = ordinal?.let { offsetRange(source, it.start, it.end) },
            )
        }
        return results
    }

    private fun parseAttributeValues(
        source: String,
        start: Int,
        end: Int,
        names: Set<String>,
        allowShorthand: Boolean,
    ): List<RawValue> {
        val results = mutableListOf<RawValue>()
        var i = start
        while (i < end) {
            i = skipWhitespaceAndCommas(source, i, end)
            if (i >= end) break
            if (source[i] == '{') {
                val close = findMatching(source, i, '{', '}') ?: break
                results += parseAttributeValues(source, i + 1, close, names, allowShorthand = true)
                i = close + 1
                continue
            }
            if (allowShorthand) {
                val shorthand = readValue(source, i, end)
                if (shorthand != null) {
                    results += shorthand
                    i = shorthand.end
                    continue
                }
            }
            val nameStart = i
            val nameEnd = readIdentifierEnd(source, i)
            if (nameEnd == nameStart) {
                i++
                continue
            }
            val name = source.substring(nameStart, nameEnd)
            i = skipWhitespace(source, nameEnd)
            if (source.getOrNull(i) != '=') continue
            i = skipWhitespace(source, i + 1)
            if (source.getOrNull(i) == '{') {
                val close = findMatching(source, i, '{', '}') ?: break
                if (name in names) {
                    results += parseAttributeValues(source, i + 1, close, names, allowShorthand = true)
                }
                i = close + 1
                continue
            }
            val value = readValue(source, i, end) ?: break
            val attributeEnd = if (source.getOrNull(value.end) == '"') value.end + 1 else value.end
            if (name in names) {
                results += value.copy(attributeStart = nameStart, attributeEnd = attributeEnd)
            }
            i = attributeEnd
        }
        return results
    }

    private fun readValue(source: String, start: Int, end: Int): RawValue? {
        if (start >= end) return null
        if (source[start] == '"') {
            var i = start + 1
            while (i < end) {
                if (source[i] == '\\') {
                    i += 2
                    continue
                }
                if (source[i] == '"') {
                    return RawValue(source.substring(start + 1, i), start + 1, i)
                }
                i++
            }
            return null
        }
        val classEnd = readClassLiteralEnd(source, start, end)
        if (classEnd > start) {
            return RawValue(source.substring(start, classEnd), start, classEnd)
        }
        return null
    }

    private fun readClassLiteralEnd(source: String, start: Int, end: Int): Int {
        var i = start
        if (i >= end || !source[i].isJavaIdentifierStart()) return start
        while (i < end) {
            when {
                source.startsWith(".class", i) -> return i + ".class".length
                source[i].isLetterOrDigit() || source[i] == '_' || source[i] == '.' -> i++
                else -> return start
            }
        }
        return start
    }

    private fun parseIntAttribute(source: String, start: Int, end: Int, name: String): RawInt? {
        var i = start
        while (i < end) {
            i = skipWhitespaceAndCommas(source, i, end)
            val attributeStart = i
            val nameEnd = readIdentifierEnd(source, i)
            if (nameEnd == i) {
                i++
                continue
            }
            val attr = source.substring(i, nameEnd)
            i = skipWhitespace(source, nameEnd)
            if (source.getOrNull(i) != '=') continue
            i = skipWhitespace(source, i + 1)
            val valueStart = i
            while (i < end && source[i].isDigit()) i++
            if (attr == name) {
                val value = source.substring(valueStart, i).toIntOrNull() ?: return null
                return RawInt(value, attributeStart, i)
            }
        }
        return null
    }

    private fun findMatching(source: String, open: Int, openChar: Char, closeChar: Char): Int? {
        var depth = 0
        var inString = false
        var i = open
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
                source[i] == openChar -> depth++
                source[i] == closeChar -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return null
    }

    private fun readQualifiedNameEnd(source: String, start: Int): Int {
        var i = start
        while (i < source.length && (source[i].isLetterOrDigit() || source[i] == '_' || source[i] == '.')) i++
        return i
    }

    private fun readIdentifierEnd(source: String, start: Int): Int {
        var i = start
        if (i >= source.length || !source[i].isJavaIdentifierStart()) return start
        while (i < source.length && (source[i].isLetterOrDigit() || source[i] == '_')) i++
        return i
    }

    private fun skipWhitespace(source: String, start: Int): Int {
        var i = start
        while (i < source.length && source[i].isWhitespace()) i++
        return i
    }

    private fun skipWhitespaceAndCommas(source: String, start: Int, end: Int): Int {
        var i = start
        while (i < end && (source[i].isWhitespace() || source[i] == ',')) i++
        return i
    }

    private fun offsetRange(source: String, start: Int, end: Int): McTextRange =
        McTextRange(offsetToPosition(source, start), offsetToPosition(source, end))

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        var line = 0
        var character = 0
        var i = 0
        while (i < offset && i < source.length) {
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
}

private fun MixinAnnotation.toInjectorKind(): InjectorKind =
    when (this) {
        MixinAnnotation.INJECT -> InjectorKind.INJECT
        MixinAnnotation.REDIRECT -> InjectorKind.REDIRECT
        MixinAnnotation.MODIFY_ARG -> InjectorKind.MODIFY_ARG
        MixinAnnotation.MODIFY_ARGS -> InjectorKind.MODIFY_ARGS
        MixinAnnotation.MODIFY_VARIABLE -> InjectorKind.MODIFY_VARIABLE
        MixinAnnotation.MODIFY_CONSTANT -> InjectorKind.MODIFY_CONSTANT
        MixinAnnotation.MODIFY_EXPRESSION_VALUE -> InjectorKind.MODIFY_EXPRESSION_VALUE
        MixinAnnotation.MODIFY_RETURN_VALUE -> InjectorKind.MODIFY_RETURN_VALUE
        MixinAnnotation.MODIFY_RECEIVER -> InjectorKind.MODIFY_RECEIVER
        MixinAnnotation.WRAP_OPERATION -> InjectorKind.WRAP_OPERATION
        MixinAnnotation.WRAP_WITH_CONDITION -> InjectorKind.WRAP_WITH_CONDITION
        MixinAnnotation.WRAP_METHOD -> InjectorKind.WRAP_METHOD
        else -> InjectorKind.INJECT
    }
