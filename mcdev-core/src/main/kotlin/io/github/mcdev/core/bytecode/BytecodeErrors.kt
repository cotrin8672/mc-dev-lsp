package io.github.mcdev.core.bytecode

sealed interface BytecodeIndexError {
    val message: String
}

data class ClassBytesMissingError(
    val internalName: String,
) : BytecodeIndexError {
    override val message: String = "class bytes missing for $internalName"
}

data class MethodNotFoundError(
    val ownerInternalName: String,
    val methodName: String,
    val methodDescriptor: String,
) : BytecodeIndexError {
    override val message: String =
        "method not found: $ownerInternalName.$methodName$methodDescriptor"
}
