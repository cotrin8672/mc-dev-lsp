package io.github.mcdev.core.bytecode

import org.objectweb.asm.ClassReader
import org.objectweb.asm.Opcodes

object ClassMemberIndexBuilder {
    fun build(provider: ClassBytesProvider, classpathEntryId: String? = null): ClassMemberIndex {
        val classes = linkedMapOf<String, ClassIndexEntry>()
        val methodsByOwner = linkedMapOf<String, MutableList<MethodIndexEntry>>()
        val fieldsByOwner = linkedMapOf<String, MutableList<FieldIndexEntry>>()

        when (provider) {
            is InMemoryClassBytesProvider -> {
                provider.allClasses().forEach { (internalName, bytes) ->
                    indexClass(bytes, internalName, classpathEntryId, classes, methodsByOwner, fieldsByOwner)
                }
            }
            else -> error("unsupported ClassBytesProvider for bulk indexing: ${provider::class.simpleName}")
        }

        return ClassMemberIndex(
            classes = classes,
            methodsByOwner = methodsByOwner.mapValues { it.value.toList() },
            fieldsByOwner = fieldsByOwner.mapValues { it.value.toList() },
        )
    }

    fun indexClass(
        bytes: ByteArray,
        internalName: String? = null,
        classpathEntryId: String? = null,
    ): ClassMemberIndex {
        val classes = linkedMapOf<String, ClassIndexEntry>()
        val methodsByOwner = linkedMapOf<String, MutableList<MethodIndexEntry>>()
        val fieldsByOwner = linkedMapOf<String, MutableList<FieldIndexEntry>>()
        indexClass(bytes, internalName, classpathEntryId, classes, methodsByOwner, fieldsByOwner)
        return ClassMemberIndex(
            classes = classes,
            methodsByOwner = methodsByOwner.mapValues { it.value.toList() },
            fieldsByOwner = fieldsByOwner.mapValues { it.value.toList() },
        )
    }

    private fun indexClass(
        bytes: ByteArray,
        forcedInternalName: String?,
        classpathEntryId: String?,
        classes: MutableMap<String, ClassIndexEntry>,
        methodsByOwner: MutableMap<String, MutableList<MethodIndexEntry>>,
        fieldsByOwner: MutableMap<String, MutableList<FieldIndexEntry>>,
    ) {
        val reader = ClassReader(bytes)
        val classInternalName = forcedInternalName ?: reader.className
        val simpleName = classInternalName.substringAfterLast('/')
        val qualifiedName = classInternalName.replace('/', '.')

        classes[classInternalName] = ClassIndexEntry(
            internalName = classInternalName,
            qualifiedName = qualifiedName,
            simpleName = simpleName,
            superclassInternalName = reader.superName?.takeUnless { it == "java/lang/Object" },
            interfaceInternalNames = reader.interfaces?.toList() ?: emptyList(),
            classpathEntryId = classpathEntryId,
        )

        val visitor = object : org.objectweb.asm.ClassVisitor(Opcodes.ASM9) {
            override fun visitMethod(
                access: Int,
                name: String,
                descriptor: String,
                signature: String?,
                exceptions: Array<out String>?,
            ): org.objectweb.asm.MethodVisitor? {
                val entry = MethodIndexEntry(
                    ownerInternalName = classInternalName,
                    name = name,
                    descriptor = descriptor,
                    isStatic = access and Opcodes.ACC_STATIC != 0,
                )
                methodsByOwner.getOrPut(classInternalName) { mutableListOf() }.add(entry)
                return null
            }

            override fun visitField(
                access: Int,
                name: String,
                descriptor: String,
                signature: String?,
                value: Any?,
            ): org.objectweb.asm.FieldVisitor? {
                val entry = FieldIndexEntry(
                    ownerInternalName = classInternalName,
                    name = name,
                    descriptor = descriptor,
                    isStatic = access and Opcodes.ACC_STATIC != 0,
                )
                fieldsByOwner.getOrPut(classInternalName) { mutableListOf() }.add(entry)
                return null
            }
        }

        reader.accept(visitor, ClassReader.SKIP_CODE or ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES)
    }
}
