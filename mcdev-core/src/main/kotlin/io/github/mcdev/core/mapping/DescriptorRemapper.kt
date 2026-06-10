package io.github.mcdev.core.mapping

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.JvmType
import io.github.mcdev.core.descriptor.MethodDescriptor
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.model.MappingNamespace

object DescriptorRemapper {
    fun remapFieldDescriptor(
        descriptor: String,
        from: MappingNamespace,
        to: MappingNamespace,
        resolver: MappingResolver,
    ): MappingLookupResult<String> {
        val parsed = when (val result = parseFieldDescriptor(descriptor)) {
            is DescriptorParseResult.Success -> result.value
            is DescriptorParseResult.Failure -> {
                return MappingLookupResult.InvalidDescriptor(
                    descriptor,
                    result.error.message,
                    result.error.offset,
                )
            }
        }
        return when (val remapped = remapType(parsed, from, to, resolver)) {
            is MappingLookupResult.Found -> MappingLookupResult.Found(DescriptorRenderer.toDescriptor(remapped.value))
            is MappingLookupResult.Missing -> remapped
            is MappingLookupResult.InvalidDescriptor -> remapped
        }
    }

    fun remapMethodDescriptor(
        descriptor: String,
        from: MappingNamespace,
        to: MappingNamespace,
        resolver: MappingResolver,
    ): MappingLookupResult<String> {
        val parsed = when (val result = parseMethodDescriptor(descriptor)) {
            is DescriptorParseResult.Success -> result.value
            is DescriptorParseResult.Failure -> {
                return MappingLookupResult.InvalidDescriptor(
                    descriptor,
                    result.error.message,
                    result.error.offset,
                )
            }
        }
        val parameters = parsed.parameters.map { type ->
            when (val remapped = remapType(type, from, to, resolver)) {
                is MappingLookupResult.Found -> remapped.value
                is MappingLookupResult.Missing -> return remapped
                is MappingLookupResult.InvalidDescriptor -> return remapped
            }
        }
        val returnType = when (val remapped = remapType(parsed.returnType, from, to, resolver)) {
            is MappingLookupResult.Found -> remapped.value
            is MappingLookupResult.Missing -> return remapped
            is MappingLookupResult.InvalidDescriptor -> return remapped
        }
        return MappingLookupResult.Found(DescriptorRenderer.toDescriptor(MethodDescriptor(parameters, returnType)))
    }

    private fun remapType(
        type: JvmType,
        from: MappingNamespace,
        to: MappingNamespace,
        resolver: MappingResolver,
    ): MappingLookupResult<JvmType> = when (type) {
        JvmType.ByteType,
        JvmType.CharType,
        JvmType.DoubleType,
        JvmType.FloatType,
        JvmType.IntType,
        JvmType.LongType,
        JvmType.ShortType,
        JvmType.BooleanType,
        JvmType.VoidType,
        -> MappingLookupResult.Found(type)
        is JvmType.ArrayType -> when (val component = remapType(type.component, from, to, resolver)) {
            is MappingLookupResult.Found -> MappingLookupResult.Found(JvmType.ArrayType(component.value))
            is MappingLookupResult.Missing -> component
            is MappingLookupResult.InvalidDescriptor -> component
        }
        is JvmType.ObjectType -> when (val remapped = resolver.remapClass(ClassRef(type.internalName, from), to)) {
            is MappingLookupResult.Found -> MappingLookupResult.Found(JvmType.ObjectType(remapped.value.internalName))
            is MappingLookupResult.Missing -> remapped
            is MappingLookupResult.InvalidDescriptor -> remapped
        }
    }
}
