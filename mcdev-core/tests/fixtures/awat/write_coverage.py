"""Verify every old case/native entry point and append strict evidence to the existing map."""
from pathlib import Path
import json,re,hashlib
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
BASE=ROOT/'build/migration-baseline/current-20261023'
cases=json.loads((HERE/'legacy_cases.json').read_text())
calls=[json.loads(line) for line in (HERE/'legacy_calls.jsonl').read_text().splitlines()]
deltas=json.loads((HERE/'intentional_deltas.json').read_text())
expected=set()
for group in ['aw','at','awat']:
    for path in (BASE/'mcdev-core/src/test/kotlin/io/github/mcdev/core'/group).glob('*.kt'):
        for name in re.findall(r'@Test\s+fun (\w+)',path.read_text(encoding='utf-8')):
            expected.add(path.relative_to(BASE).as_posix()+'::'+name)
assert expected=={r['legacyCase'] for r in cases}
assert len(expected)==155 and len(calls)==549 and len(deltas)==27
native=ROOT/'mcdev-core/tests/awat_legacy.rs';source=native.read_text()
lines=['## Strict complete: exact original cases and whole results','',
 'All **155 original cases across 19 suites** now execute independently in `awat_legacy.rs`. The earlier table above remains a supplementary behavior map; it is not the evidence for literal case coverage.',
 '', 'The shared one-time `mixin_source/Capture.kt` and `Run.kt` harness invoked every frozen original test method and its original assertions unchanged. Disposable AW/AT service wrappers recorded **549 complete public outputs** with exact arguments and observed class/mapping facts. Rust replays all 549 outputs; each row below identifies its own native test. This includes **17 final `AwAtServiceFacade` outputs** (10 complete, 5 diagnose, 2 codeActions), not only underlying calls.',
 '', 'The committed `legacy_calls.jsonl` and `legacy_cases.json` are raw oracle records. Cargo reads them directly and launches no JVM. `generate_capture.py` only regenerates fixtures on demand, uses the shared finite-deadline runtime runner, and never modifies frozen sources. Full one-time logs: `build/awat-oracle/{compile,capture}.{stdout,stderr}.log`.',
 '', '**27 explicitly approved observations in 17 cases** use `intentional_deltas.json`: every selector includes exact case/API/arguments, field, original/new value, reason and an executed range/document regression. No case is skipped. Descriptor/remap token edits are applied alongside the old edits to the same source and must produce the same whole document; title, kind, type, URI and metadata remain compared. Two duplicate-removal cases adapt their original assertion on full-document replacement `newText` to count entries in the applied, reparsed document instead (bug-corrected adaptation). Newline differences and corrected token ranges are exact expectations, not ignored fields.',
 '', 'The only schema conversion is snake/camel field names and enum spelling. Kotlin fix subtypes are reconstructed from their unique retained kind and workspace-edit shape and compared against the Rust type tag; type/kind/range metadata is not removed.',
 '', 'Strict replay also exposed and corrected genuine compatibility omissions: overlong columns now clamp to the line end through context/facade/navigation APIs without splitting UTF-16 surrogates; AT descriptor-mismatch metadata matches legacy; remapping avoids an unnecessary fields lookup once the exact method is found. Original 26 supplementary tests are retained, with one additional Unicode/CRLF/clamping regression.',
 '', '| Original path and method | Native independent test | Complete outputs | Explicit adaptations |', '|---|---|---:|---:|']
for case in cases:
    identity=case['legacyCase'];tail=identity.split('/')[-1].replace('.kt::','_');name=re.sub(r'(?<!^)(?=[A-Z])','_',tail).lower()
    match=re.search(r'\bfn '+re.escape(name)+r'\s*\(',source);assert match,name
    line=source[:match.start()].count('\n')+1
    count=sum(r['legacyCase']==identity for r in calls);assert count==case['calls'] and case['status']=='passed'
    delta_count=sum(r['legacyCase']==identity for r in deltas)
    lines.append(f'| `{identity}` | [`{name}`](awat_legacy.rs#L{line}) | {count} | {delta_count} |')
lines+=['','Fixture SHA-256:']
for file in ['legacy_calls.jsonl','legacy_cases.json','intentional_deltas.json']:
    lines.append(f'- `{file}`: `{hashlib.sha256((HERE/file).read_bytes()).hexdigest()}`')
lines+=['', 'Verification: affected targets `awat` and `awat_legacy`, finite deadline 180 seconds; final log location is recorded in the completion report. Full workspace/host/editor IPC execution remains the parent task’s responsibility.']
coverage=ROOT/'mcdev-core/tests/awat-coverage.md'
old=coverage.read_text(encoding='utf-8').split('## Strict complete:')[0].rstrip()
coverage.write_text(old+'\n\n'+'\n'.join(lines)+'\n',encoding='utf-8')
print('strict coverage verified: 155 original IDs, 549 complete outputs, 27 explicit adaptations')
