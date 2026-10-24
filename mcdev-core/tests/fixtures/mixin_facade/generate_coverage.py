"""Exact facade-case ledger; leaves/callers are never inferred from callee traces."""
from pathlib import Path
import json,hashlib,collections,re
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];BASE=ROOT/'build/migration-baseline/current-20261023'
inventory=json.loads((HERE/'original-case-inventory.json').read_text(encoding='utf8'));assert len(inventory)==len(set(inventory))==189
cases={};rows=collections.defaultdict(list)
for domain in ['core','host']:
 for case in json.loads((HERE/f'{domain}-cases.json').read_text(encoding='utf8')):
  assert case['status']=='passed';assert case['legacyCase']not in cases;cases[case['legacyCase']]=(domain,case)
 for line,text in enumerate((HERE/f'{domain}-calls.jsonl').read_text(encoding='utf8').splitlines(),1):
  row=json.loads(text);rows[row['legacyCase']].append((line,row))
normalizations={
 'bufferOnlyWrapCompletionOmitsUnknownWrapWithConditionVariants':2,
 'definitionIdCompletionDoesNotLeakAdjacentHandlerDefinitions':8,
 'definitionIdCompletionWorksWithoutJavaProjectResolver':8,
}
native_actions={
 'generateAccessorFixContainsFieldName':'generate_accessor_fix_contains_field_name',
 'producesGenerateInvokerFix':'produces_generate_invoker_fix',
 'producesGenerateAccessorFix':'produces_generate_accessor_fix_after_facade_diagnosis',
}
source_only={
 'expressionValueCompletionUsesTokenOnlyReplacementRange':'AnnotationContextExtractor.extractAtOffset',
 'mixinArrayClassFormParsesMultipleTargets':'AnnotationContextExtractor.parseMixinTargetValues',
 'overwriteContextDetectedOnMethodName':'AnnotationContextExtractor.extractAtOffset',
}
ledger=[]
for exact in inventory:
 cls,name=exact.split('#');name=name.removesuffix('()');module='mcdev-jdtls-extension'if'.jdtls.'in cls else'mcdev-core';path=module+'/src/test/kotlin/'+cls.replace('.','/')+'.kt';key=path+'::'+name
 domain,case=cases[key];events=rows[key];assert sum('throwable'not in row for _,row in events)==case['calls']
 api_counts=collections.Counter(row['api']for _,row in events)
 final=[line for line,row in events if row['api'].startswith(('CoreMixinServiceFacade.','HostMixinServiceFacade.','MixinDefinitionService.','MixinCodeActionService.'))]
 entry=dict(originalId=exact,legacyTestPath=path,legacyCase=name,baselineSha256=hashlib.sha256((BASE/path).read_bytes()).hexdigest(),status='ported_native',rustTest='mcdev-core/tests/mixin_facade_legacy.rs::'+('every_original_core_facade_case_calls_native_public_api_and_preserves_all_outputs'if domain=='core'else'every_original_host_facade_case_runs_normalized_host_boundary'),fixture=f'{domain}-calls.jsonl',successfulCalls=case['calls'],throwableEvents=sum('throwable'in row for _,row in events),apiCounts=dict(api_counts),finalApiRows=final,fullPayloadAndVariantComparison=True,externalCallbackOrderAndCount=True)
 if name in native_actions:
  entry['rustTest']='mcdev-core/tests/mixin_facade_legacy.rs::'+native_actions[name]
  entry['nativeFinalApi']='MixinCodeActionService.fixes_for_diagnostics'
  entry['finalApiCoverage']='independent_native_original_assertions; source/facade golden rows are supplemental'
  entry['assertionPreservation']='Exact original source, diagnostic fields/metadata, independent Accessor/Invoker configuration and complete frozen FakeClassIndex facts; native final fix variant and methodSource assertion. producesGenerateAccessorFix also executes the original facade diagnosis and diagnostic fallback before its action tail.'
  entry['supplementalReplayTest']='mcdev-core/tests/mixin_facade_legacy.rs::every_original_core_facade_case_calls_native_public_api_and_preserves_all_outputs'
 elif name=='overwriteAnnotationIsRecognized':
  entry['rustTest']='mcdev-core/tests/mixin_facade_legacy.rs::overwrite_annotation_is_recognized'
  entry['directAssertion']=entry['rustTest'];entry['nativeFinalApi']='MixinAnnotation.from_simple_name'
  entry['finalApiCoverage']='independent_native_original_enum_assertion'
  entry['assertionPreservation']='Direct original enum assertion; a zero-call golden replay is not its native witness.'
 elif name in source_only:
  entry['finalApiRows']=[line for line,row in events if row['api']==source_only[name]]
  assert entry['finalApiRows']
  entry['assertionPreservation']='The original case directly asserts this source API, not a facade/caller. Every whole returned source payload and business enum variant is compared, including its original range/target assertions.'
 elif not final:raise AssertionError('Missing final native API witness: '+exact)
 if name=='routesShadowMemberCompletion':
  entry['tailAssertion']='mcdev-core/tests/mixin_facade_legacy.rs::routes_shadow_member_completion_preserves_direct_field_assertion'
  entry['supplementalReplayTest']=entry['rustTest'];entry['rustTest']=entry['tailAssertion']
  entry['assertionPreservation']='Whole facade outputs plus direct ShadowValidationService.complete_fields with complete frozen FakeClassIndex facts and original currentScreen field assertion.'
 if name in ['facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries','codeActionFixesWrapOperationBadReturnThroughFacade']:
  entry['tailAssertion']='mcdev-core/tests/mixin_facade_legacy.rs::original_edit_application_tails_use_native_facade_outputs'
  entry['supplementalReplayTest']=entry['rustTest'];entry['rustTest']=entry['tailAssertion']
  entry['assertionPreservation']='Whole facade output and strict callbacks plus original native source concatenation/workspace edit application and all original post-application substring assertions.'
 if name=='kotlinSourcesUseSemicolonlessPackagesForConfigAndRegisteredShareLookup':
  entry['tailAssertion']='mcdev-core/tests/mixin_facade_legacy.rs::registered_source_changes_use_real_files_and_live_host_facts'
  entry['supplementalReplayTest']=entry['rustTest'];entry['rustTest']=entry['tailAssertion']
  entry['assertionPreservation']='Original selected_mixin_config equality and collect_source_entries set equality plus whole registered-source sequence; exact source/config snapshots materialized as native files, host-owned source inventory, unsaved current-buffer override.'
 if name in normalizations:entry['representationNormalization']={'kind':'private_buffer_only_noop_singleton','internalEmptyCalls':normalizations[name],'scope':'only this exact original ID; all captured outputs checked null/[]; every external callback remains strict','reason':'The original assertion observes no-op availability/final completion and host laziness, not private singleton method invocations. Rust uses its private in-process empty index and cannot read a host index here.','parentReviewed':True}
 if name=='cancelledLocalCaptureCompletionPropagatesAndNextRequestRecovers':entry['statefulAssertion']='mixin_facade_legacy.rs::cancelled_local_capture_recovers_on_the_same_facade'
 if name=='registeredMixinSourcesPreferJdtAndTrackSavedDeletedFiles':entry['statefulAssertion']='mixin_facade_legacy.rs::registered_source_changes_use_real_files_and_live_host_facts'
 if name=='mixinClassAndInjectMethodCompletionUseAuthoritativeSourceQueryWithoutClasspath':entry['authorityAssertion']='mixin_facade_legacy.rs::source_authority_never_initializes_the_binary_delegate'
 ledger.append(entry)
assert len(cases)==len(ledger)==189
for entry in ledger:
 for field in ['rustTest','directAssertion','tailAssertion','statefulAssertion','authorityAssertion','supplementalReplayTest']:
  if field not in entry:continue
  path,function=entry[field].split('::')
  source=ROOT/path if path.startswith('mcdev-core/') else ROOT/'mcdev-core/tests'/path
  assert re.search(r'\bfn\s+'+re.escape(function)+r'\s*\(',source.read_text(encoding='utf8')),(entry['originalId'],field,entry[field])
(HERE/'coverage.json').write_text(json.dumps(ledger,indent=2)+'\n',encoding='utf8')
summary=dict(originalCases=189,coreCases=159,hostCases=30,originalAssertionsPassedOnFrozenJvm=True,successfulApiOutputs=sum(c['calls']for _,c in cases.values()),throwableEvents=sum(e['throwableEvents']for e in ledger),coreFacadeApiEventsInCoreCorpus=sum(1 for values in rows.values()for _,r in values if r['legacyCase'].startswith('mcdev-core/')and r['api'].startswith('CoreMixinServiceFacade.')),coreFacadeApiEventsInHostCorpus=sum(1 for values in rows.values()for _,r in values if r['legacyCase'].startswith('mcdev-jdtls-extension/')and r['api'].startswith('CoreMixinServiceFacade.')),hostFacadeApiEvents=sum(1 for v in rows.values()for _,r in v if r['api'].startswith('HostMixinServiceFacade.')),kotlinCompositeNavigationTailOutputs=sum(1 for v in rows.values()for _,r in v if r['legacyCase'].startswith('mcdev-core/')and r['api']=='MixinDefinitionService.definitionsAtOffset'),hostNavigationOffsetOutputs=sum(1 for v in rows.values()for _,r in v if r['legacyCase'].startswith('mcdev-jdtls-extension/')and r['api']=='MixinDefinitionService.definitionsAtOffset'),appliedFixOutputs=sum(1 for v in rows.values()for _,r in v if r['api'].startswith('MixinCodeActionService.apply')),additionalOfficialUtf16Outputs=30,normalizationCases=normalizations,wireCapabilities='Host source/semantic SDK wire capabilities remain separately pending; fixtures and native library tests provide complete owned host facts, not zero-filled unknown generations.')
(HERE/'coverage-summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf8');print(json.dumps(summary),flush=True)
