package oracle

import java.lang.reflect.Modifier
import java.nio.file.Path
import java.util.IdentityHashMap

/** Test-only snapshot of fake object storage. It never invokes a fake resolver,
 * getter, supplier or folded-constant method an extra time. */
object HostCapture {
    fun arg(value: Any?): Any? = snapshot(value, IdentityHashMap(), 0)
    private fun snapshot(value: Any?, seen: IdentityHashMap<Any,Int>, depth: Int): Any? {
        if (value == null || value is String || value is Number || value is Boolean) return value
        if (value is Char) return mapOf("_class" to "Char", "unit" to value.code)
        if (value is Enum<*>) return value.name
        if (value is Path) return value.toString()
        if (value is Throwable) return mapOf("_class" to value.javaClass.name,"message" to value.message)
        if (value is IntRange) return mapOf("_class" to "kotlin.ranges.IntRange","first" to value.first,"last" to value.last,"step" to value.step)
        if (value is kotlin.Function<*>) return mapOf("_class" to value.javaClass.name,"_callback" to true)
        if (depth > 30) return mapOf("_truncated" to value.javaClass.name)
        if (value is Iterable<*>) return value.map { snapshot(it,seen,depth+1) }
        if (value is Map<*,*>) return value.entries.associate { it.key.toString() to snapshot(it.value,seen,depth+1) }
        if (value.javaClass.isArray) return (0 until java.lang.reflect.Array.getLength(value)).map { snapshot(java.lang.reflect.Array.get(value,it),seen,depth+1) }
        seen[value]?.let { return mapOf("_ref" to it) }
        seen[value] = seen.size + 1
        val out=linkedMapOf<String,Any?>("_class" to value.javaClass.name,
            "_methods" to value.javaClass.methods.map { it.name+"/"+it.parameterCount }.distinct().sorted())
        if (value.javaClass.name.startsWith("java.")) return out
        if (value.javaClass.name.startsWith("io.github.mcdev.jdtls.mixin.")) {
            // Preserve the exact legacy presentation value as fixture data,
            // including Object.toString identity text in malformed-node cases.
            // Production providers instead supply actual snapshot source text.
            out["_sourceText"]=value.toString()
        }
        var type: Class<*>?=value.javaClass
        while(type!=null && type!=Any::class.java) {
            for(field in type.declaredFields) {
                if(Modifier.isStatic(field.modifiers) || field.name.startsWith("this$")) continue
                try { field.trySetAccessible();out[field.name]=snapshot(field.get(value),IdentityHashMap(seen),depth+1) }
                catch(error:Throwable) { out[field.name]=mapOf("_unavailable" to error.javaClass.name) }
            }
            type=type.superclass
        }
        return out
    }
}
