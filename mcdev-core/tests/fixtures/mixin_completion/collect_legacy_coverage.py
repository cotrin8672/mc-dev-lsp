"""Inventory exact original tests; pending entries are never test-parity evidence."""
from pathlib import Path
import hashlib
import json
import re


def collect(repo: Path):
    baseline = repo / "build/migration-baseline/current-20261023"
    config = json.loads((Path(__file__).parent / "capture_targets.json").read_text(encoding="utf-8"))
    roots = [
        baseline / "mcdev-core/src/test/kotlin/io/github/mcdev/core/project",
        baseline / "mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin",
        baseline / "mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/project",
    ]
    records = []
    for root in roots:
        for path in sorted(root.glob("*.kt")):
            source = path.read_text(encoding="utf-8")
            classes = list(re.finditer(r"(?m)^class\s+(\w+)", source))
            for test in re.finditer(r"@Test\s+fun\s+(\w+)", source):
                owner = next((m.group(1) for m in reversed(classes) if m.start() < test.start()), None)
                case = test.group(1)
                if root.name == "project":
                    if "mcdev-jdtls-extension" in str(root) and owner != "FileBasedProjectContextServiceTest":
                        continue
                    group = "project"
                elif owner in config["testClasses"] or case in config["selectedTestMethods"].get(owner, []):
                    group = "mixin_completion"
                else:
                    continue
                records.append({
                    "group": group,
                    "legacyTestPath": path.relative_to(baseline).as_posix(),
                    "legacyClass": owner,
                    "legacyCase": case,
                    "baselineSha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                    "rustTest": None,
                    "status": "pending_exact_replay",
                })
    assert len({(r["legacyTestPath"], r["legacyClass"], r["legacyCase"]) for r in records}) == len(records)
    assert sum(r["group"] == "project" for r in records) == 127
    assert sum(r["group"] == "mixin_completion" for r in records) == 162
    return records


if __name__ == "__main__":
    repo = Path(__file__).resolve().parents[4]
    records = collect(repo)
    output = Path(__file__).parent / "legacy_coverage.json"
    if output.exists():
        old = {(r["legacyTestPath"], r["legacyClass"], r["legacyCase"]): r for r in json.loads(output.read_text(encoding="utf-8"))}
        for record in records:
            prior = old.get((record["legacyTestPath"], record["legacyClass"], record["legacyCase"]))
            if prior and prior["baselineSha256"] == record["baselineSha256"]:
                record.update({k: v for k,v in prior.items() if k not in ("group","legacyTestPath","legacyClass","legacyCase","baselineSha256")})
    output.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    print(f"legacy inventory: {len(records)} cases; completed={sum(r['status'] == 'ported' for r in records)}; pending={sum(r['status'] != 'ported' for r in records)}")
