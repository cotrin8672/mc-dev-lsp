package oracle
import io.github.mcdev.core.mixin.*
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.jdtls.mixin.JdtParseEnvironment
import io.github.mcdev.jdtls.mixin.JdtInjectionPointFeatureProbe
import io.github.mcdev.jdtls.mixin.ProjectSourceQuery
import io.github.mcdev.jdtls.project.McdevProjectSession
internal object HostFacadeCapture {
 fun project(value:Any?):Any?=value?.let { mapOf("present" to true) }
 fun files(context:ProjectContext):Map<String,String> {
  val files=linkedMapOf<String,String>()
  for(config in context.mixinConfigs){if(java.nio.file.Files.isRegularFile(config.path))files[config.path.toAbsolutePath().normalize().toString()]=java.nio.file.Files.readString(config.path)}
  for(root in context.sourceSets.flatMap { it.sourceDirectories })if(java.nio.file.Files.isDirectory(root))java.nio.file.Files.walk(root).use {paths->paths.filter { java.nio.file.Files.isRegularFile(it)&&it.toString().substringAfterLast('.') in setOf("java","kt","kts") }.forEach {files[it.toAbsolutePath().normalize().toString()]=java.nio.file.Files.readString(it)}}
  return files
 }
 fun session(value:McdevProjectSession):Any=mapOf("context" to value.context,"files" to files(value.context))
 fun documentProject(provider:(String)->Any?,uri:String):Any?=provider(uri).also { Capture.read("Host.documentProject",listOf(uri),project(it)) }
 fun workspaceProject(provider:(String)->Any?,uri:String):Any?=provider(uri).also { Capture.read("Host.workspaceProject",listOf(uri),project(it)) }
 fun semantic(provider:(String,String,JdtParseEnvironment)->MixinClassModel,source:String,uri:String,environment:JdtParseEnvironment):MixinClassModel=Capture.read("Host.semanticProvider",listOf(source,uri,environment),provider(source,uri,environment))
 fun atCodes(provider:(Any?,Long)->List<String>,project:Any?,version:Long):List<String> = Capture.read("Host.atCodes",listOf(project(project),version),provider(project,version))
 fun specifier(probe:JdtInjectionPointFeatureProbe,project:Any?,version:Long):Boolean = Capture.read("Host.specifier",listOf(project(project),version),probe.injectionPointSpecifierSupported(project,version))
 fun sourceQuery(provider:(Any)->ProjectSourceQuery,project:Any):ProjectSourceQuery {Capture.read("Host.sourceQueryFactory",listOf(project(project)),true);return provider(project)}
 fun factory(provider:((ClassIndex,BytecodeIndex)->MixinServiceFacade)?,index:ClassIndex,bytecode:BytecodeIndex):MixinServiceFacade? {if(provider==null)return null;Capture.read("Host.coreFactory",emptyList<String>(),true);return provider(Capture.index(index)!!,HandlerCapture.bytecode(bytecode)!!)}
 fun completeOverride(provider:((McdevProjectSession,String,Int,Int,MixinCompletionOptions)->List<McCompletionItem>)?,session:McdevProjectSession,source:String,line:Int,character:Int,options:MixinCompletionOptions):List<McCompletionItem>?=provider?.let { Capture.read("Host.completeOverride",listOf(source,line,character,options),it(session,source,line,character,options)) }
 fun collect(context:ProjectContext,source:String,uri:String,project:Any?,body:()->Sequence<String>):Sequence<String> = sequence {
  yieldAll(Capture.capture("HostMixinServiceFacade.collectRegisteredMixinSources",mapOf("projectContext" to context,"source" to source,"documentUri" to uri,"javaProject" to project(project),"files" to files(context))){body().toList()})
 }
}
