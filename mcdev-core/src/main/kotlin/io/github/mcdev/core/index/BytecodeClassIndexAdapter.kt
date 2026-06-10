package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.ClassMemberIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry

class BytecodeClassIndexAdapter(
    private val memberIndex: ClassMemberIndex,
) : ClassIndex {
    private val classes: List<ClassIndexEntry> =
        memberIndex.classes.values.map(BytecodeIndexEntryMapper::toMixinClass)

    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
        classes
            .filter {
                it.simpleName.startsWith(prefix, ignoreCase = true) ||
                    it.fqn.startsWith(prefix, ignoreCase = true) ||
                    it.internalName.startsWith(prefix, ignoreCase = true)
            }
            .take(limit)

    override fun findClass(internalName: String): ClassIndexEntry? =
        memberIndex.findClass(internalName)?.let(BytecodeIndexEntryMapper::toMixinClass)

    override fun findClassByFqn(fqn: String): ClassIndexEntry? =
        classes.find { it.fqn == fqn }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
        memberIndex.methodsByOwner[ownerInternalName]
            .orEmpty()
            .map(BytecodeIndexEntryMapper::toMixinMethod)

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
        memberIndex.fieldsByOwner[ownerInternalName]
            .orEmpty()
            .map(BytecodeIndexEntryMapper::toMixinField)
}
