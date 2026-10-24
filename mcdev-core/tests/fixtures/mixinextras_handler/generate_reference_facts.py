"""One-time frozen-VM host hierarchy facts; native Rust tests require no JVM."""
from pathlib import Path
import importlib.util,json,re,os,sys
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
shared=HERE.parent/'mixin_source/generate_capture.py'
spec=importlib.util.spec_from_file_location('reference_oracle',shared)
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
helper.OUT=ROOT/'build/handler-reference-oracle'
names=set()
def visit(value):
    if isinstance(value,str):
        names.update(re.findall(r'L([A-Za-z0-9_$/]+);',value))
    elif isinstance(value,dict):
        for key in ['typeName','returnTypeName']:
            if isinstance(value.get(key),str):
                name=value[key].strip()
                while name.endswith('[]'):name=name[:-2].strip()
                name=name.split('<')[0].strip()
                if '.'in name:names.add(name.replace('.','/'))
                elif name:names.update([name,'java/lang/'+name])
        for entry in value.values():visit(entry)
    elif isinstance(value,list):
        for entry in value:visit(entry)
capture=Path(sys.argv[1])if len(sys.argv)>1 else HERE/'handler-calls.jsonl'
for line in capture.read_text(encoding='utf8').splitlines():
    row=json.loads(line)
    if row['api'].startswith('HandlerSignatureService.'):
        visit(row['args'])
        visit(row['output'])
runtime=helper.prepare_runtime()
source=HERE/'ReferenceFacts.kt'
dest=helper.OUT/'classes';dest.mkdir(parents=True,exist_ok=True)
input_file=helper.OUT/'reference-names.json';input_file.write_text(json.dumps(sorted(names)),encoding='utf8')
helper.run_java(runtime,runtime['classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-classpath',runtime['classpath'],'-d',str(dest),str(source)],'compile-reference',180)
output=Path(sys.argv[2])if len(sys.argv)>2 else HERE/'host-reference-facts.json'
helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.ReferenceFactsKt',[str(input_file),str(output)],'capture-reference',120)
facts=json.loads(output.read_text());print(json.dumps({'known':len(facts['known']),'assignable':len(facts['assignable'])}),flush=True)
