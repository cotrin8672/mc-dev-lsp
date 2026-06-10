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

data class AddAccessTransformerEntryFix(
    override val title: String,
    val documentUri: String,
    val modifier: String,
    val owner: String,
    val memberName: String?,
    val memberDescriptor: String?,
    val insertLine: Int,
) : McFix {
    override val kind: String = "quickfix.at.generateEntry"
}

data class RemapAccessTransformerEntryFix(
    override val title: String,
    val documentUri: String,
    val line: Int,
) : McFix {
    override val kind: String = "quickfix.at.remapNamespace"
}

data class AddAtMethodDescriptorFix(
    override val title: String,
    val documentUri: String,
    val line: Int,
    val memberName: String,
    val descriptor: String,
) : McFix {
    override val kind: String = "quickfix.at.addDescriptor"
}

data class RemoveDuplicateAtEntryFix(
    override val title: String,
    val documentUri: String,
    val line: Int,
) : McFix {
    override val kind: String = "quickfix.at.removeDuplicate"
}

data class GenerateAccessWidenerEntryFix(
    override val title: String,
    val documentUri: String,
    val insertLine: Int,
    val entry: String,
) : McFix {
    override val kind: String = "quickfix.aw.generateEntry"
}

data class RemapAccessWidenerEntryFix(
    override val title: String,
    val documentUri: String,
    val lineNumber: Int,
    val newLine: String,
) : McFix {
    override val kind: String = "quickfix.aw.remapNamespace"
}

data class AddAccessWidenerDescriptorFix(
    override val title: String,
    val documentUri: String,
    val startOffset: Int,
    val endOffset: Int,
    val descriptor: String,
) : McFix {
    override val kind: String = "quickfix.aw.addDescriptor"
}

data class FixAccessWidenerDescriptorFix(
    override val title: String,
    val documentUri: String,
    val startOffset: Int,
    val endOffset: Int,
    val descriptor: String,
) : McFix {
    override val kind: String = "quickfix.aw.fixDescriptor"
}

data class RemoveDuplicateAccessWidenerEntryFix(
    override val title: String,
    val documentUri: String,
    val lineNumber: Int,
) : McFix {
    override val kind: String = "quickfix.aw.removeDuplicate"
}
