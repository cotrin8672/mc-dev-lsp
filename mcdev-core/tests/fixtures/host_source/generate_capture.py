"""One-time frozen JDT assertions and normalized adapter oracle. Never a test dependency."""
from pathlib import Path
import importlib.util,json,os,re
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('commands_capture',ROOT/'mcdev-companion/tests/fixtures/commands/generate_capture.py')
commands=importlib.util.module_from_spec(spec);spec.loader.exec_module(commands)
c=commands.c;c.OUT=ROOT/'build/host-source-oracle'
NAMES=['CompletionIndexCaches','JdtAnnotationConstantExtractor','JdtBindingDescriptorConverter','JdtInjectionPointAtCodeIndex','JdtInjectionPointFeatureProbe','JdtMixinExtrasResolvedContextExtractor','JdtMixinSemanticModelParser','JdtProjectSourceQuery','JdtTypeSignatureConverter','SemanticModelCache','SourceBackedClassIndex']
TEST=c.BASE/'mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/mixin'
MAIN=c.BASE/'mcdev-jdtls-extension/src/main/kotlin/io/github/mcdev/jdtls/mixin'
APIS={
    'JdtAnnotationConstantExtractor':{'constantString','constantBoolean','constantInt','stringArray','typeLiteralDescriptor','typeLiteralSourceName','typeArray','typeArraySourceNames','nestedAnnotationArray'},
    'JdtBindingDescriptorConverter':{'descriptorFromBinding'},
    'JdtTypeSignatureConverter':{'toJvmDescriptorOrNull'},
    'JdtMixinExtrasResolvedContextExtractor':{'extract'},
}

def instrument(owner,names):
    text=(MAIN/(owner+'.kt')).read_text(encoding='utf8');changes=[]
    for match in re.finditer(r'(?m)^(    |        )((?:(?:internal|private|override) )?fun) (\w+)\s*\(',text):
        indent,qualifier,name=match[1],match[2],match[3]
        if name not in names:continue
        op=text.index('(',match.start());end=c.closing(text,op);params=text[op+1:end];after=end+1
        while text[after].isspace():after+=1
        assert text[after]==':',(owner,name,'explicit return type')
        ret_start=after+1;after=ret_start;depth=0
        while after<len(text):
            ch=text[after]
            if ch in '<(':depth+=1
            elif ch in '>)':depth-=1
            if depth==0 and ch in '={':break
            after+=1
        ret=text[ret_start:after].strip();args=[];entries=[]
        for param in c.split_params(params):
            name_arg=param.split(':',1)[0].strip();args.append(name_arg);entries.append('"'+name_arg+'" to HostCapture.arg('+name_arg+')')
        if owner=='JdtProjectSourceQuery':entries.append('"_instanceState" to HostCapture.arg(this)')
        signature=text[match.start():end+1].strip()
        wrapper=f'{indent}{signature}: {ret} = Capture.capture("{owner}.{name}",mapOf<String,Any?>({",".join(entries)})) {{ capture_impl_{name}({",".join(args)}) }}\n'
        renamed=text[match.start():match.start(3)].replace('override ','')+'capture_impl_'+name
        changes.append((match.start(),match.end(3),wrapper+renamed))
    for start,end,new in reversed(changes):text=text[:start]+new+text[end:]
    package=text.index('\n',text.index('package '));text=text[:package+1]+'\nimport oracle.Capture\nimport oracle.HostCapture\n'+text[package+1:]
    out=c.OUT/'src'/(owner+'.kt');out.parent.mkdir(parents=True,exist_ok=True);out.write_text(text,encoding='utf8');return out

def capture_adapters(runtime,apis=None,prefix='adapter',test_names=None):
    apis=APIS if apis is None else apis
    (c.OUT/'src').mkdir(parents=True,exist_ok=True)
    recorder=(ROOT/'mcdev-core/tests/fixtures/mixin_source/Capture.kt').read_text(encoding='utf8')
    recorder=recorder.replace('"outputType" to value?.let { it::class.java.name }, "output" to value,','"frameDepth" to frames.get().size, "outputType" to value?.let { it::class.java.name }, "output" to HostCapture.arg(value),')
    path=c.OUT/'src/Capture.kt';path.write_text(recorder,encoding='utf8')
    sources=[instrument(owner,names)for owner,names in apis.items()]+[path,HERE/'HostCapture.kt',ROOT/'mcdev-companion/tests/fixtures/commands/Run_commands.kt']
    destination=c.OUT/(prefix+'-classes');destination.mkdir(exist_ok=True)
    c.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',[
        '-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-jdtls-extension',
        '-Xfriend-paths='+str(c.BASE/'mcdev-jdtls-extension/build/classes/kotlin/main'),
        '-classpath',runtime['compile_classpath'],'-d',str(destination)]+list(map(str,sources)),'compile-'+prefix,180)
    classes=[name+'Test' for name in apis] if test_names is None else test_names
    c.run_java(runtime,str(destination)+os.pathsep+runtime['classpath'],'oracle.Run_commandsKt',[
        str(c.OUT/(prefix+'-calls.jsonl')),str(c.OUT/(prefix+'-cases.json'))]+['io.github.mcdev.jdtls.mixin.'+name+'|'+str((TEST/(name+'.kt')).relative_to(c.BASE)).replace('\\','/') for name in classes],'capture-'+prefix,180)
    cases=json.loads((c.OUT/(prefix+'-cases.json')).read_text());assert all(r['status']=='passed'for r in cases)
    for suffix in ['-calls.jsonl','-cases.json']:(HERE/(prefix+suffix)).write_bytes((c.OUT/(prefix+suffix)).read_bytes())
    print(json.dumps({'adapterCases':len(cases),'calls':sum(r['calls']for r in cases)}),flush=True)

def original_cases():
    result=[]
    for name in NAMES:
        path=TEST/(name+'Test.kt');text=path.read_text(encoding='utf8')
        for match in re.finditer(r'@Test\s+fun\s+(\w+)\s*\(',text):
            start=text.index('{',match.end());end=commands.closing_brace(text,start)
            result.append({'legacyCase':str(path.relative_to(c.BASE)).replace('\\','/')+'::'+match[1],
                'legacyClass':'io.github.mcdev.jdtls.mixin.'+name+'Test',
                'originalCaseBody':text[match.start():end+1],
                'originalAssertionCount':len(re.findall(r'\bassert\w*\s*\(',text[start:end+1])),
                'status':'pending-native',
            })
    assert len(result)==170,len(result)
    (HERE/'coverage.json').write_text(json.dumps({'baseline':'current-20261023','legacyCases':170,'wireIntegration':'pending','cases':result},ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    return result

def main():
    cases=original_cases();runtime=commands.runtime()
    # Start with original assertion execution. No API is considered migrated by
    # this alone; normalized inputs and native public outputs are tracked separately.
    destination=c.OUT/'classes';destination.mkdir(exist_ok=True)
    sources=[ROOT/'mcdev-core/tests/fixtures/mixin_source/Capture.kt',ROOT/'mcdev-companion/tests/fixtures/commands/Run_commands.kt']
    c.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',[
        '-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-jdtls-extension',
        '-Xfriend-paths='+str(c.BASE/'mcdev-jdtls-extension/build/classes/kotlin/main'),
        '-classpath',runtime['compile_classpath'],'-d',str(destination)]+list(map(str,sources)),'compile-original',180)
    classes={row['legacyClass']:row['legacyCase'].split('::')[0] for row in cases}
    previous=Path.cwd()
    try:
        os.chdir(c.BASE)
        c.run_java(runtime,str(destination)+os.pathsep+runtime['classpath'],'oracle.Run_commandsKt',[
            str(c.OUT/'original-calls.jsonl'),str(c.OUT/'original-cases.json')]+[cls+'|'+path for cls,path in classes.items()],'original-assertions',240)
    finally:os.chdir(previous)
    captured=json.loads((c.OUT/'original-cases.json').read_text());assert len(captured)==170
    failed=[r for r in captured if r['status']!='passed'];assert not failed,failed
    (HERE/'original-cases.json').write_bytes((c.OUT/'original-cases.json').read_bytes())
    print(json.dumps({'originalCases':170,'assertions':sum(r['originalAssertionCount']for r in cases),'status':'passed'}),flush=True)
    capture_adapters(runtime)
if __name__=='__main__':main()
