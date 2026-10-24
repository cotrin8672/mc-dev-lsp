"""One-time locals oracle using the shared source/engine capture runtime."""
from pathlib import Path
import importlib.util,json,os,re,hashlib
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location("completion_capture",HERE.parent/"mixin_completion/generate_completion_capture.py")
completion=importlib.util.module_from_spec(spec);spec.loader.exec_module(completion)
capture=completion.capture;capture.OUT=ROOT/"build/locals-oracle"
SERVICES={
 "LocalCaptureExtractor":{"extract"},"LocalCaptureTypeResolver":{"resolveParameter","resolveDefinition"},
 "LocalCaptureValidationService":{"validate"},"LocalDiscriminatorResolver":{"resolve"},
 "LocalCompletionService":{"complete"},"ShareCompletionService":{"complete"},
 "MixinExtrasCompletionService":{"complete","completeAtTargetForWrapOperation"},
}
TESTS=["LocalCaptureExtractorTest","LocalCaptureTypeResolverTest","LocalCaptureValidationServiceTest","LocalDiscriminatorResolverTest","LocalCompletionServiceTest","MixinExtrasCompletionServiceTest","LocalCaptureRuntimeSemanticsTest"]
SHARE=["routesShareIdCompletionAcrossMatchingHandlersAndExplicitNamespaces","shareIdCompletionIncludesMatchingIdsFromAdditionalSources","routesShareIdCompletionWithJavaEscapedValues","shareSourceProviderIsNotCalledForNonShareCompletion"]

def main():
 runtime=capture.prepare_runtime();sources=[]
 for name,methods in SERVICES.items():
  path=completion.instrument(name,methods,"mixinextras");text=path.read_text(encoding="utf-8");text=text.replace("import oracle.Capture","import oracle.Capture\nimport oracle.HandlerCapture\nimport oracle.LocalOracle")
  if name=="LocalCaptureExtractor":text=text.replace("instructionOccurrenceIndices, cancellationChecker)","instructionOccurrenceIndices, LocalOracle.checker(cancellationChecker))")
  # Observe the sequence when the original service consumes it. Capturing must
  # not drain lazy additional sources before the original early-return guards.
  text=text.replace("Capture.arg(additionalSources)","emptyList<String>()")
  text=text.replace("capture_impl_complete(source, context, additionalSources)","capture_impl_complete(source, context, LocalOracle.sources(additionalSources))")
  if "private val bytecodeIndex: BytecodeIndex" in text:
   text=text.replace("private val bytecodeIndex: BytecodeIndex","bytecodeIndex: BytecodeIndex",1)
   start=text.index("(",text.index("class "+name));body=text.index("{",capture.closing(text,start)+1)+1
   text=text[:body]+"\n    private val bytecodeIndex = HandlerCapture.bytecode(bytecodeIndex)!!\n"+text[body:]
  path.write_text(text,encoding="utf-8");sources.append(path)
 sources +=[capture.instrument(capture.MAIN/path,names)for path,names in capture.APIS.items()]
 sources +=[completion.instrument(name,methods)for name,methods in {"InjectMethodCompletionService":{"complete"},"AtTargetCompletionService":{"complete","formatTarget"}}.items()]
 # Engine capture serializers and callback wrappers are reused, not a new oracle framework.
 engine_base=ROOT/"build/expression-oracle/Capture.kt"
 assert engine_base.exists(),"Run the shared expression capture generator once to prepare its serializers"
 text=engine_base.read_text(encoding="utf-8");text=text.replace("val gson = GsonBuilder()","val gson = GsonBuilder().registerTypeHierarchyAdapter(AtShiftSpec::class.java,HandlerCapture.shiftSerializer).registerTypeHierarchyAdapter(HandlerParameterSugarSpec::class.java,HandlerCapture.sugarSerializer).registerTypeHierarchyAdapter(LocalCaptureValidationService.Result::class.java,LocalOracle.validationSerializer).registerTypeHierarchyAdapter(LocalDiscriminatorResolution::class.java,LocalOracle.discriminatorSerializer).registerTypeHierarchyAdapter(LocalCaptureError::class.java,LocalOracle.errorSerializer).registerTypeHierarchyAdapter(LocalCaptureResult::class.java,LocalOracle.resultSerializer)")
 dest_capture=capture.OUT/"Capture.kt";dest_capture.write_text(text,encoding="utf-8");sources.append(dest_capture)
 for name,methods in {"OfficialExpressionMatcher":{"match"},"OfficialExpressionIdentifierPoolBuilder":{"build"}}.items():
  path=capture.instrument(capture.MAIN/"mixinextras"/(name+".kt"),methods);text=path.read_text(encoding="utf-8").replace("commonSuperClass, cancellationChecker)","Capture.resolver(commonSuperClass), LocalOracle.checker(cancellationChecker))").replace("capture_impl_build(index, typeNameResolver)","capture_impl_build(index, Capture.typeResolver(typeNameResolver))");text=text.replace("import oracle.Capture","import oracle.Capture\nimport oracle.LocalOracle");path.write_text(text,encoding="utf-8");sources.append(path)
 sources +=[HERE.parent/"mixinextras_handler/HandlerCapture.kt",HERE/"LocalOracle.kt",HERE.parent/"mixin_completion/Run_completion.kt"]
 dest=capture.OUT/"classes";dest.mkdir(exist_ok=True)
 capture.run_java(runtime,runtime["compiler_classpath"],"org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",["-no-stdlib","-no-reflect","-module-name","mcdev-core","-jvm-target","21","-Xfriend-paths="+str(capture.BASE/"mcdev-core/build/classes/kotlin/main"),"-classpath",runtime["classpath"],"-d",str(dest),*map(str,sources)],"compile",180)
 cases=["io.github.mcdev.core.mixinextras."+name+"|mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/"+name+".kt"for name in TESTS]
 cases +=["io.github.mcdev.core.mixin.e2e.MixinServiceFacadeRoutingTest|mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/e2e/MixinServiceFacadeRoutingTest.kt|"+",".join(SHARE)]
 capture.run_java(runtime,str(dest)+os.pathsep+runtime["classpath"],"oracle.Run_completionKt",[str(capture.OUT/"calls.jsonl"),str(capture.OUT/"cases.json"),*cases],"capture",180)
 rows=json.loads((capture.OUT/"cases.json").read_text(encoding="utf-8"));assert len(rows)==83;assert all(r["status"]=="passed"for r in rows)
 (HERE/"legacy_calls.jsonl").write_text((capture.OUT/"calls.jsonl").read_text(encoding="utf-8"));(HERE/"legacy_cases.json").write_text(json.dumps(rows,indent=2)+"\n")
 tests=['#[path="fixtures/mixinextras_locals/replay.rs"] mod legacy_replay;']
 ledger=[]
 for row in rows:
  cls=row["legacyClass"].split('.')[-1];method=row["legacyCase"].split("::")[-1];path=row["legacyCase"].split("::")[0];name=completion.rust_name(row)
  direct=cls!="MixinServiceFacadeRoutingTest"
  if direct:tests.append('#[test]\nfn '+name+'(){legacy_replay::replay_case('+json.dumps(row['legacyCase'])+');}')
  ledger.append(dict(legacyTestPath=path,legacyClass=cls,legacyCase=method,baselineSha256=hashlib.sha256((capture.BASE/path).read_bytes()).hexdigest(),rustTest="mcdev-core/tests/mixinextras_locals_legacy.rs::"+name if direct else None,status="pending_native_replay"if direct else "pending_caller_integration",calleeEvidenceCaptured=row["calls"]>0))
 (ROOT/"mcdev-core/tests/mixinextras_locals_legacy.rs").write_text("\n".join(tests));(HERE/"legacy_coverage.json").write_text(json.dumps(ledger,indent=2)+"\n")
 print("original tests passed",len(rows),"direct",sum(r["legacyClass"].endswith(tuple(TESTS))for r in rows))
if __name__=="__main__":main()
