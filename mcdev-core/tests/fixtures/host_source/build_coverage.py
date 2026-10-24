"""Build the exact 170-case ledger; final status requires a successful native proof."""
from pathlib import Path
import hashlib,json,re,sys,importlib.util
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('host_fixture_capture',HERE/'generate_capture.py')
capture=importlib.util.module_from_spec(spec);spec.loader.exec_module(capture)
def assertions(body):
    result=[];i=0
    while i<len(body):
        literal=capture.c.skip_literal(body,i)
        if literal is not None:i=literal;continue
        if body.startswith('//',i):i=body.find('\n',i)if'\n'in body[i:]else len(body);continue
        if body.startswith('/*',i):i=body.index('*/',i+2)+2;continue
        match=re.match(r'assert\w*\b',body[i:])if i==0 or not(body[i-1].isalnum()or body[i-1]=='_')else None
        if match:
            at=i+match.end()
            while at<len(body)and body[at].isspace():at+=1
            if body[at:at+1]=='<':
                depth=1;at+=1
                while depth:
                    depth+=(body[at]=='<')-(body[at]=='>');at+=1
                while at<len(body)and body[at].isspace():at+=1
            if body[at:at+1]in('(','{'):
                end=capture.c.closing(body,at)if body[at]=='('else capture.commands.closing_brace(body,at)
                result.append({'callee':match[0],'sourceStart':i,'expression':body[i:end+1]})
            i+=match.end()
        else:i+=1
    return result
def snake(s):return re.sub(r'(?<!^)(?=[A-Z])','_',s).lower()
FILES={
 'CompletionIndexCaches':'host_source_completion_cache.rs',
 'JdtAnnotationConstantExtractor':'host_source_legacy.rs',
 'JdtBindingDescriptorConverter':'host_source_types.rs',
 'JdtInjectionPointAtCodeIndex':'host_source_cache.rs',
 'JdtInjectionPointFeatureProbe':'host_source_cache.rs',
 'JdtMixinExtrasResolvedContextExtractor':'host_source_legacy.rs',
 'JdtMixinSemanticModelParser':'host_source_semantic.rs',
 'JdtProjectSourceQuery':'host_source_project_legacy.rs',
 'JdtTypeSignatureConverter':'host_source_types.rs',
 'SemanticModelCache':'host_source_cache.rs',
 'SourceBackedClassIndex':'host_source_query.rs',
}
ALIASES={
 'nullProjectReturnsFalseWithoutThrowing':'null_project_returns_unavailable_without_lookup',
 'missingTypeReturnsFalse':'missing_type_returns_missing',
 'lookupExceptionFailsClosed':'lookup_exception_preserves_failure_instead_of_claiming_absence',
 'equalButDistinctProjectObjectsDoNotShareCache':'equal_names_with_distinct_host_project_identity_do_not_share_cache',
 'missingInjectionPointFailsClosedToEmpty':'missing_injection_point_is_known_empty',
 'nullProjectFailsClosedToEmpty':'null_at_project_preserves_unavailable',
 'reflectiveHierarchyQueryUsesTypeNewTypeHierarchyAndDollarFqn':'normalized_hierarchy_retains_selected_injection_point_descendants',
 'reflectiveHierarchyQueryPrefersDollarSeparatorFqnWithDotFallback':'normalized_hierarchy_preserves_dollar_names_and_explicit_dot_fallback',
 'reflectiveAtCodeReaderMatchesElementNameVariantsViaMemberValuePairs':'at_code_reader_matches_element_name_variants_via_member_value_pairs',
 'reflectiveAtCodeReaderIgnoresNonExistentAnnotations':'at_code_reader_ignores_nonexistent_annotations',
 'reflectiveAtCodeReaderFormatsNamespaceFromMemberValuePairs':'at_code_reader_formats_namespace_from_member_value_pairs',
 'reflectiveEndToEndIndexUsesProductionReadersAgainstJdtShapedFakes':'end_to_end_index_uses_normalized_host_hierarchy_and_annotation_facts',
 'transientJdtFallbackIsRecomputedSoRecoveryCanUseAstModel':'transient_fallback_is_recomputed_so_recovery_can_use_host_normalized_model',
 'canceledComputationLeavesNoEntryForTheNextRequest':'canceled_semantic_computation_leaves_no_entry_for_next_request',
 'compositeProjectSourceQueryPrefersBufferOverJdtStyleDelegate':'composite_prefers_buffer_over_host_delegate',
 'jdtFindTypeFailureFallsBackForExactLookupsButAuthoritativeEmptyStaysEmpty':'host_find_type_failure_falls_back_but_authoritative_empty_stays_empty',
 'compositeKeepsBufferedMembersWhenJdtFindTypeFails':'composite_keeps_buffered_members_when_host_find_type_fails',
 'perKeystrokeOverlayDoesNotRereadUnchangedDiskSources':'per_keystroke_overlay_does_not_reread_unchanged_sources',
 'modifyingOneDiskFileIncrementsOnlyThatFileReadAndParse':'modifying_one_source_document_increments_only_that_file_read_and_projection',
 'deletingDiskSourceRemovesItsClass':'deleting_source_document_removes_its_class',
 'diskCacheStaysBoundedToExplicitMax':'source_snapshot_cache_stays_bounded_to_explicit_max',
 'largeSourceTreeDoesNotFullyRereadUnchangedDiskSourcesOnSecondScan':'large_source_tree_does_not_fully_reread_unchanged_documents_on_second_snapshot',
 'reportsUnderlyingReflectionFailureInsteadOfInvocationWrapper':'typed_provider_failure_preserves_root_cause_instead_of_reflection_wrapper',
 'propagatesCancellationHiddenBehindReflectionWrapper':'typed_cancellation_propagates_through_semantic_cache_without_wrapper_or_entry',
 'fallsBackWithVisibleWarningWhenJdtAstIsUnavailableInUnitTests':'unavailable_host_semantics_fall_back_with_visible_warning',
 'resolveJavaProjectReturnsNullWhenJdtWorkspaceIsUnavailable':'missing_workspace_authority_remains_unavailable_and_has_no_project_handle',
 'mixinTargetsUsesStringConstantInsteadOfStringExpressionBinding':'mixin_targets_use_string_constant_instead_of_string_expression_binding',
 'recoveredTargetBindingUsesExplicitImportAndLaterUsesResolvedBinding':'recovered_target_binding_uses_explicit_import_and_later_resolved_binding',
 'collectorKeepsMixedMixinExtrasWrapWithConditionAndStandardHandlers':'collector_keeps_mixed_mixinextras_wrap_with_condition_and_standard_handlers',
}
TRANSFORMS={
 'CompletionIndexCaches':'Reference delegate identities become stable host index IDs. Owned entries and read sets only; all eight generations and fresh current-evaluation read registration guard reuse. Byte arrays retain content and borrowed-slice identity; no JVM array handles.',
 'JdtAnnotationConstantExtractor':'Fake AST storage maps to typed HostExpressionFact, preserving folded values, width, validity, scalar/array order and null/wrong-type failure. Cancellation is a typed provider error; constant folding belongs to the host.',
 'JdtBindingDescriptorConverter':'Fake binding storage maps to HostTypeFact primitive/class/array/recovered facts; native recursive descriptor projection runs for every original input.',
 'JdtInjectionPointAtCodeIndex':'Host-selected InjectionPoint descendants replace reflection hierarchy traversal. Exact owned FQNs, annotation existence/names/pairs, namespace text, deduplication and ordering run natively. Actual host wire traversal remains pending.',
 'JdtInjectionPointFeatureProbe':'Typed host capability facts replace JDT reflection. Missing remains confirmed absence; null project/unavailable remains Unknown and ordinary failure remains an error, rather than false. Positive/negative generations and concurrent build/retry assertions remain active.',
 'JdtMixinExtrasResolvedContextExtractor':'Fake annotation/constant storage maps to owned resolved facts. Official identity, defaults, duplicates, malformed nodes, fail-soft Definition issues, order and complete typed results run natively; offsets are checked UTF16-to-UTF8.',
 'JdtMixinSemanticModelParser':'JDT_AST success becomes HostNormalized; actual host capability absence supplies an explicit stable/transient reason. Existing MC parser supplies fallback, imports and source-domain semantics. No JDT availability claim or reflection utility is manufactured.',
 'JdtProjectSourceQuery':'Private JDT signatures normalize only in test fixtures. Actual recorded resolveType candidate arrays feed HostTypeFact::Candidates; no expected public descriptor is used as input. Project queries consume typed host facts; thread-local last-query authority becomes an immutable authority on each operation.',
 'JdtTypeSignatureConverter':'All fourteen original private signature strings and resolver outcomes remain in an explicit table. The host supplies normalized type/erasure facts instead of shipping an obsolete JDT signature parser. Unknown types never become Object; type-variable erasure is explicit.',
 'SemanticModelCache':'Successful backend is HostNormalized, transient fallback never caches, typed cancellation leaves no entry. SHA256 plus complete host identity replaces Java String.hashCode/source-only identity; Aa/BB collision and all generation dimensions have native regressions.',
 'SourceBackedClassIndex':'Source precedence/fallback/deduplication execute on actual indices. A host-selected document catalog/revision/declaration provider replaces the disk scanner and buffer language parser. Original per-file read/parse counts map to host document reads/projections; overlay counts, deletion, 1000 documents and bounded project snapshots are asserted.',
}
def main():
    ledger=json.loads((HERE/'coverage.json').read_text())
    calls={prefix:[json.loads(line)for line in (HERE/(prefix+'-calls.jsonl')).read_text().splitlines()]for prefix in ['adapter','project']}
    original=json.loads((HERE/'original-cases.json').read_text());assert len(original)==170 and all(c['status']=='passed'for c in original)
    proof=None
    if len(sys.argv)>1:
        proof_path=ROOT/sys.argv[1];proof=json.loads(proof_path.read_text());assert proof['ExitCode']==0 and not proof['TimedOut'];ledger['nativeProof']={'path':str(proof_path.relative_to(ROOT)).replace('\\','/'),**proof}
    for case in ledger['cases']:
        case['originalAssertionExpressions']=assertions(case['originalCaseBody'])
        case['originalAssertionCount']=len(case['originalAssertionExpressions'])
        cls=case['legacyClass'].rsplit('.',1)[-1].removesuffix('Test');name=case['legacyCase'].split('::')[-1];file=FILES[cls]
        if cls=='JdtAnnotationConstantExtractor':
            if name=='cancelledConstantResolutionPropagatesAndCanRecover':file='host_source_cache.rs';native='cancelled_constant_resolution_propagates_and_can_recover'
            else:native='all_legacy_constant_binding_and_resolved_context_outputs_use_normalized_host_facts'
        elif cls=='JdtMixinExtrasResolvedContextExtractor':native='all_legacy_constant_binding_and_resolved_context_outputs_use_normalized_host_facts'
        elif cls=='JdtTypeSignatureConverter':native='all_fourteen_legacy_private_signatures_have_explicit_normalized_type_facts';case['nativeTableCase']=name
        elif cls=='JdtProjectSourceQuery':
            if name.startswith('prefixToSearch')or name.startswith('sortClassIndex'):native='normalized_search_preserves_both_legacy_prefix_requests_and_sort_order'
            elif name.startswith('requestorType'):native='normalized_host_names_preserve_all_four_requestor_entry_cases';case['nativeTableCase']=name
            elif name.startswith('coversProjectDependencies'):native='search_capability_is_explicit_and_each_operation_keeps_its_own_authority'
            else:native='normalized_host_project_query_replays_every_final_captured_query'
            if name in ['findTypeFailureOnOneThreadDoesNotAffectExactLookupAuthorityOnAnotherThread','missingSourceTypeDoesNotTaintTheNextSuccessfulQuery']:case['additionalNativeTest']='search_capability_is_explicit_and_each_operation_keeps_its_own_authority'
            if name.startswith(('incompleteJdtMethod','incompleteJdtField')):case['additionalNativeTest']='incomplete_normalized_method_and_field_results_preserve_actual_bytecode_fallback'
        elif cls=='JdtInjectionPointAtCodeIndex'and name=='cachesExactlyOnceForSameProjectIdentityAndVersion':native='at_codes_cache_exactly_once_for_same_project_identity_and_version'
        elif cls=='JdtInjectionPointAtCodeIndex'and name=='recomputesWhenProjectSessionVersionChanges':native='at_codes_recompute_when_project_session_version_changes'
        else:native=ALIASES.get(name,snake(name).replace('composite_project_source_query_','composite_'))
        if cls=='SourceBackedClassIndex'and name in ['perKeystrokeOverlayDoesNotRereadUnchangedDiskSources','modifyingOneDiskFileIncrementsOnlyThatFileReadAndParse','deletingDiskSourceRemovesItsClass','diskCacheStaysBoundedToExplicitMax','largeSourceTreeDoesNotFullyRereadUnchangedDiskSourcesOnSecondScan']:file='host_source_snapshot.rs'
        path=ROOT/'mcdev-core/tests'/file;assert re.search(r'fn\s+'+native+r'\s*\(',path.read_text()),(case['legacyCase'],file,native)
        case.update({'status':'native-verified'if proof else'native-awaiting-final-proof','nativePath':'mcdev-core/tests/'+file,'nativeTest':native,'inputTransformation':TRANSFORMS[cls],'originalExecution':'passed'})
        case['captureRows1Based']={prefix:[i+1 for i,row in enumerate(rows)if row['legacyCase']==case['legacyCase']]for prefix,rows in calls.items()}
        assert case['originalCaseBody'].count('{')>=1
    changed=next(row for row in calls['adapter']if row['legacyCase'].endswith('::mapsJdtDefinitionParseIssueToOffendingValueRange')and row['api'].endswith('.extract'))
    old=changed['output']['context']['context']['definitionIndex']['definitions'][0]['parseIssues'][0]['rawValue']
    source=changed['args']['source'];definition=next(a for a in changed['args']['methodDeclaration']['modifierList']if a['typeName']=='Definition');node=definition['members']['method'];start=node['startPosition'];end=start+node['nodeLength'];new=source.encode('utf-16-le')[start*2:end*2].decode('utf-16-le');assert new=='123'
    ledger.update({'legacyCases':170,'originalCaseExecution':'all 170 original bodies passed','nativeCases':170 if proof else 0,'wireIntegration':'pending: actual host source/semantic capabilities are not connected by these fixtures',
        'sourceOffsets':'Owned source fact ranges are checked UTF8 bytes; McTextRange remains UTF16. Old scalar offsets are converted at fixture boundary.',
        'captures':{prefix:{'rows':len(rows),'sha256':hashlib.sha256((HERE/(prefix+'-calls.jsonl')).read_bytes()).hexdigest()}for prefix,rows in calls.items()},
        'bugCorrections':[{'legacyCase':changed['legacyCase'],'api':changed['api'],'field':'context.context.definitionIndex.definitions[0].parseIssues[0].rawValue','old':old,'new':new,'originalValueRangeUtf16':[start,end],'reason':'The old fake IntLiteral inherited Object.toString, leaking an identity address instead of source text. The host supplies the actual immutable source slice. Only this approved case is adapted; all other rawValue/metadata remain exact.','regression':'malformed_definition_issue_retains_actual_host_snapshot_value_text'}],
        'nativeRegressions':['semantic_hash_collision_missing_generation_and_environment_changes_never_reuse_models','positive_and_negative_reuse_registers_current_reads_and_all_generation_dimensions_invalidate','unavailable_empty_results_are_never_cached_as_authoritative_absence','utf16_expression_presence_does_not_emit_a_false_missing_annotation'],
    })
    assert len({c['legacyCase']for c in ledger['cases']})==170
    (HERE/'coverage.json').write_text(json.dumps(ledger,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    print(json.dumps({'cases':170,'originalAssertionExpressions':sum(c['originalAssertionCount']for c in ledger['cases']),'nativeStatus':'verified'if proof else'awaiting-proof','mappedClasses':len(FILES)}))
if __name__=='__main__':main()
