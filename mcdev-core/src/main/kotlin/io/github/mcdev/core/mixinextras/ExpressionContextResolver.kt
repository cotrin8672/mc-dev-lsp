package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry

data class ExpressionContext(
    val definitionId: String?,
    val expression: String?,
)

object ExpressionContextResolver {
    private val definitionPattern = Regex("""@Definition\s*\(\s*id\s*=\s*"([^"]*)"\s*\)""")
    private val expressionPattern = Regex("""@Expression\s*\(\s*"([^"]*)"\s*\)""")
    private val invokeExpressionPattern = Regex("""^(?:this\.)?([\w.]+)\.(\w+)\s*\(([^)]*)\)\s*$""")
    private val bareInvokePattern = Regex("""^(\w+)\s*\(([^)]*)\)\s*$""")
    private val fieldAccessPattern = Regex("""^(?:this\.)?(\w+)\.(\w+)\s*$""")

    fun parseHandlerAnnotations(handlerRegion: String): ExpressionContext {
        val definitionId = definitionPattern.find(handlerRegion)?.groupValues?.get(1)
        val expression = expressionPattern.find(handlerRegion)?.groupValues?.get(1)
        return ExpressionContext(definitionId = definitionId, expression = expression)
    }

    fun handlerRegion(source: String, site: MixinExtrasAnnotationSite): String {
        val start = positionToOffset(source, site.annotationRange.end)
        val end = site.handlerMethod?.let { positionToOffset(source, it.range.start) } ?: start
        return source.substring(start.coerceIn(0, source.length), end.coerceIn(0, source.length))
    }

    fun inferExpressionValueType(
        source: String,
        site: MixinExtrasAnnotationSite,
        targetMethod: MethodIndexEntry,
        mixinTargets: List<String>,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
    ): String? {
        val context = parseHandlerAnnotations(handlerRegion(source, site))
        val expression = context.expression?.trim() ?: return null
        return inferFromExpression(
            expression = expression,
            ownerInternalName = mixinTargets.firstOrNull() ?: return null,
            targetMethod = targetMethod,
            bytecodeIndex = bytecodeIndex,
            classIndex = classIndex,
        )
    }

    fun inferFromExpression(
        expression: String,
        ownerInternalName: String,
        targetMethod: MethodIndexEntry,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
    ): String? {
        invokeExpressionPattern.matchEntire(expression)?.let { match ->
            val receiver = match.groupValues[1]
            val methodName = match.groupValues[2]
            val receiverOwner = resolveExpressionOwner(receiver, classIndex)
            return inferInvokeReturnType(ownerInternalName, targetMethod, methodName, receiverOwner, bytecodeIndex)
        }
        bareInvokePattern.matchEntire(expression)?.let { match ->
            val methodName = match.groupValues[1]
            return inferInvokeReturnType(ownerInternalName, targetMethod, methodName, null, bytecodeIndex)
        }
        fieldAccessPattern.matchEntire(expression)?.let { match ->
            val fieldName = match.groupValues[2]
            return inferFieldType(ownerInternalName, targetMethod, fieldName, bytecodeIndex, classIndex)
        }
        return null
    }

    private fun inferInvokeReturnType(
        ownerInternalName: String,
        targetMethod: MethodIndexEntry,
        methodName: String,
        receiverOwner: String?,
        bytecodeIndex: BytecodeIndex,
    ): String? {
        val candidates = bytecodeIndex.getAtTargetCandidates(
            ownerInternalName,
            targetMethod.name,
            targetMethod.descriptor,
            "INVOKE",
        )
        val match = candidates.filter {
            it.kind == AtTargetKind.INVOKE &&
                it.name == methodName &&
                (receiverOwner == null || it.owner == receiverOwner)
        }.firstOrNull() ?: return null
        return methodReturnDescriptor(match.descriptor)
    }

    private fun resolveExpressionOwner(receiver: String, classIndex: ClassIndex): String? {
        if (!receiver.contains('.')) return null
        return classIndex.findClassByFqn(receiver)?.internalName ?: receiver.replace('.', '/')
    }

    private fun inferFieldType(
        ownerInternalName: String,
        targetMethod: MethodIndexEntry,
        fieldName: String,
        bytecodeIndex: BytecodeIndex,
        classIndex: ClassIndex,
    ): String? {
        val candidates = bytecodeIndex.getAtTargetCandidates(
            ownerInternalName,
            targetMethod.name,
            targetMethod.descriptor,
            "FIELD",
        )
        val match = candidates.filter {
            it.kind == AtTargetKind.FIELD && it.name == fieldName
        }.firstOrNull()
        if (match != null) return match.descriptor
        return classIndex.getFields(ownerInternalName).find { it.name == fieldName }?.descriptor
    }

    private fun methodReturnDescriptor(descriptor: String): String? {
        val close = descriptor.indexOf(')')
        return if (close >= 0 && close + 1 < descriptor.length) descriptor.substring(close + 1) else null
    }

    private fun positionToOffset(source: String, position: McTextPosition): Int {
        var line = 0
        var offset = 0
        while (offset < source.length && line < position.line) {
            if (source[offset] == '\n') line++
            offset++
        }
        return (offset + position.character).coerceAtMost(source.length)
    }
}
