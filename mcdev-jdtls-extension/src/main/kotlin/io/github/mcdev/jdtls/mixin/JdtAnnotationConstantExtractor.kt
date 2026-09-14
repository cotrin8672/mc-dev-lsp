package io.github.mcdev.jdtls.mixin

internal object JdtAnnotationConstantExtractor {
    fun constantString(node: Any): String? {
        val value = resolveConstant(node) ?: return null
        return value as? String
    }

    fun constantBoolean(node: Any): Boolean? {
        val value = resolveConstant(node) ?: return null
        return value as? Boolean
    }

    fun constantInt(node: Any): Int? {
        val value = resolveConstant(node) ?: return null
        return value as? Int
    }

    fun stringArray(node: Any): List<String>? =
        mapArrayElements(node) { element -> constantString(element) }

    fun typeLiteralDescriptor(node: Any): String? {
        val binding = typeBinding(node) ?: return null
        return JdtBindingDescriptorConverter.descriptorFromBinding(binding)
    }

    /** Convert JDT primitive descriptors to the source names expected by the core resolver. */
    fun typeLiteralSourceName(node: Any): String? =
        typeLiteralDescriptor(node)?.let { descriptor ->
            PRIMITIVE_SOURCE_NAMES_BY_DESCRIPTOR[descriptor] ?: descriptor
        }

    internal fun typeBinding(node: Any): Any? {
        if (!isUsableNode(node)) {
            return null
        }
        val binding = if (node.javaClass.simpleName == "TypeLiteral") {
            call(node, "getType")?.let { call(it, "resolveBinding") }
        } else {
            call(node, "resolveBinding") ?: call(node, "resolveTypeBinding")
        }
        return binding?.takeUnless { bool(it, "isRecovered") }
    }

    fun typeArray(node: Any): List<String>? =
        mapArrayElements(node) { element -> typeLiteralDescriptor(element) }

    fun typeArraySourceNames(node: Any): List<String>? =
        mapArrayElements(node) { element -> typeLiteralSourceName(element) }

    fun nestedAnnotationArray(node: Any): List<Any>? {
        if (!isUsableNode(node)) {
            return null
        }
        val elements = annotationElements(node) ?: return null
        val result = ArrayList<Any>(elements.size)
        for (element in elements) {
            if (element == null || !isAnnotationNode(element) || !isUsableNode(element)) {
                return null
            }
            result += element
        }
        return result
    }

    private fun mapArrayElements(
        node: Any,
        extract: (Any) -> String?,
    ): List<String>? {
        if (!isUsableNode(node)) {
            return null
        }
        val elements = arrayElements(node) ?: return null
        val result = ArrayList<String>(elements.size)
        for (element in elements) {
            if (element == null) {
                return null
            }
            val value = extract(element) ?: return null
            result += value
        }
        return result
    }

    private fun resolveConstant(node: Any): Any? {
        if (!isUsableNode(node)) {
            return null
        }
        return call(node, "resolveConstantExpressionValue")
    }

    private fun arrayElements(node: Any): List<Any?>? =
        when (node.javaClass.simpleName) {
            "ArrayInitializer" -> expressionList(node)
            else -> listOf(node)
        }

    private fun annotationElements(node: Any): List<Any?>? =
        when {
            isAnnotationNode(node) -> listOf(node)
            node.javaClass.simpleName == "ArrayInitializer" -> expressionList(node)
            else -> null
        }

    private fun expressionList(node: Any): List<Any?>? =
        runCatching {
            @Suppress("UNCHECKED_CAST")
            node.javaClass.getMethod("expressions").apply { trySetAccessible() }.invoke(node) as? List<Any?>
        }.onFailure { JdtMixinSemanticModelParser.throwIfJdtAbort(it) }.getOrNull()

    private fun isAnnotationNode(node: Any): Boolean =
        node.javaClass.simpleName.endsWith("Annotation")

    private fun isUsableNode(node: Any): Boolean =
        !isRecoveredOrMalformed(node)

    private fun isRecoveredOrMalformed(node: Any): Boolean {
        if (hasMethod(node, "getFlags")) {
            val flags = int(node, "getFlags") ?: return false
            val recoveredFlag = astNodeFlag(node, "RECOVERED")
            val malformedFlag = astNodeFlag(node, "MALFORMED")
            if (recoveredFlag != null || malformedFlag != null) {
                if (recoveredFlag != null && flags and recoveredFlag != 0) {
                    return true
                }
                if (malformedFlag != null && flags and malformedFlag != 0) {
                    return true
                }
                return false
            }
        }
        return bool(node, "isRecovered") || bool(node, "isMalformed")
    }

    private fun astNodeFlag(node: Any, name: String): Int? {
        var clazz: Class<*>? = node.javaClass
        while (clazz != null) {
            val field = runCatching { clazz.getField(name) }.getOrNull()
            if (field != null && field.type == Int::class.javaPrimitiveType) {
                return runCatching { field.getInt(null) }.getOrNull()
            }
            clazz = clazz.superclass
        }
        val astNodeClass = runCatching { Class.forName("org.eclipse.jdt.core.dom.ASTNode") }.getOrNull()
        if (astNodeClass != null) {
            val field = runCatching { astNodeClass.getField(name) }.getOrNull()
            if (field != null && field.type == Int::class.javaPrimitiveType) {
                return runCatching { field.getInt(null) }.getOrNull()
            }
        }
        return null
    }

    private fun hasMethod(node: Any, name: String): Boolean =
        node.javaClass.methods.any { it.name == name && it.parameterCount == 0 }

    private fun call(node: Any, name: String): Any? =
        runCatching {
            node.javaClass.methods
                .firstOrNull { it.name == name && it.parameterCount == 0 }
                ?.apply { trySetAccessible() }
                ?.invoke(node)
        }.onFailure { JdtMixinSemanticModelParser.throwIfJdtAbort(it) }.getOrNull()

    private fun int(node: Any, name: String): Int? =
        call(node, name) as? Int

    private fun bool(node: Any, name: String): Boolean =
        call(node, name) as? Boolean ?: false

    private val PRIMITIVE_SOURCE_NAMES_BY_DESCRIPTOR = mapOf(
        "V" to "void",
        "Z" to "boolean",
        "B" to "byte",
        "C" to "char",
        "S" to "short",
        "I" to "int",
        "J" to "long",
        "F" to "float",
        "D" to "double",
    )
}
