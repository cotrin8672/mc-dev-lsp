package oracle

import io.github.mcdev.core.mapping.*
import io.github.mcdev.core.model.MappingNamespace
import io.github.mcdev.core.aw.AccessWidenerDiagnosticRequest
import io.github.mcdev.core.at.AtDiagnosticRequest
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.definition.McDefinitionTarget

/** Domain adapters only; execution and assertion recording use the shared recorder. */
object AwAtCapture {
    fun arg(value: Any?): Any? = when (value) {
        is ProjectMappingContext -> mapOf("sourceNamespace" to value.sourceNamespace, "runtimeNamespace" to value.runtimeNamespace,
            "awNamespace" to value.awNamespace,"atNamespace" to value.atNamespace,"availableNamespaces" to value.availableNamespaces)
        is AccessWidenerDiagnosticRequest -> mapOf("source" to value.source,"documentUri" to value.documentUri,"mappingContext" to arg(value.mappingContext))
        is AtDiagnosticRequest -> mapOf("source" to value.source,"documentUri" to value.documentUri,"mappingContext" to arg(value.mappingContext),"classIndex" to Capture.arg(value.classIndex))
        is McFix -> mapOf("_type" to value.javaClass.simpleName,"_value" to value)
        is McDefinitionTarget -> mapOf("_type" to value.javaClass.simpleName,"_value" to value)
        else -> Capture.arg(value)
    }
    fun context(value: ProjectMappingContext?): ProjectMappingContext? = value?.copy(resolver = RecordingResolver(value.resolver))
    fun request(value: AccessWidenerDiagnosticRequest) = value.copy(mappingContext=context(value.mappingContext))
    fun request(value: AtDiagnosticRequest) = value.copy(mappingContext=context(value.mappingContext),classIndex=Capture.index(value.classIndex))
    private class RecordingResolver(private val target: MappingResolver): MappingResolver {
        override val namespaces get() = target.namespaces
        override fun remapClass(ref: ClassRef,to: MappingNamespace) = Capture.read("MappingResolver.remapClass",listOf(ref,to),target.remapClass(ref,to))
        override fun remapField(ref: FieldRef,to: MappingNamespace) = Capture.read("MappingResolver.remapField",listOf(ref,to),target.remapField(ref,to))
        override fun remapMethod(ref: MethodRef,to: MappingNamespace) = Capture.read("MappingResolver.remapMethod",listOf(ref,to),target.remapMethod(ref,to))
        override fun remapDescriptor(descriptor: String,from: MappingNamespace,to: MappingNamespace) = Capture.read("MappingResolver.remapDescriptor",listOf(descriptor,from,to),target.remapDescriptor(descriptor,from,to))
    }
}
