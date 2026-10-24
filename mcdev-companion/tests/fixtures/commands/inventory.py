"""Frozen command caller IDs; generated independently of Rust implementations."""
from pathlib import Path
import hashlib,json,re
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
BASE=ROOT/'build/migration-baseline/current-20261023'
TEST=BASE/'mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls'
PATHS=[TEST/'command/McdevCommandDispatcherTest.kt',TEST/'McdevDelegateCommandHandlerTest.kt',*sorted((TEST/'handler').glob('*.kt')),TEST/'convert/CompletionTruncationTest.kt',TEST/'transport/McdevCompletionTransportTest.kt',TEST/'stdio/McdevStdioMainTest.kt']
def main():
    rows=[]
    for path in PATHS:
        source=path.read_text(encoding='utf-8')
        for case in re.findall(r'@Test\s+fun (\w+)',source):
            rows.append({'legacyCase':path.relative_to(BASE).as_posix()+'::'+case,'legacyClass':'io.github.mcdev.jdtls.'+path.relative_to(TEST).as_posix().removesuffix('.kt').replace('/','.'),'sourceSha256':hashlib.sha256(path.read_bytes()).hexdigest(),'status':'pending'})
    assert len(rows)==127
    ledger=(ROOT/'docs/rust-migration-inventory.md').read_text(encoding='utf-8')
    assert all('`'+r['legacyCase'].split('::')[-1]+'`' in ledger for r in rows)
    (HERE/'original_cases.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
    print(f'original command caller cases={len(rows)}; classes={len(PATHS)}; all exact IDs present in master ledger')
if __name__=='__main__':main()
