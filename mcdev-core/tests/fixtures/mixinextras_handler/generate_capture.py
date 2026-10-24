"""One-time frozen JVM oracle; Rust tests consume committed captures without a JVM."""
from pathlib import Path
import importlib.util,os,re,json
ROOT=Path(__file__).resolve().parents[4]
shared=ROOT/'mcdev-core/tests/fixtures/mixin_source/generate_capture.py'
spec=importlib.util.spec_from_file_location('shared_source_oracle',shared);helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
helper.OUT=ROOT/'build/handler-oracle'
BASE=ROOT/'build/migration-baseline/current-20261023'
MAIN=BASE/'mcdev-core/src/main/kotlin/io/github/mcdev/core/mixinextras'
APIS={
 'ConstantAtArgsParser.kt':{'parse'},
 'OperationSignatureRenderer.kt':{'renderOperationType','readableType','descriptorFromTypeName'},
 'InjectionPointOccurrenceResolver.kt':{'resolve'},
 'OperationCallValidator.kt':{'resolveOperationCallLayout','validate'},
 'HandlerSignatureService.kt':{'resolveTargetMethod','wrapWithConditionTargetStatus','expectedSignature','generateHandlerStub','validateCommonSugarConstraints','returnTypeMatches','parameterTypeMatches','specializeIntLikeSignature','validateHandler','inferExpressionValueType','shareRefValueDescriptor','findAnnotationSites','findSugarHandlerAnnotationSites','parseHandlerMethod','isKotlinCompanionMember','enrichHandlerTypes'},
}
TESTS=['ConstantAtArgsParserTest','InjectionPointOccurrenceResolverTest','OperationCallValidatorTest','HandlerSignatureServiceTest','CoreHandlerSignatureCompletionTest','KotlinHandlerSignatureTest']
def instrument(path,names):
 text=path.read_text(encoding='utf8');owner=path.stem
 changes=[]
 for m in re.finditer(r'(?m)^(    |        )((?:internal )?fun) (\w+)\s*\(',text):
  indent,qualifier,name=m.group(1),m.group(2),m.group(3)
  if name not in names:continue
  op=text.index('(',m.start());end=helper.closing(text,op);params=text[op+1:end];after=end+1
  while text[after].isspace():after+=1
  if text[after]!=':':raise ValueError((path,name,'return type'))
  ret_start=after+1;after=ret_start;depth=0
  while after<len(text):
   c=text[after]
   if c in '<(':depth+=1
   elif c in '>)':depth-=1
   if depth==0 and c in '={':break
   after+=1
  ret=text[ret_start:after].strip();args=[];entries=[]
  for p in helper.split_params(params):
   n,ty=p.split(':',1);n=n.strip();ty=ty.strip();nullable=ty.split('=')[0].strip().endswith('?')
   if 'OperationCallArgumentTypeResolver'in ty:args.append('HandlerCapture.typing('+n+')');entries.append('"'+n+'" to mapOf("hasResolver" to ('+n+' != null))')
   elif 'ClassIndex'in ty:args.append('Capture.index('+n+')'+(''if nullable else'!!'));entries.append('"'+n+'" to Capture.arg('+n+')')
   else:args.append(n);entries.append('"'+n+'" to Capture.arg('+n+')')
  if owner=='HandlerSignatureService' and indent=='    ':entries.append('"hasBytecodeIndex" to (bytecodeIndex != null)')
  signature=text[m.start():end+1].strip()
  wrapper=f'{indent}{signature}: {ret} = Capture.capture("{owner}.{name}", mapOf({", ".join(entries)})) {{ capture_impl_{name}({", ".join(args)}) }}\n'
  renamed=text[m.start():m.start(3)]+'capture_impl_'+name
  changes.append((m.start(),m.end(3),wrapper+renamed))
 for start,end,new in reversed(changes):text=text[:start]+new+text[end:]
 if owner in ['HandlerSignatureService','InjectionPointOccurrenceResolver']:
  text=text.replace('private val classIndex: ClassIndex','classIndex: ClassIndex',1).replace('private val bytecodeIndex: BytecodeIndex','bytecodeIndex: BytecodeIndex',1)
  begin=text.index('{',text.index('class '+owner))+1
  fields='\n    private val bytecodeIndex = HandlerCapture.bytecode(bytecodeIndex)'+(''if owner=='HandlerSignatureService'else'!!')+'\n'
  if owner=='HandlerSignatureService':fields+='    private val classIndex=Capture.index(classIndex)!!\n'
  text=text[:begin]+fields+text[begin:]
 package_end=text.index('\n',text.index('package '));text=text[:package_end+1]+'\nimport oracle.Capture\nimport oracle.HandlerCapture\n'+text[package_end+1:]
 target=helper.OUT/'src'/path.name;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(text,encoding='utf8');return target
def main():
 runtime=helper.prepare_runtime();sources=[instrument(MAIN/path,names)for path,names in APIS.items()]
 capture_source=shared.with_name('Capture.kt').read_text()
 capture_source=capture_source.replace('val gson = GsonBuilder()', 'val gson = GsonBuilder().registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.AtShiftSpec::class.java,HandlerCapture.shiftSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec::class.java,HandlerCapture.sugarSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.bytecode.ConstantValue::class.java,HandlerCapture.constantSerializer)')
 capture_file=helper.OUT/'src/Capture.kt';capture_file.write_text(capture_source,encoding='utf8')
 sources += [capture_file,shared.with_name('Run.kt'),Path(__file__).with_name('HandlerCapture.kt')]
 dest=helper.OUT/'classes';dest.mkdir(exist_ok=True)
 helper.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['classpath'],'-d',str(dest)]+[str(p)for p in sources],'compile-handler',240)
 capture=helper.OUT/'handler-calls.jsonl';summary=helper.OUT/'handler-cases.json'
 helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[str(capture),str(summary)]+['io.github.mcdev.core.mixinextras.'+name for name in TESTS],'capture-handler',300)
 cases=json.loads(summary.read_text());assert all(c['status']=='passed'for c in cases)
 here=Path(__file__).parent;(here/'handler-calls.jsonl').write_bytes(capture.read_bytes());(here/'handler-cases.json').write_text(json.dumps(cases,indent=2),encoding='utf8')
 print('strict original cases:',len(cases),'captured calls:',sum(c['calls']for c in cases),flush=True)
if __name__=='__main__':main()
