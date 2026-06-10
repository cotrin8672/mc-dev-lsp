package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.bytecode.AtTargetCandidate as BytecodeAtTargetCandidate
import io.github.mcdev.core.bytecode.AtTargetKind as BytecodeAtTargetKind
import io.github.mcdev.core.bytecode.ClassIndexEntry as BytecodeClassIndexEntry
import io.github.mcdev.core.bytecode.FieldIndexEntry as BytecodeFieldIndexEntry
import io.github.mcdev.core.bytecode.MethodIndexEntry as BytecodeMethodIndexEntry

internal object BytecodeIndexEntryMapper {
    fun toMixinClass(entry: BytecodeClassIndexEntry): ClassIndexEntry {
        val packageName = entry.qualifiedName.substringBeforeLast('.', "")
        return ClassIndexEntry(
            simpleName = entry.simpleName,
            packageName = packageName,
            internalName = entry.internalName,
        )
    }

    fun toMixinMethod(entry: BytecodeMethodIndexEntry): MethodIndexEntry {
        val readable = when (val parsed = parseMethodDescriptor(entry.descriptor)) {
            is DescriptorParseResult.Success ->
                "${entry.name}${DescriptorRenderer.renderMethod(parsed.value)}"
            else -> "${entry.name}${entry.descriptor}"
        }
        return MethodIndexEntry(
            name = entry.name,
            descriptor = entry.descriptor,
            isStatic = entry.isStatic,
            readableSignature = readable,
        )
    }

    fun toMixinField(entry: BytecodeFieldIndexEntry): FieldIndexEntry {
        val readable = when (val parsed = parseFieldDescriptor(entry.descriptor)) {
            is DescriptorParseResult.Success -> DescriptorRenderer.render(parsed.value)
            else -> entry.descriptor
        }
        return FieldIndexEntry(
            name = entry.name,
            descriptor = entry.descriptor,
            isStatic = entry.isStatic,
            readableType = readable,
        )
    }

    fun toMixinAtTarget(candidate: BytecodeAtTargetCandidate): AtTargetCandidate {
        val mixinKind = toMixinAtTargetKind(candidate.kind)
        val ownerSimple = candidate.owner.substringAfterLast('/')
        val displayLabel = when (mixinKind) {
            AtTargetKind.FIELD -> {
                val readable = when (val parsed = parseFieldDescriptor(candidate.descriptor)) {
                    is DescriptorParseResult.Success -> DescriptorRenderer.render(parsed.value)
                    else -> candidate.descriptor
                }
                "${candidate.name}: $readable"
            }
            AtTargetKind.NEW ->
                if (candidate.name == "<init>") {
                    "$ownerSimple.<init>${candidate.descriptor}"
                } else {
                    ownerSimple
                }
            AtTargetKind.RETURN -> "RETURN"
            AtTargetKind.CONSTANT -> formatConstantLabel(candidate.constantValue)
            else -> {
                when (val parsed = parseMethodDescriptor(candidate.descriptor)) {
                    is DescriptorParseResult.Success ->
                        "${candidate.name}${DescriptorRenderer.renderMethod(parsed.value)}"
                    else -> "${candidate.name}${candidate.descriptor}"
                }
            }
        }
        return AtTargetCandidate(
            owner = candidate.owner,
            name = candidate.name,
            descriptor = candidate.descriptor,
            displayLabel = displayLabel,
            detail = ownerSimple,
            kind = mixinKind,
            ordinal = candidate.ordinal,
            namespace = MappingNamespace.NAMED,
            constantValue = candidate.constantValue,
        )
    }

    fun matchesAtValue(kind: BytecodeAtTargetKind, atValue: String): Boolean =
        when (atValue.uppercase()) {
            "INVOKE", "INVOKE_ASSIGN" -> kind in INVOKE_KINDS
            "FIELD" -> kind in FIELD_KINDS
            "NEW" -> kind == BytecodeAtTargetKind.NEW
            "RETURN" -> kind == BytecodeAtTargetKind.RETURN
            "CONSTANT" -> kind == BytecodeAtTargetKind.CONSTANT
            else -> false
        }

    private val INVOKE_KINDS = setOf(
        BytecodeAtTargetKind.INVOKE_VIRTUAL,
        BytecodeAtTargetKind.INVOKE_STATIC,
        BytecodeAtTargetKind.INVOKE_SPECIAL,
        BytecodeAtTargetKind.INVOKE_INTERFACE,
    )

    private val FIELD_KINDS = setOf(
        BytecodeAtTargetKind.FIELD_GET_INSTANCE,
        BytecodeAtTargetKind.FIELD_PUT_INSTANCE,
        BytecodeAtTargetKind.FIELD_GET_STATIC,
        BytecodeAtTargetKind.FIELD_PUT_STATIC,
    )

    private fun toMixinAtTargetKind(kind: BytecodeAtTargetKind): AtTargetKind =
        when (kind) {
            BytecodeAtTargetKind.INVOKE_VIRTUAL,
            BytecodeAtTargetKind.INVOKE_STATIC,
            BytecodeAtTargetKind.INVOKE_SPECIAL,
            BytecodeAtTargetKind.INVOKE_INTERFACE,
                -> AtTargetKind.INVOKE
            BytecodeAtTargetKind.FIELD_GET_INSTANCE,
            BytecodeAtTargetKind.FIELD_PUT_INSTANCE,
            BytecodeAtTargetKind.FIELD_GET_STATIC,
            BytecodeAtTargetKind.FIELD_PUT_STATIC,
                -> AtTargetKind.FIELD
            BytecodeAtTargetKind.NEW -> AtTargetKind.NEW
            BytecodeAtTargetKind.RETURN -> AtTargetKind.RETURN
            BytecodeAtTargetKind.CONSTANT -> AtTargetKind.CONSTANT
        }

    private fun formatConstantLabel(value: ConstantValue?): String =
        when (value) {
            is ConstantValue.StringValue -> "\"${value.value}\""
            is ConstantValue.IntValue -> value.value.toString()
            is ConstantValue.LongValue -> "${value.value}L"
            is ConstantValue.FloatValue -> "${value.value}f"
            is ConstantValue.DoubleValue -> value.value.toString()
            is ConstantValue.ClassLiteral -> value.internalName.substringAfterLast('/')
            ConstantValue.NullValue -> "null"
            null -> "CONSTANT"
        }
}
