package oracle
import io.github.mcdev.core.mixin.*
import io.github.mcdev.core.mixinextras.*
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Opcodes
import kotlin.test.Test
import kotlin.test.assertTrue
import kotlin.test.assertEquals

class Utf16FacadeOracle {
 @Test fun losslessSourceThroughPublicFacade(){
  for((raw,units)in listOf("\\uD800" to listOf(0xd800),"\\uDC00" to listOf(0xdc00),"\\u0000" to listOf(0),"\\uD83D\\uDE42" to listOf(0xd83d,0xde42),"�" to listOf(0xfffd))){for(kotlin in listOf(false,true)){
   val source=source("someMethod('$raw').tr",kotlin);val cursor=source.indexOf(".tr")+3;val before=source.substring(0,cursor);val line=before.count{it=='\n'};val character=before.substringAfterLast('\n').length;val uri=if(kotlin)"file:///Patch.kt"else"file:///Patch.java"
   val args=mapOf("source" to source,"cursor" to cursor,"line" to line,"character" to character,"documentUri" to uri,"units" to units,"kotlin" to kotlin)
   val sites=HandlerSignatureService.findAnnotationSites(source);val site=sites.single()
   val values=Capture.capture("Utf16Facade.context",args){ExpressionContextResolver.resolveExpressionContext(source,site).expressionIndex.expressions.flatMap{it.values}.map{it.toCharArray().map(Char::code)}}
   assertEquals(listOf("someMethod('$raw').tr".replace(raw,String(units.map(Int::toChar).toCharArray())).toCharArray().map(Char::code)),values)
   val ctx=AnnotationContextExtractor.extractAtOffset(source,cursor)!!
   Capture.capture("Utf16Facade.prefix",args){mapOf("units" to ctx.decodedExpressionPrefix!!.prefix.toCharArray().map(Char::code),"cursor" to ctx.decodedExpressionPrefix!!.cursor,"startOffset" to ctx.valueStartOffset,"endOffset" to ctx.valueEndOffset,"position" to ctx.expressionCompletionPosition!!.name)}
   val index=Capture.index(index())!!;val bytes=bytes(units)
   val bytecode=HandlerCapture.bytecode(object:BytecodeIndex{override fun getAtTargetCandidates(ownerInternalName:String,methodName:String,methodDescriptor:String?,atValue:String)=emptyList<AtTargetCandidate>();override fun getReturnOrdinalCount(ownerInternalName:String,methodName:String,methodDescriptor:String?)=1;override fun getClassBytes(ownerInternalName:String)=if(ownerInternalName=="Utf16Expressions")bytes else null})!!
   val result=Capture.capture("Utf16Facade.complete",args){MixinServiceFacade(index,bytecode).completeWithDebug(MixinFacadeRequest(source,line,character,uri),MixinCompletionOptions(),"mcdev.completion",if(kotlin)"kotlin"else"java")}
   assertTrue(result.items.any{it.metadata.name=="trim"},"$raw kotlin=$kotlin")
  }}
 }
 private fun source(expression:String,kotlin:Boolean):String=if(kotlin) "import Utf16Expressions\r\nimport org.spongepowered.asm.mixin.Mixin\r\nimport com.llamalad7.mixinextras.expression.Expression\r\nimport com.llamalad7.mixinextras.injector.ModifyExpressionValue\r\nimport org.spongepowered.asm.mixin.injection.At\r\n@Mixin(Utf16Expressions::class)\r\nabstract class Patch {\r\n @Expression(\"$expression\")\r\n @ModifyExpressionValue(method = [\"run\"], at = [At(\"MIXINEXTRAS:EXPRESSION\")])\r\n private fun handler(original: String): String = original\r\n}" else "@Mixin(Utf16Expressions.class)\r\nabstract class Patch {\r\n @Expression(\"$expression\")\r\n @ModifyExpressionValue(method = \"run\", at = @At(\"MIXINEXTRAS:EXPRESSION\"))\r\n private static String handler(String original) {return original;}\r\n}"
 private fun index()=object:ClassIndex {
  override fun findClasses(prefix:String,limit:Int)=listOfNotNull(findClass("Utf16Expressions"),findClass("java/lang/String")).filter{it.simpleName.startsWith(prefix)}.take(limit)
  override fun findClass(internalName:String)=if(internalName in setOf("Utf16Expressions","java/lang/String"))ClassIndexEntry(internalName.substringAfterLast('/'),internalName.substringBeforeLast('/',"").replace('/','.'),internalName)else null
  override fun findClassByFqn(fqn:String)=findClass(fqn.replace('.','/'))
  override fun getMethods(ownerInternalName:String)=if(ownerInternalName=="Utf16Expressions")listOf(MethodIndexEntry("run","()Ljava/lang/String;",true,"run(): String"))else emptyList()
  override fun getFields(ownerInternalName:String)=emptyList<FieldIndexEntry>()
 }
 private fun bytes(units:List<Int>):ByteArray {
  val writer=ClassWriter(ClassWriter.COMPUTE_MAXS);writer.visit(52,Opcodes.ACC_PUBLIC,"Utf16Expressions",null,"java/lang/Object",null);val method=writer.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC,"run","()Ljava/lang/String;",null,null);method.visitCode();method.visitLdcInsn(String(units.map(Int::toChar).toCharArray()));method.visitMethodInsn(Opcodes.INVOKESTATIC,"Helper","someMethod","(Ljava/lang/String;)Ljava/lang/String;",false);method.visitMethodInsn(Opcodes.INVOKEVIRTUAL,"java/lang/String","trim","()Ljava/lang/String;",false);method.visitInsn(Opcodes.ARETURN);method.visitMaxs(0,0);method.visitEnd();writer.visitEnd();return writer.toByteArray()
 }
}
