"""Mark verified original cases; caller coverage remains an explicit handoff."""
from pathlib import Path
import json,re,hashlib
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
BASE=ROOT/"build/migration-baseline/current-20261023"
rows=json.loads((HERE/"legacy_coverage.json").read_text())
calls=[json.loads(line)for line in (HERE/"legacy_calls.jsonl").read_text(encoding="utf-8").splitlines()]
source=(ROOT/"mcdev-core/tests/mixinextras_locals_legacy.rs").read_text()
for row in rows:
 if row["status"]=="pending_caller_integration":
  row["calleeEvidenceCaptured"]=any(call["api"]=="ShareCompletionService.complete"and call["legacyCase"]==row["legacyTestPath"]+"::"+row["legacyCase"]for call in calls)
  continue
 name=row["rustTest"].split("::")[-1]
 assert re.search(r"\bfn\s+"+name+r"\s*\(",source),name
 row.update(status="ported",evidenceLog="build/rust-test-logs/mixinextras-locals-final/affected.stdout.log",resultComparison="Complete native typed service result; source helper variants/offsets and final expression matcher/builder results are also executed where originally asserted.")
(HERE/"legacy_coverage.json").write_text(json.dumps(rows,indent=2)+"\n")
path="mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/e2e/MixinServiceFacadeRoutingTest.kt"
text=(BASE/path).read_text();names=[name for name in re.findall(r"@Test\s+fun\s+(\w+)",text)if "local"in name.lower()or"share"in name.lower()]
assert len(names)==23,len(names)
handoff=[dict(legacyTestPath=path,legacyClass="MixinServiceFacadeRoutingTest",legacyCase=name,baselineSha256=hashlib.sha256((BASE/path).read_bytes()).hexdigest(),status="pending_caller_integration",requiredProof="Run the native Facade's final completion/diagnostic/cancellation/provider behavior with every original assertion; a Local/Share callee capture does not establish caller parity.",calleeEvidence=("legacy_calls.jsonl ShareCompletionService.complete"if name in ["routesShareIdCompletionAcrossMatchingHandlersAndExplicitNamespaces","shareIdCompletionIncludesMatchingIdsFromAdditionalSources","routesShareIdCompletionWithJavaEscapedValues"]else None))for name in names]
(HERE/"caller_handoff.json").write_text(json.dumps(handoff,indent=2)+"\n")
print("direct ported",sum(row["status"]=="ported"for row in rows),"caller handoff",len(handoff))
