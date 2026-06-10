package io.github.mcdev.jdtls.project

import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.core.project.ProjectIndexState

data class McdevProjectSession(
    val context: ProjectContext,
    val classBytesProvider: ClasspathClassBytesProvider,
    val classIndex: ClassIndex,
    val bytecodeIndex: BytecodeIndex,
    private val bytecodeIndexAdapter: BytecodeIndexAdapter,
) {
    fun reindex(): McdevProjectSession {
        val provider = ClasspathClassBytesProvider(
            entries = context.classpath.allEntries,
            entryTimestamps = context.classpath.entryTimestamps,
        )
        val memberIndex = ClasspathIndexBuilder.build(provider)
        val classIndex = ClassMemberIndexAdapter(memberIndex)
        val bytecodeIndexAdapter = BytecodeIndexAdapter(provider, classIndex)
        return copy(
            context = context.copy(indexState = ProjectIndexState.READY),
            classBytesProvider = provider,
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndexAdapter,
            bytecodeIndexAdapter = bytecodeIndexAdapter,
        )
    }

    companion object {
        fun create(context: ProjectContext): McdevProjectSession {
            val provider = ClasspathClassBytesProvider(
                entries = context.classpath.allEntries,
                entryTimestamps = context.classpath.entryTimestamps,
            )
            val indexState = if (provider.classCount() > 0) ProjectIndexState.READY else ProjectIndexState.NOT_READY
            val resolvedContext = context.copy(indexState = indexState)
            val memberIndex = if (indexState == ProjectIndexState.READY) {
                ClasspathIndexBuilder.build(provider)
            } else {
                io.github.mcdev.core.bytecode.ClassMemberIndex(
                    classes = emptyMap(),
                    methodsByOwner = emptyMap(),
                    fieldsByOwner = emptyMap(),
                )
            }
            val classIndex = ClassMemberIndexAdapter(memberIndex)
            val bytecodeIndexAdapter = BytecodeIndexAdapter(provider, classIndex)
            return McdevProjectSession(
                context = resolvedContext,
                classBytesProvider = provider,
                classIndex = classIndex,
                bytecodeIndex = bytecodeIndexAdapter,
                bytecodeIndexAdapter = bytecodeIndexAdapter,
            )
        }
    }
}
