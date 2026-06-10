package io.github.mcdev.core.mixin

import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.model.MappingNamespace

data class ClassIndexEntry(
    val simpleName: String,
    val packageName: String,
    val internalName: String,
) {
    val fqn: String
        get() = if (packageName.isEmpty()) simpleName else "$packageName.$simpleName"
}

data class MethodIndexEntry(
    val name: String,
    val descriptor: String,
    val isStatic: Boolean,
    val readableSignature: String,
)

data class FieldIndexEntry(
    val name: String,
    val descriptor: String,
    val isStatic: Boolean,
    val readableType: String,
)

enum class AtTargetKind {
    INVOKE,
    FIELD,
    NEW,
    RETURN,
    CONSTANT,
}

data class AtTargetCandidate(
    val owner: String,
    val name: String,
    val descriptor: String,
    val displayLabel: String,
    val detail: String,
    val kind: AtTargetKind,
    val ordinal: Int? = null,
    val namespace: MappingNamespace = MappingNamespace.NAMED,
    val constantValue: ConstantValue? = null,
)

interface ClassIndex {
    fun findClasses(prefix: String, limit: Int = 50): List<ClassIndexEntry>

    fun findClass(internalName: String): ClassIndexEntry?

    fun findClassByFqn(fqn: String): ClassIndexEntry?

    fun getMethods(ownerInternalName: String): List<MethodIndexEntry>

    fun getFields(ownerInternalName: String): List<FieldIndexEntry>
}

interface BytecodeIndex {
    fun getAtTargetCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
        atValue: String,
    ): List<AtTargetCandidate>

    fun getReturnOrdinalCount(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): Int
}
