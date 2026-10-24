"""Capture all original definition/reference assertions with the shared oracle."""
from pathlib import Path
import importlib.util,json,os,hashlib,re
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location("completion_capture",HERE.parent/"mixin_completion/generate_completion_capture.py")
completion=importlib.util.module_from_spec(spec);spec.loader.exec_module(completion)
capture=completion.capture;capture.OUT=ROOT/"build/navigation-oracle"
def main():
 runtime=capture.prepare_runtime();sources=[]
 for name,methods in {"MixinDefinitionService":{"definitionsAt","definitionsAtOffset"},"MixinReferenceService":{"findReferences"}}.items():
  path=completion.instrument(name,methods);text=path.read_text(encoding="utf-8")
  if name=="MixinDefinitionService":
   text=text.replace("fun parse(target: String): ParsedAtTarget?", "fun capture_impl_parse(target: String): ParsedAtTarget?",1)
   text=text.replace("internal object AtTargetParser {",'internal object AtTargetParser {\n    fun parse(target:String):ParsedAtTarget?=Capture.capture("AtTargetParser.parse",mapOf("target" to target)){capture_impl_parse(target)}',1)
  path.write_text(text,encoding="utf-8");sources.append(path)
 sources +=[capture.instrument(capture.MAIN/path,names)for path,names in capture.APIS.items()]
 sources +=[HERE.parent/"mixin_source/Capture.kt",HERE.parent/"mixin_completion/Run_completion.kt"]
 dest=capture.OUT/"classes";dest.mkdir(exist_ok=True)
 capture.run_java(runtime,runtime["compiler_classpath"],"org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",["-no-stdlib","-no-reflect","-module-name","mcdev-core","-jvm-target","21","-Xfriend-paths="+str(capture.BASE/"mcdev-core/build/classes/kotlin/main"),"-classpath",runtime["classpath"],"-d",str(dest),*map(str,sources)],"compile",180)
 classes=["MixinDefinitionServiceTest","MixinReferenceServiceTest"]
 specs=["io.github.mcdev.core.mixin."+name+"|mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/"+name+".kt"for name in classes]
 capture.run_java(runtime,str(dest)+os.pathsep+runtime["classpath"],"oracle.Run_completionKt",[str(capture.OUT/"calls.jsonl"),str(capture.OUT/"cases.json"),*specs],"capture",180)
 cases=json.loads((capture.OUT/"cases.json").read_text(encoding="utf-8"));assert len(cases)==69 and all(c["status"]=="passed"for c in cases)
 (HERE/"legacy_calls.jsonl").write_bytes((capture.OUT/"calls.jsonl").read_bytes());(HERE/"legacy_cases.json").write_text(json.dumps(cases,indent=2)+"\n",encoding="utf-8")
 tests=['#[path="fixtures/mixin_navigation/replay.rs"] mod replay;'];ledger=[]
 for case in cases:
  name=completion.rust_name(case);path,method=case["legacyCase"].split("::");cls=case["legacyClass"].split('.')[-1]
  tests.append('#[test]\nfn '+name+'(){replay::replay_case('+json.dumps(case["legacyCase"])+');}')
  ledger.append(dict(legacyTestPath=path,legacyClass=cls,legacyCase=method,baselineSha256=hashlib.sha256((capture.BASE/path).read_bytes()).hexdigest(),status="pending_native_replay",rustTest="mcdev-core/tests/mixin_navigation_legacy.rs::"+name))
 (ROOT/"mcdev-core/tests/mixin_navigation_legacy.rs").write_text("\n".join(tests),encoding="utf-8");(HERE/"legacy_coverage.json").write_text(json.dumps(ledger,indent=2)+"\n",encoding="utf-8")
 print("original navigation cases passed",len(cases))
if __name__=="__main__":main()
