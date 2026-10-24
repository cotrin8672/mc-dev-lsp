"""Refresh owned migration evidence after the finite native suite succeeds."""
import collections
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
FIXTURES = ROOT / "mcdev-core/tests/fixtures"
ENGINE = FIXTURES / "expression_engine"
LOG = "build/rust-test-logs/expression-engine-20261024/owned-final-parity"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")


def rows(path):
    # splitlines also splits valid literal NEL characters inside JSON strings.
    return [json.loads(line) for line in path.read_text(encoding="utf-8").split("\n") if line]


stdout = (ROOT / (LOG + ".stdout.log")).read_text(encoding="utf-8")
results = re.findall(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", stdout)
assert len(results) == 9, results
assert "test result: FAILED" not in stdout
bytecode = read_json(FIXTURES / "bytecode_legacy_coverage.json")
assert len(bytecode) == 153
for case in bytecode:
    native_name = case["rustTest"].split("::")[-1]
    assert f"test {native_name} ... ok" in stdout, native_name
    case.update(status="passed", evidence=LOG + ".stdout.log")
write_json(FIXTURES / "bytecode_legacy_coverage.json", bytecode)

api_test = {
    "OfficialExpressionParser.parse": "original_parser_requests",
    "OfficialExpressionMemberCompletionProbes.build": "original_receiver_probe_requests",
    "OfficialExpressionIdentifierPoolBuilder.build": "original_identifier_build_requests",
    "IdentifierPool.matchesMember": "original_identifier_predicates",
    "IdentifierPool.matchesType": "original_identifier_predicates",
    "IdentifierPool.memberExists": "original_identifier_predicates",
    "IdentifierPool.typeExists": "original_identifier_predicates",
    "MemberDefinition.matches": "original_identifier_predicates",
    "OfficialExpressionMatcher.match": "original_match_requests",
    "OfficialExpressionMatcher.matchComponents": "every_official_virtual_component_keeps_exact_type_decorations_and_capture_location",
    "OfficialExpressionMatcher.completeReceiverMembers": "original_completion_requests",
}
legacy = rows(ENGINE / "legacy_calls.jsonl")
additional = rows(ENGINE / "additional_calls.jsonl")
cases = read_json(ENGINE / "legacy_cases.json")
assert len(cases) == 82 and len(legacy) == 432
assert all(row["api"] in api_test for row in legacy + additional)
coverage = []
for case in cases + read_json(ENGINE / "additional_cases.json"):
    corpus = legacy if case in cases else additional
    relevant = [(i + 1, row) for i, row in enumerate(corpus) if row["legacyCase"] == case["legacyCase"]]
    # The original Run.kt summary counts successful captures; cancellation/fatal
    # rows are recorded and replayed as well, but excluded from that count.
    assert sum("throwable" not in row for _, row in relevant) == case["calls"], case
    tests = sorted({api_test[row["api"]] for _, row in relevant})
    for test in tests:
        assert f"test {test} ... ok" in stdout
    coverage.append({
        **case,
        "corpus": "legacy_calls.jsonl" if case in cases else "additional_calls.jsonl",
        "corpusLineNumbers": [i for i, _ in relevant],
        "capturedCallsIncludingThrowables": len(relevant),
        "rustTests": ["mcdev-core/tests/expression_engine_legacy.rs::" + test for test in tests],
        "apis": dict(collections.Counter(row["api"] for _, row in relevant)),
        "nativeStatus": "passed",
        "evidence": LOG + ".stdout.log",
    })
write_json(ENGINE / "case_coverage.json", coverage)
ambiguities = read_json(ENGINE / "identity_ambiguities.json")
deltas = read_json(ENGINE / "jsr_fatal_deltas.json")
summary = {
    "originalBytecodeCases": len(bytecode),
    "originalOfficialEngineCases": len(cases),
    "originalCases": len(bytecode) + len(cases),
    "additionalOracleCases": len(coverage) - len(cases),
    "nativeTestFunctions": sum(map(int, results)),
    "nativeTestBinaries": len(results),
    "nativeEvidence": LOG + ".stdout.log",
    "nativeCompilerEvidence": LOG + ".stderr.log",
    "originalEngineApiCalls": len(legacy),
    "additionalEngineApiCalls": len(additional),
    "engineCallsByApi": dict(collections.Counter(row["api"] for row in legacy + additional)),
    "legacyEngineSha256": hashlib.sha256((ENGINE / "legacy_calls.jsonl").read_bytes()).hexdigest(),
    "identityAmbiguityGroups": len(ambiguities["cases"]),
    "jsrFatalDeltaRows": len(deltas),
    "bytecodeCases": "mcdev-core/tests/fixtures/bytecode_legacy_coverage.json",
    "engineCases": "mcdev-core/tests/fixtures/expression_engine/case_coverage.json",
    "remainingOwnedImplementationGaps": [],
}
write_json(ENGINE / "verification.json", summary)
print(json.dumps(summary, ensure_ascii=True, indent=2))
