package io.github.mcdev.core.codeaction

data class McTextEdit(
    val startOffset: Int,
    val endOffset: Int,
    val newText: String,
)

sealed interface McFix {
    val title: String
    val kind: String
}

data class WorkspaceEditFix(
    override val title: String,
    override val kind: String,
    val documentUri: String,
    val edits: List<McTextEdit>,
    val metadata: Map<String, String> = emptyMap(),
) : McFix

data class AddMixinConfigEntryFix(
    override val title: String,
    val configPath: String,
    val mixinClassName: String,
    val mixinPackage: String?,
    val arrayName: String = "mixins",
) : McFix {
    override val kind: String = "quickfix.mixin.config"
}

data class AddMethodDescriptorFix(
    override val title: String,
    val documentUri: String,
    val startOffset: Int,
    val endOffset: Int,
    val methodName: String,
    val descriptor: String,
) : McFix {
    override val kind: String = "quickfix.mixin.methodDescriptor"
}

data class GenerateAccessorMethodFix(
    override val title: String,
    val documentUri: String,
    val insertOffset: Int,
    val methodSource: String,
    val fieldName: String,
    val isGetter: Boolean,
) : McFix {
    override val kind: String = "quickfix.mixin.generateAccessor"
}

data class GenerateInvokerMethodFix(
    override val title: String,
    val documentUri: String,
    val insertOffset: Int,
    val methodSource: String,
    val targetMethodName: String,
) : McFix {
    override val kind: String = "quickfix.mixin.generateInvoker"
}
