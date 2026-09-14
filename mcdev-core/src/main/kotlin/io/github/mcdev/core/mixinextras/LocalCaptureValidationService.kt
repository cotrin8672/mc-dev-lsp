package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.MethodIndexEntry

class LocalCaptureValidationService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
) {
    private val typeResolver = LocalCaptureTypeResolver(classIndex)
    private val occurrenceResolver = InjectionPointOccurrenceResolver(bytecodeIndex)

    data class Point(
        val owner: String,
        val targetMethod: MethodIndexEntry,
        val site: MixinExtrasAnnotationSite,
        val expressionInstructionIndices: Set<Int>? = null,
    )

    sealed interface Result {
        val point: Point
        val snapshots: List<LocalCaptureSnapshot>

        data class Resolved(
            override val point: Point,
            override val snapshots: List<LocalCaptureSnapshot>,
            val candidates: List<LocalCaptureCandidate>,
        ) : Result

        data class NotFound(
            override val point: Point,
            override val snapshots: List<LocalCaptureSnapshot>,
        ) : Result

        data class Ambiguous(
            override val point: Point,
            override val snapshots: List<LocalCaptureSnapshot>,
        ) : Result

        data class Unavailable(
            override val point: Point,
            override val snapshots: List<LocalCaptureSnapshot> = emptyList(),
        ) : Result
    }

    fun validate(
        source: String,
        parameter: HandlerParameterDeclaration,
        points: List<Point>,
    ): List<Result> {
        val cancellationChecker = OfficialExpressionCancellationContext.current()
        cancellationChecker.checkCancelled()
        val spec = parameter.sugarSpec as? HandlerParameterSugarSpec.Local
        val targetDescriptor = spec?.let { typeResolver.resolveParameter(source, parameter) }
        if (spec == null || targetDescriptor == null) {
            return points.map(Result::Unavailable)
        }
        return points.map { validatePoint(it, spec, targetDescriptor, cancellationChecker) }
    }

    private fun validatePoint(
        point: Point,
        spec: HandlerParameterSugarSpec.Local,
        targetDescriptor: String,
        cancellationChecker: OfficialExpressionCancellationChecker,
    ): Result {
        cancellationChecker.checkCancelled()
        val classBytes = bytecodeIndex.getClassBytes(point.owner)
        if (classBytes == null || classBytes.isEmpty()) {
            return Result.Unavailable(point)
        }

        val site = enrichSite(point.site)
        val indices = occurrenceResolver.resolve(
            owner = point.owner,
            targetMethod = point.targetMethod,
            site = site,
            expressionInstructionIndices = point.expressionInstructionIndices,
        ) ?: return Result.Unavailable(point)
        if (indices.isEmpty()) {
            return Result.NotFound(point, emptyList())
        }

        val extraction = LocalCaptureExtractor.extract(
            classBytes = classBytes,
            methodName = point.targetMethod.name,
            methodDescriptor = point.targetMethod.descriptor,
            instructionOccurrenceIndices = indices,
            cancellationChecker = cancellationChecker,
        )
        val snapshots = (extraction as? LocalCaptureResult.Success)?.snapshots
            ?: return Result.Unavailable(point)
        if (snapshots.isEmpty()) {
            return Result.NotFound(point, snapshots)
        }

        val resolutions = snapshots.map { snapshot ->
            LocalDiscriminatorResolver.resolve(spec, targetDescriptor, snapshot.candidates)
        }
        return when {
            resolutions.any { it is LocalDiscriminatorResolution.Ambiguous } ->
                Result.Ambiguous(point, snapshots)
            resolutions.any { it is LocalDiscriminatorResolution.NotFound } ->
                Result.NotFound(point, snapshots)
            else -> Result.Resolved(
                point = point,
                snapshots = snapshots,
                candidates = resolutions.map { (it as LocalDiscriminatorResolution.Resolved).candidate },
            )
        }
    }

    private fun enrichSite(site: MixinExtrasAnnotationSite): MixinExtrasAnnotationSite {
        val handler = site.handlerMethod ?: return site
        return site.copy(handlerMethod = HandlerSignatureService.enrichHandlerTypes(handler, classIndex))
    }
}
