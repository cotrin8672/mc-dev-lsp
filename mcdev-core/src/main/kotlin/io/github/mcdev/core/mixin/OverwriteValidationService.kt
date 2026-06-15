package io.github.mcdev.core.mixin

import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity

data class OverwriteMethodDeclaration(
    val name: String,
    val descriptor: String,
    val isStatic: Boolean,
    val range: io.github.mcdev.core.diagnostics.McTextRange,
    val parseSource: ParseSource = ParseSource.HAND_WRITTEN,
    val confidence: ParseConfidence = ParseConfidence.HIGH,
    val warnings: List<String> = emptyList(),
)

class OverwriteValidationService(
    private val classIndex: ClassIndex,
) {
    fun validate(
        mixinTargets: List<String>,
        declaration: OverwriteMethodDeclaration,
    ): List<McDiagnostic> {
        if (mixinTargets.isEmpty()) return emptyList()
        for (owner in mixinTargets) {
            val diagnostics = validateMethod(owner, declaration.name, declaration)
            if (diagnostics.isEmpty()) return emptyList()
            if (mixinTargets.size == 1) return diagnostics
        }
        return listOf(
            McDiagnostic(
                code = MixinDiagnosticCodes.OVERWRITE_TARGET_NOT_FOUND,
                severity = McSeverity.ERROR,
                message = "@Overwrite target '${declaration.name}' not found in mixin targets",
                range = declaration.range,
                metadata = mapOf("name" to declaration.name),
            ),
        )
    }

    private fun validateMethod(
        owner: String,
        targetName: String,
        declaration: OverwriteMethodDeclaration,
    ): List<McDiagnostic> {
        val methods = classIndex.getMethods(owner).filter { it.name == targetName }
        if (methods.isEmpty()) {
            return listOf(
                McDiagnostic(
                    code = MixinDiagnosticCodes.OVERWRITE_TARGET_NOT_FOUND,
                    severity = McSeverity.ERROR,
                    message = "@Overwrite target not found: $targetName",
                    range = declaration.range,
                    metadata = mapOf("name" to targetName),
                ),
            )
        }
        val parsed = parseMethodDescriptor(declaration.descriptor)
        if (parsed !is io.github.mcdev.core.descriptor.DescriptorParseResult.Success) {
            return listOf(descriptorMismatch(declaration, targetName))
        }
        val expected = DescriptorRenderer.toDescriptor(parsed.value)
        val match = methods.find { it.descriptor == expected }
        if (match == null) {
            return listOf(descriptorMismatch(declaration, targetName))
        }
        if (match.isStatic != declaration.isStatic) {
            return listOf(
                McDiagnostic(
                    code = MixinDiagnosticCodes.OVERWRITE_STATIC_MISMATCH,
                    severity = McSeverity.ERROR,
                    message = "@Overwrite static mismatch for method '$targetName'",
                    range = declaration.range,
                ),
            )
        }
        return emptyList()
    }

    private fun descriptorMismatch(declaration: OverwriteMethodDeclaration, targetName: String) = McDiagnostic(
        code = MixinDiagnosticCodes.OVERWRITE_DESCRIPTOR_MISMATCH,
        severity = McSeverity.ERROR,
        message = "@Overwrite descriptor mismatch for '$targetName'",
        range = declaration.range,
        metadata = mapOf("name" to targetName),
    )
}
