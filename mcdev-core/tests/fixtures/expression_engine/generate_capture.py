"""One-time exact legacy engine test capture. Rust consumes the resulting JSON only."""
from pathlib import Path
import importlib.util,json,os,sys,re,hashlib
ROOT=Path(__file__).resolve().parents[4]
HELPER=ROOT/'mcdev-core/tests/fixtures/mixin_source/generate_capture.py'
spec=importlib.util.spec_from_file_location('source_capture',HELPER)
capture=importlib.util.module_from_spec(spec);spec.loader.exec_module(capture)
OUT=ROOT/'build/expression-oracle';OUT.mkdir(parents=True,exist_ok=True)
previous_legacy=(Path(__file__).parent/'legacy_calls.jsonl').read_bytes() if (Path(__file__).parent/'legacy_calls.jsonl').exists() else None
capture.OUT=OUT
runtime=json.loads((ROOT/'build/source-oracle/runtime.json').read_text(encoding='utf-8'))
runtime['output']=str(OUT)
def modern_asm(classpath):
    entries=classpath.split(os.pathsep);chosen={}
    candidates=entries+[str(path)for path in (capture.CACHE/'org.ow2.asm').glob('*/*/*/*.jar') if '-sources'not in path.name and '-javadoc'not in path.name]
    for entry in candidates:
        parts=re.split(r'[\\/]+',entry)
        if 'org.ow2.asm'not in parts:continue
        parts=parts[parts.index('org.ow2.asm'):]
        if parts[0]!='org.ow2.asm' or parts[1]=='asm-debug-all':continue
        if parts[2]!='9.7.1':continue
        version=tuple(int(value)for value in parts[2].split('.'))
        if parts[1]not in chosen or version>chosen[parts[1]][0]:chosen[parts[1]]=(version,entry)
    return os.pathsep.join([entry for entry in entries if 'org.ow2.asm'not in entry]+[entry for _,entry in chosen.values()])
runtime['classpath']=modern_asm(runtime['classpath'])
runtime['compiler_classpath']=modern_asm(runtime['compiler_classpath'])
if '--show-cp'in sys.argv:
    print('pathsep',repr(os.pathsep),'classpath items',len(runtime['classpath'].split(os.pathsep)))
    for entry in runtime['classpath'].split(os.pathsep):
        if 'asm'in Path(entry).name:print('asm',Path(entry).name,Path(entry).exists())
    sys.exit(0)
APIS={
 'mixinextras/OfficialExpressionParser.kt':{'parse'},
 'mixinextras/OfficialExpressionMatcher.kt':{'match','completeReceiverMembers'},
 'mixinextras/OfficialExpressionIdentifierPoolBuilder.kt':{'build'},
 'mixinextras/OfficialExpressionMemberCompletion.kt':{'build'},
}
sources=[]
for path,names in APIS.items():
    target=capture.instrument(capture.MAIN/path,names)
    text=target.read_text(encoding='utf-8')
    text=text.replace('commonSuperClass, cancellationChecker)', 'Capture.resolver(commonSuperClass), cancellationChecker)')
    text=text.replace('capture_impl_build(index, typeNameResolver)', 'capture_impl_build(index, Capture.typeResolver(typeNameResolver))')
    text=text.replace('if (!seen.add(dedupeKey))', 'if (!seen.add(dedupeKey) && !Capture.allComponents())')
    target.write_text(text,encoding='utf-8');sources.append(target)
predicate_test=capture.BASE/'mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/OfficialExpressionIdentifierPoolBuilderTest.kt'
predicate_source=predicate_test.read_text(encoding='utf-8').replace('import kotlin.test.Test','import oracle.Capture\nimport kotlin.test.Test')
for method in ['matchesMember','matchesType','memberExists','typeExists']:
    predicate_source=re.sub(r'(\w+)\.pool\.delegate\.'+method+r'\(',r'Capture.'+method+r'(\1.pool, ',predicate_source)
predicate_source=predicate_source.replace('definition.matches(', 'Capture.handle(definition, ')
predicate_dest=OUT/'OfficialExpressionIdentifierPoolBuilderTest.kt'
predicate_dest.write_text(predicate_source,encoding='utf-8');sources.append(predicate_dest)
capture_source=HELPER.with_name('Capture.kt').read_text(encoding='utf-8')
capture_source=capture_source.replace('import io.github.mcdev.core.mixin.*','import io.github.mcdev.core.mixin.*\nimport io.github.mcdev.core.mixinextras.*\nimport org.objectweb.asm.Type as AsmType')
capture_source=capture_source.replace('else -> value\n    }','''is OfficialExpressionIdentifierPool -> pool(value)
        is OfficialExpressionCancellationChecker -> mapOf("cancellationChecker" to true)
        is CommonSuperClassResolver -> mapOf("commonSuperClass" to true)
        is ClassLiteralTypeNameResolver -> mapOf("typeNameResolver" to true)
        else -> value
    }''',1)
capture_source=capture_source.replace('    fun <T> capture(', '''    private fun fields(value: Any): Map<String,Any?> = value.javaClass.declaredFields.filter { !java.lang.reflect.Modifier.isStatic(it.modifiers) }.associate { it.isAccessible = true; it.name to it.get(value) }
    private fun hasUnpaired(value:String):Boolean {var index=0;while(index<value.length){val ch=value[index++];if(Character.isHighSurrogate(ch)){if(index==value.length||!Character.isLowSurrogate(value[index++]))return true}else if(Character.isLowSurrogate(ch))return true};return false}
    private var componentMode = false
    fun allComponents():Boolean = componentMode
    fun <T> components(body:()->T):T { val previous=componentMode;componentMode=true;try{return body()}finally{componentMode=previous} }
    private fun pool(value: OfficialExpressionIdentifierPool): Any {
        val data = fields(value)
        val delegate = fields(data.getValue("delegate")!!)
        fun definitions(key: String) = (delegate[key] as Map<*,*>).mapValues { (_, entries) -> (entries as List<*>).map { entry -> mapOf("kind" to entry!!.javaClass.simpleName, "value" to fields(entry)) } }
        return mapOf("members" to definitions("members"), "types" to definitions("types"), "locals" to data["locals"])
    }
    fun resolver(target: CommonSuperClassResolver): CommonSuperClassResolver = CommonSuperClassResolver { left, right -> read("CommonSuperClassResolver.resolve",listOf(left,right),target.resolve(left,right)) }
    fun typeResolver(target: ClassLiteralTypeNameResolver): ClassLiteralTypeNameResolver = ClassLiteralTypeNameResolver { name -> read("ClassLiteralTypeNameResolver.resolve",listOf(name),target.resolve(name)) }
    fun matchesMember(pool: OfficialExpressionIdentifierPool, id: String, node: com.llamalad7.mixinextras.expression.impl.flow.FlowValue): Boolean = capture("IdentifierPool.matchesMember",mapOf("pool" to pool,"id" to id,"node" to node)) { pool.delegate.matchesMember(id,node) }
    fun matchesType(pool: OfficialExpressionIdentifierPool, id: String, type: AsmType): Boolean = capture("IdentifierPool.matchesType",mapOf("pool" to pool,"id" to id,"type" to type)) { pool.delegate.matchesType(id,type) }
    fun memberExists(pool: OfficialExpressionIdentifierPool, id: String): Boolean = capture("IdentifierPool.memberExists",mapOf("pool" to pool,"id" to id)) { pool.delegate.memberExists(id) }
    fun typeExists(pool: OfficialExpressionIdentifierPool, id: String): Boolean = capture("IdentifierPool.typeExists",mapOf("pool" to pool,"id" to id)) { pool.delegate.typeExists(id) }
    fun handle(definition: Any, input: Any): Boolean = capture("MemberDefinition.matches",mapOf("definition" to mapOf("kind" to definition.javaClass.simpleName,"value" to fields(definition)),"input" to input)) {
        val method = definition.javaClass.methods.single { it.name == "matches" && it.parameterTypes.singleOrNull()?.isInstance(input) == true }
        method.isAccessible = true
        method.invoke(definition,input) as Boolean
    }
    fun <T> capture(''',1)
capture_source=capture_source.replace('val gson = GsonBuilder()', '''val gson = GsonBuilder().registerTypeHierarchyAdapter(Throwable::class.java,
        JsonSerializer<Throwable> { value, _, context -> context.serialize(mapOf("type" to value.javaClass.name,"message" to value.message)) })
        .registerTypeHierarchyAdapter(AsmType::class.java,JsonSerializer<AsmType> { value,_,context -> context.serialize(value.descriptor) })
        .registerTypeAdapter(String::class.java,JsonSerializer<String> { value,_,context -> if(hasUnpaired(value))context.serialize(mapOf("utf16" to value.map { it.code })) else com.google.gson.JsonPrimitive(value) })
        .registerTypeHierarchyAdapter(io.github.mcdev.core.descriptor.JvmType::class.java,JsonSerializer<io.github.mcdev.core.descriptor.JvmType> { value,_,context -> context.serialize(io.github.mcdev.core.descriptor.DescriptorRenderer.toDescriptor(value)) })
        .registerTypeHierarchyAdapter(io.github.mcdev.core.descriptor.Pattern::class.java,JsonSerializer<io.github.mcdev.core.descriptor.Pattern<*>> { value,_,context -> context.serialize(mapOf("kind" to value.javaClass.simpleName,"fields" to fields(value))) })
        .registerTypeHierarchyAdapter(com.llamalad7.mixinextras.expression.impl.flow.FlowValue::class.java,JsonSerializer<com.llamalad7.mixinextras.expression.impl.flow.FlowValue> { value,_,context ->
            context.serialize(mapOf("type" to value.type,"instruction" to value.insn?.let { mapOf("opcode" to it.opcode,"kind" to it.javaClass.simpleName,"fields" to fields(it)) })) })
        .registerTypeHierarchyAdapter(org.objectweb.asm.tree.AbstractInsnNode::class.java,JsonSerializer<org.objectweb.asm.tree.AbstractInsnNode> { value,_,context -> context.serialize(mapOf("opcode" to value.opcode,"kind" to value.javaClass.simpleName,"fields" to fields(value))) })
        .registerTypeHierarchyAdapter(com.llamalad7.mixinextras.expression.impl.ast.identifiers.TypeIdentifier::class.java,JsonSerializer<com.llamalad7.mixinextras.expression.impl.ast.identifiers.TypeIdentifier> { value,_,context -> context.serialize(mapOf("kind" to value.javaClass.simpleName,"fields" to fields(value))) })
        .registerTypeHierarchyAdapter(com.llamalad7.mixinextras.expression.impl.ast.identifiers.MemberIdentifier::class.java,JsonSerializer<com.llamalad7.mixinextras.expression.impl.ast.identifiers.MemberIdentifier> { value,_,context -> context.serialize(mapOf("kind" to value.javaClass.simpleName,"fields" to fields(value))) })
        .registerTypeHierarchyAdapter(OfficialExpressionIdentifierPool::class.java,JsonSerializer<OfficialExpressionIdentifierPool> { value,_,context -> context.serialize(pool(value)) })
        .registerTypeHierarchyAdapter(com.llamalad7.mixinextras.expression.impl.ast.expressions.Expression::class.java,
            JsonSerializer<com.llamalad7.mixinextras.expression.impl.ast.expressions.Expression> { value,_,context ->
                context.serialize(mapOf("kind" to value.javaClass.simpleName,"source" to value.src,"fields" to value.javaClass.fields.filter { !java.lang.reflect.Modifier.isStatic(it.modifiers) }.associate { it.name to it.get(value) })) })
        .registerTypeHierarchyAdapter(OfficialExpressionTypeConstraint::class.java,JsonSerializer<OfficialExpressionTypeConstraint> { value,_,context -> context.serialize(mapOf("kind" to value.javaClass.simpleName,"fields" to fields(value))) })''',1)
capture_source=capture_source.replace('fun <T> capture(api: String, args: Any?, body: () -> T): T {','fun <T> capture(apiName: String, args: Any?, body: () -> T): T {\n        val api=if(componentMode && apiName=="OfficialExpressionMatcher.match") "OfficialExpressionMatcher.matchComponents" else apiName')
dest=OUT/'classes';dest.mkdir(exist_ok=True)
own=Path(__file__).parent
(OUT/'Capture.kt').write_text(capture_source,encoding='utf-8')
sources+=[OUT/'Capture.kt',HELPER.with_name('Run.kt')]
sources+=[own/'EngineAdditionalOracle.kt']
if '--capture-only'not in sys.argv and '--export-only'not in sys.argv:
    capture.run_java(runtime,runtime['compiler_classpath'],'com.sun.tools.javac.Main',[
        '-proc:none','-g','-d',str(dest),str(own/'SemanticSamples.java')],'javac-semantic',60)
    capture.run_java(runtime,runtime['compiler_classpath'],'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',[
     '-no-stdlib','-no-reflect','-jvm-target','21','-Xfriend-paths='+str(capture.BASE/'mcdev-core/build/classes/kotlin/main'),
     '-classpath',runtime['classpath'],'-d',str(dest)]+[str(path)for path in sources],'compile',180)
tests=['OfficialExpressionParserTest','OfficialExpressionMatcherTest','OfficialExpressionMatcherMemberCompletionTest','OfficialExpressionIdentifierPoolBuilderTest']
tests+=['EngineAdditionalOracle|mcdev-core/tests/fixtures/expression_engine/EngineAdditionalOracle.kt']
if '--export-only'not in sys.argv:
    capture.run_java(runtime,str(dest)+os.pathsep+runtime['classpath'],'oracle.RunKt',[
     str(OUT/'calls.jsonl'),str(OUT/'cases.json')]+['io.github.mcdev.core.mixinextras.'+name for name in tests],'capture',180)
rows=json.loads((OUT/'cases.json').read_text(encoding='utf-8'))
if any(row.get('status')=='failed' for row in rows):raise RuntimeError('Cannot export failing original test cases')
calls=[json.loads(line)for line in (OUT/'calls.jsonl').read_text(encoding='utf-8').split('\n')if line]
class_bytes={}
for call in calls:
    args=call['args']
    if 'classBytes'in args:
        raw=args.pop('classBytes');key=hashlib.sha256(bytes(value%256 for value in raw)).hexdigest()
        class_bytes[key]=raw;args['classBytesKey']=key
(own/'class_bytes.json').write_text(json.dumps(class_bytes,separators=(',',':')),encoding='utf-8')
fatal_deltas=[]
for call in calls:
    if call.get('throwable')=='java.lang.AssertionError':
        assert '[JSR]'in call.get('message','') and 'EngineAdditionalOracle'in call['legacyClass'],'Unexpected unsupported fatal oracle outcome'
        key=call['args']['classBytesKey'];data=class_bytes[key]
        fatal_deltas.append({'legacyCase':call['legacyCase'],'api':call['api'],'classSha256':key,'classfileVersion':data[6]*256+data[7],'methodName':call['args']['methodName'],'methodDescriptor':call['args']['methodDescriptor'],'opcode':168,'instructionIndex':0,'bytecodeOffset':0,'originalErrorClass':call['throwable'],'originalErrorMessage':call['message'],'nativeReason':'Failed to analyze method bytecode flow','nativeCause':'Failed to analyze method bytecode flow: MixinExtras 0.0.6 does not define a JSR expression value','reason':'Only the official engine undefined JSR AssertionError is represented as typed Unavailable to keep the LSP process alive; cancellation still escapes unchanged; ExactLocalCapture retains JSR/RET support.'})
(own/'jsr_fatal_deltas.json').write_text(json.dumps(fatal_deltas,ensure_ascii=True,indent=2),encoding='utf-8')
ambiguities=[]
for call in calls:
    if call['api']!='OfficialExpressionMatcher.matchComponents' or not call.get('outputType','').endswith('$Available'):continue
    grouped={}
    for value in call['output']['matches']:
        key=tuple(value[name]for name in ['expressionIndex','expressionStartOffset','originalInstructionIndex','originalInstructionOpcode'])
        grouped.setdefault(key,{})[json.dumps(value,sort_keys=True)]=value
    for key,alternatives in grouped.items():
        if len(alternatives)>1:ambiguities.append({'case':call['legacyCase'],'args':call['args'],'key':key,'legalAlternatives':list(alternatives.values())})
(own/'identity_ambiguities.json').write_text(json.dumps({'cause':'official FlowInterpreter.finish uses an IdentityHashMap-backed set; physical-instruction dedup keeps the first virtual component','nativeOrder':'physical instruction index, then original node before expansion insertion order','cases':ambiguities},ensure_ascii=True,indent=2),encoding='utf-8')
for label,is_extra in [('legacy',False),('additional',True)]:
    selected=[call for call in calls if ('EngineAdditionalOracle'in call['legacyCase'])==is_extra]
    (own/(label+'_calls.jsonl')).write_text(''.join(json.dumps(call,ensure_ascii=True,separators=(',',':'))+'\n'for call in selected),encoding='utf-8')
    (own/(label+'_cases.json')).write_text(json.dumps([row for row in rows if ('EngineAdditionalOracle'in row['legacyCase'])==is_extra],ensure_ascii=True,indent=2),encoding='utf-8')
if previous_legacy is not None:
    current=(own/'legacy_calls.jsonl').read_bytes()
    (OUT/'legacy-stability.json').write_text(json.dumps({'previousSha256':hashlib.sha256(previous_legacy).hexdigest(),'currentSha256':hashlib.sha256(current).hexdigest(),'byteForByteEqual':previous_legacy==current,'rows':len(current.splitlines())},indent=2),encoding='utf-8')
    (OUT/'previous-legacy-calls.jsonl').write_bytes(previous_legacy)
    assert previous_legacy==current,'Existing strict legacy corpus changed; inspect legacy-stability.json and previous-legacy-calls.jsonl'
print(f'engine legacy cases={len(rows)}; output={OUT}')
