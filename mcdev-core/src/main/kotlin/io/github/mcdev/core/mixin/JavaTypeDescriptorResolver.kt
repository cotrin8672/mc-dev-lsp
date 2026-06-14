package io.github.mcdev.core.mixin

internal data class JavaSourceImports(
    val packageName: String?,
    val explicit: Map<String, String>,
)

internal object JavaTypeDescriptorResolver {
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
        Regex("""(?m)^\s*import\s+(?!static\b)([\w.]+)\s*;""")
            .findAll(source)
            .forEach { match ->
                val fqn = match.groupValues[1]
                explicit[fqn.substringAfterLast('.')] = fqn
            }
        return JavaSourceImports(packageName, explicit)
    }

    fun methodDescriptor(returnType: String, parameterTypes: List<String>, imports: JavaSourceImports): String =
        "(${parameterTypes.joinToString("") { descriptor(it, imports) }})${descriptor(returnType, imports)}"

    fun descriptor(rawType: String, imports: JavaSourceImports): String {
        val (base, arrayDepth) = normalize(rawType)
        primitives[base]?.let { return "[".repeat(arrayDepth) + it }
        val erased = eraseGeneric(base)
        primitives[erased]?.let { return "[".repeat(arrayDepth) + it }
        val fqn = resolveClassName(erased, imports)
        return "[".repeat(arrayDepth) + "L${fqnToInternalName(fqn)};"
    }

    fun parameterTypes(rawParams: String, imports: JavaSourceImports): List<String> =
        splitTopLevel(rawParams, ',')
            .mapNotNull { parameterType(it) }
            .map { descriptor(it, imports) }

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
