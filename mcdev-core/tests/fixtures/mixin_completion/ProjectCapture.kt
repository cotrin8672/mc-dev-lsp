package oracle

import io.github.mcdev.core.mapping.*
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.project.ModPlatform
import java.nio.file.Files
import java.nio.file.Path
import java.util.Base64

/** Filesystem snapshots are fixtures, captured before each deterministic operation. */
object ProjectCapture {
    lateinit var root:Path
    private fun args(values:Map<String,Any?>):Map<String,Any?> = values + mapOf("__root" to root.toString(),"__filesystem" to Files.walk(root).use { paths -> paths.sorted().map { path -> mapOf("path" to root.relativize(path).toString(),"directory" to Files.isDirectory(path),"bytes" to if(Files.isRegularFile(path)) Base64.getEncoder().encodeToString(Files.readAllBytes(path)) else null) }.toList() })
    @Suppress("UNCHECKED_CAST")
    fun <T> capture(api:String,values:Map<String,Any?>,body:()->T):T=Capture.capture(api,args(values)) {
        val value=body()
        if(api=="MappingDiscoveryService.buildMappingContext" && value is ProjectMappingContext && value.resolver !== io.github.mcdev.core.project.EmptyMappingResolver) {
            value.copy(resolver=RecordedResolver(value.resolver,values["files"] as List<Path>,values["platform"] as ModPlatform)) as T
        }else value
    }
    private class RecordedResolver(private val target:MappingResolver,private val files:List<Path>,private val platform:ModPlatform):MappingResolver {
        private fun inputs(values:Map<String,Any?>)=values+mapOf("files" to files,"platform" to platform)
        override val namespaces:Set<MappingNamespace> get()=capture("MappingResolver.namespaces",inputs(emptyMap())){target.namespaces}
        override fun remapClass(ref:ClassRef,to:MappingNamespace):MappingLookupResult<ClassRef> = capture("MappingResolver.remapClass",inputs(mapOf("ref" to ref,"to" to to))){target.remapClass(ref,to)}
        override fun remapField(ref:FieldRef,to:MappingNamespace):MappingLookupResult<FieldRef> = capture("MappingResolver.remapField",inputs(mapOf("ref" to ref,"to" to to))){target.remapField(ref,to)}
        override fun remapMethod(ref:MethodRef,to:MappingNamespace):MappingLookupResult<MethodRef> = capture("MappingResolver.remapMethod",inputs(mapOf("ref" to ref,"to" to to))){target.remapMethod(ref,to)}
        override fun remapDescriptor(descriptor:String,from:MappingNamespace,to:MappingNamespace):MappingLookupResult<String> = capture("MappingResolver.remapDescriptor",inputs(mapOf("descriptor" to descriptor,"from" to from,"to" to to))){target.remapDescriptor(descriptor,from,to)}
    }
}
