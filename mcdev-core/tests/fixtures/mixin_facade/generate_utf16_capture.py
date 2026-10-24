"""Additional official JVM oracle. JVM strings cross JSON only as numeric units."""
from pathlib import Path
import importlib.util,json,os
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('facade_generator',HERE/'generate_capture.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
helper=base.helper;helper.OUT=base.ROOT/'build/facade-utf16-oracle'
runtime=helper.prepare_runtime();sources=[HERE/'Utf16FacadeOracle.kt',HERE/'Run_facade.kt',HERE.parent/'mixin_source/Capture.kt',HERE.parent/'mixinextras_handler/HandlerCapture.kt']
dest=helper.OUT/'classes';dest.mkdir(exist_ok=True)
helper.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(helper.BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['classpath'],'-d',str(dest),*map(str,sources)],'compile-utf16',180)
calls=helper.OUT/'calls.jsonl';cases=helper.OUT/'cases.json'
helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.Run_facadeKt',[str(calls),str(cases),'oracle.Utf16FacadeOracle#losslessSourceThroughPublicFacade()'],'capture-utf16',180)
manifest=json.loads(cases.read_text());assert all(r['status']=='passed'for r in manifest)
(HERE/'utf16-calls.jsonl').write_bytes(calls.read_bytes());(HERE/'utf16-cases.json').write_bytes(cases.read_bytes());print('official UTF16 top facade outputs',sum(r['calls']for r in manifest))
