"""One-time faithful javac21 fixture export. Rust test execution needs no JVM."""
from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path(__file__).resolve().parents[3]
BASE=ROOT/'build/migration-baseline/current-20261023'
SOURCES=BASE/'mcdev-core/src/test/resources/bytecode/fixtures'
OUT=ROOT/'build/bytecode-fixture-capture';OUT.mkdir(parents=True,exist_ok=True)
CLASSES=OUT/'classes';CLASSES.mkdir(exist_ok=True)
sources=sorted(SOURCES.glob('*.java'))
assert len(sources)==9
with(OUT/'javac.stdout.log').open('w',encoding='utf-8')as stdout,(OUT/'javac.stderr.log').open('w',encoding='utf-8')as stderr:
    result=subprocess.run(['C:/Program Files/Java/jdk-21/bin/javac.exe','-proc:none','--release','21','-d',str(CLASSES)]+[str(path)for path in sources],stdout=stdout,stderr=stderr,timeout=60,creationflags=subprocess.CREATE_NO_WINDOW if os.name=='nt' else 0)
    assert result.returncode==0,'javac failed: see '+str(OUT)
classes={path.relative_to(CLASSES).as_posix().removesuffix('.class'):list(path.read_bytes())for path in sorted(CLASSES.rglob('*.class'))}
simple=BASE/'mcdev-test-fixtures/build/generated/fixture-classes/com/example/target/SimpleTarget.class'
classes['com/example/target/SimpleTarget']=list(simple.read_bytes())
dest=Path(__file__).with_name('bytecode_classes.json');dest.write_text(json.dumps(classes,separators=(',',':')),encoding='utf-8')
manifest={'compiler':'JDK21 --release 21; same legacy BytecodeFixtureCompiler flags; -proc:none suppresses unrelated annotation processors','sources':{path.name:hashlib.sha256(path.read_bytes()).hexdigest()for path in sources},'classes':{name:hashlib.sha256(bytes(data)).hexdigest()for name,data in classes.items()},'simpleTargetSource':str(simple.relative_to(BASE))}
dest.with_name('bytecode_classes.provenance.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print('legacy fixture classes='+str(len(classes))+'; output='+str(dest))
