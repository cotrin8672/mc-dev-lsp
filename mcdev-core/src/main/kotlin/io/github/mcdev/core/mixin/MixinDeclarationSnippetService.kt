package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.JvmType
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor

/**
 * Builds declaration snippets for annotations whose target is a member
 * declaration rather than an injector handler.
 *
 * The caller should invoke [complete] only after the annotation has been
 * completed. In particular, this service intentionally does not guess an
 * accessor or invoker target while the annotation value is still being typed.
 */
class MixinDeclarationSnippetService(
    private val classIndex: ClassIndex,
) {
    /**
     * Returns snippets for the annotation immediately preceding the cursor.
     *
     * [AnnotationSlot.HANDLER] is the integration contract used by the facade:
     * it means that the annotation arguments are complete and the cursor is at
     * the declaration position. The explicit overload below is useful to
     * callers which already have the annotation offset.
     */
    fun complete(
        source: String,
        context: AnnotationContext,
    ): List<McCompletionItem> {
        if (context.slot != AnnotationSlot.HANDLER) return emptyList()
        val annotationStart = findAnnotationStart(source, context) ?: return emptyList()
        val targets = context.mixinTargetInternalNames.ifEmpty {
            MixinTargetResolver.resolveTargetsFromSource(source, classIndex)
        }
        return complete(
            source = source,
            annotation = context.annotation,
            annotationStartOffset = annotationStart,
            mixinTargets = targets,
            targetPrefix = context.partialValue,
        )
    }

    /**
     * Generates declaration candidates for a completed annotation.
     *
     * For Accessor and Invoker, the target is read from the annotation's
     * explicit value. For Overwrite, [targetPrefix] only narrows the list; the
     * target is ultimately selected by the method name the user enters.
     */
    fun complete(
        source: String,
        annotation: MixinAnnotation,
        annotationStartOffset: Int,
        mixinTargets: List<String>,
        targetPrefix: String = "",
    ): List<McCompletionItem> {
        if (annotation !in declarationAnnotations) return emptyList()
        if (mixinTargets.isEmpty()) return emptyList()
        if (hasFollowingMember(source, annotationStartOffset)) return emptyList()

        return when (annotation) {
            MixinAnnotation.ACCESSOR -> completeAccessors(
                source = source,
                annotationStartOffset = annotationStartOffset,
                mixinTargets = mixinTargets,
            )

            MixinAnnotation.INVOKER -> completeInvokers(
                source = source,
                annotationStartOffset = annotationStartOffset,
                mixinTargets = mixinTargets,
            )

            MixinAnnotation.OVERWRITE -> completeOverwrites(
                source = source,
                mixinTargets = mixinTargets,
                targetPrefix = targetPrefix,
            )

            else -> emptyList()
        }
    }

    private fun completeAccessors(
        source: String,
        annotationStartOffset: Int,
        mixinTargets: List<String>,
    ): List<McCompletionItem> {
        val fieldName = annotationStringValue(source, annotationStartOffset, "value")
            ?.takeIf { it.isNotEmpty() }
            ?: return emptyList()
        val fieldsByOwner = mixinTargets.map { owner ->
            owner to classIndex.getFields(owner).filter { it.name == fieldName }
        }
        if (fieldsByOwner.any { it.second.size != 1 }) return emptyList()
        val fields = fieldsByOwner.map { it.first to it.second.single() }
        val signature = fields.first().second.descriptor to fields.first().second.isStatic
        if (fields.any { (it.second.descriptor to it.second.isStatic) != signature }) return emptyList()

        return fields.flatMap { (owner, field) ->
            listOf(
                accessorItem(source, annotationStartOffset, owner, field, AccessorKind.GETTER),
                accessorItem(source, annotationStartOffset, owner, field, AccessorKind.SETTER),
            )
        }.filterNotNull().distinctBy { it.insertText }
    }

    private fun accessorItem(
        source: String,
        annotationStartOffset: Int,
        owner: String,
        field: FieldIndexEntry,
        kind: AccessorKind,
    ): McCompletionItem? {
        val parsed = parseFieldDescriptor(field.descriptor)
        if (parsed !is DescriptorParseResult.Success) return null
        val type = renderType(source, parsed.value, setOfNotNull(objectInternalName(parsed.value)))
        val parameters = if (kind == AccessorKind.SETTER) {
            listOf("${type.text} value")
        } else {
            emptyList()
        }
        val modifier = if (field.isStatic) "static" else "public abstract"
        val returnType = if (kind == AccessorKind.SETTER) "void" else type.text
        val signature = "$modifier $returnType $METHOD_NAME_PLACEHOLDER(${parameters.joinToString(", ")})"
        val insertText = if (field.isStatic) {
            bodySnippet(signature)
        } else {
            "$signature;$FINAL_STOP"
        }
        val declarationEdits = instanceDeclarationEdits(source, annotationStartOffset, field.isStatic)
            ?: return null
        val ownerName = classIndex.findClass(owner)?.fqn ?: AnnotationContextExtractor.internalToFqn(owner)
        val action = if (kind == AccessorKind.GETTER) "getter" else "setter"
        return McCompletionItem(
            label = "@Accessor $action ${field.name}: ${type.text}",
            detail = "$ownerName ${if (field.isStatic) "static " else ""}field",
            documentation = signature,
            filterText = "${field.name} $action ${type.text}",
            insertText = insertText,
            kind = McCompletionKind.METHOD,
            sortKey = "0800_accessor_${field.name}_${kind.name}",
            metadata = McCompletionMetadata(
                source = "mixin.declaration",
                owner = owner,
                name = field.name,
                descriptor = field.descriptor,
            ),
            additionalEdits = declarationEdits + buildImportEdits(source, type.importFqns),
            insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
        )
    }

    private fun completeInvokers(
        source: String,
        annotationStartOffset: Int,
        mixinTargets: List<String>,
    ): List<McCompletionItem> {
        val targetName = annotationStringValue(source, annotationStartOffset, "value")
            ?.takeIf { it.isNotEmpty() }
            ?: return emptyList()
        val selectorsByOwner = mixinTargets.map { owner ->
            parseInvokerSelector(targetName, owner)?.let { owner to it }
        }
        if (selectorsByOwner.any { it == null }) return emptyList()
        val methodsByOwner = mixinTargets.map { owner ->
            val selector = selectorsByOwner.first { it?.first == owner }!!.second
            owner to classIndex.getMethods(owner)
                .filter { it.name == selector.name && it.name != "<clinit>" }
                .filter { selector.descriptor == null || it.descriptor == selector.descriptor }
        }
        if (methodsByOwner.any { it.second.isEmpty() }) return emptyList()
        val commonKeys = methodsByOwner.first().second
            .map { it.descriptor to it.isStatic }
            .toMutableSet()
        methodsByOwner.drop(1).forEach { (_, methods) ->
            commonKeys.retainAll(methods.map { it.descriptor to it.isStatic }.toSet())
        }
        return commonKeys
            .sortedWith(compareBy<Pair<String, Boolean>> { it.first }.thenBy { it.second })
            .mapNotNull { key ->
                val (descriptor, isStatic) = key
                val (owner, methods) = methodsByOwner.first()
                val method = methods.firstOrNull { it.descriptor == descriptor && it.isStatic == isStatic }
                    ?: return@mapNotNull null
                invokerItem(source, annotationStartOffset, owner, method)
            }
    }

    private fun invokerItem(
        source: String,
        annotationStartOffset: Int,
        owner: String,
        method: MethodIndexEntry,
    ): McCompletionItem? {
        val parsed = parseMethodDescriptor(method.descriptor)
        if (parsed !is DescriptorParseResult.Success) return null
        val descriptor = parsed.value
        val constructor = method.name == "<init>"
        val allTypes = buildSet {
            descriptor.parameters.forEach { collectObjectNames(it, this) }
            if (constructor) add(owner) else collectObjectNames(descriptor.returnType, this)
        }
        val renderedParameters = descriptor.parameters.mapIndexed { index, parameter ->
            val type = renderType(source, parameter, allTypes)
            RenderedParameter("${type.text} arg$index", type.importFqns)
        }
        val returnType = if (constructor) {
            renderType(source, JvmType.ObjectType(owner), allTypes)
        } else {
            renderType(source, descriptor.returnType, allTypes)
        }
        val isStatic = constructor || method.isStatic
        val modifier = if (isStatic) "static" else "public abstract"
        val signature = "$modifier ${returnType.text} $METHOD_NAME_PLACEHOLDER(" +
            renderedParameters.joinToString(", ") { it.text } + ")"
        val insertText = if (isStatic) bodySnippet(signature) else "$signature;$FINAL_STOP"
        val declarationEdits = instanceDeclarationEdits(source, annotationStartOffset, isStatic)
            ?: return null
        val ownerName = classIndex.findClass(owner)?.fqn ?: AnnotationContextExtractor.internalToFqn(owner)
        val label = if (constructor) {
            "@Invoker constructor ${returnType.text}(${renderedParameters.joinToString(", ") { it.text }})"
        } else {
            "@Invoker ${method.name}(${renderedParameters.joinToString(", ") { it.text }}): ${returnType.text}"
        }
        return McCompletionItem(
            label = label,
            detail = "$ownerName ${if (isStatic) "static " else ""}method",
            documentation = signature,
            filterText = "${method.name} ${method.readableSignature}",
            insertText = insertText,
            kind = McCompletionKind.METHOD,
            sortKey = "0810_invoker_${method.name}_${method.descriptor}",
            metadata = McCompletionMetadata(
                source = "mixin.declaration",
                owner = owner,
                name = method.name,
                descriptor = method.descriptor,
            ),
            additionalEdits = declarationEdits + buildImportEdits(
                source,
                renderedParameters.flatMap { it.importFqns }.toSet() + returnType.importFqns,
            ),
            insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
        )
    }

    private fun completeOverwrites(
        source: String,
        mixinTargets: List<String>,
        targetPrefix: String,
    ): List<McCompletionItem> {
        if (mixinTargets.size != 1) return emptyList()
        val prefix = targetPrefix.trim()
        val methodsByOwner = mixinTargets.map { owner ->
            owner to classIndex.getMethods(owner)
                .filter { it.name != "<init>" && it.name != "<clinit>" }
                .filter { prefix.isEmpty() || it.name.startsWith(prefix) }
        }
        if (methodsByOwner.any { it.second.isEmpty() }) return emptyList()
        val commonKeys = methodsByOwner.first().second
            .map { Triple(it.name, it.descriptor, it.isStatic) }
            .toMutableSet()
        methodsByOwner.drop(1).forEach { (_, methods) ->
            commonKeys.retainAll(methods.map { Triple(it.name, it.descriptor, it.isStatic) }.toSet())
        }
        return commonKeys
            .sortedWith(
                compareBy<Triple<String, String, Boolean>> { it.first }
                    .thenBy { it.second }
                    .thenBy { it.third },
            )
            .mapNotNull { key ->
                val (name, descriptor, isStatic) = key
                val (owner, methods) = methodsByOwner.first()
                val method = methods.firstOrNull {
                    it.name == name && it.descriptor == descriptor && it.isStatic == isStatic
                } ?: return@mapNotNull null
                overwriteItem(source, owner, method)
            }
    }

    private fun overwriteItem(
        source: String,
        owner: String,
        method: MethodIndexEntry,
    ): McCompletionItem? {
        val parsed = parseMethodDescriptor(method.descriptor)
        if (parsed !is DescriptorParseResult.Success) return null
        val descriptor = parsed.value
        val allTypes = buildSet {
            descriptor.parameters.forEach { collectObjectNames(it, this) }
            collectObjectNames(descriptor.returnType, this)
        }
        val parameters = descriptor.parameters.mapIndexed { index, parameter ->
            val type = renderType(source, parameter, allTypes)
            RenderedParameter("${type.text} arg$index", type.importFqns)
        }
        val returnType = renderType(source, descriptor.returnType, allTypes)
        val modifier = if (method.isStatic) "public static" else "public"
        val signature = "$modifier ${returnType.text} $METHOD_NAME_PLACEHOLDER(" +
            parameters.joinToString(", ") { it.text } + ")"
        val insertText = if (descriptor.returnType == JvmType.VoidType) {
            "$signature {\n    $FINAL_STOP\n}"
        } else {
            bodySnippet(signature)
        }
        val ownerName = classIndex.findClass(owner)?.fqn ?: AnnotationContextExtractor.internalToFqn(owner)
        return McCompletionItem(
            label = "@Overwrite ${method.name}(${parameters.joinToString(", ") { it.text }}): ${returnType.text}",
            detail = "$ownerName ${if (method.isStatic) "static " else ""}method",
            documentation = signature,
            filterText = "${method.name} ${method.readableSignature}",
            insertText = insertText,
            kind = McCompletionKind.METHOD,
            sortKey = "0820_overwrite_${method.name}_${method.descriptor}",
            metadata = McCompletionMetadata(
                source = "mixin.declaration",
                owner = owner,
                name = method.name,
                descriptor = method.descriptor,
            ),
            additionalEdits = buildImportEdits(
                source,
                parameters.flatMap { it.importFqns }.toSet() + returnType.importFqns,
            ),
            insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
        )
    }

    private fun bodySnippet(signature: String): String =
        "$signature {\n    $FINAL_STOP\n    throw new AssertionError();\n}"

    private fun renderType(
        source: String,
        type: JvmType,
        additionalInternalNames: Set<String>,
    ): RenderedType = when (type) {
        JvmType.ByteType -> RenderedType("byte")
        JvmType.CharType -> RenderedType("char")
        JvmType.DoubleType -> RenderedType("double")
        JvmType.FloatType -> RenderedType("float")
        JvmType.IntType -> RenderedType("int")
        JvmType.LongType -> RenderedType("long")
        JvmType.ShortType -> RenderedType("short")
        JvmType.BooleanType -> RenderedType("boolean")
        JvmType.VoidType -> RenderedType("void")
        is JvmType.ObjectType -> {
            val reference = MixinImportEditBuilder.referenceForInternalName(
                source = source,
                internalName = type.internalName,
                additionalInternalNames = additionalInternalNames,
            )
            RenderedType(reference.text, setOfNotNull(reference.importFqn))
        }

        is JvmType.ArrayType -> {
            val component = renderType(source, type.component, additionalInternalNames)
            RenderedType("${component.text}[]", component.importFqns)
        }
    }

    private fun objectInternalName(type: JvmType): String? = when (type) {
        is JvmType.ObjectType -> type.internalName
        is JvmType.ArrayType -> objectInternalName(type.component)
        else -> null
    }

    private fun collectObjectNames(type: JvmType, into: MutableSet<String>) {
        when (type) {
            is JvmType.ObjectType -> into += type.internalName
            is JvmType.ArrayType -> collectObjectNames(type.component, into)
            else -> Unit
        }
    }

    private fun findAnnotationStart(source: String, context: AnnotationContext): Int? {
        val explicit = context.annotationStartOffset.takeIf {
            source.getOrNull(it) == '@'
        }
        if (explicit != null) return explicit
        val valueOffset = context.valueStartOffset.coerceIn(0, source.length)
        return AnnotationContextExtractor.findAnnotationOffsets(source, context.annotation)
            .asSequence()
            .filter { it <= valueOffset }
            .maxByOrNull { it }
    }

    private fun parseInvokerSelector(value: String, owner: String): InvokerSelector? {
        val selector = value.trim()
        if (selector.isEmpty()) return null
        val descriptorStart = selector.indexOf('(')
        val rawName: String
        val descriptor: String?
        if (descriptorStart < 0) {
            rawName = selector
            descriptor = null
        } else {
            val candidateDescriptor = selector.substring(descriptorStart)
            if (parseMethodDescriptor(candidateDescriptor) !is DescriptorParseResult.Success) return null
            rawName = selector.substring(0, descriptorStart)
            descriptor = candidateDescriptor
        }
        val name = rawName.substringAfterLast(';').substringAfterLast('.').trim()
        if (name.isEmpty()) return null
        val simpleOwnerName = classIndex.findClass(owner)?.simpleName
            ?: owner.substringAfterLast('/').substringAfterLast('$')
        val resolvedName = if (name == simpleOwnerName) "<init>" else name
        return InvokerSelector(resolvedName, descriptor)
    }

    private fun annotationStringValue(source: String, annotationStart: Int, name: String): String? {
        val body = annotationBody(source, annotationStart) ?: return null
        var index = body.first
        var firstValue = true
        while (index < body.second) {
            index = skipSeparators(source, index, body.second)
            if (index >= body.second) break
            if (source[index] == '"') {
                val value = readQuoted(source, index, body.second) ?: return null
                if (firstValue && name == "value") return value.first
                index = value.second
                firstValue = false
                continue
            }
            val attributeStart = index
            while (index < body.second && (source[index].isJavaIdentifierPart() || source[index] == '.')) index++
            if (attributeStart == index) {
                index++
                continue
            }
            val attribute = source.substring(attributeStart, index)
            index = skipWhitespace(source, index, body.second)
            if (source.getOrNull(index) != '=') {
                firstValue = false
                continue
            }
            index = skipWhitespace(source, index + 1, body.second)
            if (source.getOrNull(index) != '"') {
                index = skipAnnotationValue(source, index, body.second)
                firstValue = false
                continue
            }
            val value = readQuoted(source, index, body.second) ?: return null
            if (attribute == name) return value.first
            index = value.second
            firstValue = false
        }
        return null
    }

    private fun annotationBody(source: String, annotationStart: Int): Pair<Int, Int>? {
        if (source.getOrNull(annotationStart) != '@') return null
        var nameEnd = annotationStart + 1
        while (nameEnd < source.length && (source[nameEnd].isJavaIdentifierPart() || source[nameEnd] == '.')) nameEnd++
        val open = skipWhitespace(source, nameEnd, source.length)
        if (source.getOrNull(open) != '(') return nameEnd to nameEnd
        val close = findMatching(source, open, '(', ')') ?: return null
        return open + 1 to close
    }

    private fun readQuoted(source: String, start: Int, end: Int): Pair<String, Int>? {
        if (source.getOrNull(start) != '"') return null
        val value = StringBuilder()
        var index = start + 1
        var escaped = false
        while (index < end) {
            val ch = source[index]
            if (escaped) {
                value.append(ch)
                escaped = false
            } else if (ch == '\\') {
                escaped = true
            } else if (ch == '"') {
                return value.toString() to (index + 1)
            } else {
                value.append(ch)
            }
            index++
        }
        return null
    }

    private fun skipAnnotationValue(source: String, start: Int, end: Int): Int {
        var index = start
        var depth = 0
        while (index < end) {
            when (source[index]) {
                '{', '(' -> depth++
                '}', ')' -> if (depth > 0) depth-- else return index
                ',' -> if (depth == 0) return index
            }
            index++
        }
        return index
    }

    private fun skipSeparators(source: String, start: Int, end: Int): Int {
        var index = start
        while (index < end && (source[index].isWhitespace() || source[index] == ',')) index++
        return index
    }

    private fun skipWhitespace(source: String, start: Int, end: Int): Int {
        var index = start
        while (index < end && source[index].isWhitespace()) index++
        return index
    }

    /**
     * An annotation completion must not offer a second declaration when the
     * source already contains the member that the annotation decorates.
     *
     * The caller does not pass a cursor offset to this service, so the
     * annotation itself is the boundary: after its arguments, skip comments
     * and any stacked annotations, then treat the first declaration token as
     * an already-existing member.
     */
    private fun hasFollowingMember(source: String, annotationStart: Int): Boolean {
        val body = annotationBody(source, annotationStart) ?: return false
        var index = if (body.second < source.length && source[body.second] == ')') {
            body.second + 1
        } else {
            body.second
        }
        while (true) {
            index = skipWhitespaceAndComments(source, index)
            if (index >= source.length) return false
            if (source[index] != '@') break
            val nextBody = annotationBody(source, index) ?: return false
            index = if (nextBody.second < source.length && source[nextBody.second] == ')') {
                nextBody.second + 1
            } else {
                nextBody.second
            }
        }
        index = skipWhitespaceAndComments(source, index)
        return index < source.length && source[index] != '}'
    }

    private fun skipWhitespaceAndComments(source: String, start: Int): Int {
        var index = start
        while (index < source.length) {
            index = skipWhitespace(source, index, source.length)
            when {
                source.startsWith("//", index) -> {
                    val newline = source.indexOf('\n', index + 2)
                    index = if (newline < 0) source.length else newline + 1
                }

                source.startsWith("/*", index) -> {
                    val close = source.indexOf("*/", index + 2)
                    index = if (close < 0) source.length else close + 2
                }

                else -> return index
            }
        }
        return index
    }

    /**
     * Instance Accessor and Invoker declarations are abstract in a mixin.
     * Make an ordinary enclosing class abstract with an additional edit so
     * the generated declaration remains valid Java. Interfaces and already
     * abstract classes need no edit; final classes, records, and enums cannot
     * receive such a declaration safely.
     *
     * A missing enclosing type is accepted for callers that provide a source
     * fragment rather than a complete compilation unit. The declaration is
     * still useful in that context and no source edit is required.
     */
    private fun instanceDeclarationEdits(
        source: String,
        annotationStart: Int,
        isStatic: Boolean,
    ): List<McTextEdit>? {
        if (isStatic) return emptyList()
        val enclosingType = findEnclosingType(source, annotationStart) ?: return emptyList()
        if (enclosingType.kind == "interface") return emptyList()
        if (enclosingType.kind != "class" || enclosingType.isFinal) return null
        if (enclosingType.isAbstract) return emptyList()
        return listOf(
            McTextEdit(
                startOffset = enclosingType.keywordStart,
                endOffset = enclosingType.keywordStart + "class".length,
                newText = "abstract class",
            ),
        )
    }

    private fun findEnclosingType(source: String, offset: Int): EnclosingType? {
        val code = AnnotationContextExtractor.maskNonCode(source)
        val declarations = Regex("""\b(class|interface|record|enum)\s+[A-Za-z_$][\w$]*""")
        return declarations.findAll(code)
            .mapNotNull { match ->
                val open = code.indexOf('{', match.range.last + 1)
                if (open < 0 || open >= offset) return@mapNotNull null
                val close = findMatching(code, open, '{', '}') ?: return@mapNotNull null
                if (offset !in (open + 1 until close)) return@mapNotNull null
                val kind = match.groupValues[1]
                val keywordStart = match.range.first + match.value.indexOf(kind)
                val boundary = sequenceOf(
                    code.lastIndexOf('{', match.range.first - 1),
                    code.lastIndexOf('}', match.range.first - 1),
                    code.lastIndexOf(';', match.range.first - 1),
                ).maxOrNull() ?: -1
                val modifiers = code.substring(boundary + 1, match.range.first)
                EnclosingType(
                    kind = kind,
                    keywordStart = keywordStart,
                    isAbstract = Regex("""\babstract\b""").containsMatchIn(modifiers),
                    isFinal = Regex("""\bfinal\b""").containsMatchIn(modifiers),
                    bodySize = close - open,
                )
            }
            .minByOrNull { it.bodySize }
    }

    private fun findMatching(source: String, open: Int, openChar: Char, closeChar: Char): Int? {
        var depth = 0
        var index = open
        var quoted = false
        var escaped = false
        while (index < source.length) {
            val ch = source[index]
            if (quoted) {
                when {
                    escaped -> escaped = false
                    ch == '\\' -> escaped = true
                    ch == '"' -> quoted = false
                }
            } else {
                when (ch) {
                    '"' -> quoted = true
                    openChar -> depth++
                    closeChar -> {
                        depth--
                        if (depth == 0) return index
                    }
                }
            }
            index++
        }
        return null
    }

    private fun buildImportEdits(source: String, fqns: Set<String>): List<McTextEdit> {
        val edits = fqns.sorted().mapNotNull { MixinImportEditBuilder.buildImportEdit(source, it) }
        if (edits.isEmpty()) return emptyList()
        val first = edits.first()
        val leadingNewline = first.newText.takeIf { it.startsWith("\n") }?.let { "\n" }.orEmpty()
        val imports = edits.joinToString(separator = "") { it.newText.trimStart('\n') }
        return listOf(McTextEdit(first.startOffset, first.endOffset, leadingNewline + imports))
    }

    private data class RenderedType(
        val text: String,
        val importFqns: Set<String> = emptySet(),
    )

    private data class RenderedParameter(
        val text: String,
        val importFqns: Set<String>,
    )

    private data class EnclosingType(
        val kind: String,
        val keywordStart: Int,
        val isAbstract: Boolean,
        val isFinal: Boolean,
        val bodySize: Int,
    )

    private data class InvokerSelector(
        val name: String,
        val descriptor: String?,
    )

    private companion object {
        val declarationAnnotations = setOf(
            MixinAnnotation.ACCESSOR,
            MixinAnnotation.INVOKER,
            MixinAnnotation.OVERWRITE,
        )
        const val METHOD_NAME_PLACEHOLDER = "\${1}"
        const val FINAL_STOP = "\$0"
    }
}
