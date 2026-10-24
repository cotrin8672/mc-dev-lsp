"""One-time frozen caller oracle, using shared Capture/Run/runtime helpers.

Compile-only dependencies remain off the runtime classpath so reflection fallback
does not change. All generated Kotlin/classes/logs are disposable build output.
"""
from pathlib import Path
import datetime,importlib.util,json,os,re,shutil
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
SHARED=ROOT/'mcdev-core/tests/fixtures/mixin_source'
spec=importlib.util.spec_from_file_location('shared_source_capture',SHARED/'generate_capture.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
c.OUT=ROOT/'build/commands-oracle'
MAIN=c.BASE/'mcdev-jdtls-extension/src/main/kotlin/io/github/mcdev/jdtls'
CALL=re.compile(r'\b(projectService|mixinFacade|awAtFacade|definitionResolver|semanticCache|documentCache|McdevBuildInfo)\.(\w+)\(')

def closing_brace(text,start):
    depth=0;i=start
    while i<len(text):
        literal=c.skip_literal(text,i)
        if literal is not None:i=literal;continue
        if text[i]=='{':depth+=1
        elif text[i]=='}':
            depth-=1
            if depth==0:return i
        i+=1
    raise ValueError('unclosed body')

def rewrite_calls(text):
    result=[];at=0
    while match:=CALL.search(text,at):
        result.append(text[at:match.start()]);opening=text.index('(',match.start());end=c.closing(text,opening)
        parameters=c.split_params(text[opening+1:end]);variables=[];arguments=[];entries=[]
        for n,parameter in enumerate(parameters):
            parameter=parameter.strip();named=re.match(r'^(\w+)\s*=\s*(?!=)(.*)$',parameter,re.S)
            key=named.group(1) if named else 'arg'+str(n);expression=named.group(2) if named else parameter
            variable='commandCaptureArg'+str(n);variables.append('val '+variable+' = '+rewrite_calls(expression)+';')
            arguments.append((key+' = ' if named else '')+variable);entries.append('"'+key+'" to CommandCapture.arg('+variable+')')
        trailing='';after=end+1
        while after<len(text)and text[after].isspace():after+=1
        if after<len(text)and text[after]=='{':
            finish=closing_brace(text,after);trailing=' '+rewrite_calls(text[after:finish+1]);end=finish
        api=match.group(1)+'.'+match.group(2)
        result.append('run { '+''.join(variables)+' CommandCapture.read("'+api+'", mapOf<String,Any?>('+','.join(entries)+')) { '+api+'('+','.join(arguments)+')'+trailing+' } }')
        at=end+1
    return ''.join(result)+text[at:]

def instrument(relative):
    path=MAIN/relative;text=path.read_text(encoding='utf-8');name=path.stem
    text=rewrite_calls(text)
    text=re.sub(r'(?<![\w.])currentTimeMillis\(\)',lambda _: 'CommandCapture.read("clock.now",emptyMap<String,Any?>()) { currentTimeMillis() }',text)
    replacements=[];wrappers=[]
    for m in re.finditer(r'(?m)^    (?:(?:internal|override) )?fun (\w+)\s*\(',text):
        method=m.group(1);opening=text.index('(',m.start());end=c.closing(text,opening);parameters=text[opening+1:end];at=end+1
        while text[at].isspace():at+=1
        if text[at]==':':
            start=at+1;at=start;depth=0
            while at<len(text):
                char=text[at]
                if char in '<(':depth+=1
                elif char in '>)':depth-=1
                if depth==0 and char in '={':break
                at+=1
            result=text[start:at].strip()
        else:
            assert text[at]=='{',(path,method,'implicit expression result requires explicit configuration')
            result='Unit'
        names=[p.split(':',1)[0].strip() for p in c.split_params(parameters)]
        signature=text[m.start():end+1].strip()
        entries=['"'+n+'" to CommandCapture.arg('+n+')'for n in names]+['"_instance" to System.identityHashCode(this)']
        wrappers.append('    '+signature+': '+result+' = Capture.capture("'+name+'.'+method+'",mapOf<String,Any?>('+','.join(entries)+')) { capture_impl_'+method+'('+','.join(names)+') }\n')
        replacement=text[m.start():m.start(1)].replace('override ','')+'capture_impl_'+method
        replacements.append((m.start(),m.end(1),replacement))
    for start,end,replacement in reversed(replacements):text=text[:start]+replacement+text[end:]
    declaration=re.search(r'(?:class|object) '+name+r'\b',text);at=declaration.end()
    while text[at].isspace():at+=1
    if text[at]=='(':at=c.closing(text,at)+1
    body=text.index('{',at)+1;text=text[:body]+'\n'+''.join(wrappers)+text[body:]
    package=text.index('\n',text.index('package '));text=text[:package+1]+'\nimport oracle.Capture\nimport oracle.CommandCapture\n'+text[package+1:]
    output=c.OUT/'src'/relative;output.parent.mkdir(parents=True,exist_ok=True);output.write_text(text,encoding='utf-8');return output

def runtime():
    r=c.prepare_runtime();paths=[]
    for module in ['mcdev-protocol','mcdev-jdtls-extension']:
        for scope in ['main','test']:
            for base in ['classes/kotlin','resources']:paths.append(str(c.BASE/module/'build'/base/scope))
    paths.append(str(c.BASE/'mcdev-jdtls-extension/build/classes/java/eclipseStub'))
    r['classpath']=os.pathsep.join(paths+[r['classpath']])
    compile_only=[]
    for group,artifact,version in [('org.osgi','org.osgi.core','6.0.0'),('org.eclipse.lsp4j','org.eclipse.lsp4j','0.23.1'),('org.eclipse.lsp4j','org.eclipse.lsp4j.jsonrpc','0.23.1')]:
        directory=c.CACHE/group/artifact/version
        compile_only += [str(p)for p in directory.rglob('*.jar')if '-sources'not in p.name and '-javadoc'not in p.name]
    r['compile_classpath']=os.pathsep.join([r['classpath']]+compile_only)
    (c.OUT/'runtime.json').write_text(json.dumps(r,indent=2),encoding='utf-8');return r

def main():
    if (c.OUT/'capture.stdout.log').exists():
        history=c.OUT/'history'/datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f');history.mkdir(parents=True)
        for pattern in ['*.log','cases.json','calls.jsonl','runtime.json']:
            for path in c.OUT.glob(pattern):shutil.copy2(path,history/path.name)
    r=runtime();relative=[Path('handler')/p.name for p in (MAIN/'handler').glob('Mcdev*Handler.kt')]
    relative += [Path('command/McdevCommandDispatcher.kt'),Path('McdevDelegateCommandHandler.kt'),Path('transport/McdevCompletionTransport.kt')]
    recorder=(SHARED/'Capture.kt').read_text(encoding='utf-8')
    recorder=recorder.replace('"outputType" to value?.let { it::class.java.name }, "output" to value,','"frameDepth" to frames.get().size, "outputType" to value?.let { it::class.java.name }, "output" to value,')
    recorder=recorder.replace('"api" to api, "args" to args, "throwable" to error.javaClass.name','"api" to api, "args" to args, "frameDepth" to frames.get().size, "throwable" to error.javaClass.name')
    recorder_path=c.OUT/'Capture.kt';recorder_path.write_text(recorder,encoding='utf-8')
    sources=[instrument(path)for path in relative]+[recorder_path,HERE/'CommandCapture.kt',HERE/'Run_commands.kt']
    # Record authoritative index reads inside converters, without changing the session DTO/API.
    session=(MAIN/'project/McdevProjectSession.kt').read_text(encoding='utf-8').replace('classIndex = classIndex,','classIndex = oracle.Capture.index(classIndex)!!,')
    path=c.OUT/'src/McdevProjectSession.kt';path.write_text(session,encoding='utf-8');sources.append(path)
    destination=c.OUT/'classes';destination.mkdir(exist_ok=True)
    c.run_java(r,r['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-jdtls-extension','-Xfriend-paths='+str(c.BASE/'mcdev-jdtls-extension/build/classes/kotlin/main'),'-classpath',r['compile_classpath'],'-d',str(destination)]+list(map(str,sources)),'compile',180)
    cases=json.loads((HERE/'original_cases.json').read_text());classes={row['legacyClass']:row['legacyCase'].split('::')[0] for row in cases}
    previous=Path.cwd()
    try:
        os.chdir(c.BASE)
        c.run_java(r,str(destination)+os.pathsep+r['classpath'],'oracle.Run_commandsKt',[str(c.OUT/'calls.jsonl'),str(c.OUT/'cases.json')]+[cls+'|'+path for cls,path in classes.items()],'capture',180)
    finally:os.chdir(previous)
    captured=json.loads((c.OUT/'cases.json').read_text(encoding='utf-8'));assert len(captured)==127
    assert {r['legacyCase']for r in captured}=={r['legacyCase']for r in cases}
    assert all(r['status']=='passed' for r in captured)
    for name in ['calls.jsonl','cases.json']:(HERE/('legacy_'+name)).write_bytes((c.OUT/name).read_bytes())
    print('original command assertions passed:127',flush=True)
if __name__=='__main__':main()
