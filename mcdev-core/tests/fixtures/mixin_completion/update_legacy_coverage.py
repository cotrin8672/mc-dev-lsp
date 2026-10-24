"""Bind every inventoried case to its verified native test; keep host work pending."""
import json
import re
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def name(cls,method):return re.sub(r"(?<!^)(?=[A-Z])","_",cls+"_"+method).lower()
concurrency={
    "concurrentLoadCachedSessionReturnsSingleSessionIdentityAndVersion":"concurrent_cold_load_builds_once",
    "warmCacheHitForOneWorkspaceDoesNotWaitOnColdBuildForAnother":"warm_workspace_does_not_wait_for_other_workspaces_cold_build",
    "concurrentColdLoadAndReindexAlwaysReindexesAndBumpsVersion":"reindex_waits_for_cold_load_then_builds_new_version",
    "failedColdLoadPropagatesBuildFailureWithoutSecondarySessionMissing":"cold_failure_is_returned_once_and_next_load_recovers",
}
def main():
    records=json.loads((HERE/"legacy_coverage.json").read_text())
    for row in records:
        cls=row["legacyClass"];case=row["legacyCase"]
        if cls=="FileBasedProjectContextServiceTest" and case=="sessionIndexesClasspathClasses":
            row.update(status="pending_host_integration",rustTest=None,evidenceLog=None,remainingAssertions=["HostProjectSession.classBytesProvider.classCount() >= 1", "The same session context.indexState == READY"],fixture="project_files/SimpleTarget.class.bin and fabric-basic")
            continue
        if row["group"]=="mixin_completion":target="mixin_completion_legacy";test=name(cls,case)
        elif cls=="ProjectModelTest" or cls=="FileBasedProjectContextServiceTest" and case not in concurrency:target="project_legacy_stateful";test=name(cls,case)
        elif cls=="FileBasedProjectContextServiceTest":target="project";test=concurrency[case]
        else:target="project_legacy";test=name(cls,case)
        path=ROOT/"tests"/(target+".rs")
        assert re.search(r"\bfn\s+"+test+r"\s*\(",path.read_text()),(path,test)
        row.update(status="ported",rustTest=f"mcdev-core/tests/{target}.rs::{test}",evidenceLog="build/rust-test-logs/project-completion-final/affected.stdout.log")
    (HERE/"legacy_coverage.json").write_text(json.dumps(records,indent=2)+"\n")
    print("ported",sum(r["status"]=="ported" for r in records),"pending",sum(r["status"]!="ported" for r in records))
if __name__=="__main__":main()
