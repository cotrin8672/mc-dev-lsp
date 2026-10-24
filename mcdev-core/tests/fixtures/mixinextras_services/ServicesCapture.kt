package oracle
import com.google.gson.GsonBuilder
import com.google.gson.JsonSerializer
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.mixinextras.HandlerSignatureService

object ServicesCapture {
 fun hasBytecode(service:HandlerSignatureService):Boolean {
  val field=HandlerSignatureService::class.java.getDeclaredField("bytecodeIndex")
  field.isAccessible=true
  return field.get(service)!=null
 }
 val fixSerializer=JsonSerializer<McFix>{value,_,_ ->
  GsonBuilder().serializeNulls().create().toJsonTree(value).asJsonObject.also { it.addProperty("type",value.javaClass.simpleName) }
 }
}
