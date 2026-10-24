package oracle
import com.google.gson.JsonSerializer
import io.github.mcdev.core.mixinextras.*

object LocalOracle{
 fun sources(values:Sequence<String>):Sequence<String> = sequence{for(value in values)yield(Capture.read("ShareSources.next",emptyList<String>(),value))}
 val validationSerializer=JsonSerializer<LocalCaptureValidationService.Result>{value,_,context->context.serialize(mapOf(value.javaClass.simpleName to (mapOf("point" to value.point,"snapshots" to value.snapshots)+if(value is LocalCaptureValidationService.Result.Resolved)mapOf("candidates" to value.candidates)else emptyMap())))}
 val discriminatorSerializer=JsonSerializer<LocalDiscriminatorResolution>{value,_,context->context.serialize(when(value){is LocalDiscriminatorResolution.Resolved->mapOf("Resolved" to value.candidate);is LocalDiscriminatorResolution.Ambiguous->mapOf("Ambiguous" to value.candidates);LocalDiscriminatorResolution.NotFound->"NotFound"})}
 val resultSerializer=JsonSerializer<LocalCaptureResult>{value,_,context->context.serialize(when(value){is LocalCaptureResult.Success->mapOf("Success" to mapOf("snapshots" to value.snapshots));is LocalCaptureResult.Failure->mapOf("Failure" to mapOf("error" to value.error))})}
 val errorSerializer=JsonSerializer<LocalCaptureError>{value,_,context->context.serialize(when(value){LocalCaptureMissingClassBytesError->"MissingClassBytes";LocalCaptureEmptyClassBytesError->"EmptyClassBytes";is LocalCaptureCorruptClassBytesError->mapOf("CorruptClassBytes" to mapOf("detail" to value.detail));is LocalCaptureMethodNotFoundError->mapOf("MethodNotFound" to mapOf("methodName" to value.methodName,"methodDescriptor" to value.methodDescriptor));is LocalCaptureAnalysisError->mapOf("Analysis" to mapOf("detail" to value.detail));is LocalCaptureInvalidInstructionIndexError->mapOf("InvalidInstructionIndex" to mapOf("instructionOccurrenceIndex" to value.instructionOccurrenceIndex,"instructionCount" to value.instructionCount))})}
 fun checker(target:OfficialExpressionCancellationChecker):OfficialExpressionCancellationChecker=OfficialExpressionCancellationChecker{
  val phase=if(Thread.currentThread().stackTrace.any{it.className.contains("LocalCaptureExtractor")&&it.className.contains("ExactDescriptorInterpreter")})"LocalAnalysis"else"Other"
  try{target.checkCancelled();Capture.read("LocalCancellation.check",listOf(phase),"ok")}catch(error:Throwable){Capture.read("LocalCancellation.check",listOf(phase),mapOf("throwable" to error.javaClass.name,"message" to error.message));throw error}
 }
}
