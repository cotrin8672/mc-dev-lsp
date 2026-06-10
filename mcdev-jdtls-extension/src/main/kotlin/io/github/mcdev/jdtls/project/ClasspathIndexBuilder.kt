package io.github.mcdev.jdtls.project

import io.github.mcdev.core.bytecode.ClassMemberIndex
import io.github.mcdev.core.bytecode.ClassMemberIndexBuilder

internal object ClasspathIndexBuilder {
    fun build(provider: ClasspathClassBytesProvider): ClassMemberIndex {
        val classes = linkedMapOf<String, io.github.mcdev.core.bytecode.ClassIndexEntry>()
        val methodsByOwner = linkedMapOf<String, MutableList<io.github.mcdev.core.bytecode.MethodIndexEntry>>()
        val fieldsByOwner = linkedMapOf<String, MutableList<io.github.mcdev.core.bytecode.FieldIndexEntry>>()

        provider.internalNames().sorted().forEach { internalName ->
            val bytes = provider.getClassBytes(internalName) ?: return@forEach
            val partial = ClassMemberIndexBuilder.indexClass(bytes, internalName)
            classes.putAll(partial.classes)
            partial.methodsByOwner.forEach { (owner, methods) ->
                methodsByOwner.getOrPut(owner) { mutableListOf() }.addAll(methods)
            }
            partial.fieldsByOwner.forEach { (owner, fields) ->
                fieldsByOwner.getOrPut(owner) { mutableListOf() }.addAll(fields)
            }
        }

        return ClassMemberIndex(
            classes = classes,
            methodsByOwner = methodsByOwner.mapValues { it.value.toList() },
            fieldsByOwner = fieldsByOwner.mapValues { it.value.toList() },
        )
    }
}
