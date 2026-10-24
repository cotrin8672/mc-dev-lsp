"""One-time provenance capture. Cargo never executes Java or explores a classpath."""
import hashlib, importlib.util, json, pathlib, subprocess, re, textwrap

ROOT = pathlib.Path(__file__).resolve().parents[4]
HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('source_capture', ROOT/'mcdev-core/tests/fixtures/mixin_source/generate_capture.py')
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

def main():
    runtime = helper.prepare_runtime()
    baseline = pathlib.Path(runtime['baseline'])
    out = ROOT/'build/host-project-oracle'
    out.mkdir(parents=True, exist_ok=True)
    cp = ';'.join([str(out), str(baseline/'mcdev-jdtls-extension/build/classes/kotlin/main'), str(baseline/'mcdev-jdtls-extension/build/classes/kotlin/test'), runtime['classpath']])
    javac = pathlib.Path(runtime['java']).with_name('javac.exe')
    result = subprocess.run([str(javac), '-encoding', 'UTF-8', '-cp', cp, '-d', str(out), str(HERE/'HostProjectOracle.java')], capture_output=True, timeout=60)
    (out/'javac.stdout.log').write_bytes(result.stdout)
    (out/'javac.stderr.log').write_bytes(result.stderr)
    result.check_returncode()
    helper.run_java(runtime, cp, 'HostProjectOracle', [str(HERE), str(out/'original-assertions.json')], 'host-project-original', 240)
    (HERE/'original_assertions.json').write_bytes((out/'original-assertions.json').read_bytes())
    helper.run_java(runtime, cp, 'HostProjectOracle', [str(HERE), str(out/'candidate_outputs.json'), 'outputs'], 'host-project-candidates', 60)
    (HERE/'candidate_outputs.json').write_bytes((out/'candidate_outputs.json').read_bytes())
    manifest = []
    for file in sorted((HERE/'classes').glob('**/*.bin')):
        manifest.append({'path':file.relative_to(HERE).as_posix(),'sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'bytes':file.stat().st_size})
    (HERE/'class_manifest.json').write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8')
    inputs=[]
    for name in ['BytecodeIndexAdapterTest','LazyClasspathClassIndexTest','McdevProjectSessionTest']:
        relative=f'mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/project/{name}.kt'
        raw=(baseline/relative).read_bytes()
        blocks=[textwrap.dedent(m.group(1)).strip()+'\n' for m in re.finditer(r'"""(.*?)"""\.trimIndent\(\)',raw.decode('utf-8'),re.S) if 'package ' in m.group(1)]
        inputs.append({'source':relative,'sha256':hashlib.sha256(raw).hexdigest(),'javaSourceBlocks':blocks})
    (HERE/'source_inputs.json').write_text(json.dumps(inputs,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    print(json.dumps({'classFixtures':len(manifest),'originalAssertions':str(out/'original-assertions.json')}))

if __name__ == '__main__': main()
