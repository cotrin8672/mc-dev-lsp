package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.ClassMemberIndex
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.parseFieldDescriptor
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry

class ClassMemberIndexAdapter(
    private val index: ClassMemberIndex,
) : ClassIndex {
    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> =
        index.classes.values
            .asSequence()
            .map { it.toMixinEntry() }
            .filter {
                it.simpleName.startsWith(prefix, ignoreCase = true) ||
                    it.fqn.startsWith(prefix, ignoreCase = true)
            }
            .take(limit)
            .toList()

    override fun findClass(internalName: String): ClassIndexEntry? =
        index.findClass(internalName)?.toMixinEntry()

    override fun findClassByFqn(fqn: String): ClassIndexEntry? =
        index.classes.values.firstOrNull { it.qualifiedName == fqn }?.toMixinEntry()

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> =
        index.methodsByOwner[ownerInternalName].orEmpty().map { it.toMixinEntry() }

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> =
        index.fieldsByOwner[ownerInternalName].orEmpty().map { it.toMixinEntry() }

    private fun io.github.mcdev.core.bytecode.ClassIndexEntry.toMixinEntry(): ClassIndexEntry {
        val packageName = qualifiedName.substringBeforeLast('.', "")
        return ClassIndexEntry(
            simpleName = simpleName,
            packageName = packageName,
            internalName = internalName,
        )
    }

    private fun io.github.mcdev.core.bytecode.MethodIndexEntry.toMixinEntry(): MethodIndexEntry =
        MethodIndexEntry(
            name = name,
            descriptor = descriptor,
            isStatic = isStatic,
            readableSignature = readableMethodSignature(name, descriptor),
        )

    private fun io.github.mcdev.core.bytecode.FieldIndexEntry.toMixinEntry(): FieldIndexEntry =
        FieldIndexEntry(
            name = name,
            descriptor = descriptor,
            isStatic = isStatic,
            readableType = readableFieldType(descriptor),
        )

    companion object {
        fun readableMethodSignature(name: String, descriptor: String): String =
            when (val parsed = parseMethodDescriptor(descriptor)) {
                is DescriptorParseResult.Success -> "$name${DescriptorRenderer.renderMethod(parsed.value)}"
                else -> "$name$descriptor"
            }

        fun readableFieldType(descriptor: String): String =
            when (val parsed = parseFieldDescriptor(descriptor)) {
                is DescriptorParseResult.Success -> DescriptorRenderer.render(parsed.value)
                else -> descriptor
            }
    }
}
