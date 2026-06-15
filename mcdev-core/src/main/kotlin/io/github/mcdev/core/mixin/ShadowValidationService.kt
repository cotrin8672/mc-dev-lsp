package io.github.mcdev.core.mixin

import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

data class ShadowMemberDeclaration(
    val name: String,
    val isMethod: Boolean,
    val descriptor: String,
    val isStatic: Boolean,
    val range: McTextRange,
    val parseSource: ParseSource = ParseSource.HAND_WRITTEN,
    val confidence: ParseConfidence = ParseConfidence.HIGH,
    val warnings: List<String> = emptyList(),
)

class ShadowValidationService(
    private val classIndex: ClassIndex,
) {
    fun validate(
        mixinTargets: List<String>,
        declaration: ShadowMemberDeclaration,
        shadowPrefix: String? = null,
        remap: Boolean = true,
    ): List<McDiagnostic> {
        if (mixinTargets.isEmpty()) return emptyList()
        val targetName = resolveShadowTargetName(declaration.name, shadowPrefix)
        for (owner in mixinTargets) {
            val diagnostics = if (declaration.isMethod) {
                validateMethod(owner, targetName, declaration)
            } else {
                validateField(owner, targetName, declaration)
            }
            if (diagnostics.isEmpty()) return emptyList()
            if (mixinTargets.size == 1) return diagnostics
        }
        return listOf(
            McDiagnostic(
                code = MixinDiagnosticCodes.SHADOW_TARGET_NOT_FOUND,
                severity = McSeverity.ERROR,
                message = "@Shadow target '$targetName' not found in mixin targets",
                range = declaration.range,
                metadata = mapOf("name" to targetName),
            ),
        )
    }

    fun completeFields(
        mixinTargets: List<String>,
        prefix: String,
        shadowPrefix: String? = null,
    ): List<FieldIndexEntry> {
        val results = mutableListOf<FieldIndexEntry>()
        for (owner in mixinTargets) {
            results += classIndex.getFields(owner).filter { field ->
                val effective = applyPrefix(field.name, shadowPrefix, inverse = true)
                effective.startsWith(prefix)
            }
        }
        return results.distinctBy { it.name }
    }

    fun completeMethods(
        mixinTargets: List<String>,
        prefix: String,
        shadowPrefix: String? = null,
    ): List<MethodIndexEntry> {
        val results = mutableListOf<MethodIndexEntry>()
        for (owner in mixinTargets) {
            results += classIndex.getMethods(owner).filter { method ->
                val effective = applyPrefix(method.name, shadowPrefix, inverse = true)
                effective.startsWith(prefix)
            }
        }
        return results.distinctBy { "${it.name}${it.descriptor}" }
    }

    private fun validateField(owner: String, targetName: String, declaration: ShadowMemberDeclaration): List<McDiagnostic> {
        val fields = classIndex.getFields(owner)
        val match = fields.find { it.name == targetName }
        if (match == null) {
            return listOf(notFoundDiagnostic(declaration, targetName))
        }
        if (match.isStatic != declaration.isStatic) {
            return listOf(
                McDiagnostic(
                    code = MixinDiagnosticCodes.SHADOW_STATIC_MISMATCH,
                    severity = McSeverity.ERROR,
                    message = "@Shadow static mismatch for field '$targetName'",
                    range = declaration.range,
                ),
            )
        }
        val parsed = parseFieldDescriptor(declaration.descriptor)
        if (parsed is io.github.mcdev.core.descriptor.DescriptorParseResult.Success) {
            val expected = DescriptorRenderer.toDescriptor(parsed.value)
            if (expected != match.descriptor) {
                return listOf(descriptorMismatch(declaration, targetName))
            }
        }
        return emptyList()
    }

    private fun validateMethod(owner: String, targetName: String, declaration: ShadowMemberDeclaration): List<McDiagnostic> {
        val methods = classIndex.getMethods(owner).filter { it.name == targetName }
        if (methods.isEmpty()) {
            return listOf(notFoundDiagnostic(declaration, targetName))
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
                    code = MixinDiagnosticCodes.SHADOW_STATIC_MISMATCH,
                    severity = McSeverity.ERROR,
                    message = "@Shadow static mismatch for method '$targetName'",
                    range = declaration.range,
                ),
            )
        }
        return emptyList()
    }

    private fun resolveShadowTargetName(shadowName: String, prefix: String?): String {
        if (prefix.isNullOrEmpty()) return shadowName
        return if (shadowName.startsWith(prefix)) {
            shadowName.removePrefix(prefix)
        } else {
            shadowName
        }
    }

    private fun applyPrefix(name: String, prefix: String?, inverse: Boolean): String {
        if (prefix.isNullOrEmpty()) return name
        return if (inverse && name.startsWith(prefix)) name.removePrefix(prefix) else name
    }

    private fun notFoundDiagnostic(declaration: ShadowMemberDeclaration, targetName: String) = McDiagnostic(
        code = MixinDiagnosticCodes.SHADOW_TARGET_NOT_FOUND,
        severity = McSeverity.ERROR,
        message = "@Shadow target not found: $targetName",
        range = declaration.range,
        metadata = mapOf("name" to targetName),
    )

    private fun descriptorMismatch(declaration: ShadowMemberDeclaration, targetName: String) = McDiagnostic(
        code = MixinDiagnosticCodes.SHADOW_DESCRIPTOR_MISMATCH,
        severity = McSeverity.ERROR,
        message = "@Shadow descriptor mismatch for '$targetName'",
        range = declaration.range,
        metadata = mapOf("name" to targetName),
    )
}
