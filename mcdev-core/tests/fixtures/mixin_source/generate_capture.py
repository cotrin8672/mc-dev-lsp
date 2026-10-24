"""One-time legacy oracle capture; cargo tests consume only the checked-in JSON.

The frozen baseline is read only. Instrumented Kotlin copies and all build logs
live under build/source-oracle. No JVM is needed to execute the Rust fixtures.
"""
from pathlib import Path
import argparse, json, re, subprocess, os, sys

ROOT=Path(__file__).resolve().parents[4]
BASE=ROOT/'build/migration-baseline/current-20261023'
OUT=ROOT/'build/source-oracle'
CACHE=ROOT/'build/migration-dependencies/cache'
if not CACHE.exists():CACHE=Path.home()/'.gradle/caches/modules-2/files-2.1'
MAIN=BASE/'mcdev-core/src/main/kotlin/io/github/mcdev/core'
APIS={
 'mixin/AnnotationContextExtractor.kt': {'extract','extractAtOffset','findAnnotationOffsets','findInjectorAnnotationOffsets','extractAtAnnotationBodies','parseMixinTargetValues','resolveMixinTargets','resolveRawMixinTargets','resolveMixinClassScope','fqnToInternal','toOffset'},
 'mixin/JavaTypeDescriptorResolver.kt': {'isKotlinSource','importsFor','descriptor','descriptorOrNull','descriptorOrDiagnostic','methodDescriptor','methodDescriptorOrNull','parameterTypes','rawParameterTypes','splitTopLevel'},
 'mixin/MixinMemberDeclarationParser.kt': {'parseShadowDeclarations','parseAccessorDeclarations','parseInvokerDeclarations','parseOverwriteDeclarations','parseDeclarationDiagnostics','findShadowPrefix','findShadowRemap','findMemberNameTextRange'},
 'mixin/MixinSemanticModelParser.kt': {'parse'},
 'mixin/MixinTargetResolver.kt': {'resolveTarget','resolveTargets','resolveTargetsFromSource'},
 'mixin/SemanticCompletionContextExtractor.kt': {'extract','toAnnotationContext'},
 'text/JavaStringContentDecoder.kt': {'decodeUnicodeEscape','decodeEscape','decodeContent'},
}
TESTS=['AnnotationContextExtractorTest','KotlinAnnotationContextExtractorTest','KotlinTypeDescriptorResolverTest','MixinMemberDeclarationParserTest','MixinSemanticModelParserTest','KotlinMixinSemanticModelTest','MixinTargetResolverTest','SemanticCompletionContextExtractorTest']

def skip_literal(text,i):
    if text.startswith('//',i): return text.find('\n',i)+1 if '\n' in text[i:] else len(text)
    if text.startswith('/*',i):
        end=text.find('*/',i+2);return len(text) if end<0 else end+2
    if text.startswith('"""',i):
        end=text.find('"""',i+3);return len(text) if end<0 else end+3
    if text[i] in '\"\'':
        q=text[i];i+=1
        while i<len(text):
            if text[i]=='\\':i+=2
            elif text[i]==q:return i+1
            else:i+=1
        return i
    return None

def closing(text,start):
    depth=0;i=start
    while i<len(text):
        lit=skip_literal(text,i)
        if lit is not None:i=lit;continue
        if text[i]=='(':depth+=1
        elif text[i]==')':
            depth-=1
            if depth==0:return i
        i+=1
    raise ValueError('unclosed signature')

def split_params(text):
    parts=[];start=0;depth=0;i=0
    while i<len(text):
        lit=skip_literal(text,i)
        if lit is not None:i=lit;continue
        if text[i] in '(<[{':depth+=1
        elif text[i] in ')>]}':depth-=1
        elif text[i]==',' and depth==0:parts.append(text[start:i]);start=i+1
        i+=1
    if text[start:].strip():parts.append(text[start:])
    return parts

def instrument(path,names):
    text=path.read_text(encoding='utf-8');obj=re.search(r'(?:internal )?object (\w+)\s*\{',text).group(1)
    replacements=[];wrappers=[]
    for m in re.finditer(r'(?m)^    (?:internal )?fun (\w+)\s*\(',text):
        name=m.group(1)
        if name not in names:continue
        op=text.index('(',m.start());end=closing(text,op);params=text[op+1:end];after=end+1
        while text[after].isspace():after+=1
        if text[after]!=':':raise ValueError((path,name,'implicit return needs config'))
        ret_start=after+1;after=ret_start;depth=0
        while after<len(text):
            c=text[after]
            if c in '<(':depth+=1
            elif c in '>)':depth-=1
            if depth==0 and c in '={':break
            after+=1
        ret=text[ret_start:after].strip();args=[];entries=[]
        for p in split_params(params):
            n,ty=p.split(':',1);n=n.strip();ty=ty.strip();args.append('Capture.context('+n+')' if 'JavaTypeResolutionContext' in ty else 'Capture.index('+n+')'+('!!' if 'ClassIndex' in ty and not ty.split('=')[0].strip().endswith('?') else '') if 'ClassIndex' in ty else n)
            entries.append('"'+n+'" to Capture.arg('+n+')')
        signature=text[m.start():end+1].strip()
        wrappers.append(f'    {signature}: {ret} = Capture.capture("{obj}.{name}", mapOf({", ".join(entries)})) {{ capture_impl_{name}({", ".join(args)}) }}\n')
        replacements.append((m.start(1),m.end(1),'capture_impl_'+name))
    for start,end,value in reversed(replacements):text=text[:start]+value+text[end:]
    text=text.replace('package io.github.mcdev.core.', 'importPLACEHOLDER io.github.mcdev.core.',1) if False else text
    package_end=text.index('\n',text.index('package '))
    text=text[:package_end+1]+'\nimport oracle.Capture\n'+text[package_end+1:]
    insert=re.search(r'(?:internal )?object '+obj+r'\s*\{',text).end()
    text=text[:insert]+'\n'+''.join(wrappers)+text[insert:]
    target=OUT/'src'/path.relative_to(MAIN);target.parent.mkdir(parents=True,exist_ok=True);target.write_text(text,encoding='utf-8')
    return target

def jars():
    groups=['org.jetbrains.kotlin','org.jetbrains.intellij.deps','org.jetbrains','org.jetbrains.kotlinx','com.google.code.gson','org.ow2.asm','org.junit.jupiter','org.junit.platform','org.opentest4j','org.apiguardian','io.github.llamalad7','org.spongepowered','org.apache.commons','com.google.guava','org.antlr']
    result=[]
    for group in groups:
        directory=CACHE/group
        if not directory.exists():continue
        for artifact in directory.iterdir():
            if not artifact.is_dir():continue
            if group=='org.ow2.asm'and 'all'in artifact.name:continue
            versions=[v for v in artifact.iterdir()if v.is_dir()and any(not any(s in p.name for s in ['-sources','-javadoc'])for p in v.rglob('*.jar'))]
            if not versions:continue
            preferred=artifact/('9.7.1'if group=='org.ow2.asm'else '2.0.21')
            version=preferred if group in ['org.jetbrains.kotlin','org.ow2.asm']and preferred in versions else max(versions,key=lambda v:tuple(int(x)for x in re.findall(r'\d+',v.name)))
            for p in version.rglob('*.jar'):
                if not any(s in p.name for s in ['-sources','-javadoc','-gradle','-compiler-runner','-kotlin-dsl','-build-tools']):result.append(str(p))
    result=sorted(set(result))
    fallback=ROOT/'build/migration-dependencies/bundle-fallback.json'
    if fallback.exists():
        # The original Mixin classes survive in this frozen bundle; restored
        # pinned dependencies and instrumented classes must take precedence.
        bundle=Path(json.loads(fallback.read_text(encoding='utf-8'))['frozenBundle'])
        if not bundle.exists():raise FileNotFoundError(bundle)
        result.append(str(bundle))
    return result

def prepare_runtime():
    OUT.mkdir(parents=True,exist_ok=True);alljars=jars();dirs=[]
    for module in ['mcdev-core','mcdev-test-fixtures']:
        for scope in ['main','test']:dirs.append(str(BASE/module/'build/classes/kotlin'/scope));dirs.append(str(BASE/module/'build/resources'/scope))
    cp=os.pathsep.join(dirs+alljars)
    compiler_jars=sorted(alljars,key=lambda p:0 if 'kotlin-compiler-embeddable-2.0.21.jar'in p else 1)
    runtime={'java':r'C:/Program Files/Java/jdk-21/bin/java.exe','classpath':cp,'compiler_classpath':os.pathsep.join(compiler_jars),'baseline':str(BASE),'output':str(OUT)}
    (OUT/'runtime.json').write_text(json.dumps(runtime,indent=2),encoding='utf-8');return runtime

def run_java(runtime,cp,main,args,name,timeout):
    argv=['-Xmx1g','-cp',cp,main]+args
    argfile=OUT/(name+'.args');argfile.write_text('\n'.join('"'+a.replace('\\','/').replace('"','\\"')+'"' for a in argv),encoding='utf-8')
    with (OUT/(name+'.stdout.log')).open('w',encoding='utf-8') as out,(OUT/(name+'.stderr.log')).open('w',encoding='utf-8') as err:
        try:result=subprocess.run([runtime['java'],'@'+str(argfile)],stdout=out,stderr=err,timeout=timeout,creationflags=subprocess.CREATE_NO_WINDOW if os.name=='nt' else 0)
        except subprocess.TimeoutExpired:raise RuntimeError(name+' timed out (owned java terminated)')
    print(f'{name}: exit={result.returncode}; logs={OUT/name}',flush=True)
    if result.returncode:raise RuntimeError(name+' failed')

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--prepare-only',action='store_true');a=parser.parse_args();runtime=prepare_runtime()
    if a.prepare_only:return
    sources=[instrument(MAIN/path,names)for path,names in APIS.items()]
    sources += [Path(__file__).with_name('Capture.kt'),Path(__file__).with_name('Run.kt')]
    dest=OUT/'classes';dest.mkdir(exist_ok=True)
    run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['classpath'],'-d',str(dest)]+[str(s)for s in sources],'compile',180)
    capture=OUT/'source-calls.jsonl';summary=OUT/'source-cases.json';run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[str(capture),str(summary)]+['io.github.mcdev.core.mixin.'+n for n in TESTS],'capture',180)
    cases=json.loads(summary.read_text(encoding='utf-8'));print(f'original cases={len(cases)}; passed={sum(c["status"]=="passed"for c in cases)}; captured calls={sum(c.get("calls",0)for c in cases)}',flush=True)
    fixtures=Path(__file__).parent
    (fixtures/'source-calls.jsonl').write_bytes(capture.read_bytes())
    (fixtures/'source-cases.json').write_text(json.dumps(cases,indent=2),encoding='utf-8')
if __name__=='__main__':main()
