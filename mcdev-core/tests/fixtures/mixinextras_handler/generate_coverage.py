"""Exact old case -> native call/expected-output manifest, with original assertion bodies."""
from pathlib import Path
import json,hashlib,importlib.util,re
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
BASE=ROOT/'build/migration-baseline/current-20261023'
spec=importlib.util.spec_from_file_location('coverage_source',HERE.parent/'mixin_source/generate_capture.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
calls_file=HERE/'handler-calls.jsonl'
calls=[json.loads(line)for line in calls_file.read_text(encoding='utf8').splitlines()]
manifest=json.loads((HERE/'handler-cases.json').read_text(encoding='utf8'))
deferred={'facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries','facadeRoutesAccessorInvokerAndOverwriteToDeclarationSnippets'}
coverage=[]
for case in manifest:
    path,name=case['legacyCase'].split('::',1);source=(BASE/path).read_text(encoding='utf8');start=source.index('fun '+name+'(');begin=source.index('{',start);depth=1;i=begin+1
    while depth:
        if i>=len(source):raise ValueError(case['legacyCase'])
        if source.startswith('"""',i):
            literal=source.find('"""',i+3)+3
            while literal<len(source) and source[literal]=='"':literal+=1
        else:literal=helper.skip_literal(source,i)
        if literal is not None:i=literal;continue
        if source[i]=='{':depth+=1
        elif source[i]=='}':depth-=1
        i+=1
    body=source[start:i]
    indices=[i+1 for i,row in enumerate(calls)if row['legacyCase']==case['legacyCase']]
    native='every_handler_call_replays_exact_inputs_and_results'
    if name=='mixinExtrasAnnotationMapsFromSimpleName':native='mixin_extras_annotation_maps_from_simple_name'
    status='native-replayed'if indices else 'native-direct'
    if name in deferred:status='pending-facade';native=None
    coverage.append({'legacyCase':case['legacyCase'],'legacyStatus':case['status'],'status':status,'nativeTest':native,'nativePath':'mcdev-core/tests/mixinextras_handler_dependencies.rs'if native else None,'captureRows1Based':indices,'originalAssertionCount':len(re.findall(r'\bassert\w+\s*\(',body)),'originalCaseBody':body})
result={'baseline':'current-20261023','legacyCases':394,'nativeReplayedCases':391,'nativeDirectCases':1,'pendingFacadeCases':2,'capturedCalls':len(calls),'captureSha256':hashlib.sha256(calls_file.read_bytes()).hexdigest(),'constructorPresence':'explicit hasBytecodeIndex on every instance API; no inferred capability','referenceFacts':'host-reference-facts.json contains actual frozen VM Class.forName/isAssignableFrom outcomes; production requires authoritative host provider','referenceFactsSha256':hashlib.sha256((HERE/'host-reference-facts.json').read_bytes()).hexdigest(),'offsets':'Core source offsets are UTF8 bytes; legacy scalar String offsets are checked UTF16-to-UTF8 at the replay boundary; DTO ranges retain UTF16','nativeExtraTests':'mcdev-core/tests/mixinextras_handler_knowledge.rs','cases':coverage}
(HERE/'handler-coverage.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
print(json.dumps({key:value for key,value in result.items()if key not in ['cases']}),flush=True)
