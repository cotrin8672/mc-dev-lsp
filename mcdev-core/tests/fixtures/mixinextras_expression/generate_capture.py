"""One-time source-expression oracle; the native tests never execute a JVM."""
from pathlib import Path
import importlib.util,os,re,json
ROOT=Path(__file__).resolve().parents[4]
shared=ROOT/'mcdev-core/tests/fixtures/mixin_source/generate_capture.py'
spec=importlib.util.spec_from_file_location('source_oracle',shared);helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
helper.OUT=ROOT/'build/expression-source-oracle'
MAIN=helper.MAIN/'mixinextras'
APIS={
 'DefinitionAnnotationParser.kt':{'parse'},'DefinitionsAnnotationParser.kt':{'parse','parseDefinitionAt'},
 'ExpressionAnnotationParser.kt':{'parse'},'ExpressionsAnnotationParser.kt':{'parse'},
 'ExpressionContextResolver.kt':{'resolveExpressionContext','findEnclosingSite','resolveExpressionContextForSite','parseHandlerAnnotations','handlerRegion','inferExpressionValueType','inferExpressionValueTypeInfo','inferSimpleOperationLayout','inferModifyReceiverLayout','inferWrapWithConditionLayouts','inferFromExpression'},
 'ExpressionReceiverParser.kt':{'parse','isValidReceiverExpression'},'ExpressionCompletionLexer.kt':{'lex'},
 'ExpressionSupport.kt':{'completeAtValue','completeExpressionAnnotations','completeExpressionValue','completeFeatureAnnotations','completeFeatureAnnotationsWithRange','isExpressionAtValue'},
 'ExpressionDefinitionEditPlanner.kt':{'plan','canonicalExactSelector','findReusableDefinitionId','allocateDefinitionId'},
 'ExpressionMemberCompletionService.kt':{'completeMembers'},
 'HandlerSignatureService.kt':{'findAnnotationSites','findSugarHandlerAnnotationSites'},
}
TESTS=['ClassLiteralTypeNameResolverTest','DefinitionAnnotationParseIssueTest','DefinitionAnnotationParserTest','DefinitionAnnotationSingleQuoteRecoveryTest','DefinitionsAnnotationParserTest','ExpressionAnnotationParserTest','ExpressionCompletionLexerTest','ExpressionContextResolverTest','ExpressionDefinitionEditPlannerTest','ExpressionMemberCompletionServiceTest','ExpressionReceiverParserTest','ExpressionsAnnotationParserTest','ExpressionSupportTest','ExpressionTypeImportCacheTest','KotlinExpressionContextTest','MixinExtrasDefinitionIndexTest','MixinExtrasExpressionIndexTest','MixinExtrasAnnotationContextTest']
CLASSES={'ExpressionSupport','ExpressionMemberCompletionService'}
def instrument(path,names):
 text=path.read_text(encoding='utf8');owner=path.stem;changes=[]
 for m in re.finditer(r'(?m)^(    |        )((?:internal )?fun) (\w+)\s*\(',text):
  indent,_,name=m.group(1),m.group(2),m.group(3)
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
   if 'ClassIndex' in ty:args.append('Capture.index('+n+')'+(''if nullable else'!!'))
   elif 'BytecodeIndex' in ty:args.append('HandlerCapture.bytecode('+n+')'+(''if nullable else'!!'))
   else:args.append(n)
   entries.append('"'+n+'" to '+('mapOf("present" to true)' if 'CancellationChecker' in ty else 'Capture.arg('+n+')'))
  if owner in CLASSES:entries.append('"__instance" to System.identityHashCode(this)')
  if owner=='ExpressionMemberCompletionService':entries += ['"__customRunner" to __customRunner','"__cacheInstance" to System.identityHashCode(reachableMembersCache)','"__cacheCapacity" to __cacheCapacity']
  if owner=='ExpressionSupport':entries += ['"__memberInstance" to memberCompletionService?.let { System.identityHashCode(it) }']
  signature=text[m.start():end+1].strip();wrapper=f'{indent}{signature}: {ret} = Capture.capture("{owner}.{name}", mapOf({", ".join(entries)})) {{ capture_impl_{name}({", ".join(args)}) }}\n'
  changes.append((m.start(),m.end(3),wrapper+text[m.start():m.start(3)]+'capture_impl_'+name))
 for start,end,new in reversed(changes):text=text[:start]+new+text[end:]
 if owner=='ExpressionMemberCompletionService':
  text=text.replace('private val classIndex: ClassIndex','classIndex: ClassIndex',1).replace('private val bytecodeIndex: BytecodeIndex','bytecodeIndex: BytecodeIndex',1)
  text=text.replace('private val matcherRunner: ExpressionMemberMatcherRunner','matcherRunner: ExpressionMemberMatcherRunner',1)
  begin=text.index('{',text.index('class '+owner))+1
  fields='''
private val classIndex=Capture.index(classIndex)!!
private val bytecodeIndex=HandlerCapture.bytecode(bytecodeIndex)!!
private val __customRunner=matcherRunner !== ExpressionMemberMatcherRunner.DEFAULT
private val __cacheCapacity=cacheCapacity
private val matcherRunner=ExpressionMemberMatcherRunner { bytes,name,descriptor,receiver,contextType,pool,common,cancellation ->
 val args=listOf(java.security.MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) },name,descriptor,receiver,contextType)
 try { Capture.read("ExpressionMemberMatcherRunner.completeReachableMembers",args,matcherRunner.completeReachableMembers(bytes,name,descriptor,receiver,contextType,pool,common,cancellation)) }
 catch(error:Throwable) { Capture.read("ExpressionMemberMatcherRunner.throwable",args,mapOf("class" to error.javaClass.name,"message" to error.message)); throw error }
}
'''
  text=text[:begin]+fields+text[begin:]
 if owner=='ExpressionSupport':
  text=text.replace('private val classIndex: ClassIndex?','classIndex: ClassIndex?',1);begin=text.index('{',text.index('class '+owner))+1;text=text[:begin]+'\nprivate val classIndex=Capture.index(classIndex)\n'+text[begin:]
 package_end=text.index('\n',text.index('package '));text=text[:package_end+1]+'\nimport oracle.Capture\nimport oracle.HandlerCapture\n'+text[package_end+1:]
 target=helper.OUT/'src'/path.name;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(text,encoding='utf8');return target
def main():
 runtime=helper.prepare_runtime();sources=[instrument(MAIN/path,names)for path,names in APIS.items()]
 sources += [helper.instrument(helper.MAIN/path,names)for path,names in helper.APIS.items()]
 capture_source=shared.with_name('Capture.kt').read_text(encoding='utf8')
 capture_source=capture_source.replace('val gson = GsonBuilder()', 'val gson = GsonBuilder().registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.AtShiftSpec::class.java,HandlerCapture.shiftSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec::class.java,HandlerCapture.sugarSerializer).registerTypeHierarchyAdapter(io.github.mcdev.core.bytecode.ConstantValue::class.java,HandlerCapture.constantSerializer).registerTypeHierarchyAdapter(org.objectweb.asm.Type::class.java,com.google.gson.JsonSerializer<org.objectweb.asm.Type>{ value,_,context -> context.serialize(value.descriptor) })')
 capture_source=capture_source.replace('val gson = GsonBuilder()', 'val gson = GsonBuilder().registerTypeHierarchyAdapter(io.github.mcdev.core.mixinextras.OfficialExpressionTypeConstraint::class.java,com.google.gson.JsonSerializer<io.github.mcdev.core.mixinextras.OfficialExpressionTypeConstraint>{ value,_,context -> context.serialize(mapOf("kind" to value.javaClass.simpleName,"fields" to value.javaClass.declaredFields.filter { !java.lang.reflect.Modifier.isStatic(it.modifiers) }.associate { it.isAccessible=true;it.name to it.get(value) })) })')
 capture_source=capture_source.replace('val gson = GsonBuilder()', 'val gson = GsonBuilder().registerTypeHierarchyAdapter(Throwable::class.java,com.google.gson.JsonSerializer<Throwable>{ value,_,context -> context.serialize(mapOf("class" to value.javaClass.name,"message" to value.message)) })')
 capture_source=capture_source.replace('val row = mapOf("legacyCase"', 'val row = mapOf("sourceHint" to frames.get().mapNotNull { (it.args as? Map<*,*>)?.get("source") as? String }.firstOrNull(), "legacyCase"')
 capture_file=helper.OUT/'src/Capture.kt';capture_file.write_text(capture_source,encoding='utf8');sources += [capture_file,shared.with_name('Run.kt'),ROOT/'mcdev-core/tests/fixtures/mixinextras_handler/HandlerCapture.kt']
 dest=helper.OUT/'classes';dest.mkdir(exist_ok=True)
 helper.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',['-no-stdlib','-no-reflect','-jvm-target','21','-module-name','mcdev-core','-Xfriend-paths='+str(helper.BASE/'mcdev-core/build/classes/kotlin/main'),'-classpath',runtime['classpath'],'-d',str(dest)]+[str(p)for p in sources],'compile-expression-source',240)
 capture=helper.OUT/'calls.jsonl';summary=helper.OUT/'cases.json'
 helper.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[str(capture),str(summary)]+['io.github.mcdev.core.mixinextras.'+name for name in TESTS],'capture-expression-source',300)
 cases=json.loads(summary.read_text(encoding='utf8'));assert all(c['status']=='passed'for c in cases)
 here=Path(__file__).parent;(here/'calls.jsonl').write_bytes(capture.read_bytes());(here/'cases.json').write_text(json.dumps(cases,indent=2),encoding='utf8')
 print('original cases:',len(cases),'calls:',sum(c['calls']for c in cases),flush=True)
if __name__=='__main__':main()
