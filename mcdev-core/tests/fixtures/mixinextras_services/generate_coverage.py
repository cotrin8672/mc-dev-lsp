"""Exact 133-case ledger; extra caller captures are explicitly partial/pending."""
from pathlib import Path
import json,re,hashlib,importlib.util
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];BASE=ROOT/'build/migration-baseline/current-20261023'
spec=importlib.util.spec_from_file_location('coverage_helpers',HERE.parent/'mixin_source/generate_capture.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
calls_file=HERE/'services-calls.jsonl';calls=[json.loads(s)for s in calls_file.read_text(encoding='utf8').splitlines()];manifest=json.loads((HERE/'services-cases.json').read_text(encoding='utf8'))
direct={'selectorFallsBackToUniqueHandlerOverlap':'selector_falls_back_to_unique_handler_overlap','selectorIgnoresHandlerRangesThatOnlyTouchAtEndpoint':'selector_ignores_handler_ranges_that_only_touch_at_endpoint','selectorPrefersUniqueAnnotationContainment':'selector_prefers_unique_annotation_containment','selectorReturnsNullWhenAnnotationMatchesAreAmbiguous':'selector_returns_null_when_annotation_matches_are_ambiguous','selectorReturnsNullWhenHandlerOverlapIsAmbiguous':'selector_returns_null_when_handler_overlap_is_ambiguous'}
coverage=[]
for case in manifest:
 path,name=case['legacyCase'].split('::');source=(BASE/path).read_text(encoding='utf8');start=source.index('fun '+name+'(');begin=source.index('{',start);depth=1;i=begin+1
 while depth:
  if i>=len(source):raise ValueError(case['legacyCase'])
  if source.startswith('"""',i):
   literal=source.find('"""',i+3)+3
   while literal<len(source)and source[literal]=='"':literal+=1
  else:literal=helper.skip_literal(source,i)
  if literal is not None:i=literal;continue
  if source[i]=='{':depth+=1
  elif source[i]=='}':depth-=1
  i+=1
 body=source[start:i];rows=[i+1 for i,r in enumerate(calls)if r['legacyCase']==case['legacyCase']and r['api'].startswith('MixinExtras')]
 assert rows or name in direct
 coverage.append({'legacyCase':case['legacyCase'],'originalStatus':case['status'],'status':'native-direct'if name in direct else'native-public-api-replay','nativePath':'mcdev-core/tests/mixinextras_services_legacy.rs','nativeTest':direct.get(name,'every_service_public_output_and_applied_document_matches_the_frozen_oracle'),'publicCaptureRows1Based':rows,'originalAssertionCount':len(re.findall(r'\bassert\w+\s*\(',body)),'originalCaseBody':body})
callers=json.loads((HERE/'caller-cases.json').read_text(encoding='utf8'))
result={'baseline':'current-20261023','legacyCases':133,'diagnosticCases':65,'actionCases':68,'publicReplayCases':128,'directSelectorCases':5,'publicCalls':sum(r['api'].startswith('MixinExtras')for r in calls),'allCapturedCalls':len(calls),'captureSha256':hashlib.sha256(calls_file.read_bytes()).hexdigest(),'hostFactsSha256':hashlib.sha256((HERE/'host-reference-facts.json').read_bytes()).hexdigest(),'comparison':'Final public API full typed outputs, checked applied documents, exact ordered callback API/arguments/count, including sequential repeated-read values. Concrete McFix subtype preserved. Core byte offsets converted from original UTF16 scalar indices.','additionalCallerPolicy':'CoreHandlerSignatureCompletionTest callers remain pending complete Facade migration; native comparison here proves only public service leaf outputs. ApplyParityTest contains new direct compatibility fixtures.','additionalCallers':[dict(case,status='pending-facade'if case['legacyCase'].startswith('mcdev-core/')else'new-native-parity-fixture')for case in callers],'cases':coverage}
(HERE/'services-coverage.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf8')
print(json.dumps({k:v for k,v in result.items()if k not in['cases','additionalCallers']},ensure_ascii=False),flush=True)
