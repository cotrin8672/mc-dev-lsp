package io.github.mcdev.core.bytecode

data class ClassIndexEntry(
    val internalName: String,
    val qualifiedName: String,
    val simpleName: String,
    val superclassInternalName: String?,
    val interfaceInternalNames: List<String>,
    val classpathEntryId: String?,
)

data class MethodIndexEntry(
    val ownerInternalName: String,
    val name: String,
    val descriptor: String,
    val isStatic: Boolean,
    val isConstructor: Boolean = name == "<init>",
)

data class FieldIndexEntry(
    val ownerInternalName: String,
    val name: String,
    val descriptor: String,
    val isStatic: Boolean,
)

data class ClassMemberIndex(
    val classes: Map<String, ClassIndexEntry>,
    val methodsByOwner: Map<String, List<MethodIndexEntry>>,
    val fieldsByOwner: Map<String, List<FieldIndexEntry>>,
) {
    fun findClass(internalName: String): ClassIndexEntry? = classes[internalName]

    fun findMethod(ownerInternalName: String, name: String, descriptor: String): MethodIndexEntry? =
        methodsByOwner[ownerInternalName]?.firstOrNull { it.name == name && it.descriptor == descriptor }

    fun findField(ownerInternalName: String, name: String, descriptor: String): FieldIndexEntry? =
        fieldsByOwner[ownerInternalName]?.firstOrNull { it.name == name && it.descriptor == descriptor }
}
