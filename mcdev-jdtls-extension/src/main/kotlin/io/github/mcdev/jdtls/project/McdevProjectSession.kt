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
        val classIndex = LazyClasspathClassIndex(provider)
        val bytecodeIndexAdapter = BytecodeIndexAdapter(provider, classIndex)
        return copy(
            context = context.copy(indexState = indexStateFor(context)),
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
            val resolvedContext = context.copy(indexState = indexStateFor(context))
            val classIndex = LazyClasspathClassIndex(provider)
            val bytecodeIndexAdapter = BytecodeIndexAdapter(provider, classIndex)
            return McdevProjectSession(
                context = resolvedContext,
                classBytesProvider = provider,
                classIndex = classIndex,
                bytecodeIndex = bytecodeIndexAdapter,
                bytecodeIndexAdapter = bytecodeIndexAdapter,
            )
        }

        private fun indexStateFor(context: ProjectContext): ProjectIndexState =
            if (context.classpath.allEntries.isEmpty()) ProjectIndexState.NOT_READY else ProjectIndexState.READY
    }
}
