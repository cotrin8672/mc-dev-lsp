"""Exact original case-to-native-test ledger, with host wiring kept separate."""
from pathlib import Path
import json,re,hashlib,sys
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];BASE=ROOT/"build/migration-baseline/current-20261023"
FINAL="--final"in sys.argv
pending={"definitionCursorOutsideValueReturnsEmpty","definitionLocalTypeAmbiguousWildcardClassLiteralReturnsEmpty","resolvesDefinitionLocalTypeClassLiteral","resolvesDefinitionLocalTypeImplicitJavaLangClassLiteral","resolvesDefinitionLocalTypeNestedClassThroughImport","resolvesDefinitionLocalTypeThroughExplicitImport","semanticMemberDefinitionsOnlyResolveTheExactNameToken"}
records=[]
groups=[("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinDefinitionServiceTest.kt","mixin_navigation_legacy"),("mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinReferenceServiceTest.kt","mixin_navigation_legacy"),("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/definition/DefinitionResolutionServiceTest.kt","mixin_navigation_boundary"),("mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/definition/SourceIndexTest.kt","mixin_navigation_boundary"),("mcdev-test-fixtures/src/test/kotlin/io/github/mcdev/fixtures/FixtureIntegrationTest.kt","fixture_integration")]
for path,target in groups:
 text=(BASE/path).read_text(encoding="utf-8");classes=list(re.finditer(r"(?m)^class\s+(\w+)",text));native=(ROOT/"mcdev-core/tests"/(target+".rs")).read_text(encoding="utf-8")
 for match in re.finditer(r"@Test\s+fun\s+(\w+)",text):
  method=match[1];cls=next(c[1]for c in reversed(classes)if c.start()<match.start());name=re.sub(r"(?<!^)(?=[A-Z])","_",cls+"_"+method).lower();assert re.search(r"\bfn\s+"+name+r"\s*\(",native),(target,name)
  status="pending_source_fidelity"if not FINAL and method in pending else "ported"
  evidence="build/rust-test-logs/fixture-integration-final/fixtures.stdout.log"if target=="fixture_integration"else "build/rust-test-logs/mixin-navigation-identifier-final/affected.stdout.log"if FINAL and target=="mixin_navigation_legacy"else "build/rust-test-logs/mixin-navigation-final/affected.stdout.log"if FINAL else "build/rust-test-logs/mixin-navigation-legacy-1/replay.stdout.log"
  row=dict(legacyTestPath=path,legacyClass=cls,legacyCase=method,baselineSha256=hashlib.sha256((BASE/path).read_bytes()).hexdigest(),rustTest="mcdev-core/tests/"+target+".rs::"+name,status=status,evidenceLog=evidence)
  if target=="mixin_navigation_boundary":row.update(scope="host-supplied normalized declaration/location boundary",runtimeHostConnection="pending parent capability/session integration; no scanner or reflection is implemented")
  records.append(row)
assert len(records)==96
(HERE/"coverage.json").write_text(json.dumps(records,indent=2)+"\n",encoding="utf-8")
print("navigation exact cases",len(records),"ported",sum(r["status"]=="ported"for r in records),"pending",sum(r["status"]!="ported"for r in records))
