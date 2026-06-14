package io.github.mcdev.core.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

internal object MixinMemberDeclarationParser {
    fun parseShadowDeclarations(source: String): List<ShadowMemberDeclaration> {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        return parseAnnotatedMembers(source, "Shadow").mapNotNull { member ->
            val declaration = member.declaration ?: return@mapNotNull null
            if (declaration.isMethod) {
                ShadowMemberDeclaration(
                    name = declaration.name,
                    isMethod = true,
                    descriptor = JavaTypeDescriptorResolver.methodDescriptor(
                        declaration.returnType,
                        declaration.parameterTypes,
                        imports,
                    ),
                    isStatic = declaration.modifiers.contains("static"),
                    range = offsetRange(source, member.start, member.end),
                )
            } else {
                ShadowMemberDeclaration(
                    name = declaration.name,
                    isMethod = false,
                    descriptor = JavaTypeDescriptorResolver.descriptor(declaration.returnType, imports),
                    isStatic = declaration.modifiers.contains("static"),
                    range = offsetRange(source, member.start, member.end),
                )
            }
        }
    }

    fun parseAccessorDeclarations(source: String): List<AccessorMethodDeclaration> {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        return parseAnnotatedMembers(source, "Accessor").mapNotNull { member ->
            val declaration = member.declaration?.takeIf { it.isMethod } ?: return@mapNotNull null
            AccessorMethodDeclaration(
                methodName = declaration.name,
                returnTypeDescriptor = JavaTypeDescriptorResolver.descriptor(declaration.returnType, imports),
                parameterDescriptors = declaration.parameterTypes.map {
                    JavaTypeDescriptorResolver.descriptor(it, imports)
                },
                explicitFieldName = member.stringValue,
                range = offsetRange(source, member.start, member.end),
            )
        }
    }

    fun parseInvokerDeclarations(source: String): List<InvokerMethodDeclaration> {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        return parseAnnotatedMembers(source, "Invoker").mapNotNull { member ->
            val declaration = member.declaration?.takeIf { it.isMethod } ?: return@mapNotNull null
            InvokerMethodDeclaration(
                methodName = declaration.name,
                parameterDescriptors = declaration.parameterTypes.map {
                    JavaTypeDescriptorResolver.descriptor(it, imports)
                },
                returnTypeDescriptor = JavaTypeDescriptorResolver.descriptor(declaration.returnType, imports),
                explicitTargetName = member.stringValue,
                range = offsetRange(source, member.start, member.end),
            )
        }
    }

    fun parseOverwriteDeclarations(source: String): List<OverwriteMethodDeclaration> {
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        return parseAnnotatedMembers(source, "Overwrite").mapNotNull { member ->
            val declaration = member.declaration?.takeIf { it.isMethod } ?: return@mapNotNull null
            OverwriteMethodDeclaration(
                name = declaration.name,
                descriptor = JavaTypeDescriptorResolver.methodDescriptor(
                    declaration.returnType,
                    declaration.parameterTypes,
                    imports,
                ),
                isStatic = declaration.modifiers.contains("static"),
                range = offsetRange(source, member.start, member.end),
            )
        }
    }

    fun findShadowPrefix(source: String): String? =
        parseAnnotatedMembers(source, "Shadow").firstNotNullOfOrNull { member ->
            member.namedStringValues["prefix"]
        }

    fun findShadowRemap(source: String): Boolean =
        parseAnnotatedMembers(source, "Shadow")
            .firstNotNullOfOrNull { member -> member.namedBooleanValues["remap"] }
            ?: true

    private data class AnnotatedMember(
        val start: Int,
        val end: Int,
        val stringValue: String?,
        val namedStringValues: Map<String, String>,
        val namedBooleanValues: Map<String, Boolean>,
        val declaration: JavaMemberSignature?,
    )

    private data class JavaMemberSignature(
        val name: String,
        val returnType: String,
        val parameterTypes: List<String>,
        val modifiers: Set<String>,
        val isMethod: Boolean,
    )

    private fun parseAnnotatedMembers(source: String, annotationSimpleName: String): List<AnnotatedMember> {
        val members = mutableListOf<AnnotatedMember>()
        var search = 0
        while (search < source.length) {
            val at = source.indexOf('@', search)
            if (at < 0) break
            val nameEnd = readQualifiedNameEnd(source, at + 1)
            val simpleName = source.substring(at + 1, nameEnd).substringAfterLast('.')
            if (simpleName != annotationSimpleName) {
                search = at + 1
                continue
            }
            val annotationEnd = annotationEnd(source, nameEnd)
            val declarationEnd = declarationEnd(source, annotationEnd)
            if (declarationEnd <= annotationEnd) {
                search = annotationEnd
                continue
            }
            val annotationBody = annotationBody(source, nameEnd)
            val declaration = parseDeclaration(source.substring(annotationEnd, declarationEnd))
            members += AnnotatedMember(
                start = at,
                end = declarationEnd,
                stringValue = firstStringLiteral(annotationBody),
                namedStringValues = namedStringAttributes(annotationBody),
                namedBooleanValues = namedBooleanAttributes(annotationBody),
                declaration = declaration,
            )
            search = declarationEnd
        }
        return members
    }

    private fun parseDeclaration(raw: String): JavaMemberSignature? {
        val header = raw.substringBefore('{')
            .substringBefore(';')
            .trim()
        if (header.isEmpty()) return null
        val methodParen = findTopLevelChar(header, '(')
        return if (methodParen >= 0) {
            parseMethodHeader(header, methodParen)
        } else {
            parseFieldHeader(header)
        }
    }

    private fun parseMethodHeader(header: String, paren: Int): JavaMemberSignature? {
        val closeParen = findMatching(header, paren, '(', ')') ?: return null
        val beforeParen = header.substring(0, paren).trim()
        val params = header.substring(paren + 1, closeParen)
        val tokens = splitWhitespaceTopLevel(beforeParen)
        if (tokens.size < 2) return null
        val name = tokens.last()
        val returnType = tokens.dropLast(1).dropModifiersAndTypeParameters().lastOrNull() ?: return null
        return JavaMemberSignature(
            name = name,
            returnType = returnType,
            parameterTypes = JavaTypeDescriptorResolver.splitTopLevel(params, ',')
                .mapNotNull(::extractParameterType),
            modifiers = tokens.filter { it in modifiers }.toSet(),
            isMethod = true,
        )
    }

    private fun parseFieldHeader(header: String): JavaMemberSignature? {
        val withoutInitializer = header.substringBefore('=').trim()
        val tokens = splitWhitespaceTopLevel(withoutInitializer)
        if (tokens.size < 2) return null
        val name = tokens.last().removeSuffix("[]")
        val type = tokens.dropLast(1).dropModifiersAndTypeParameters().lastOrNull() ?: return null
        return JavaMemberSignature(
            name = name,
            returnType = type,
            parameterTypes = emptyList(),
            modifiers = tokens.filter { it in modifiers }.toSet(),
            isMethod = false,
        )
    }

    private fun List<String>.dropModifiersAndTypeParameters(): List<String> {
        val result = mutableListOf<String>()
        var skippingTypeParams = false
        for (token in this) {
            if (token in modifiers) continue
            if (token.startsWith("<")) {
                skippingTypeParams = !token.endsWith(">")
                continue
            }
            if (skippingTypeParams) {
                if (token.endsWith(">")) skippingTypeParams = false
                continue
            }
            result += token
        }
        return result
    }

    private fun extractParameterType(parameter: String): String? {
        val cleaned = stripLeadingAnnotations(parameter)
            .replace(Regex("""\bfinal\b"""), "")
            .trim()
        if (cleaned.isEmpty()) return null
        val tokens = splitWhitespaceTopLevel(cleaned)
        if (tokens.isEmpty()) return null
        return if (tokens.size == 1) tokens[0] else tokens.dropLast(1).joinToString(" ")
    }

    private fun stripLeadingAnnotations(value: String): String {
        var i = 0
        while (i < value.length) {
            while (i < value.length && value[i].isWhitespace()) i++
            if (value.getOrNull(i) != '@') break
            i++
            i = readQualifiedNameEnd(value, i)
            while (i < value.length && value[i].isWhitespace()) i++
            if (value.getOrNull(i) == '(') {
                i = (findMatching(value, i, '(', ')') ?: i) + 1
            }
        }
        return value.substring(i)
    }

    private fun annotationEnd(source: String, nameEnd: Int): Int {
        var i = skipWhitespace(source, nameEnd)
        if (source.getOrNull(i) == '(') {
            i = (findMatching(source, i, '(', ')') ?: i) + 1
        }
        return i
    }

    private fun annotationBody(source: String, nameEnd: Int): String {
        val paren = skipWhitespace(source, nameEnd)
        if (source.getOrNull(paren) != '(') return ""
        val close = findMatching(source, paren, '(', ')') ?: return ""
        return source.substring(paren + 1, close)
    }

    private fun declarationEnd(source: String, start: Int): Int {
        var i = start
        var genericDepth = 0
        var parenDepth = 0
        var inString = false
        while (i < source.length) {
            val ch = source[i]
            when {
                inString -> {
                    if (ch == '\\') {
                        i += 2
                        continue
                    }
                    if (ch == '"') inString = false
                }
                ch == '"' -> inString = true
                ch == '<' -> genericDepth++
                ch == '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                ch == '(' -> parenDepth++
                ch == ')' -> parenDepth = (parenDepth - 1).coerceAtLeast(0)
                (ch == ';' || ch == '{') && genericDepth == 0 && parenDepth == 0 -> return i + 1
            }
            i++
        }
        return source.length
    }

    private fun findTopLevelChar(value: String, target: Char): Int {
        var genericDepth = 0
        for (i in value.indices) {
            when (value[i]) {
                '<' -> genericDepth++
                '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                target -> if (genericDepth == 0) return i
            }
        }
        return -1
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

    private fun skipWhitespace(source: String, start: Int): Int {
        var i = start
        while (i < source.length && source[i].isWhitespace()) i++
        return i
    }

    private fun splitWhitespaceTopLevel(value: String): List<String> {
        val result = mutableListOf<String>()
        var start = -1
        var genericDepth = 0
        for (i in value.indices) {
            val ch = value[i]
            when {
                ch == '<' -> {
                    if (start < 0) start = i
                    genericDepth++
                }
                ch == '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                ch.isWhitespace() && genericDepth == 0 -> {
                    if (start >= 0) {
                        result += value.substring(start, i)
                        start = -1
                    }
                }
                start < 0 -> start = i
            }
        }
        if (start >= 0) result += value.substring(start)
        return result
    }

    private fun firstStringLiteral(value: String): String? =
        Regex(""""([^"]*)"""").find(value)?.groupValues?.get(1)

    private fun namedStringAttributes(value: String): Map<String, String> =
        Regex("""(\w+)\s*=\s*"([^"]*)"""")
            .findAll(value)
            .associate { it.groupValues[1] to it.groupValues[2] }

    private fun namedBooleanAttributes(value: String): Map<String, Boolean> =
        Regex("""(\w+)\s*=\s*(true|false)""")
            .findAll(value)
            .associate { it.groupValues[1] to (it.groupValues[2] == "true") }

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

    private val modifiers = setOf(
        "public",
        "protected",
        "private",
        "abstract",
        "static",
        "final",
        "native",
        "synchronized",
        "strictfp",
        "default",
    )
}
