package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity

data class InvokerMethodDeclaration(
    val methodName: String,
    val parameterDescriptors: List<String>,
    val returnTypeDescriptor: String?,
    val explicitTargetName: String?,
    val range: io.github.mcdev.core.diagnostics.McTextRange,
    val parseSource: ParseSource = ParseSource.HAND_WRITTEN,
    val confidence: ParseConfidence = ParseConfidence.HIGH,
    val warnings: List<String> = emptyList(),
)

class InvokerService(
    private val classIndex: ClassIndex,
) {
    fun completeMethods(
        mixinTargets: List<String>,
        prefix: String,
    ): List<McCompletionItem> {
        val items = mutableListOf<McCompletionItem>()
        for (owner in mixinTargets) {
            classIndex.getMethods(owner)
                .filter { it.name.startsWith(prefix) }
                .forEach { method ->
                    items += McCompletionItem(
                        label = method.readableSignature,
                        detail = AnnotationContextExtractor.internalToFqn(owner),
                        documentation = method.descriptor,
                        filterText = "${method.name} ${method.readableSignature}",
                        insertText = method.name,
                        kind = McCompletionKind.METHOD,
                        sortKey = "0600_${method.name}",
                        metadata = McCompletionMetadata(
                            source = "mixin.invoker",
                            owner = owner,
                            name = method.name,
                            descriptor = method.descriptor,
                        ),
                    )
                }
        }
        return items
    }

    fun inferTargetName(declaration: InvokerMethodDeclaration): String? {
        declaration.explicitTargetName?.let { return it }
        val methodName = declaration.methodName
        if (methodName.startsWith("invoke") && methodName.length > 6) {
            val remainder = methodName.substring(6)
            return remainder.replaceFirstChar { it.lowercase() }
        }
        return null
    }

    fun validate(
        mixinTargets: List<String>,
        declaration: InvokerMethodDeclaration,
    ): List<McDiagnostic> {
        val targetName = inferTargetName(declaration) ?: return listOf(
            McDiagnostic(
                code = MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND,
                severity = McSeverity.ERROR,
                message = "Cannot infer @Invoker target from '${declaration.methodName}'",
                range = declaration.range,
            ),
        )
        for (owner in mixinTargets) {
            val methods = classIndex.getMethods(owner).filter { it.name == targetName }
            if (methods.isEmpty()) continue
            val paramDesc = "(${declaration.parameterDescriptors.joinToString("")})${declaration.returnTypeDescriptor ?: "V"}"
            val match = methods.find { it.descriptor == paramDesc }
            if (match != null) return emptyList()
            return listOf(
                McDiagnostic(
                    code = MixinDiagnosticCodes.INVOKER_DESCRIPTOR_MISMATCH,
                    severity = McSeverity.ERROR,
                    message = "@Invoker descriptor mismatch for '$targetName'",
                    range = declaration.range,
                    metadata = mapOf("name" to targetName, "descriptor" to paramDesc),
                ),
            )
        }
        return listOf(
            McDiagnostic(
                code = MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND,
                severity = McSeverity.ERROR,
                message = "@Invoker target method '$targetName' not found",
                range = declaration.range,
                metadata = mapOf("name" to targetName),
            ),
        )
    }

    fun generateMethodStub(
        method: MethodIndexEntry,
        indent: String = "    ",
    ): String {
        val invokerName = "invoke${method.name.replaceFirstChar { it.uppercase() }}"
        return "${indent}@Invoker(\"${method.name}\")\n${indent}${method.readableSignature.replace(method.name, invokerName)};\n"
    }
}
