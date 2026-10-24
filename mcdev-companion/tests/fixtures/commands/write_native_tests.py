"""Keep every frozen caller ID executable; timing predicates describe measurements only."""
from pathlib import Path
import json,re
HERE=Path(__file__).resolve().parent
cases=json.loads((HERE/'original_cases.json').read_text())
rows=[json.loads(line)for line in (HERE/'legacy_calls.jsonl').read_text().splitlines()]
tests=['#[path="fixtures/commands/replay.rs"] mod replay;']
for case in cases:
    identity=case['legacyCase'];name=identity.split('/')[-1].replace('.kt::','_');name=re.sub(r'(?<!^)(?=[A-Z])','_',name).lower()
    tests.append('#[test]\nfn '+name+'(){replay::replay_case('+json.dumps(identity)+');}')
(HERE.parents[1]/'commands_legacy.rs').write_text('\n\n'.join(tests)+'\n',encoding='utf-8')
timing=[]
for index,row in enumerate(rows):
    if row.get('frameDepth')!=1 or not isinstance(row.get('output'),dict):continue
    result=row['output'].get('result')
    if not isinstance(result,dict)or not isinstance(result.get('debug'),dict):continue
    debug=result['debug'];keys=['totalMs']
    if not debug.get('negativeCacheHit'):keys.append('dtoConvertMs')
    if debug.get('semanticCacheHit')is not None:keys+=['loadSessionMs','documentSnapshotMs']
    for key in keys:
        if debug.get(key)is not None:
            assert isinstance(debug[key],int)and debug[key]>=0
            timing.append({'legacyCase':row['legacyCase'],'observation':index,'api':row['api'],'field':'/result/debug/'+key,'originalMeasurement':debug[key],'assertion':'integer >= 0','reason':'Elapsed duration is measured, not a deterministic semantic value. All cache flags, forwarded metrics and constant-zero branch fields stay exact.'})
(HERE/'timing_assertions.json').write_text(json.dumps(timing,indent=2)+'\n',encoding='utf-8')
print(f'executable original IDs={len(cases)}; explicit nonnegative-duration assertions={len(timing)}')
