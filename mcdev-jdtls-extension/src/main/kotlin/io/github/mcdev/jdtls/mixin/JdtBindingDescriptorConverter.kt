package io.github.mcdev.jdtls.mixin

internal object JdtBindingDescriptorConverter {
    fun descriptorFromBinding(binding: Any): String? {
        if (bool(binding, "isRecovered")) return null
        val isPrimitive = bool(binding, "isPrimitive")
        if (isPrimitive) {
            return when (string(binding, "getName")) {
                "void" -> "V"
                "boolean" -> "Z"
                "byte" -> "B"
                "char" -> "C"
                "short" -> "S"
                "int" -> "I"
                "long" -> "J"
                "float" -> "F"
                "double" -> "D"
                else -> null
            }
        }
        if (bool(binding, "isArray")) {
            val elementType = call(binding, "getElementType")
            if (elementType != null) {
                val dims = int(binding, "getDimensions") ?: 1
                val descriptor = descriptorFromBinding(elementType) ?: return null
                return "[".repeat(dims) + descriptor
            }
            val component = call(binding, "getComponentType") ?: return null
            return "[" + (descriptorFromBinding(component) ?: return null)
        }
        val erasure = call(binding, "getErasure") ?: binding
        val binaryName = bindingBinaryName(erasure) ?: return null
        return "L${binaryName.replace('.', '/')};"
    }

    private fun bindingBinaryName(binding: Any): String? =
        string(binding, "getBinaryName")
            ?: string(binding, "getQualifiedName")

    private fun call(node: Any, name: String): Any? =
        runCatching {
            node.javaClass.methods
                .firstOrNull { it.name == name && it.parameterCount == 0 }
                ?.apply { trySetAccessible() }
                ?.invoke(node)
        }.onFailure { JdtMixinSemanticModelParser.throwIfJdtAbort(it) }.getOrNull()

    private fun string(node: Any, name: String): String? =
        call(node, name) as? String

    private fun int(node: Any, name: String): Int? =
        call(node, name) as? Int

    private fun bool(node: Any, name: String): Boolean =
        call(node, name) as? Boolean ?: false
}
