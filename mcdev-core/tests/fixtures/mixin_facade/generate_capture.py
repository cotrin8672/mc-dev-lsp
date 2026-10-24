"""Disposable facade instrumentation; only saved data is used by native Rust tests."""
from pathlib import Path
import importlib.util,json,os,re
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
expr=load('expression_capture',HERE.parent/'mixinextras_expression/generate_capture.py')
helper=expr.helper;helper.OUT=ROOT/'build/facade-oracle'
commands=load('command_capture',ROOT/'mcdev-companion/tests/fixtures/commands/generate_capture.py')
commands.c=helper

def instrument_core():
 path=helper.MAIN/'mixin/MixinServiceFacade.kt'
 target=expr.instrument(path,{'complete','completeWithDebug','diagnose','codeActions'})
 text=target.read_text(encoding='utf8')
 # Constructor ABI and original default arguments remain identical; dependencies
 # are wrapped before the default leaf services receive them.
 begin=text.index('class MixinServiceFacade(');end=helper.closing(text,text.index('(',begin))
 header=text[begin:end+1]
 header=header.replace('private val classIndex: ClassIndex','classIndex: ClassIndex').replace('private val bytecodeIndex: BytecodeIndex','bytecodeIndex: BytecodeIndex')
 header=re.sub(r'\bclassIndex\b(?!\s*:)', 'Capture.index(classIndex)!!',header)
 header=re.sub(r'\bbytecodeIndex\b(?!\s*:)', 'HandlerCapture.bytecode(bytecodeIndex)!!',header)
 text=text[:begin]+header+text[end+1:]
 body=text.index('{',begin+len(header))+1
 tail=re.sub(r'\bclassIndex\b(?!\s*=)', '_recordingClassIndex',text[body:])
 tail=re.sub(r'\bbytecodeIndex\b(?!\s*=)', '_recordingBytecodeIndex',tail)
 text=text[:body]+'\nprivate val _recordingClassIndex=Capture.index(classIndex)!!\nprivate val _recordingBytecodeIndex=HandlerCapture.bytecode(bytecodeIndex)!!\n'+tail
 text=text.replace('shareSources()', 'oracle.FacadeCapture.sources(shareSources)')
 text=text.replace('"MixinServiceFacade.', '"CoreMixinServiceFacade.')
 output=target.with_name('CoreMixinServiceFacade.kt');output.write_text(text,encoding='utf8');return output

def instrument_definition():
 path=helper.MAIN/'mixin/MixinDefinitionService.kt';target=expr.instrument(path,{'definitionsAt','definitionsAtOffset'});text=target.read_text(encoding='utf8')
 begin=text.index('class MixinDefinitionService(');end=helper.closing(text,text.index('(',begin));header=text[begin:end+1].replace('private val classIndex: ClassIndex','classIndex: ClassIndex').replace('private val bytecodeIndex: BytecodeIndex','bytecodeIndex: BytecodeIndex');text=text[:begin]+header+text[end+1:];body=text.index('{',begin+len(header))+1
 tail=re.sub(r'\bclassIndex\b(?!\s*=)', '_recordingClassIndex',text[body:]);tail=re.sub(r'\bbytecodeIndex\b(?!\s*=)', '_recordingBytecodeIndex',tail);text=text[:body]+'\nprivate val _recordingClassIndex=Capture.index(classIndex)!!\nprivate val _recordingBytecodeIndex=HandlerCapture.bytecode(bytecodeIndex)!!\n'+tail
 target.write_text(text,encoding='utf8');return target

def instrument_host():
 path=commands.MAIN/'mixin/MixinServiceFacade.kt';target=expr.instrument(path,{'complete','completeWithDebug','tryCompleteBufferOnly','semanticModel','analyzeDiagnostics','codeActions','definitions','hasJavaProject'})
 text=target.read_text(encoding='utf8').replace('"MixinServiceFacade.','"HostMixinServiceFacade.').replace('import oracle.Capture','import oracle.Capture\nimport oracle.HostFacadeCapture')
 for method in ['complete','completeWithDebug','tryCompleteBufferOnly']:
  text=text.replace('Capture.capture("HostMixinServiceFacade.'+method+'", mapOf(', 'Capture.capture("HostMixinServiceFacade.'+method+'", mapOf("__hasFactory" to (facadeFactory != null),"__hasOverride" to (completeOverride != null),')
 text=text.replace('Capture.arg(session)','HostFacadeCapture.session(session)')
 text=text.replace('javaProjectResolver(documentUri)','HostFacadeCapture.documentProject(javaProjectResolver,documentUri)')
 text=text.replace('workspaceRootUri?.let(workspaceJavaProjectResolver)','workspaceRootUri?.let { HostFacadeCapture.workspaceProject(workspaceJavaProjectResolver,it) }')
 text=text.replace('semanticModelProvider(', 'HostFacadeCapture.semantic(semanticModelProvider,')
 text=text.replace('atCodeValuesProvider(', 'HostFacadeCapture.atCodes(atCodeValuesProvider,')
 text=text.replace('featureProbe.injectionPointSpecifierSupported(', 'HostFacadeCapture.specifier(featureProbe,')
 text=text.replace('projectSourceQueryFactory(it)','HostFacadeCapture.sourceQuery(projectSourceQueryFactory,it)')
 text=text.replace('facadeFactory?.invoke(classIndex, bytecodeIndex)','HostFacadeCapture.factory(facadeFactory,classIndex,bytecodeIndex)')
 text=text.replace('completeOverride?.invoke(session, source, line, character, options)','HostFacadeCapture.completeOverride(completeOverride,session,source,line,character,options)')
 original='    private fun collectRegisteredMixinSources('
 wrapper='''    private fun collectRegisteredMixinSources(projectContext:ProjectContext,source:String,documentUri:String,javaProject:Any?):Sequence<String> =
        HostFacadeCapture.collect(projectContext,source,documentUri,javaProject) { capture_impl_collectRegisteredMixinSources(projectContext,source,documentUri,javaProject) }
    private fun capture_impl_collectRegisteredMixinSources('''
 text=text.replace(original,wrapper,1)
 text=text.replace('    private fun findCompilationUnitSource(','''    private fun findCompilationUnitSource(javaProject:Any,fqn:String,currentPath:Path?):CompilationUnitLookup? = capture_impl_findCompilationUnitSource(javaProject,fqn,currentPath).also {
        Capture.read("Host.unitSource",listOf(fqn,currentPath?.toString()),when(it){null->mapOf("status" to "missing");CompilationUnitLookup.Current->mapOf("status" to "current");is CompilationUnitLookup.Source->mapOf("status" to "source","text" to it.text)})
    }
    private fun capture_impl_findCompilationUnitSource(''',1)
 target=target.with_name('HostMixinServiceFacade.kt');target.write_text(text,encoding='utf8');return target

def main():
 inventory=json.loads((HERE/'original-case-inventory.json').read_text())
 extra='io.github.mcdev.core.mixinextras.CoreHandlerSignatureCompletionTest#facadeRoutesImmediatelyAfterAnnotationToHandlerSnippet()'
 if extra not in inventory:inventory.append(extra);(HERE/'original-case-inventory.json').write_text(json.dumps(inventory,indent=2),encoding='utf8')
 runtime=commands.runtime()
 sources=[instrument_core(),instrument_host(),instrument_definition(),expr.instrument(helper.MAIN/'mixin/MixinCodeActionService.kt',{'applyMixinConfigFix','applyMethodDescriptorFix'}),expr.instrument(helper.MAIN/'mixinextras/HandlerSignatureService.kt',{'findAnnotationSites','findSugarHandlerAnnotationSites'})]+[helper.instrument(helper.MAIN/path,names)for path,names in helper.APIS.items()]
 # Observe callback laziness/order without invoking any callback a second time.
 at=helper.MAIN/'mixin/AtValueCompletionService.kt';text=at.read_text(encoding='utf8')
 for name in ['injectionPointSpecifierSupported','additionalAtValues']:
  text=text.replace(name+'()',f'oracle.Capture.read("AtValue.{name}",emptyList<String>(),{name}())')
 output=helper.OUT/'src/AtValueCompletionService.kt';output.write_text(text,encoding='utf8');sources.append(output)
 source=(HERE.parent/'mixin_source/Capture.kt').read_text(encoding='utf8')
 source=source.replace('val gson = GsonBuilder()', 'val gson = GsonBuilder().registerTypeHierarchyAdapter(io.github.mcdev.core.codeaction.McFix::class.java,ServicesCapture.fixSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.AtShiftSpec::class.java,HandlerCapture.shiftSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec::class.java,HandlerCapture.sugarSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.bytecode.ConstantValue::class.java,HandlerCapture.constantSerializer)')
 source=source.replace('val row = mapOf("legacyCase"', 'val row = mapOf("sourceHint" to frames.get().mapNotNull { (it.args as? Map<*,*>)?.get("source") as? String ?: ((it.args as? Map<*,*>)?.get("request") as? io.github.mcdev.core.mixin.MixinFacadeRequest)?.bufferText }.firstOrNull(), "legacyCase"')
 source=source.replace('for (frame in frames.get()) frame.reads += mapOf(', 'for (frame in frames.get()) frame.reads += mapOf("origin" to (if(caseId.endsWith("filteredCodeActionExcludesOtherDiagnosticFixes")) Thread.currentThread().stackTrace.filter { it.className.startsWith("io.github.mcdev.core.") }.take(8).map { it.className.substringAfterLast(\'.\')+"."+it.methodName } else emptyList<String>()), ')
 output=helper.OUT/'src/Capture.kt';output.write_text(source,encoding='utf8');sources.extend([output,HERE/'FacadeCapture.kt',HERE/'HostFacadeCapture.kt',HERE/'Run_facade.kt',HERE.parent/'mixinextras_handler/HandlerCapture.kt',HERE.parent/'mixinextras_services/ServicesCapture.kt'])
 dest=helper.OUT/'classes';dest.mkdir(exist_ok=True)
 host_sources=[p for p in sources if p.name in ['HostMixinServiceFacade.kt','HostFacadeCapture.kt']]
 helper.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(helper.BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['compile_classpath'],'-d',str(dest)]+[str(p)for p in sources if p not in host_sources],'compile-facade',240)
 host_dest=helper.OUT/'host-classes';host_dest.mkdir(exist_ok=True)
 helper.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-jdtls-extension','-Xfriend-paths='+str(helper.BASE/'mcdev-jdtls-extension/build/classes/kotlin/main'),'-classpath',str(dest)+os.pathsep+runtime['compile_classpath'],'-d',str(host_dest)]+list(map(str,host_sources)),'compile-host-facade',240)
 calls=helper.OUT/'calls.jsonl';cases=helper.OUT/'cases.json'
 core=[case for case in inventory if case.startswith('io.github.mcdev.core.')]
 helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.Run_facadeKt',[str(calls),str(cases)]+core,'capture-facade',300)
 manifest=json.loads(cases.read_text());assert len(manifest)==159 and all(c['status']=='passed'for c in manifest)
 (HERE/'core-calls.jsonl').write_bytes(calls.read_bytes());(HERE/'core-cases.json').write_bytes(cases.read_bytes())
 print(json.dumps({'cases':len(manifest),'calls':sum(c['calls']for c in manifest)}),flush=True)
 host=[case for case in inventory if case.startswith('io.github.mcdev.jdtls.')]
 helper.run_java(runtime,str(host_dest)+os.pathsep+str(dest)+os.pathsep+runtime['classpath'],'oracle.Run_facadeKt',[str(calls),str(cases)]+host,'capture-host-facade',300)
 manifest=json.loads(cases.read_text());assert len(manifest)==30 and all(c['status']=='passed'for c in manifest)
 (HERE/'host-calls.jsonl').write_bytes(calls.read_bytes());(HERE/'host-cases.json').write_bytes(cases.read_bytes())
 print(json.dumps({'hostCases':len(manifest),'hostCalls':sum(c['calls']for c in manifest)}),flush=True)
if __name__=='__main__':main()
