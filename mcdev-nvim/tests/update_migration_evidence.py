"""Attach finite native results to the complete original client inventory."""
from pathlib import Path
import collections, difflib, hashlib, json
import migration_inventory as inventory

ROOT = inventory.ROOT
OUT = ROOT/'mcdev-nvim/tests'
LOGS = ROOT/'build/nvim-test-logs'
names = ['fixture-build','syntax','adapters','helper','teardown-unit','teardown-process']
results = {}
execution = collections.Counter()
sites = collections.Counter()
runtime_sites = collections.defaultdict(collections.Counter)
runtime_counts_by_site = collections.Counter()
for name in names:
    prefix = 'native-client-final-' + name
    result = json.loads((LOGS/(prefix+'.result.json')).read_text(encoding='utf-8-sig'))
    assert result['ExitCode'] == 0 and not result['TimedOut'], result
    result['evidence'] = 'build/nvim-test-logs/' + prefix
    results[name] = result
    assertions = LOGS/(prefix+'.assertions.json')
    if assertions.exists():
        for site, count in json.loads(assertions.read_text(encoding='utf-8')).items():
            if 'mcdev-nvim/tests/' not in site: continue
            site_value = site[site.index('mcdev-nvim/tests/'):]
            site_parts = site_value.rsplit(':', 2)
            path = site_parts[0]
            sites[path] += 1
            execution[path] += count
            runtime_counts_by_site[site_value] += count
            try:
                runtime_sites[path][int(site_parts[1])] += count
            except (IndexError, ValueError):
                raise AssertionError({'invalidAssertionSite': site})
manifest = json.loads((OUT/'migration_inventory.json').read_text(encoding='utf-8'))
if not manifest['fidelity']['strictPassed']:
    raise AssertionError({
        'strictSourceFidelityFailed': manifest['fidelity']['strictFailureFiles'],
    })
loaded = json.loads((LOGS/'native-client-final-adapters.modules.json').read_text(encoding='utf-8'))
expected_modules = {m['replacementPath'].split('/lua/')[1].removesuffix('.lua').replace('/','.') for m in manifest['modules']}
expected_modules = {m.removesuffix('.init') for m in expected_modules}
assert expected_modules <= set(loaded), sorted(expected_modules-set(loaded))
sources = json.loads((OUT/'legacy_sources.json').read_text(encoding='utf-8'))
deltas = []

BACKEND_ONLY_FILES = {
    'mcdev-nvim/tests/jdtls_workspace.lua',
    'mcdev-nvim/tests/run.lua',
    'mcdev-nvim/lua/mcdev/jdtls.lua',
}


def backend_reason(source, call):
    lower = call.lower()
    if source in BACKEND_ONLY_FILES:
        return 'Native ktlsp/companion routing replaces the original JDT launcher or workspace provider; retain the exact client-visible observer.'
    if any(marker in lower for marker in (
        'extension_jar', 'jdtls_helper', 'jdtls_workspace', 'bundles',
        'usedcompilationunit', 'usedjavaproject', 'jdt_ast', 'jdt://',
        'jdtls_cmd', 'nvim-jdtls', 'jarlocation', 'bundle_jar',
        'java/classfilecontents', 'resolution',
    )):
        return 'Native executable/provider metadata replaces the JVM/JAR/JDT operation; retain declaration identity, text/range, routing, and lifecycle observations.'
    if '/e2e/' in source:
        return 'This original editor-host operation remains pending real MC host observation; no client-only equivalent is claimed as an E2E pass.'
    return None


def observer_for(source, replacement, item, counterparts, status):
    if counterparts:
        ids = ', '.join(
            f"{replacement}::assert@{candidate['line']}" for candidate in counterparts
        )
        return (
            f"Observe the replacement assertion site(s) {ids} and preserve the original "
            f"input/result relationship for {source}::assert@{item['line']}."
        )
    if '/e2e/' in source:
        return (
            'Pending real indexed editor/MC host observation: execute the original input '
            'against the native host and compare the original expected result; this record '
            'does not claim a client-only pass.'
        )
    return (
        'No direct replacement assertion exists; the native host must expose the original '
        'observable result through the scoped command/session contract before this case can pass.'
    )


def assertion_id(path, item):
    return f"{path}::assert@{item['line']}"


def current_line_for_equal(op, old_line):
    tag, old_start, old_end, new_start, new_end = op
    if tag == 'equal' and old_start <= old_line - 1 < old_end:
        return new_start + (old_line - 1 - old_start) + 1
    return None


def replacement_hunk_for(opcodes, old_line, old_call, current_assertions):
    for op in opcodes:
        tag, old_start, old_end, new_start, new_end = op
        if not (old_start <= old_line - 1 < old_end):
            continue
        if tag == 'equal':
            return []
        candidates = [
            candidate for candidate in current_assertions
            if new_start <= candidate['line'] - 1 < new_end
        ]
        if not candidates:
            return []
        expected = inventory.normalize(inventory.backend_equivalent(old_call))
        return [max(
            candidates,
            key=lambda candidate: (
                difflib.SequenceMatcher(
                    None,
                    expected,
                    inventory.normalize(inventory.backend_equivalent(candidate['call'])),
                    autojunk=False,
                ).ratio(),
                -abs(candidate['line'] - (new_start + 1)),
            ),
        )]
    return []


def runtime_body_counts(site_counts, start_line, end_line):
    return {
        str(line): count
        for line, count in sorted(site_counts.items())
        if start_line < line <= end_line and count > 0
    }


def loop_execution(source, blocks, site_counts):
    observed = []
    for block in blocks:
        body_counts = runtime_body_counts(
            site_counts,
            block['line'],
            block.get('bodyEndLine', block.get('endLine', block['line'])),
        )
        expected_rows = len(block.get('rows', []))
        aggregate_calls = sum(body_counts.values())
        lower_bound = max(body_counts.values()) if body_counts else None
        inferred_iterations = None
        inference_basis = 'No row-level loop counter or input identity was logged.'
        if expected_rows and aggregate_calls and aggregate_calls % expected_rows == 0:
            inferred_iterations = expected_rows
            inference_basis = (
                f'Static table has {expected_rows} rows and the aggregate body '
                f'assertion count is {aggregate_calls}; this is an inference, '
                'not row-level observation.'
            )
        if source == 'mcdev-nvim/tests/blink_routing.lua' and block['line'] == 142:
            assert body_counts == {'245': 8, '247': 4}, body_counts
            assert aggregate_calls == 12
            assert inferred_iterations == 12
        observed.append({
            'line': block['line'],
            'tableEndLine': block.get('endLine'),
            'bodyEndLine': block.get('bodyEndLine', block.get('endLine')),
            'inputRows': [
                {
                    'index': index + 1,
                    'input': row,
                    'rowObservation': 'unknown',
                }
                for index, row in enumerate(block.get('rows', []))
            ],
            'observedLoopIterations': None,
            'inferredLoopIterations': inferred_iterations,
            'expectedLoopIterations': expected_rows,
            'allRowsObserved': None,
            'allRowsInferred': inferred_iterations == expected_rows if inferred_iterations is not None else None,
            'runtimeAssertionCounts': body_counts,
            'aggregateAssertionCalls': aggregate_calls,
            'runtimeIterationLowerBound': lower_bound,
            'rowAttribution': 'unknown',
            'observer': (
                'Per-site counters are aggregate over the loop body. No row-specific '
                'pass count is claimed.'
            ),
            'inferenceBasis': inference_basis,
        })
    return observed


def case_execution(cases, site_counts):
    observed = []
    for item in cases:
        end_line = item['line'] + item['call'].count('\n')
        body_counts = runtime_body_counts(site_counts, item['line'] - 1, end_line)
        observed.append({
            'line': item['line'],
            'input': item['call'].split('\n', 1)[0],
            'runtimeAssertionCounts': body_counts,
            'aggregateAssertionCalls': sum(body_counts.values()),
            'rowAttribution': 'single direct case body; no additional input counter',
        })
    return observed


for file in manifest['files']:
    source = file['legacyPath']
    if file['classification']=='unit':
        status = 'passed'
        evidence = results['adapters']['evidence']
    elif 'teardown_process_test' in source or source.endswith('.java'):
        status = 'passed'
        evidence = results['teardown-process']['evidence']
    elif 'teardown_test.lua' in source:
        status = 'passed'
        evidence = results['teardown-unit']['evidence']
    elif source.endswith('tests/test_helpers.lua') or source.endswith('tests/minimal_init.lua'):
        status = 'support exercised'
        evidence = results['adapters']['evidence']
    elif source.endswith('run_bundle_e2e.lua'):
        status = 'teardown exercised; editor workflows pending host bridge'
        evidence = results['teardown-process']['evidence']
    else:
        status = 'pending real host E2E'
        evidence = results['syntax']['evidence']
    file['nativeStatus'] = status
    file['nativeEvidence'] = evidence
    file['executedAssertionCalls'] = execution[source]
    file['executedAssertionSites'] = sites[source]
    file['nativeLoopExecution'] = loop_execution(
        source,
        file.get('loopInputs', []),
        runtime_sites[source],
    )
    file['nativeCaseExecution'] = case_execution(
        file.get('caseEntryCalls', []),
        runtime_sites[source],
    )
    old = sources[source]
    new = (ROOT/file['replacementPath']).read_text(encoding='utf-8')
    new_assertions = inventory.calls(new) if source.endswith('.lua') else []
    new_by_line = {item['line']: item for item in new_assertions}
    new_by_call = collections.defaultdict(list)
    for candidate in new_assertions:
        new_by_call[inventory.normalize(candidate['call'])].append(candidate)
    opcodes = difflib.SequenceMatcher(None,old.splitlines(),new.splitlines(),autojunk=False).get_opcodes()
    for assertion in file['assertionSites']:
        call = assertion['call']
        if 'extension_jar' in call or 'jdtls_helper' in call or 'jdtls_workspace' in source or 'bundles' in call:
            reason = 'Authorized removal of JVM/JAR/Mason/-data workflow; observe explicit argv, idempotent provider merge and workspace isolation instead.'
        elif 'usedCompilationUnit' in call or 'usedJavaProject' in call or 'JDT_AST' in call:
            reason = 'Use HOST_NORMALIZED, HIGH confidence and an authoritative projectSessionVersion; never claim JDT object use.'
        elif 'jarLocation' in call or 'bundle_jar' in call or 'jdtls_cmd' in call or 'nvim-jdtls' in call:
            reason = 'Native executable availability/artifact identity replaces a JVM extension artifact and JDT launcher.'
        elif 'jdt://' in call or 'resolution' in call or 'java/classFileContents' in call:
            reason = 'Existing generic selected-source file URI/read replaces JDT virtual classfile source; same declaration identity/text/range remains required.'
        elif 'jdt' in call.lower() or 'JDT' in call:
            reason = 'Host-neutral client naming or typed native metadata; original semantic effect and routing/cancellation assertion retained. Unavailable real host operations stay pending.'
        else:
            reason = backend_reason(source, call)

        old_normalized = inventory.normalize(call)
        mapped_line = next(
            current_line_for_equal(op, assertion['line']) for op in opcodes
            if current_line_for_equal(op, assertion['line']) is not None
        ) if any(current_line_for_equal(op, assertion['line']) is not None for op in opcodes) else None
        translated = new_by_line.get(mapped_line) if mapped_line is not None else None
        if translated and inventory.normalize(translated['call']) == old_normalized:
            continue

        counterparts = replacement_hunk_for(
            opcodes,
            assertion['line'],
            call,
            new_assertions,
        )
        if not counterparts:
            exact_candidates = new_by_call.get(old_normalized, [])
            if len(exact_candidates) == 1:
                # An exact assertion moved across a replacement hunk. Record no
                # waiver and do not use an unrelated same-call occurrence.
                continue

        if not reason:
            if file['classification'] in ('unit', 'support', 'process-fixture'):
                raise AssertionError({
                    'unmappedNonBackendAssertion': assertion_id(source, assertion),
                    'oldCall': call,
                })
            reason = backend_reason(source, call)
            if not reason:
                raise AssertionError({
                    'unmappedAssertion': assertion_id(source, assertion),
                    'oldCall': call,
                })

        if counterparts:
            mapping = 'explicit replacement-hunk assertion mapping'
        elif source in BACKEND_ONLY_FILES:
            mapping = 'explicit backend assertion removal with host observer'
        else:
            mapping = 'explicit host-pending assertion with required observer'
        delta = {
            'legacyAssertionId': assertion_id(source, assertion),
            'oldCall': call,
            'newAssertionIds': [assertion_id(file['replacementPath'], candidate) for candidate in counterparts],
            'newAssertions': counterparts,
            'mapping': mapping,
            'reason': reason,
            'equivalentObservation': observer_for(source, file['replacementPath'], assertion, counterparts, status),
            'nativeStatus': status,
            'evidence': evidence,
        }
        deltas.append(delta)
summary = {
    'originalTestAndSupportFiles':manifest['legacyFiles'],'originalLuaModules':manifest['legacyModules'],
    'originalStaticAssertionCallSites':manifest['staticAssertionCallSites'],
    'loadedClientModules':loaded,
    'nativeAssertCallsByFile':dict(execution),'nativeAssertCallTotal':sum(execution.values()),
    'nativeAssertionCountsBySite': dict(runtime_counts_by_site),
    'nativeLoopExecutionByFile': {
        file['legacyPath']: file['nativeLoopExecution']
        for file in manifest['files']
        if file.get('nativeLoopExecution')
    },
    'finiteResults':results,
    'independentNativeSuite':'passed',
    'realIndexedEditorE2E':'pending host MC command bridge and Java extract-local',
    'knownHostGaps':'docs/neovim-host-contract.md',
    'caseCountNote':'The old Lua suite has named checks, table loops and script assertions, not a single test-case abstraction. Assertion sites and actual executions are separate counts.',
}
blink = next(
    file for file in manifest['files']
    if file['legacyPath'] == 'mcdev-nvim/tests/blink_routing.lua'
)
summary['nativeKotlinRoutingInputExecution'] = {
    'source': blink['legacyPath'],
    'loops': [
        item for item in blink['nativeLoopExecution']
        if item['line'] >= 350
    ],
    'directCases': [
        item for item in blink['nativeCaseExecution']
        if item['line'] >= 350 and not any(
            loop['line'] < item['line'] <= loop['bodyEndLine']
            for loop in blink['nativeLoopExecution']
        )
    ],
    'inputRowCount': sum(len(item['inputRows']) for item in blink['nativeLoopExecution'] if item['line'] >= 350),
    'inferredInputRowCount': sum(
        len(item['inputRows'])
        for item in blink['nativeLoopExecution']
        if item['line'] >= 350 and item['allRowsInferred'] is True
    ),
    'directCaseCount': sum(
        item['aggregateAssertionCalls'] > 0
        for item in blink['nativeCaseExecution']
        if item['line'] >= 350 and not any(
            loop['line'] < item['line'] <= loop['bodyEndLine']
            for loop in blink['nativeLoopExecution']
        )
    ),
    'strictSource': 'build/migration-baseline/current-20261023/mcdev-nvim/tests/blink_routing.lua',
    'note': 'Runtime assertion counts are per-site aggregates. Input rows retain static fidelity, while row-level pass attribution is unknown; inferred loop counts are explicitly marked as inference.',
}
materialized_result = json.loads((LOGS/'native-materialized-f595.result.json').read_text(encoding='utf-8-sig'))
assert materialized_result['ExitCode'] == 0 and not materialized_result['TimedOut']
materialized = json.loads((LOGS/'materialized-f595/proof.json').read_text(encoding='utf-8'))
assert materialized.get('error') is None and materialized['after'] == materialized['expected']
assert materialized['shutdown'] == {'exited':True,'code':0,'signal':0}
summary['standardHostMaterializedAction'] = {
    **materialized_result,
    'test':'mcdev-nvim/tests/e2e/native_materialized_action.lua',
    'evidence':'build/nvim-test-logs/native-materialized-f595',
    'wholeProof':'build/nvim-test-logs/materialized-f595/proof.json',
    'scope':'Actual standard host action, unique merged materialized effect, complete applied buffer and stale rejection; not MC wire or extract-local proof.',
}
for file in manifest['files']:
    if file['legacyPath'].endswith('jdt_action_aliases_e2e.lua'):
        file['additionalNativeEvidence'] = summary['standardHostMaterializedAction']
        file['nativeStatus'] = 'native materialized action verified; original extract-local workflow pending host implementation'
(OUT/'migration_inventory.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(OUT/'explicit_deltas.json').write_text(json.dumps(deltas,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(OUT/'migration_evidence.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'files':manifest['legacyFiles'],'assertionSites':manifest['staticAssertionCallSites'],
                  'nativeAssertionCalls':sum(execution.values()),'adaptedAssertionSites':len(deltas),
                  'independentSuite':'passed','realIndexedE2E':'pending','seconds':sum(r['Seconds']for r in results.values())},indent=2))
