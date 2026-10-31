"""Strict, lossless source/testcase inventory for the native Neovim migration."""
from pathlib import Path
import collections
import difflib
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / "build/migration-baseline/current-20261023"
LEDGER_PATH = ROOT / "docs/rust-test-migration.json"


def mask_lua(text):
    """Hide comments/strings while preserving offsets and line breaks."""
    out = list(text)
    i = 0
    while i < len(text):
        start = i
        comment = text.startswith("--", i)
        if comment:
            i += 2
        long_match = re.match(r"\[(=*)\[", text[i:])
        if long_match:
            end = text.find("]" + long_match[1] + "]", i + len(long_match[0]))
            i = len(text) if end < 0 else end + len(long_match[1]) + 2
        elif comment:
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
        elif text[i] in "\"'":
            quote = text[i]
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                elif text[i] == quote:
                    i += 1
                    break
                else:
                    i += 1
        else:
            i += 1
            continue
        for p in range(start, min(i, len(text))):
            if out[p] != "\n":
                out[p] = " "
    return "".join(out)


def normalize(text):
    return re.sub(r"\s+", " ", text.replace("\r\n", "\n")).strip()


def backend_equivalent(text):
    """Only the approved client-name vocabulary is normalized automatically."""
    return (
        text.replace("JDT LS", "ktlsp")
        .replace("JDTLS", "ktlsp")
        .replace("jdtls", "ktlsp")
        .replace("JDT", "ktlsp")
    )


def literal_strings(text):
    return re.findall(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'', text)


def calls(text, assertion=True):
    masked = mask_lua(text)
    if assertion:
        pattern = r"\b(?:[\w]+\.)*assert(?:_[a-z]+)*\s*\("
    else:
        pattern = (
            r"\b(?:[\w]+\.)?(?:with_buffer|with_named_buffer|run_case|"
            r"run_snapshot_failure_case|check)\s*\("
        )
    result = []
    for match in re.finditer(pattern, masked):
        if re.search(r"\bfunction\s*$", masked[max(0, match.start() - 20):match.start()]):
            continue
        depth = 1
        end = match.end()
        while end < len(masked) and depth:
            depth += (masked[end] == "(") - (masked[end] == ")")
            end += 1
        result.append({
            "line": text.count("\n", 0, match.start()) + 1,
            "call": text[match.start():end],
        })
    return result


def loop_blocks(text):
    """Return each table-driven input loop as a local block, never a file-wide match."""
    masked = mask_lua(text)
    result = []
    for match in re.finditer(r"\bfor\b[^\n]*\bipairs\s*\(\s*\{", masked):
        brace_start = masked.find("{", match.start(), match.end())
        if brace_start < 0:
            continue
        depth = 0
        end = None
        for index in range(brace_start, len(masked)):
            if masked[index] == "{":
                depth += 1
            elif masked[index] == "}":
                depth -= 1
                if depth == 0:
                    end = index + 1
                    break
        if end is None:
            continue
        raw = text[match.start():end]
        entries = literal_strings(raw)
        rows = []
        row_start = brace_start + 1
        row_depth = 0
        for index in range(brace_start + 1, end - 1):
            if masked[index] == "{":
                row_depth += 1
            elif masked[index] == "}":
                row_depth -= 1
            elif masked[index] == "," and row_depth == 0:
                row = normalize(text[row_start:index])
                if row:
                    rows.append(row)
                row_start = index + 1
        final_row = normalize(text[row_start:end - 1])
        if final_row:
            rows.append(final_row)
        body_end_line = text.count("\n", 0, end - 1) + 1
        for line_number, line in enumerate(text.splitlines(), 1):
            if line_number > body_end_line and line.strip() == "end":
                body_end_line = line_number
                break
        result.append({
            "line": text.count("\n", 0, match.start()) + 1,
            "endLine": text.count("\n", 0, end - 1) + 1,
            "bodyEndLine": body_end_line,
            "anchor": normalize(raw),
            "sha256": hashlib.sha256(raw.replace("\r\n", "\n").encode()).hexdigest(),
            "entries": entries,
            "rows": rows,
        })
    return result


def state_markers(text):
    """Capture callback/state declarations as auditable anchors."""
    markers = []
    for line_no, line in enumerate(text.splitlines(), 1):
        if re.search(
            r"\b(?:local\s+)?(?:original_|pending|state|clients|started|"
            r"callbacks?|source_requests|request_count|calls|notices)\w*\s*(?:=|,)"
            r"|\bfunction\b|\bfunction\s*\(",
            line,
        ):
            value = normalize(line)
            if value:
                markers.append({"line": line_no, "text": value})
    return markers


def region_candidates(opcodes, old_line, current_items):
    """Restrict a legacy item to its equal-line or replacement-hunk region."""
    for tag, old_start, old_end, new_start, new_end in opcodes:
        if not (old_start <= old_line - 1 < old_end):
            continue
        if tag == "equal":
            mapped_line = new_start + (old_line - 1 - old_start) + 1
            return [item for item in current_items if item["line"] == mapped_line]
        return [
            item for item in current_items
            if new_start <= item["line"] - 1 < new_end
        ]
    return []


def item_present(item, old, new, current_items, field, backend=False):
    opcodes = difflib.SequenceMatcher(
        None,
        old.splitlines(),
        new.splitlines(),
        autojunk=False,
    ).get_opcodes()
    expected = normalize(item[field])
    for candidate in region_candidates(opcodes, item["line"], current_items):
        actual = normalize(candidate[field])
        if actual == expected:
            return True
        if backend and normalize(backend_equivalent(actual)) == normalize(backend_equivalent(expected)):
            return True
    return False


def case_present(item, old, new, current_cases, backend=False):
    """Match one input/callback call, never one string anywhere in the file."""
    raw = item["call"]
    if re.search(r"(?:with_buffer|with_named_buffer|run_case|run_snapshot_failure_case)\s*\(", raw):
        raw = re.split(r"\bfunction\b", raw, maxsplit=1)[0]
        required_values = literal_strings(raw)
    else:
        required_values = literal_strings(raw)[:1]
    if backend:
        required_values = [
            value for value in required_values
            if value.strip("\"'").lower() not in {"jdtls", "jdt ls", "ktlsp"}
        ]
    required = collections.Counter(required_values)
    opcodes = difflib.SequenceMatcher(
        None,
        old.splitlines(),
        new.splitlines(),
        autojunk=False,
    ).get_opcodes()
    for candidate in region_candidates(opcodes, item["line"], current_cases):
        candidate_raw = candidate["call"]
        if re.search(
            r"(?:with_buffer|with_named_buffer|run_case|run_snapshot_failure_case)\s*\(",
            candidate_raw,
        ):
            candidate_raw = re.split(r"\bfunction\b", candidate_raw, maxsplit=1)[0]
            available_values = literal_strings(candidate_raw)
        else:
            available_values = literal_strings(candidate_raw)[:1]
        if backend:
            available_values = [
                value for value in available_values
                if value.strip("\"'").lower() not in {"jdtls", "jdt ls", "ktlsp"}
            ]
        available = collections.Counter(available_values)
        if all(available[key] >= value for key, value in required.items()):
            return True
    return False


def loop_present(block, old, new, backend=False):
    current_blocks = loop_blocks(new)
    opcodes = difflib.SequenceMatcher(
        None,
        old.splitlines(),
        new.splitlines(),
        autojunk=False,
    ).get_opcodes()
    candidates = region_candidates(opcodes, block["line"], current_blocks)
    if any(
        block["anchor"] == candidate["anchor"]
        and block.get("rows", []) == candidate.get("rows", [])
        for candidate in candidates
    ):
        return True
    if backend:
        expected = normalize(backend_equivalent(block["anchor"]))
        expected_rows = [normalize(backend_equivalent(row)) for row in block.get("rows", [])]
        return any(
            expected == normalize(backend_equivalent(candidate["anchor"]))
            and expected_rows == [normalize(backend_equivalent(row)) for row in candidate.get("rows", [])]
            for candidate in candidates
        )
    return False


def classify(path):
    if path.endswith(".java"):
        return "process-fixture"
    if path.endswith(("minimal_init.lua", "test_helpers.lua")):
        return "support"
    if "/e2e/" in path:
        return "e2e"
    return "unit"


def module_diff_class(path, old, new):
    if old == new:
        return "unchanged baseline module"
    if path.endswith("/jdtls.lua"):
        return "native ktlsp replacement for legacy JDT module"
    if path.endswith("/buffer.lua"):
        return "baseline Kotlin/Lapis parser retained with native snapshot/stale-edit extension"
    return "native client/backend adaptation with baseline semantic anchors retained"


# These are the only whole-file backend rewrites. They still retain a fidelity
# record; they are never used as a waiver for another unit or for E2E inputs.
BACKEND_ONLY_FILES = {
    "mcdev-nvim/tests/jdtls_workspace.lua",
    "mcdev-nvim/tests/run.lua",
    "mcdev-nvim/lua/mcdev/jdtls.lua",
}


def fidelity(path, old, new, classification):
    backend = path in BACKEND_ONLY_FILES
    old_assertions = calls(old)
    current_assertions = calls(new)
    old_cases = calls(old, False)
    current_cases = calls(new, False)
    old_loops = loop_blocks(old)
    old_state = state_markers(old)
    missing_assertions = [
        item for item in old_assertions
        if not item_present(item, old, new, current_assertions, "call", backend=backend)
    ]
    missing_cases = [
        item for item in old_cases
        if not case_present(
            item,
            old,
            new,
            current_cases,
            backend=backend or classification == "e2e",
        )
    ]
    missing_loops = [
        item for item in old_loops
        if not loop_present(item, old, new, backend=backend)
    ]
    missing_state = [
        item for item in old_state
        if not item_present(
            item,
            old,
            new,
            state_markers(new),
            "text",
            backend=backend,
        )
    ]
    # E2E assertion spelling may be backend-adapted, but its table/loop inputs
    # remain strict. Non-backend unit/support assertions and callbacks are strict.
    strict_assertions = classification in ("unit", "support") and not backend
    strict_cases = classification in ("unit", "support", "e2e") and not backend
    strict_state = classification in ("unit", "support") and not backend
    strict_failure = (
        (strict_assertions and missing_assertions)
        or (strict_cases and missing_cases)
        or (strict_state and missing_state)
        or bool(missing_loops)
    )
    return {
        "status": "failed" if strict_failure else ("backend-adapted" if backend else "verified"),
        "strict": strict_assertions or strict_cases or strict_state,
        "oldLineCount": len(old.splitlines()),
        "newLineCount": len(new.splitlines()),
        "assertionSiteCount": len(old_assertions),
        "caseEntryCount": len(old_cases),
        "loopInputCount": len(old_loops),
        "stateMarkerCount": len(old_state),
        "missingAssertions": missing_assertions,
        "missingCaseEntries": missing_cases,
        "missingLoopInputs": missing_loops,
        "missingStateMarkers": missing_state,
    }


ledger = json.loads(LEDGER_PATH.read_text(encoding="utf-8"))
files = []
legacy_sources = {}
for entry in ledger["neovimTestAndSupportFiles"]:
    path = entry["source"]
    baseline_path = BASE / path
    data = baseline_path.read_bytes()
    sha = hashlib.sha256(data).hexdigest()
    if sha.upper() != entry["sha256"].upper():
        raise AssertionError(f"baseline SHA mismatch: {path}")
    old = data.decode("utf-8")
    legacy_sources[path] = old
    replacement = (
        "mcdev-companion/examples/teardown_process.rs"
        if path.endswith(".java")
        else path
    )
    replacement_path = ROOT / replacement
    new = replacement_path.read_text(encoding="utf-8")
    kind = classify(path)
    audit = fidelity(path, old, new, kind)
    files.append({
        "legacyPath": path,
        "sha256": sha,
        "replacementPath": replacement,
        "replacementSha256": hashlib.sha256(replacement_path.read_bytes()).hexdigest(),
        "classification": kind,
        "assertionSites": [
            {**item, "legacyAssertionId": f"{path}::assert@{item['line']}"}
            for item in calls(old)
        ],
        "caseEntryCalls": calls(old, False),
            "loopInputs": loop_blocks(old),
        "fidelity": audit,
        "nativeExecution": (
            "pending host contract"
            if kind == "e2e" and "teardown_" not in path and "run_bundle" not in path
            else "see execution evidence"
        ),
    })

modules = []
module_failures = []
for baseline_path in sorted((BASE / "mcdev-nvim/lua").rglob("*.lua")):
    relative = baseline_path.relative_to(BASE).as_posix()
    replacement = relative.replace("/jdtls.lua", "/ktlsp.lua")
    replacement_path = ROOT / replacement
    old = baseline_path.read_text(encoding="utf-8")
    new = replacement_path.read_text(encoding="utf-8")
    audit = fidelity(relative, old, new, "module")
    modules.append({
        "legacyPath": relative,
        "sha256": hashlib.sha256(baseline_path.read_bytes()).hexdigest(),
        "replacementPath": replacement,
        "replacementSha256": hashlib.sha256(replacement_path.read_bytes()).hexdigest(),
        "diffClass": module_diff_class(relative, old, new),
        "fidelity": audit,
    })
    if audit["status"] == "failed":
        module_failures.append(relative)

failed_files = [
    item["legacyPath"] for item in files
    if item["fidelity"]["status"] == "failed"
] + module_failures
fidelity_summary = {
    "baselineRoot": "build/migration-baseline/current-20261023",
    "originalTestAndSupportFiles": len(files),
    "originalLuaModules": len(modules),
    "strictFailureFiles": failed_files,
    "strictPassed": not failed_files,
    "assertionSites": sum(item["fidelity"]["assertionSiteCount"] for item in files),
    "caseEntryCalls": sum(item["fidelity"]["caseEntryCount"] for item in files),
    "loopInputTables": sum(item["fidelity"]["loopInputCount"] for item in files),
    "loopInputRows": sum(
        sum(len(block.get("rows", [])) for block in loop_blocks(
            legacy_sources[item["legacyPath"]]
        ))
        for item in files
    ),
    "stateMarkers": sum(item["fidelity"]["stateMarkerCount"] for item in files),
    "rule": (
        "Every original assertion/callback/state and every table-loop input must "
        "remain exact or be represented by a narrow backend adapter. A repeated "
        "call string elsewhere never proves loop migration."
    ),
}
fidelity_summary["moduleDiffClasses"] = dict(
    collections.Counter(item["diffClass"] for item in modules)
)
result = {
    "legacyFiles": len(files),
    "legacyModules": len(modules),
    "classifications": dict(collections.Counter(item["classification"] for item in files)),
    "staticAssertionCallSites": fidelity_summary["assertionSites"],
    "fidelity": fidelity_summary,
    "note": (
        "Assertion call sites, loop input tables and runtime executions are "
        "separate counts; no count is inflated into a JUnit-style case total."
    ),
    "files": files,
    "modules": modules,
}
(ROOT / "mcdev-nvim/tests/migration_inventory.json").write_text(
    json.dumps(result, ensure_ascii=False, indent=2) + "\n",
    encoding="utf-8",
)
(ROOT / "mcdev-nvim/tests/legacy_sources.json").write_text(
    json.dumps(legacy_sources, ensure_ascii=False, separators=(",", ":")) + "\n",
    encoding="utf-8",
)
print(json.dumps({
    "legacyFiles": result["legacyFiles"],
    "legacyModules": result["legacyModules"],
    "classifications": result["classifications"],
    "fidelity": fidelity_summary,
}, ensure_ascii=False, indent=2))
