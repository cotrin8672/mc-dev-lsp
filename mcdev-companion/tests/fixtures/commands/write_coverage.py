"""Derive the caller ledger from frozen IDs and a completed native test log."""
from pathlib import Path
import collections,hashlib,json,re,sys
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
cases=json.loads((HERE/'original_cases.json').read_text())
rows=[json.loads(s) for s in (HERE/'legacy_calls.jsonl').read_text().splitlines()]
source=(HERE.parents[1]/'commands_legacy.rs').read_text()
functions=dict((identity,function) for function,identity in re.findall(r'fn (\w+)\(\s*\)\s*\{\s*replay::replay_case\(\s*"([^"]+)"\s*,?\s*\)',source))
assert set(functions)=={c['legacyCase'] for c in cases}, (len(functions), sorted({c['legacyCase'] for c in cases}-set(functions)))
log=Path(sys.argv[1]).resolve()
text=log.read_text(encoding='utf-8')
assert 'test result: ok.' in text and 'test result: FAILED.' not in text
ledger=[]
for case in cases:
 identity=case['legacyCase'];function=functions[identity]
 assert re.search(r'^test '+re.escape(function)+r' \.\.\. ok$',text,re.M),function
 strategy='whole top-handler DTO + exact host callback sequence'
 if '/transport/' in identity:strategy='real authenticated TCP requests, cancellation and deadline assertions'
 elif '/stdio/' in identity:strategy='real Rust process: original five requests, recovery and clean EOF'
 elif 'CompletionTruncationTest' in identity:strategy='original capped-source table and wire assertions'
 elif identity.endswith('registryListsImplementedCommands'):strategy='exact original ordered eleven-command registry'
 ledger.append({**{k:v for k,v in case.items() if k!='status'},'nativeFunction':function,'strategy':strategy,'topLevelObservations':sum(r['legacyCase']==identity and r['frameDepth']==1 for r in rows),'status':'passed'})
receipt={'nativeLog':str(log.relative_to(ROOT)).replace('\\','/'),'originalCaseCount':len(cases),'nativeCaseCount':len(ledger),'captureSha256':hashlib.sha256((HERE/'legacy_calls.jsonl').read_bytes()).hexdigest(),'caseSet':'exact, independently checked by manifest_is_the_exact_frozen_executable_case_set','cases':ledger}
(HERE/'coverage.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(f'original/native exact case set: {len(ledger)}; all native functions passed; ledger: {HERE/"coverage.json"}')
