package oracle

import com.google.gson.GsonBuilder
import com.google.gson.JsonElement
import com.google.gson.JsonSerializationContext
import com.google.gson.JsonSerializer
import io.github.mcdev.core.mixin.*
import java.io.File
import java.lang.reflect.Type
import java.nio.file.Path

/** Test-only recorder. It observes callbacks and index reads without evaluating them twice. */
object Capture {
    var caseId = ""
    var legacyClass = ""
    lateinit var output: File
    val counts = linkedMapOf<String, Int>()
    private data class Frame(val api: String, val args: Any?, val reads: MutableList<Any> = mutableListOf())
    private val frames = ThreadLocal.withInitial { mutableListOf<Frame>() }
    val gson = GsonBuilder().serializeNulls().disableHtmlEscaping().registerTypeHierarchyAdapter(Path::class.java,
        JsonSerializer<Path> { value: Path, _: Type, context: JsonSerializationContext -> context.serialize(value.toString()) }).create()

    fun arg(value: Any?): Any? = when(value) {
        is JavaTypeResolutionContext -> mapOf("imports" to value.imports, "hasLookup" to (value.lookup != null))
        is ClassIndex -> mapOf("hasIndex" to true)
        else -> value
    }
    fun <T> capture(api: String, args: Any?, body: () -> T): T {
        if (caseId.isEmpty()) return body()
        val frame = Frame(api, args)
        frames.get().add(frame)
        try {
            val value = body()
            val row = mapOf("legacyCase" to caseId, "legacyClass" to legacyClass, "api" to api, "args" to args,
                "outputType" to value?.let { it::class.java.name }, "output" to value, "reads" to frame.reads)
            synchronized(this) { output.appendText(gson.toJson(row) + "\n"); counts[caseId] = (counts[caseId] ?: 0) + 1 }
            return value
        } catch (error: Throwable) {
            synchronized(this) { output.appendText(gson.toJson(mapOf("legacyCase" to caseId, "legacyClass" to legacyClass,
                "api" to api, "args" to args, "throwable" to error.javaClass.name, "message" to error.message, "reads" to frame.reads)) + "\n") }
            throw error
        } finally { frames.get().removeAt(frames.get().lastIndex) }
    }
    fun <T> read(api: String, args: Any?, value: T): T {
        for (frame in frames.get()) frame.reads += mapOf("api" to api, "args" to args, "output" to value,
            "outputType" to value?.let { it::class.java.name })
        return value
    }
    fun index(index: ClassIndex?): ClassIndex? = if(index == null || index is RecordingIndex) index else RecordingIndex(index)
    private class RecordingIndex(private val target: ClassIndex): ClassIndex {
        override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = read("ClassIndex.findClasses",listOf(prefix,limit),target.findClasses(prefix,limit))
        override fun findClass(internalName: String): ClassIndexEntry? = read("ClassIndex.findClass",listOf(internalName),target.findClass(internalName))
        override fun findClassByFqn(fqn: String): ClassIndexEntry? = read("ClassIndex.findClassByFqn",listOf(fqn),target.findClassByFqn(fqn))
        override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = read("ClassIndex.getMethods",listOf(ownerInternalName),target.getMethods(ownerInternalName))
        override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = read("ClassIndex.getFields",listOf(ownerInternalName),target.getFields(ownerInternalName))
    }
    fun context(context: JavaTypeResolutionContext): JavaTypeResolutionContext = context.copy(lookup = context.lookup?.let { if(it is RecordingLookup) it else RecordingLookup(it) })
    private class RecordingLookup(private val target: JavaTypeLookup): JavaTypeLookup {
        override fun findInternalNameByQualifiedName(fqcn: String): String? = read("JavaTypeLookup.findInternalNameByQualifiedName",listOf(fqcn),target.findInternalNameByQualifiedName(fqcn))
        override fun findInPackage(packageName: String, simpleName: String): TypeLookupResult = read("JavaTypeLookup.findInPackage",listOf(packageName,simpleName),target.findInPackage(packageName,simpleName))
        override fun findInWildcardImports(imports: List<String>, simpleName: String): TypeLookupResult = read("JavaTypeLookup.findInWildcardImports",listOf(imports,simpleName),target.findInWildcardImports(imports,simpleName))
    }
}
