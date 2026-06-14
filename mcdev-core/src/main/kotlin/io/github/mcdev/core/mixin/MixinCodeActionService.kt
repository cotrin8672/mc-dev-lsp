package io.github.mcdev.core.mixin

import io.github.mcdev.core.codeaction.AddMethodDescriptorFix
import io.github.mcdev.core.codeaction.AddMixinConfigEntryFix
import io.github.mcdev.core.codeaction.GenerateAccessorMethodFix
import io.github.mcdev.core.codeaction.GenerateInvokerMethodFix
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.codeaction.McTextEdit
import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.diagnostics.McDiagnostic

class MixinCodeActionService(
    private val configEditor: MixinConfigEditor = MixinConfigEditor(),
    private val accessorService: AccessorService? = null,
    private val invokerService: InvokerService? = null,
) {
    fun fixesForDiagnostics(
        diagnostics: List<McDiagnostic>,
        documentUri: String,
        source: String,
        mixinConfigContent: String?,
        mixinConfigPath: String?,
        mixinPackage: String?,
        classIndex: ClassIndex? = null,
    ): List<McFix> {
        val fixes = mutableListOf<McFix>()
        for (diagnostic in diagnostics) {
            fixes += when (diagnostic.code) {
                MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG -> {
                    val mixinClass = diagnostic.metadata["mixinClass"] ?: continue
                    val configPath = mixinConfigPath ?: diagnostic.metadata["configPath"] ?: continue
                    listOf(
                        AddMixinConfigEntryFix(
                            title = "Add '$mixinClass' to mixin config",
                            configPath = configPath,
                            mixinClassName = mixinClass,
                            mixinPackage = mixinPackage,
                        ),
                    )
                }
                MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD -> {
                    val method = diagnostic.metadata["method"] ?: continue
                    val descriptor = findDescriptorForAmbiguousMethod(source, method, classIndex) ?: continue
                    listOf(
                        AddMethodDescriptorFix(
                            title = "Add descriptor to method target",
                            documentUri = documentUri,
                            startOffset = rangeStart(source, diagnostic),
                            endOffset = rangeEnd(source, diagnostic),
                            methodName = method,
                            descriptor = descriptor,
                        ),
                    )
                }
                MixinDiagnosticCodes.ACCESSOR_FIELD_NOT_FOUND -> {
                    val field = diagnostic.metadata["field"] ?: continue
                    if (accessorService == null || classIndex == null) emptyList() else {
                        generateAccessorFix(documentUri, source, field, classIndex)
                    }
                }
                MixinDiagnosticCodes.INVOKER_METHOD_NOT_FOUND -> {
                    val name = diagnostic.metadata["name"] ?: continue
                    if (invokerService == null || classIndex == null) emptyList() else {
                        generateInvokerFix(documentUri, source, name, classIndex)
                    }
                }
                else -> emptyList()
            }
        }
        return fixes
    }

    fun applyMixinConfigFix(
        fix: AddMixinConfigEntryFix,
        currentContent: String,
    ): WorkspaceEditFix? {
        val result = configEditor.addEntry(currentContent, fix.mixinClassName, fix.arrayName)
        if (!result.added) return null
        return WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.configPath,
            edits = listOf(
                McTextEdit(
                    startOffset = 0,
                    endOffset = currentContent.length,
                    newText = result.content,
                ),
            ),
            metadata = mapOf("arrayName" to result.arrayName),
        )
    }

    fun applyMethodDescriptorFix(
        fix: AddMethodDescriptorFix,
        currentSource: String,
    ): WorkspaceEditFix {
        val newText = "${fix.methodName}${fix.descriptor}"
        return WorkspaceEditFix(
            title = fix.title,
            kind = fix.kind,
            documentUri = fix.documentUri,
            edits = listOf(
                McTextEdit(fix.startOffset, fix.endOffset, newText),
            ),
        )
    }

    private fun generateAccessorFix(
        documentUri: String,
        source: String,
        fieldName: String,
        classIndex: ClassIndex,
    ): List<McFix> {
        val mixinTargets = findMixinTargets(source, classIndex)
        val field = mixinTargets.firstNotNullOfOrNull { owner ->
            classIndex.getFields(owner).find { it.name == fieldName }
        } ?: return emptyList()
        val stub = (accessorService ?: AccessorService(classIndex))
            .generateMethodStub(field, AccessorKind.GETTER)
        val insertOffset = findClassBodyInsertOffset(source)
        return listOf(
            GenerateAccessorMethodFix(
                title = "Generate @Accessor getter for '$fieldName'",
                documentUri = documentUri,
                insertOffset = insertOffset,
                methodSource = stub,
                fieldName = fieldName,
                isGetter = true,
            ),
        )
    }

    private fun generateInvokerFix(
        documentUri: String,
        source: String,
        methodName: String,
        classIndex: ClassIndex,
    ): List<McFix> {
        val mixinTargets = findMixinTargets(source, classIndex)
        val method = mixinTargets.flatMap { owner -> classIndex.getMethods(owner) }
            .filter { it.name == methodName }
            .singleOrNull()
            ?: return emptyList()
        val stub = (invokerService ?: InvokerService(classIndex)).generateMethodStub(method)
        val insertOffset = findClassBodyInsertOffset(source)
        return listOf(
            GenerateInvokerMethodFix(
                title = "Generate @Invoker for '$methodName'",
                documentUri = documentUri,
                insertOffset = insertOffset,
                methodSource = stub,
                targetMethodName = methodName,
            ),
        )
    }

    private fun findDescriptorForAmbiguousMethod(
        source: String,
        methodName: String,
        classIndex: ClassIndex?,
    ): String? {
        if (classIndex == null) return null
        val owners = findMixinTargets(source, classIndex)
        val methods = owners.flatMap { classIndex.getMethods(it) }.filter { it.name == methodName }
        return methods.singleOrNull()?.descriptor
    }

    private fun findMixinTargets(source: String, classIndex: ClassIndex): List<String> =
        MixinTargetResolver.resolveTargetsFromSource(source, classIndex)

    private fun findClassBodyInsertOffset(source: String): Int {
        val openBrace = source.indexOf('{')
        return if (openBrace >= 0) openBrace + 1 else source.length
    }

    private fun rangeStart(source: String, diagnostic: McDiagnostic): Int {
        val lineStart = source.lineSequence().take(diagnostic.range.start.line).sumOf { it.length + 1 }
        return lineStart + diagnostic.range.start.character
    }

    private fun rangeEnd(source: String, diagnostic: McDiagnostic): Int {
        val lineStart = source.lineSequence().take(diagnostic.range.end.line).sumOf { it.length + 1 }
        return lineStart + diagnostic.range.end.character
    }
}
