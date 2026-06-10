package io.github.mcdev.core.model

enum class MappingNamespace {
    NAMED,
    INTERMEDIARY,
    OFFICIAL,
    SRG,
    MCP,
}

enum class MemberKind {
    CLASS,
    METHOD,
    FIELD,
    CONSTRUCTOR,
}

data class ClassRef(
    val internalName: String,
    val namespace: MappingNamespace,
)

data class MethodRef(
    val owner: ClassRef,
    val name: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)

data class FieldRef(
    val owner: ClassRef,
    val name: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)
