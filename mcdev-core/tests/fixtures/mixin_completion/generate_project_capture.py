"""Exact file-backed project oracle; all writes are disposable test output."""
from pathlib import Path
import importlib.util
import json
import os
import re

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location("completion_capture",HERE/"generate_completion_capture.py")
completion=importlib.util.module_from_spec(spec);spec.loader.exec_module(completion)
capture=completion.capture
capture.OUT=capture.ROOT/"build/project-oracle"
SERVICES={
    "AwAtDiscoveryService": {"discoverAccessWideners","discoverAccessTransformers","isAccessWidenerFile","isAccessTransformerFile","parseAccessWidener","parseAccessTransformer"},
    "InfoService":{"formatLines","format"},
    "MappingDiscoveryService":{"discoverMappingFiles","discoverMappingContext","buildMappingContext","parseMappingFile"},
    "MixinConfigDiscoveryService":{"discover","isMixinConfigFile","parse","parseContent","normalizeJsonContent","readContent","sharedConfigs","configsForSourceSet","selectForMixin","isMixinListed"},
    "PlatformDetector":{"detect","detectFromRoot"},
    "ProjectContextBuilder":{"build","empty"},
    "ProjectTreeWalker":{"walkRegularFiles","walkMappingDiscoveryRegularFiles"},
    "ProjectPathFilters":{"isExcludedDirectorySegment","isUnderExcludedDirectory","isExcludedFromMappingDiscovery","shouldPruneMappingDiscoveryDirectory"},
    "SourceSetResolver":{"containingSourceSet"},
}
TESTS=["AwAtDiscoveryServiceTest","InfoServiceTest","MappingDiscoveryServiceTest","MixinConfigDiscoveryServiceTest","PlatformDetectorTest","ProjectContextBuilderTest","ProjectModelTest","ProjectTreeWalkerTest","SourceSetResolverTest"]

def normalize(value,root):
    if isinstance(value,str):return value.replace(root,"$ROOT").replace("\\","/") if root in value else value
    if isinstance(value,list):return [normalize(v,root)for v in value]
    if isinstance(value,dict):return {normalize(k,root):normalize(v,root)for k,v in value.items()}
    return value

def main():
    runtime=capture.prepare_runtime();sources=[]
    for name,methods in SERVICES.items():
        path=completion.instrument(name,methods,"project")
        text=path.read_text(encoding="utf-8").replace("import oracle.Capture","import oracle.Capture\nimport oracle.ProjectCapture").replace("= Capture.capture(","= ProjectCapture.capture(")
        path.write_text(text,encoding="utf-8");sources.append(path)
    sources +=[completion.SHARED/"Capture.kt",HERE/"ProjectCapture.kt",HERE/"Run_project.kt"]
    destination=capture.OUT/"classes";destination.mkdir(exist_ok=True)
    capture.run_java(runtime,runtime["compiler_classpath"],"org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",[
        "-no-stdlib","-no-reflect","-module-name","mcdev-core","-jvm-target","21","-Xfriend-paths="+str(capture.BASE/"mcdev-core/build/classes/kotlin/main"),"-classpath",runtime["classpath"],"-d",str(destination),*map(str,sources),
    ],"compile",180)
    capture.run_java(runtime,str(destination)+os.pathsep+runtime["classpath"],"oracle.Run_projectKt",[str(capture.OUT/"calls.jsonl"),str(capture.OUT/"cases.json"),*["io.github.mcdev.core.project."+n for n in TESTS]],"capture",180)
    cases=json.loads((capture.OUT/"cases.json").read_text(encoding="utf-8"));assert len(cases)==106
    assert all(c["status"]=="passed" for c in cases)
    rows=[]
    for line in (capture.OUT/"calls.jsonl").read_text(encoding="utf-8").splitlines():
        row=json.loads(line);rows.append(normalize(row,row["args"]["__root"]))
    (HERE/"project_legacy_calls.jsonl").write_text("\n".join(json.dumps(r,ensure_ascii=True)for r in rows)+"\n",encoding="utf-8")
    (HERE/"project_legacy_cases.json").write_text(json.dumps(cases,indent=2)+"\n",encoding="utf-8")
    print("legacy project cases",len(cases),"captured events",len(rows))
    write_tests(cases)

def write_tests(cases):
    tests=['#[path="fixtures/mixin_completion/project_replay.rs"] mod legacy_replay;']
    for case in cases:
        if not case["calls"]:continue
        name=completion.rust_name(case)
        tests.append('#[test]\nfn '+name+'(){legacy_replay::replay_case('+json.dumps(case['legacyCase'])+');}')
    (HERE.parents[1]/"project_legacy.rs").write_text("\n".join(tests),encoding="utf-8")

if __name__=="__main__":main()
