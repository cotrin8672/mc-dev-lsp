"""Reuse the existing locals oracle classes for branch/debug-frame regressions."""
import generate_capture as local
import json,os
from pathlib import Path
capture=local.capture
capture.OUT=local.ROOT/"build/locals-additional-oracle";capture.OUT.mkdir(parents=True,exist_ok=True)
runtime=capture.prepare_runtime();dest=capture.OUT/"classes";dest.mkdir(exist_ok=True)
for mode,flag in [("debug","-g"),("no-debug","-g:none")]:
 output=capture.OUT/mode;output.mkdir(exist_ok=True)
 source=local.HERE/"LocalFlowSamples.java";compiled=output/"LocalFlowSamples.class"
 if not compiled.exists()or compiled.stat().st_mtime<source.stat().st_mtime:
  capture.run_java(runtime,runtime["compiler_classpath"],"com.sun.tools.javac.Main",["-proc:none",flag,"-encoding","UTF-8","-d",str(output),str(source)],"javac-"+mode,60)
base=local.ROOT/"build/locals-oracle/classes"
classpath=str(base)+os.pathsep+runtime["classpath"]
capture.run_java(runtime,runtime["compiler_classpath"],"org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",["-no-stdlib","-no-reflect","-module-name","mcdev-core","-jvm-target","21","-classpath",classpath,"-d",str(dest),str(local.HERE/"LocalFlowOracle.kt")],"compile",120)
capture.run_java(runtime,str(dest)+os.pathsep+classpath,"oracle.Run_completionKt",[str(capture.OUT/"calls.jsonl"),str(capture.OUT/"cases.json"),"io.github.mcdev.core.mixinextras.LocalFlowOracle|mcdev-core/tests/fixtures/mixinextras_locals/LocalFlowOracle.kt"],"capture",120)
cases=json.loads((capture.OUT/"cases.json").read_text());assert len(cases)==2 and all(c["status"]=="passed"for c in cases)
(local.HERE/"additional_calls.jsonl").write_bytes((capture.OUT/"calls.jsonl").read_bytes());(local.HERE/"additional_cases.json").write_text(json.dumps(cases,indent=2)+"\n",encoding="utf-8")
print("additional frame oracle cases",len(cases))
