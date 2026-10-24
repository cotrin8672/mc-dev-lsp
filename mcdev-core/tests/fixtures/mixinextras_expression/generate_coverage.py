"""Record an exact original path::case -> executable native assertion mapping."""
from pathlib import Path
import json,re,collections
HERE=Path(__file__).parent
ROOT=HERE.resolve().parents[3]
def snake(name):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'(.)([A-Z][a-z]+)',r'\1_\2',name)).lower()
cases=json.loads((HERE/'cases.json').read_text(encoding='utf8'))
native='mcdev-core/tests/mixinextras_expression_models.rs'
replay='mcdev-core/tests/mixinextras_expression_legacy.rs'
states={'sharedCacheReusesMatcherAcrossDistinctServiceInstancesWhenOnlyMemberPrefixChanges','reusesCachedReachableMembersWhenOnlyMemberPrefixChanges','filtersCachedCandidatesByMemberPrefixWithoutReRunningMatcher','cacheMissesWhenReceiverExpressionChanges','cacheMissesWhenClassBytesContentChanges','cacheMissesWhenDefinitionSemanticContentChanges','cacheMissesWhenTargetMethodOrContextTypeChanges','doesNotCacheUnavailableOrCancelledMatcherResults','evictsOldestCacheEntryAndReRunsMatcherOnCapacityMiss'}
rows=[]
for case in cases:
 path,name=case['legacyCase'].split('::');owner=Path(path).stem;direct=[]
 if owner in {'ClassLiteralTypeNameResolverTest','MixinExtrasAnnotationContextTest'}:direct.append((native,snake(name)))
 if owner=='MixinExtrasDefinitionIndexTest':direct.append((native,{'emptyIndexHasNoDefinitions':'definition_empty_index_has_no_definitions','preservesDocumentOrder':'definition_preserves_document_order','definitionsWithIdReturnsAllMatchesInDocumentOrder':'definitions_with_id_returns_all_matches_in_document_order'}[name]))
 if owner=='MixinExtrasExpressionIndexTest':direct.append((native,{'emptyIndexHasNoExpressions':'expression_empty_index_has_no_expressions','preservesDocumentOrder':'expression_preserves_document_order'}.get(name,snake(name))))
 if owner=='DefinitionAnnotationParseIssueTest' and name in {'mapsParseIssueToSourceAndKeepsValidIdentifierBinding','malformedIdProducesOneParseIssueInsteadOfDuplicateMissingIdIssues'}:direct.append((native,snake(name)))
 if name in states:direct.append((replay,'original_cache_transitions_check_native_state_and_callback_counts_after_every_call#'+name))
 if name=='propagatesCancellation':direct.append((replay,'original_cancellation_marker_propagates_by_identity'))
 if owner=='ExpressionTypeImportCacheTest':direct.append((replay,'changing_import_recomputes_bindings_but_unchanged_import_reuses_candidates'))
 if name=='officialMatcherInvokesCommonSuperClassResolverForBranchMergeFixture':direct.append((replay,'official_matcher_invokes_common_super_class_resolver_for_branch_merge_fixture'))
 if name=='completeExpressionValueKeywordAndDefinitionIdCompletionDoesNotInvokeMemberService':direct.append((replay,'keyword_and_definition_id_completion_does_not_invoke_member_matcher'))
 if owner=='KotlinExpressionContextTest' and name=='aliasImportsReachHandlerRegionAndKeepDefinitionsAndSourceRanges':direct.append((replay,'kotlin_definition_context_retains_valid_pool_binding_and_slices_original_ids'))
 for file,function in direct:
  assert re.search(r'\bfn\s+'+re.escape(function.split('#')[0])+r'\b',(ROOT/file).read_text(encoding='utf8')),(file,function)
 assert case['status']=='passed'
 assert case['calls'] or direct,case['legacyCase']
 rows.append({'legacyCase':case['legacyCase'],'oracleStatus':case['status'],'capturedCalls':case['calls'],'wholeOutputReplay':replay+'::every_original_expression_case_and_full_output#'+case['legacyCase'] if case['calls'] else None,'nativeStateOrHelperAssertions':[file+'::'+function for file,function in direct],'status':'ported'})
assert len(rows)==316
out={'originalCases':len(rows),'originalAssertionsPassed':True,'capturedSuccessfulCalls':sum(c['calls']for c in cases),'capturedCancellationEvents':2,'cases':rows}
(HERE/'coverage.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf8')
print('exact original cases:',len(rows),'full API calls:',out['capturedSuccessfulCalls'],'direct native assertion entries:',sum(len(r['nativeStateOrHelperAssertions'])for r in rows))
print(dict(collections.Counter(Path(c['legacyCase'].split('::')[0]).stem for c in cases)))
