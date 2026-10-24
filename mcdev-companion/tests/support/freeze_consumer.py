"""Receipt helper for the immutable real-editor consumer proof (test-only)."""
import hashlib,json,pathlib,shutil,sys
ROOT=pathlib.Path(__file__).resolve().parents[3]
def sources():
    paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock']
    for crate in ['mcdev-core','mcdev-protocol','mcdev-companion']:
        paths.append(ROOT/crate/'Cargo.toml')
        paths.extend(p for p in (ROOT/crate/'src').rglob('*') if p.is_file())
    return {p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}
def main():
    mode,destination=sys.argv[1:3];destination=(ROOT/destination).resolve()
    assert destination.parent==(ROOT/'build/companion-artifacts').resolve()
    if mode=='prepare':
        destination.mkdir(parents=True,exist_ok=False)
        (destination/'before-source-hashes.json').write_text(json.dumps(sources(),indent=2)+'\n',encoding='utf-8')
    elif mode=='finalize':
        before=json.loads((destination/'before-source-hashes.json').read_text(encoding='utf-8'))
        after=sources()
        assert before==after, f'sources changed during build: {[p for p in before.keys()|after.keys() if before.get(p)!=after.get(p)]}'
        binary=destination/'mcdev-companion.exe';shutil.copyfile(ROOT/'target/debug/mcdev-companion.exe',binary)
        receipt={'executable':str(binary),'sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'sourceHashes':after,'stockHostCheckpoint':'f5953350928eb8af1ab34bb39db23ba7370d14af','protocol':{'major':1,'minor':0,'capabilities':['semantic-v1']},'fix':'configuration is opaque JSON; stock f595 [] is not an invalid evaluation','sourceHashesUnchangedDuringBuild':True}
        (destination/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
        print(json.dumps({'executable':str(binary),'sha256':receipt['sha256'],'sources':len(after)}))
    else:raise ValueError(mode)
if __name__=='__main__':main()
