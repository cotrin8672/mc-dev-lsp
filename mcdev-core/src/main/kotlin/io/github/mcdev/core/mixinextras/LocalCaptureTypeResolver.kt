package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexJavaTypeLookup
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.JavaTypeResolutionContext

class LocalCaptureTypeResolver(
    private val classIndex: ClassIndex,
) {
    fun resolveParameter(source: String, parameter: HandlerParameterDeclaration): String? {
        val context = resolutionContext(source)
        val typeName = parameter.typeName
        val genericStart = typeName.indexOf('<')
        if (genericStart >= 0) {
            val baseType = typeName.substring(0, genericStart).trim()
            val baseDescriptor = JavaTypeDescriptorResolver.descriptorOrNull(baseType, context) ?: return null
            val baseInternalName = internalNameFromObjectDescriptor(baseDescriptor)
            if (baseInternalName == OFFICIAL_LOCAL_REF_INTERNAL_NAME) {
                return unwrapLocalRefTypeArgument(typeName, context)
            }
            if (isLocalRefSimpleName(baseType)) {
                return null
            }
        }

        val outerDescriptor = JavaTypeDescriptorResolver.descriptorOrNull(typeName, context)
            ?: return null
        val internalName = internalNameFromObjectDescriptor(outerDescriptor) ?: return outerDescriptor
        if (!internalName.startsWith(SUGAR_REF_PREFIX)) {
            return outerDescriptor
        }
        primitiveRefDescriptor(internalName)?.let { return it }
        if (internalName == OFFICIAL_LOCAL_REF_INTERNAL_NAME) {
            return unwrapLocalRefTypeArgument(typeName, context)
        }
        return outerDescriptor
    }

    fun resolveDefinition(source: String, spec: HandlerParameterSugarSpec.Local): String? {
        val typeClassName = spec.typeClassName ?: return null
        if (typeClassName == "void") {
            return null
        }
        val context = resolutionContext(source)
        val descriptor = JavaTypeDescriptorResolver.descriptorOrNull(typeClassName, context) ?: return null
        if (descriptor == "V") {
            return null
        }
        return descriptor
    }

    private fun resolutionContext(source: String): JavaTypeResolutionContext =
        JavaTypeResolutionContext(
            imports = JavaTypeDescriptorResolver.importsFor(source),
            lookup = ClassIndexJavaTypeLookup(classIndex),
        )

    private fun unwrapLocalRefTypeArgument(
        typeName: String,
        context: JavaTypeResolutionContext,
    ): String? {
        val trimmed = typeName.trim()
        val genericStart = trimmed.indexOf('<')
        if (genericStart < 0) {
            return null
        }
        val baseType = trimmed.substring(0, genericStart).trim()
        val baseDescriptor = JavaTypeDescriptorResolver.descriptorOrNull(baseType, context) ?: return null
        val baseInternalName = internalNameFromObjectDescriptor(baseDescriptor) ?: return null
        if (baseInternalName != OFFICIAL_LOCAL_REF_INTERNAL_NAME) {
            return null
        }

        val genericEnd = findMatchingGenericClose(trimmed, genericStart) ?: return null
        if (trimmed.substring(genericEnd + 1).isNotBlank()) {
            return null
        }
        val genericContent = trimmed.substring(genericStart + 1, genericEnd).trim()
        val typeArgument = singleTopLevelTypeArgument(genericContent) ?: return null
        if (typeArgument.contains('?')) {
            return null
        }

        val erasedTypeArgument = eraseGeneric(typeArgument).trim()
        if (erasedTypeArgument.isEmpty()) {
            return null
        }
        return JavaTypeDescriptorResolver.descriptorOrNull(erasedTypeArgument, context)
    }

    private fun singleTopLevelTypeArgument(genericContent: String): String? {
        if (genericContent.isEmpty()) {
            return null
        }
        var genericDepth = 0
        var start = 0
        var index = 0
        val arguments = mutableListOf<String>()
        while (index <= genericContent.length) {
            if (index == genericContent.length || (genericContent[index] == ',' && genericDepth == 0)) {
                val argument = genericContent.substring(start, index).trim()
                if (argument.isNotEmpty()) {
                    arguments += argument
                }
                start = index + 1
            } else when (genericContent[index]) {
                '<' -> genericDepth++
                '>' -> genericDepth = (genericDepth - 1).coerceAtLeast(0)
            }
            index++
        }
        return arguments.singleOrNull()
    }

    private fun findMatchingGenericClose(typeName: String, genericStart: Int): Int? {
        var depth = 0
        for (index in genericStart until typeName.length) {
            when (typeName[index]) {
                '<' -> depth++
                '>' -> {
                    depth--
                    if (depth == 0) {
                        return index
                    }
                }
            }
        }
        return null
    }

    private fun eraseGeneric(type: String): String {
        val start = type.indexOf('<')
        if (start < 0) {
            return type.trim()
        }
        return type.substring(0, start).trim()
    }

    private fun internalNameFromObjectDescriptor(descriptor: String): String? {
        if (descriptor.startsWith('L') && descriptor.endsWith(';')) {
            return descriptor.substring(1, descriptor.length - 1)
        }
        return null
    }

    private fun primitiveRefDescriptor(internalName: String): String? =
        PRIMITIVE_REF_DESCRIPTORS[internalName]

    private fun isLocalRefSimpleName(type: String): Boolean =
        type.substringAfterLast('.').trim() == "LocalRef"

    private companion object {
        const val SUGAR_REF_PREFIX = "com/llamalad7/mixinextras/sugar/ref/"
        const val OFFICIAL_LOCAL_REF_INTERNAL_NAME = "com/llamalad7/mixinextras/sugar/ref/LocalRef"

        val PRIMITIVE_REF_DESCRIPTORS = mapOf(
            "${SUGAR_REF_PREFIX}LocalBooleanRef" to "Z",
            "${SUGAR_REF_PREFIX}LocalByteRef" to "B",
            "${SUGAR_REF_PREFIX}LocalCharRef" to "C",
            "${SUGAR_REF_PREFIX}LocalShortRef" to "S",
            "${SUGAR_REF_PREFIX}LocalIntRef" to "I",
            "${SUGAR_REF_PREFIX}LocalLongRef" to "J",
            "${SUGAR_REF_PREFIX}LocalFloatRef" to "F",
            "${SUGAR_REF_PREFIX}LocalDoubleRef" to "D",
        )
    }
}
