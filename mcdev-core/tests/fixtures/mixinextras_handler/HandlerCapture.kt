package oracle
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixinextras.OperationCallArgumentTypeResolver
import io.github.mcdev.core.mixinextras.AtShiftSpec
import io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec
import io.github.mcdev.core.bytecode.ConstantValue
import com.google.gson.JsonSerializer
import java.lang.reflect.Type

object HandlerCapture {
 val shiftSerializer=JsonSerializer<AtShiftSpec>{ value,_,context -> context.serialize(when(value){AtShiftSpec.Before->"Before";AtShiftSpec.After->"After";is AtShiftSpec.By->mapOf("By" to mapOf("offset" to value.offset));AtShiftSpec.Unresolved->"Unresolved"}) }
 val sugarSerializer=JsonSerializer<HandlerParameterSugarSpec>{ value,_,context -> context.serialize(when(value){is HandlerParameterSugarSpec.Local->mapOf("Local" to mapOf("argsOnly" to value.argsOnly,"index" to value.index,"ordinal" to value.ordinal,"names" to value.names,"print" to value.print,"typeClassName" to value.typeClassName));is HandlerParameterSugarSpec.Share->mapOf("Share" to mapOf("value" to value.value,"namespace" to value.namespace,"namespaceSpecified" to value.namespaceSpecified));HandlerParameterSugarSpec.Cancellable->"Cancellable"}) }
 val constantSerializer=JsonSerializer<ConstantValue>{ value,_,context -> context.serialize(when(value){is ConstantValue.StringValue->mapOf("StringValue" to value.value);is ConstantValue.IntValue->mapOf("IntValue" to value.value);is ConstantValue.LongValue->mapOf("LongValue" to value.value);is ConstantValue.FloatValue->mapOf("FloatValue" to value.value);is ConstantValue.DoubleValue->mapOf("DoubleValue" to value.value);is ConstantValue.ClassLiteral->mapOf("ClassLiteral" to value.internalName);ConstantValue.NullValue->"NullValue"}) }
 fun bytecode(index:BytecodeIndex?):BytecodeIndex?=index?.let{ if(it is RecordingBytecode) it else RecordingBytecode(it) }
 private class RecordingBytecode(private val delegate:BytecodeIndex):BytecodeIndex {
  override fun getAtTargetCandidates(ownerInternalName:String,methodName:String,methodDescriptor:String?,atValue:String):List<AtTargetCandidate> = Capture.read("BytecodeIndex.getAtTargetCandidates",listOf(ownerInternalName,methodName,methodDescriptor,atValue),delegate.getAtTargetCandidates(ownerInternalName,methodName,methodDescriptor,atValue))
  override fun getReturnOrdinalCount(ownerInternalName:String,methodName:String,methodDescriptor:String?):Int=Capture.read("BytecodeIndex.getReturnOrdinalCount",listOf(ownerInternalName,methodName,methodDescriptor),delegate.getReturnOrdinalCount(ownerInternalName,methodName,methodDescriptor))
  override fun getClassBytes(ownerInternalName:String):ByteArray?=Capture.read("BytecodeIndex.getClassBytes",listOf(ownerInternalName),delegate.getClassBytes(ownerInternalName))
  override fun resolveCommonSuperClass(type1Descriptor:String,type2Descriptor:String):String?=Capture.read("BytecodeIndex.resolveCommonSuperClass",listOf(type1Descriptor,type2Descriptor),delegate.resolveCommonSuperClass(type1Descriptor,type2Descriptor))
 }
 fun typing(resolver:OperationCallArgumentTypeResolver?):OperationCallArgumentTypeResolver? = resolver?.let{ target -> OperationCallArgumentTypeResolver{ expression -> Capture.read("OperationCallArgumentTypeResolver.resolveType",listOf(expression),target.resolveType(expression)) } }
}
