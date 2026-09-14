package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.BytecodeFixtureCompiler
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.FakeBytecodeIndex
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import org.objectweb.asm.ClassReader
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.MethodInsnNode

class LocalCaptureValidationServiceTest {
    private val owner = BytecodeFixtureCompiler.internalName("LocalCaptureSamples")
    private val classBytes = BytecodeFixtureCompiler.classBytes("LocalCaptureSamples")
    private val service = LocalCaptureValidationService(FakeClassIndex(), bytecodeIndex(classBytes))

    @Test
    fun resolvedCaptureRetainsRawSnapshot() {
        val method = method("instanceWithArgs", "(Ljava/lang/String;I)I")
        val occurrence = invokeIndex(method, "java/lang/String", "length")

        val result = service.validate(
            source = "class Mixin {}",
            parameter = parameter(HandlerParameterSugarSpec.Local(index = 3)),
            points = listOf(point(method, occurrence)),
        ).single()

        val resolved = assertIs<LocalCaptureValidationService.Result.Resolved>(result)
        assertEquals(occurrence, resolved.snapshots.single().instructionOccurrenceIndex)
        assertEquals(3, resolved.candidates.single().slotIndex)
    }

    @Test
    fun ambiguousSiteDoesNotDiscardResolvedSite() {
        val resolvedMethod = method("staticWithWide", "(JDI)V")
        val ambiguousMethod = method("instanceWithArgs", "(Ljava/lang/String;I)I")

        val results = service.validate(
            source = "class Mixin {}",
            parameter = parameter(HandlerParameterSugarSpec.Local()),
            points = listOf(
                point(resolvedMethod, invokeIndex(resolvedMethod, owner, "staticWithWideMarker")),
                point(ambiguousMethod, invokeIndex(ambiguousMethod, "java/lang/String", "length")),
            ),
        )

        assertIs<LocalCaptureValidationService.Result.Resolved>(results[0])
        val ambiguous = assertIs<LocalCaptureValidationService.Result.Ambiguous>(results[1])
        assertEquals(2, ambiguous.snapshots.single().candidates.count { it.descriptor == "I" })
    }

    @Test
    fun missingClassBytesWithEmptyInvokeCandidatesIsUnavailableNotNotFound() {
        val method = method("instanceWithArgs", "(Ljava/lang/String;I)I")
        val invokePoint = LocalCaptureValidationService.Point(
            owner = owner,
            targetMethod = method,
            site = MixinExtrasAnnotationSite(
                annotation = MixinExtrasAnnotation.WRAP_OPERATION,
                methodAttribute = method.name,
                atValue = "INVOKE",
                atTarget = null,
                annotationRange = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
                handlerMethod = null,
            ),
        )
        val service = LocalCaptureValidationService(FakeClassIndex(), FakeBytecodeIndex())

        assertIs<LocalCaptureValidationService.Result.Unavailable>(
            service.validate("class Mixin {}", parameter(HandlerParameterSugarSpec.Local()), listOf(invokePoint)).single(),
        )
    }

    @Test
    fun unresolvedTypeOccurrenceAndExtractionAreUnavailableNotNotFound() {
        val method = method("instanceWithArgs", "(Ljava/lang/String;I)I")
        val validPoint = point(method, invokeIndex(method, "java/lang/String", "length"))
        val missingExpressionIndices = validPoint.copy(expressionInstructionIndices = null)

        assertIs<LocalCaptureValidationService.Result.Unavailable>(
            service.validate("class Mixin {}", parameter(HandlerParameterSugarSpec.Local(), "MissingType"), listOf(validPoint)).single(),
        )
        assertIs<LocalCaptureValidationService.Result.Unavailable>(
            service.validate("class Mixin {}", parameter(HandlerParameterSugarSpec.Local()), listOf(missingExpressionIndices)).single(),
        )
        val missingBytesService = LocalCaptureValidationService(FakeClassIndex(), bytecodeIndex(null))
        assertIs<LocalCaptureValidationService.Result.Unavailable>(
            missingBytesService.validate("class Mixin {}", parameter(HandlerParameterSugarSpec.Local()), listOf(validPoint)).single(),
        )
    }

    private fun parameter(
        spec: HandlerParameterSugarSpec.Local,
        typeName: String = "int",
    ) = HandlerParameterDeclaration(
        name = "captured",
        typeName = typeName,
        typeDescriptor = null,
        isOperation = false,
        operationGenericName = null,
        isSugar = true,
        sugarSpec = spec,
    )

    private fun method(name: String, descriptor: String) = MethodIndexEntry(
        name = name,
        descriptor = descriptor,
        isStatic = name == "staticWithWide",
        readableSignature = "$name$descriptor",
    )

    private fun point(method: MethodIndexEntry, occurrence: Int) = LocalCaptureValidationService.Point(
        owner = owner,
        targetMethod = method,
        site = MixinExtrasAnnotationSite(
            annotation = MixinExtrasAnnotation.MODIFY_EXPRESSION_VALUE,
            methodAttribute = method.name,
            atValue = "MIXINEXTRAS:EXPRESSION",
            atTarget = null,
            annotationRange = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
            handlerMethod = null,
        ),
        expressionInstructionIndices = setOf(occurrence),
    )

    private fun invokeIndex(method: MethodIndexEntry, invokeOwner: String, invokeName: String): Int {
        val classNode = ClassNode()
        ClassReader(classBytes).accept(classNode, ClassReader.SKIP_FRAMES)
        val methodNode = classNode.methods.single { it.name == method.name && it.desc == method.descriptor }
        var occurrenceIndex = 0
        var instruction: AbstractInsnNode? = methodNode.instructions.first
        while (instruction != null) {
            if (instruction.opcode >= 0) {
                if (instruction is MethodInsnNode && instruction.owner == invokeOwner && instruction.name == invokeName) {
                    return occurrenceIndex
                }
                occurrenceIndex++
            }
            instruction = instruction.next
        }
        error("invoke not found")
    }

    private fun bytecodeIndex(bytes: ByteArray?) = object : BytecodeIndex {
        override fun getAtTargetCandidates(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
            atValue: String,
        ): List<AtTargetCandidate> = emptyList()

        override fun getReturnOrdinalCount(
            ownerInternalName: String,
            methodName: String,
            methodDescriptor: String?,
        ): Int = 0

        override fun getClassBytes(ownerInternalName: String): ByteArray? = bytes
    }
}
