"""Frozen 133-case service oracle. Native test execution never launches a JVM."""
from pathlib import Path
import importlib.util,json,os,re
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('handler_generator',HERE.parent/'mixinextras_handler/generate_capture.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
helper=base.helper;helper.OUT=ROOT/'build/extras-services-oracle'
def instrument(path,names):
 target=base.instrument(path,names);text=target.read_text(encoding='utf8')
 if path.stem.startswith('MixinExtras'):
  text=text.replace('private val classIndex: ClassIndex','classIndex: ClassIndex',1)
  start=text.index('{',text.index('class '+path.stem))+1
  text=text[:start]+'\n    private val classIndex=Capture.index(classIndex)!!\n'+text[start:]
  for name in names-{'deduplicateFixes'}:
   original=f'Capture.capture("{path.stem}.{name}", mapOf('
   text=text.replace(original,original+'"hasBytecodeIndex" to oracle.ServicesCapture.hasBytecode(signatureService), ',1)
 target.write_text(text,encoding='utf8');return target
def main():
 runtime=helper.prepare_runtime()
 apis={'MixinExtrasDiagnosticsService.kt':{'analyze'},'MixinExtrasCodeActionService.kt':{'fixesForDiagnostics','generateHandlerFixes','generateHandlerFixesAtPosition','completeHandler','applyHandlerFix','deduplicateFixes'},'HandlerSignatureService.kt':base.APIS['HandlerSignatureService.kt']}
 sources=[instrument(base.MAIN/name,names)for name,names in apis.items()]
 capture_source=(HERE.parent/'mixin_source/Capture.kt').read_text(encoding='utf8')
 capture_source=capture_source.replace('val gson = GsonBuilder()', 'val gson = GsonBuilder().registerTypeHierarchyAdapter(io.github.mcdev.core.codeaction.McFix::class.java,ServicesCapture.fixSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.AtShiftSpec::class.java,HandlerCapture.shiftSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec::class.java,HandlerCapture.sugarSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.bytecode.ConstantValue::class.java,HandlerCapture.constantSerializer)')
 capture=helper.OUT/'src/Capture.kt';capture.write_text(capture_source,encoding='utf8')
 sources.extend([capture,HERE/'ServicesCapture.kt',HERE/'ApplyParityTest.kt',HERE.parent/'mixinextras_handler/HandlerCapture.kt',HERE.parent/'mixin_source/Run.kt'])
 dest=helper.OUT/'classes';dest.mkdir(exist_ok=True)
 helper.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(base.BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['classpath'],'-d',str(dest)]+[str(p)for p in sources],'compile-services',240)
 calls=helper.OUT/'services-calls.jsonl';cases=helper.OUT/'services-cases.json'
 helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[str(calls),str(cases),'io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticsServiceTest','io.github.mcdev.core.mixinextras.MixinExtrasCodeActionServiceTest'],'capture-services',300)
 manifest=json.loads(cases.read_text());assert len(manifest)==133 and all(r['status']=='passed'for r in manifest)
 (HERE/'services-calls.jsonl').write_bytes(calls.read_bytes());(HERE/'services-cases.json').write_bytes(cases.read_bytes())
 print(json.dumps({'cases':len(manifest),'calls':sum(r['calls']for r in manifest),'zeroCalls':[r['legacyCase']for r in manifest if r['calls']==0]}),flush=True)
 callers=helper.OUT/'caller-calls.jsonl';caller_cases=helper.OUT/'caller-cases.json'
 helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[str(callers),str(caller_cases),'io.github.mcdev.core.mixinextras.CoreHandlerSignatureCompletionTest','oracle.ApplyParityTest|native-parity/ApplyParityTest.kt'],'capture-callers',240)
 assert all(r['status']=='passed'for r in json.loads(caller_cases.read_text()))
 (HERE/'caller-calls.jsonl').write_bytes(callers.read_bytes());(HERE/'caller-cases.json').write_bytes(caller_cases.read_bytes())
if __name__=='__main__':main()
