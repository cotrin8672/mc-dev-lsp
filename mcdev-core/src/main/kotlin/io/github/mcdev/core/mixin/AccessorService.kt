package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity

data class AccessorMethodDeclaration(
    val methodName: String,
    val returnTypeDescriptor: String?,
    val parameterDescriptors: List<String>,
    val explicitFieldName: String?,
    val range: io.github.mcdev.core.diagnostics.McTextRange,
)

enum class AccessorKind {
    GETTER,
    SETTER,
    UNKNOWN,
}

class AccessorService(
    private val classIndex: ClassIndex,
) {
    fun completeFields(
        mixinTargets: List<String>,
        prefix: String,
    ): List<McCompletionItem> {
        val items = mutableListOf<McCompletionItem>()
        for (owner in mixinTargets) {
            classIndex.getFields(owner)
                .filter { it.name.startsWith(prefix) }
                .forEach { field ->
                    items += McCompletionItem(
                        label = field.name,
                        detail = field.readableType,
                        documentation = field.descriptor,
                        filterText = "${field.name} ${field.readableType}",
                        insertText = field.name,
                        kind = McCompletionKind.FIELD,
                        sortKey = "0500_${field.name}",
                        metadata = McCompletionMetadata(
                            source = "mixin.accessor",
                            owner = owner,
                            name = field.name,
                            descriptor = field.descriptor,
                        ),
                    )
                }
        }
        return items
    }

    fun inferFieldName(declaration: AccessorMethodDeclaration): String? {
        declaration.explicitFieldName?.let { return it }
        val methodName = declaration.methodName
        return when {
            methodName.startsWith("get") && methodName.length > 3 ->
                decapitalize(methodName.substring(3))
            methodName.startsWith("is") && methodName.length > 2 ->
                decapitalize(methodName.substring(2))
            methodName.startsWith("set") && methodName.length > 3 ->
                decapitalize(methodName.substring(3))
            else -> null
        }
    }

    fun inferKind(declaration: AccessorMethodDeclaration): AccessorKind = when {
        declaration.methodName.startsWith("set") && declaration.parameterDescriptors.size == 1 -> AccessorKind.SETTER
        declaration.methodName.startsWith("get") || declaration.methodName.startsWith("is") -> AccessorKind.GETTER
        declaration.returnTypeDescriptor != null && declaration.returnTypeDescriptor != "V" &&
            declaration.parameterDescriptors.isEmpty() -> AccessorKind.GETTER
        declaration.returnTypeDescriptor == "V" && declaration.parameterDescriptors.size == 1 -> AccessorKind.SETTER
        else -> AccessorKind.UNKNOWN
    }

    fun validate(
        mixinTargets: List<String>,
        declaration: AccessorMethodDeclaration,
    ): List<McDiagnostic> {
        val fieldName = inferFieldName(declaration) ?: return listOf(
            McDiagnostic(
                code = MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND,
                severity = McSeverity.ERROR,
                message = "Cannot infer @Accessor field name from '${declaration.methodName}'",
                range = declaration.range,
            ),
        )
        val kind = inferKind(declaration)
        for (owner in mixinTargets) {
            val field = classIndex.getFields(owner).find { it.name == fieldName }
            if (field == null) continue
            val signatureIssues = validateSignature(field, declaration, kind)
            if (signatureIssues.isEmpty()) return emptyList()
            return signatureIssues
        }
        return listOf(
            McDiagnostic(
                code = MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND,
                severity = McSeverity.ERROR,
                message = "@Accessor field '$fieldName' not found in mixin targets",
                range = declaration.range,
                metadata = mapOf("field" to fieldName),
            ),
        )
    }

    fun generateMethodStub(
        field: FieldIndexEntry,
        kind: AccessorKind,
        indent: String = "    ",
    ): String = when (kind) {
        AccessorKind.GETTER -> {
            val typeName = field.readableType
            val methodName = "get${field.name.replaceFirstChar { it.uppercase() }}"
            "${indent}@Accessor(\"${field.name}\")\n${indent}${typeName} $methodName();\n"
        }
        AccessorKind.SETTER -> {
            val typeName = field.readableType
            val methodName = "set${field.name.replaceFirstChar { it.uppercase() }}"
            "${indent}@Accessor(\"${field.name}\")\n${indent}void $methodName($typeName value);\n"
        }
        AccessorKind.UNKNOWN -> ""
    }

    private fun validateSignature(
        field: FieldIndexEntry,
        declaration: AccessorMethodDeclaration,
        kind: AccessorKind,
    ): List<McDiagnostic> {
        if (kind == AccessorKind.UNKNOWN) {
            return listOf(
                McDiagnostic(
                    code = MixinDiagnosticCodes.ACCESSOR_SIGNATURE_MISMATCH,
                    severity = McSeverity.ERROR,
                    message = "Unrecognized @Accessor method shape for '${declaration.methodName}'",
                    range = declaration.range,
                ),
            )
        }
        val fieldType = field.descriptor
        if (kind == AccessorKind.GETTER) {
            if (declaration.returnTypeDescriptor != fieldType) {
                return listOf(signatureMismatch(declaration))
            }
        } else {
            if (declaration.returnTypeDescriptor != "V" || declaration.parameterDescriptors.singleOrNull() != fieldType) {
                return listOf(signatureMismatch(declaration))
            }
        }
        return emptyList()
    }

    private fun signatureMismatch(declaration: AccessorMethodDeclaration) = McDiagnostic(
        code = MixinDiagnosticCodes.ACCESSOR_SIGNATURE_MISMATCH,
        severity = McSeverity.ERROR,
        message = "@Accessor signature mismatch for '${declaration.methodName}'",
        range = declaration.range,
    )

    private fun decapitalize(value: String): String =
        if (value.isEmpty()) value else value.replaceFirstChar { it.lowercase() }

    fun readableFieldType(descriptor: String): String =
        when (val parsed = parseFieldDescriptor(descriptor)) {
            is io.github.mcdev.core.descriptor.DescriptorParseResult.Success ->
                DescriptorRenderer.render(parsed.value)
            else -> descriptor
        }
}
