"""One-time frozen AW/AT oracle. The checked-in replay has no JVM dependency."""
from pathlib import Path
import importlib.util, json, os, re

HERE=Path(__file__).resolve().parent
SHARED=HERE.parent/'mixin_source'
spec=importlib.util.spec_from_file_location('source_capture',SHARED/'generate_capture.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
c.OUT=c.ROOT/'build/awat-oracle'
GROUPS={
 'aw':['AccessWidenerCodeActionService','AccessWidenerCompletionService','AccessWidenerDefinitionService','AccessWidenerDiagnosticsService','AccessWidenerEditor','AccessWidenerParser','AccessWidenerReferenceService','AwContextExtractor'],
 'at':['AccessTransformerCodeActionService','AccessTransformerCompletionService','AccessTransformerDefinitionService','AccessTransformerDiagnosticsService','AccessTransformerEditor','AccessTransformerParser','AccessTransformerReferenceService','AtContextExtractor','AtMemberInsertFormatter'],
 'awat':['AwAtBufferSupport','AwAtServiceFacade'],
}

def instrument(group,name):
    path=c.MAIN/group/('AwAtFileType.kt' if name=='AwAtBufferSupport' else name+'.kt')
    text=path.read_text(encoding='utf-8')
    text=text.replace('fun parseFileNamespace(source: String) =','fun parseFileNamespace(source: String): io.github.mcdev.core.model.MappingNamespace? =')
    replacements=[];wrappers=[]
    for m in re.finditer(r'(?m)^    fun (\w+)\s*\(',text):
        method=m.group(1);op=text.index('(',m.start());end=c.closing(text,op);params=text[op+1:end]
        at=end+1
        while text[at].isspace():at+=1
        assert text[at]==':',(name,method)
        retstart=at+1;at=retstart;depth=0
        while at<len(text):
            char=text[at]
            if char in '<(':depth+=1
            elif char in '>)':depth-=1
            if depth==0 and char in '={':break
            at+=1
        ret=text[retstart:at].strip();entries=[];args=[]
        for p in c.split_params(params):
            arg,ty=p.split(':',1);arg=arg.strip();entries.append(f'"{arg}" to AwAtCapture.arg({arg})')
            args.append('Capture.index('+arg+')'+('' if ty.split('=')[0].strip().endswith('?') else '!!') if re.match(r'\s*ClassIndex[? =]*$',ty.split('=')[0]) else 'AwAtCapture.context('+arg+')'+('' if ty.split('=')[0].strip().endswith('?') else '!!') if 'ProjectMappingContext' in ty else 'AwAtCapture.request('+arg+')' if 'DiagnosticRequest' in ty else arg)
        # Constructor context must be present on final facade captures as well.
        constructor=text[:text.index('    fun ')]
        if 'private val mappingContext:' in constructor:
            entries.append('"_mappingContext" to AwAtCapture.arg(this.mappingContext)')
        wrappers.append(f'    {text[m.start():end+1].strip()}: {ret} = Capture.capture("{name}.{method}",mapOf({",".join(entries)})) {{ capture_impl_{method}({",".join(args)}) }}\n')
        replacements.append((m.start(1),m.end(1),'capture_impl_'+method))
    for a,b,r in reversed(replacements):text=text[:a]+r+text[b:]
    decl=re.search(r'(?:class|object) '+name+r'\b',text);at=decl.end()
    while text[at].isspace():at+=1
    if text[at]=='(':at=c.closing(text,at)+1
    body=text.index('{',at)+1
    header=text[:body];extra=''
    for param,ty,wrap in [('classIndex','ClassIndex','Capture.index(classIndex)!!'),('mappingContext','ProjectMappingContext?','AwAtCapture.context(mappingContext)')]:
        token=f'private val {param}: {ty}'
        if token in header:
            header=header.replace(token,f'{param}: {ty}')
            extra+=f'    private val {param}: {ty} = {wrap}\n'
    text=header+'\n'+extra+''.join(wrappers)+text[body:]
    end=text.index('\n',text.index('package '));text=text[:end+1]+'\nimport oracle.Capture\nimport oracle.AwAtCapture\n'+text[end+1:]
    output=c.OUT/'src'/(name+'.kt');output.parent.mkdir(parents=True,exist_ok=True);output.write_text(text,encoding='utf-8');return output

def main():
    runtime=c.prepare_runtime();sources=[instrument(group,name) for group,names in GROUPS.items() for name in names]
    sources += [SHARED/'Capture.kt',SHARED/'Run.kt',HERE/'AwAtCapture.kt']
    dest=c.OUT/'classes';dest.mkdir(exist_ok=True)
    c.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(c.BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['classpath'],'-d',str(dest)]+list(map(str,sources)),'compile',180)
    tests=[]
    for group,names in GROUPS.items():
        for name in names:
            cls=name+('E2ETest' if name=='AwAtServiceFacade' else 'Test')
            tests.append('io.github.mcdev.core.'+group+'.'+cls)
    c.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[str(c.OUT/'calls.jsonl'),str(c.OUT/'cases.json')]+tests,'capture',180)
    cases=json.loads((c.OUT/'cases.json').read_text(encoding='utf-8'));assert len(cases)==155,len(cases)
    assert all(r['status']=='passed'and r['calls']>0 for r in cases)
    for name in ['calls.jsonl','cases.json']:(HERE/('legacy_'+name)).write_bytes((c.OUT/name).read_bytes())
    tests=['#[path="fixtures/awat/replay.rs"] mod replay;\n\n#[test]\nfn original_manifest_and_explicit_adaptations_are_complete() { replay::validate_manifest(); }\n']
    for row in cases:
        cls=row['legacyCase'].split('/')[-1].replace('.kt::','_');name=re.sub(r'(?<!^)(?=[A-Z])','_',cls).lower()
        tests.append(f'#[test]\nfn {name}() {{ replay::replay_case({json.dumps(row["legacyCase"])}); }}\n')
    (HERE.parents[1]/'awat_legacy.rs').write_text('\n'.join(tests),encoding='utf-8')
    print('original AWAT cases passed='+str(len(cases)),flush=True)

if __name__=='__main__':main()
