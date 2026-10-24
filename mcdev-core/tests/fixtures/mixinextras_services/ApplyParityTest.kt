package oracle
import io.github.mcdev.core.codeaction.*
import io.github.mcdev.core.mixin.*
import io.github.mcdev.core.mixinextras.MixinExtrasCodeActionService
import org.junit.jupiter.api.Test

class ApplyParityTest {
 private val index=object:ClassIndex {
  override fun findClasses(prefix:String,limit:Int)=emptyList<ClassIndexEntry>()
  override fun findClass(internalName:String):ClassIndexEntry?=null
  override fun findClassByFqn(fqn:String):ClassIndexEntry?=null
  override fun getMethods(ownerInternalName:String)=emptyList<MethodIndexEntry>()
  override fun getFields(ownerInternalName:String)=emptyList<FieldIndexEntry>()
 }
 @Test fun applyPreservesTheWorkspaceValue() {
  val fix=WorkspaceEditFix("title","quickfix.mixinextras.test","file:///X.java",listOf(McTextEdit(0,1,"B")),mapOf("key" to "value"))
  check(MixinExtrasCodeActionService(index).applyHandlerFix(fix,"A")==fix)
 }
 @Test fun legacyApplyRetainsOffsetsEvenWhenSourceChanged() {
  val fix=WorkspaceEditFix("title","quickfix.mixinextras.test","file:///X.java",listOf(McTextEdit(0,1,"B")))
  check(MixinExtrasCodeActionService(index).applyHandlerFix(fix,"changed source")==fix)
 }
 @Test fun legacyApplyDoesNotValidateInvalidOffsets() {
  val fix=WorkspaceEditFix("title","quickfix.mixinextras.test","file:///X.java",listOf(McTextEdit(98,100,"B")))
  check(MixinExtrasCodeActionService(index).applyHandlerFix(fix,"")==fix)
 }
}
