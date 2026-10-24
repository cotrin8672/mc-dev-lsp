"""Preserve all original primary resources by SHA, reusing existing exact copies."""
from pathlib import Path
import hashlib,json,re,shutil,os
HERE=Path(__file__).resolve().parent;CORE=HERE.parents[2];ROOT=CORE.parent
BASE=ROOT/"build/migration-baseline/current-20261023"
source=BASE/"mcdev-test-fixtures/src/main/resources/fixtures"
files={"fixtures/"+p.relative_to(source).as_posix():p for p in source.rglob("*")if p.is_file()}
files["fixtures/shared/classes/com/example/target/SimpleTarget.class"]=BASE/"mcdev-test-fixtures/build/generated/fixture-classes/com/example/target/SimpleTarget.class"
assert len(files)==40,len(files) # 39 primary resources plus one generated class.
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
existing={}
for path in (CORE/"tests/fixtures").rglob("*"):
 if path.is_file():existing.setdefault(sha(path),path)
constants={}
paths=(BASE/"mcdev-test-fixtures/src/main/kotlin/io/github/mcdev/fixtures/FixturePaths.kt").read_text(encoding="utf-8")
for key,value in re.findall(r'const val (\w+) = "([^"]*)"',paths):
 value=re.sub(r"\$(\w+)",lambda m:constants[m[1]],value);constants[key]=value
required=[constants[name]for name in re.search(r"val REQUIRED_RESOURCES = listOf\((.*?)\n    \)",paths,re.S)[1].replace(","," ").split()]
manifest=[];reused=0;stored=0;generated=[]
for resource,path in sorted(files.items()):
 digest=sha(path);preferred=CORE/"tests/fixtures/mixin_completion/project_files"/resource.removeprefix("fixtures/")
 if preferred.is_file()and sha(preferred)==digest:destination=preferred;reused+=1
 elif digest in existing:destination=existing[digest];reused+=1
 else:
  destination=HERE/"primary"/resource.removeprefix("fixtures/");destination.parent.mkdir(parents=True,exist_ok=True)
  if destination.suffix==".class":destination=destination.with_suffix(".class.bin")
  shutil.copyfile(path,destination);existing[digest]=destination;stored+=1
 assert sha(destination)==digest
 manifest.append(dict(resource=resource,rustPath=destination.relative_to(CORE).as_posix(),sha256=digest,bytes=path.stat().st_size,required=resource in required,baselineSource=path.relative_to(BASE).as_posix()))
 relative=os.path.relpath(destination,HERE).replace("\\","/")
 generated.append('    ('+json.dumps(resource)+',include_bytes!('+json.dumps(relative)+')),' )
(HERE/"manifest.json").write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf-8")
(HERE/"catalog.rs").write_text('pub const RESOURCES:&[(&str,&[u8])]=&[\n'+"\n".join(generated)+'\n];\n',encoding="utf-8")
inputs=json.loads((ROOT/"docs/rust-test-migration.json").read_text(encoding="utf-8"))["fixtures"]
assert len(inputs)==49
source_manifest=[]
for row in inputs:
 path=BASE/row["source"];digest=sha(path);assert digest.upper()==row["sha256"].upper(),row["source"]
 if digest in existing:destination=existing[digest]
 else:
  destination=HERE/"provenance"/row["source"];destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,destination);existing[digest]=destination;stored+=1
 assert sha(destination)==digest
 source_manifest.append(dict(source=row["source"],sha256=digest.upper(),replacement=destination.relative_to(ROOT).as_posix(),status="preserved"))
(HERE/"source_manifest.json").write_text(json.dumps(source_manifest,indent=2)+"\n",encoding="utf-8")
(HERE/"derived_class_provenance.json").write_text(json.dumps(dict(resource="fixtures/shared/classes/com/example/target/SimpleTarget.class",baselineClass="mcdev-test-fixtures/build/generated/fixture-classes/com/example/target/SimpleTarget.class",inputSource="mcdev-test-fixtures/src/fixture-java/com/example/target/SimpleTarget.java",inputSha256=sha(BASE/"mcdev-test-fixtures/src/fixture-java/com/example/target/SimpleTarget.java"),classSha256=sha(files["fixtures/shared/classes/com/example/target/SimpleTarget.class"]),runtimeRequiresJava=False),indent=2)+"\n",encoding="utf-8")
print("primary resources",len(manifest),"input fixtures",len(source_manifest),"reused",reused,"newly stored",stored,"required",len(required))
