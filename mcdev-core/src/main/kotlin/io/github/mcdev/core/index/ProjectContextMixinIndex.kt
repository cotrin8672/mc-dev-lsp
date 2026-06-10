package io.github.mcdev.core.index

import io.github.mcdev.core.bytecode.BytecodeIndexService
import io.github.mcdev.core.bytecode.ClassBytesProvider
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.project.ProjectContext

class ProjectContextMixinIndex(
    private val bytecodeService: BytecodeIndexService = BytecodeIndexService(),
) {
    fun buildClassIndex(
        context: ProjectContext,
        provider: ClassBytesProvider,
    ): ClassIndex {
        context.classpath
        val memberIndex = bytecodeService.buildIndex(provider, context.projectId)
        return BytecodeClassIndexAdapter(memberIndex)
    }

    fun buildBytecodeIndex(
        context: ProjectContext,
        provider: ClassBytesProvider,
    ): BytecodeIndex {
        context.classpath
        return BytecodeAtTargetAdapter(bytecodeService, provider)
    }
}
