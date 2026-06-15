package io.github.mcdev.core.mixin

data class JavaSourceImports(
    val packageName: String?,
    val explicit: Map<String, String>,
    val wildcardPackages: List<String> = emptyList(),
)

data class JavaTypeResolutionContext(
    val imports: JavaSourceImports,
    val lookup: JavaTypeLookup? = null,
)

interface JavaTypeLookup {
    fun findInternalNameByQualifiedName(fqcn: String): String?

    fun findInPackage(packageName: String, simpleName: String): TypeLookupResult

    fun findInWildcardImports(imports: List<String>, simpleName: String): TypeLookupResult
}

class ClassIndexJavaTypeLookup(
    private val classIndex: ClassIndex,
) : JavaTypeLookup {
    override fun findInternalNameByQualifiedName(fqcn: String): String? =
        classIndex.findClassByFqn(fqcn)?.internalName

    override fun findInPackage(packageName: String, simpleName: String): TypeLookupResult =
        classIndex.findClassByFqn("$packageName.$simpleName")
            ?.let { TypeLookupResult.Found(it.internalName) }
            ?: TypeLookupResult.NotFound

    override fun findInWildcardImports(imports: List<String>, simpleName: String): TypeLookupResult {
        val matches = imports.mapNotNull { pkg ->
            classIndex.findClassByFqn("$pkg.$simpleName")?.internalName
        }.distinct()
        return when (matches.size) {
            0 -> TypeLookupResult.NotFound
            1 -> TypeLookupResult.Found(matches.single())
            else -> TypeLookupResult.Ambiguous(matches)
        }
    }
}

sealed interface TypeLookupResult {
    data class Found(val internalName: String) : TypeLookupResult
    data class Ambiguous(val internalNames: List<String>) : TypeLookupResult
    data object NotFound : TypeLookupResult
}

sealed interface TypeDescriptorResult {
    data class Resolved(val descriptor: String) : TypeDescriptorResult
    data class Unresolved(val rawType: String, val normalizedType: String) : TypeDescriptorResult
    data class Ambiguous(val rawType: String, val normalizedType: String, val candidates: List<String>) : TypeDescriptorResult
}

object JavaTypeDescriptorResolver {
    private val primitives = mapOf(
        "void" to "V",
        "boolean" to "Z",
        "byte" to "B",
        "char" to "C",
        "short" to "S",
        "int" to "I",
        "long" to "J",
        "float" to "F",
        "double" to "D",
    )

    private val javaLang = setOf(
        "Boolean",
        "Byte",
        "Character",
        "CharSequence",
        "Class",
        "Double",
        "Enum",
        "Float",
        "Integer",
        "Iterable",
        "Long",
        "Object",
        "Short",
        "String",
        "Throwable",
        "Void",
    )

    private val wellKnown = mapOf(
        "CallbackInfo" to "org.spongepowered.asm.mixin.injection.callback.CallbackInfo",
        "CallbackInfoReturnable" to "org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable",
        "ItemStack" to "net.minecraft.item.ItemStack",
        "BlockPos" to "net.minecraft.util.math.BlockPos",
        "Identifier" to "net.minecraft.util.Identifier",
        "ResourceLocation" to "net.minecraft.resources.ResourceLocation",
        "Component" to "net.minecraft.network.chat.Component",
        "Text" to "net.minecraft.text.Text",
    )

    fun importsFor(source: String): JavaSourceImports {
        val packageName = Regex("""(?m)^\s*package\s+([\w.]+)\s*;""")
            .find(source)
            ?.groupValues
            ?.get(1)
        val explicit = linkedMapOf<String, String>()
        val wildcards = mutableListOf<String>()
        Regex("""(?m)^\s*import\s+(?!static\b)([\w.]+(?:\.\*)?)\s*;""")
            .findAll(source)
            .forEach { match ->
                val fqn = match.groupValues[1]
                if (fqn.endsWith(".*")) {
                    wildcards += fqn.removeSuffix(".*")
                } else {
                    explicit[fqn.substringAfterLast('.')] = fqn
                }
            }
        return JavaSourceImports(packageName, explicit, wildcards)
    }

    fun methodDescriptor(returnType: String, parameterTypes: List<String>, imports: JavaSourceImports): String =
        "(${parameterTypes.joinToString("") { descriptor(it, imports) }})${descriptor(returnType, imports)}"

    fun methodDescriptorOrNull(returnType: String, parameterTypes: List<String>, imports: JavaSourceImports): String? {
        return methodDescriptorOrNull(returnType, parameterTypes, JavaTypeResolutionContext(imports))
    }

    fun methodDescriptorOrNull(
        returnType: String,
        parameterTypes: List<String>,
        context: JavaTypeResolutionContext,
    ): String? {
        val parameterDescriptors = parameterTypes.map { descriptorOrNull(it, context) ?: return null }
        val returnDescriptor = descriptorOrNull(returnType, context) ?: return null
        return "(${parameterDescriptors.joinToString("")})$returnDescriptor"
    }

    fun descriptor(rawType: String, imports: JavaSourceImports): String {
        val (base, arrayDepth) = normalize(rawType)
        primitives[base]?.let { return "[".repeat(arrayDepth) + it }
        val erased = eraseGeneric(base)
        primitives[erased]?.let { return "[".repeat(arrayDepth) + it }
        val fqn = resolveClassName(erased, imports)
        return "[".repeat(arrayDepth) + "L${fqnToInternalName(fqn)};"
    }

    fun descriptorOrNull(rawType: String, imports: JavaSourceImports): String? {
        return descriptorOrNull(rawType, JavaTypeResolutionContext(imports))
    }

    fun descriptorOrNull(rawType: String, context: JavaTypeResolutionContext): String? =
        when (val result = descriptorOrDiagnostic(rawType, context)) {
            is TypeDescriptorResult.Resolved -> result.descriptor
            else -> null
        }

    fun descriptorOrDiagnostic(rawType: String, context: JavaTypeResolutionContext): TypeDescriptorResult {
        val (base, arrayDepth) = normalize(rawType)
        primitives[base]?.let { return TypeDescriptorResult.Resolved("[".repeat(arrayDepth) + it) }
        val erased = eraseGeneric(base)
        primitives[erased]?.let { return TypeDescriptorResult.Resolved("[".repeat(arrayDepth) + it) }
        return when (val resolved = resolveInternalNameOrNull(erased, context)) {
            is TypeLookupResult.Found -> TypeDescriptorResult.Resolved("[".repeat(arrayDepth) + "L${resolved.internalName};")
            is TypeLookupResult.Ambiguous -> TypeDescriptorResult.Ambiguous(rawType, erased, resolved.internalNames)
            TypeLookupResult.NotFound -> TypeDescriptorResult.Unresolved(rawType, erased)
        }
    }

    fun parameterTypes(rawParams: String, imports: JavaSourceImports): List<String> =
        splitTopLevel(rawParams, ',')
            .mapNotNull { parameterType(it) }
            .map { descriptor(it, imports) }

    fun rawParameterTypes(rawParams: String): List<String> =
        splitTopLevel(rawParams, ',').mapNotNull { parameterType(it) }

    fun splitTopLevel(value: String, delimiter: Char): List<String> {
        val results = mutableListOf<String>()
        var start = 0
        var genericDepth = 0
        var parenDepth = 0
        for (i in value.indices) {
            when (value[i]) {
                '<' -> genericDepth++
                '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                '(' -> parenDepth++
                ')' -> parenDepth = (parenDepth - 1).coerceAtLeast(0)
                delimiter -> if (genericDepth == 0 && parenDepth == 0) {
                    results += value.substring(start, i)
                    start = i + 1
                }
            }
        }
        results += value.substring(start)
        return results.map { it.trim() }.filter { it.isNotEmpty() }
    }

    private fun parameterType(parameter: String): String? {
        var cleaned = removeAnnotations(parameter)
            .replace(Regex("""\bfinal\b"""), "")
            .trim()
        if (cleaned.isEmpty()) return null
        val lastSpace = lastTopLevelWhitespace(cleaned)
        if (lastSpace >= 0) {
            val maybeName = cleaned.substring(lastSpace + 1).trim()
            if (maybeName.matches(Regex("""[\w$]+"""))) {
                cleaned = cleaned.substring(0, lastSpace).trim()
            }
        }
        return cleaned.takeIf { it.isNotEmpty() }
    }

    private fun removeAnnotations(value: String): String {
        val result = StringBuilder()
        var i = 0
        while (i < value.length) {
            if (value[i] == '@') {
                i++
                while (i < value.length && (value[i].isLetterOrDigit() || value[i] == '_' || value[i] == '.')) i++
                while (i < value.length && value[i].isWhitespace()) i++
                if (value.getOrNull(i) == '(') {
                    i = skipBalanced(value, i, '(', ')') + 1
                }
                result.append(' ')
                continue
            }
            result.append(value[i])
            i++
        }
        return result.toString()
    }

    private fun lastTopLevelWhitespace(value: String): Int {
        var genericDepth = 0
        for (i in value.indices.reversed()) {
            when (value[i]) {
                '>' -> genericDepth++
                '<' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
                else -> if (genericDepth == 0 && value[i].isWhitespace()) return i
            }
        }
        return -1
    }

    private fun normalize(rawType: String): Pair<String, Int> {
        var type = removeAnnotations(rawType)
            .replace(Regex("""\b(final|volatile|transient)\b"""), "")
            .trim()
        var arrayDepth = 0
        if (type.endsWith("...")) {
            arrayDepth++
            type = type.removeSuffix("...").trim()
        }
        while (type.endsWith("[]")) {
            arrayDepth++
            type = type.removeSuffix("[]").trim()
        }
        return type to arrayDepth
    }

    private fun eraseGeneric(type: String): String {
        val start = type.indexOf('<')
        if (start < 0) return type.trim()
        return type.substring(0, start).trim()
    }

    private fun resolveClassName(type: String, imports: JavaSourceImports): String {
        val normalized = type.trim()
        imports.explicit[normalized]?.let { return it }
        wellKnown[normalized]?.let { return it }
        if (normalized in javaLang) return "java.lang.$normalized"
        if ('.' in normalized) return normalized
        return imports.packageName?.let { "$it.$normalized" } ?: normalized
    }

    private fun resolveClassNameOrNull(type: String, imports: JavaSourceImports): String? {
        val normalized = type.trim()
        imports.explicit[normalized]?.let { return it }
        wellKnown[normalized]?.let { return it }
        if (normalized in javaLang) return "java.lang.$normalized"
        if ('.' in normalized && isKnownPackage(normalized)) return normalized
        return null
    }

    private fun resolveInternalNameOrNull(type: String, context: JavaTypeResolutionContext): TypeLookupResult {
        val normalized = type.trim()
        if (normalized.isEmpty()) return TypeLookupResult.NotFound
        val imports = context.imports
        val lookup = context.lookup

        if ('.' in normalized) {
            lookup?.findInternalNameByQualifiedName(normalized)?.let { return TypeLookupResult.Found(it) }
            if (isKnownPackage(normalized)) return TypeLookupResult.Found(fqnToInternalName(normalized))
            return TypeLookupResult.NotFound
        }

        imports.explicit[normalized]?.let { return TypeLookupResult.Found(fqnToInternalName(it)) }
        if (normalized in javaLang) return TypeLookupResult.Found("java/lang/$normalized")

        if (lookup != null && imports.packageName != null) {
            when (val samePackage = lookup.findInPackage(imports.packageName, normalized)) {
                is TypeLookupResult.Found -> return samePackage
                is TypeLookupResult.Ambiguous -> return samePackage
                TypeLookupResult.NotFound -> Unit
            }
        }

        if (lookup != null && imports.wildcardPackages.isNotEmpty()) {
            when (val wildcard = lookup.findInWildcardImports(imports.wildcardPackages, normalized)) {
                is TypeLookupResult.Found -> return wildcard
                is TypeLookupResult.Ambiguous -> return wildcard
                TypeLookupResult.NotFound -> Unit
            }
        }

        wellKnown[normalized]?.let { return TypeLookupResult.Found(fqnToInternalName(it)) }
        return TypeLookupResult.NotFound
    }

    private fun isKnownPackage(fqn: String): Boolean =
        fqn.startsWith("java.") ||
            fqn.startsWith("javax.") ||
            fqn.startsWith("org.spongepowered.") ||
            fqn.startsWith("net.minecraft.") ||
            fqn.startsWith("com.mojang.")

    private fun fqnToInternalName(fqn: String): String {
        val parts = fqn.split('.')
        val firstClass = parts.indexOfFirst { it.firstOrNull()?.isUpperCase() == true }
        if (firstClass < 0) return fqn.replace('.', '/')
        val pkg = parts.take(firstClass).joinToString("/")
        val cls = parts.drop(firstClass).joinToString("\$")
        return listOf(pkg, cls).filter { it.isNotEmpty() }.joinToString("/")
    }

    private fun skipBalanced(value: String, open: Int, openChar: Char, closeChar: Char): Int {
        var depth = 0
        var inString = false
        var i = open
        while (i < value.length) {
            when {
                inString -> {
                    if (value[i] == '\\') {
                        i += 2
                        continue
                    }
                    if (value[i] == '"') inString = false
                }
                value[i] == '"' -> inString = true
                value[i] == openChar -> depth++
                value[i] == closeChar -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return value.lastIndex
    }
}
