package oracle

import io.github.mcdev.jdtls.project.CachedProjectSession
import io.github.mcdev.jdtls.project.McdevProjectSession
import org.eclipse.core.runtime.IProgressMonitor

/** Only owned facts cross the one-time oracle boundary; JDT objects never reach Rust. */
object CommandCapture {
    fun arg(value:Any?):Any?=when(value){
        is McdevProjectSession -> mapOf("context" to value.context)
        is CachedProjectSession -> mapOf("session" to arg(value.session),"version" to value.version,"cacheHit" to value.cacheHit)
        is IProgressMonitor -> mapOf("monitor" to true)
        is Function<*> -> mapOf("callback" to true)
        is Throwable -> mapOf("type" to value.javaClass.name,"message" to value.message)
        Unit -> null
        else -> Capture.arg(value)
    }
    fun <T> read(api:String,args:Any?,body:()->T):T{
        try{val value=body();Capture.read(api,args,arg(value));return value}
        catch(error:Throwable){Capture.read(api,args,mapOf("throwable" to error.javaClass.name,"message" to error.message));throw error}
    }
}
