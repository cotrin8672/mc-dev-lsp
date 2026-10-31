"""Check the lexical assertion scanner without importing the inventory generator."""
from pathlib import Path
import ast
import hashlib
import json
import re


SCANNER = Path(__file__).with_name("migration_inventory.py")


def load_scanner(path=SCANNER):
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    functions = [node for node in tree.body if isinstance(node, ast.FunctionDef)
                 and node.name in {"mask_lua", "calls"}]
    assert {node.name for node in functions} == {"mask_lua", "calls"}
    namespace = {"re": re}
    exec(compile(ast.Module(body=functions, type_ignores=[]), str(path), "exec"), namespace)
    return namespace["calls"]


def main():
    root = SCANNER.resolve().parents[2]
    metadata = [root/path for path in (
        "mcdev-nvim/tests/migration_inventory.json",
        "mcdev-nvim/tests/migration_evidence.json",
        "mcdev-nvim/tests/explicit_deltas.json",
        "mcdev-nvim/tests/legacy_sources.json",
        "docs/rust-test-migration.json",
    )]
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    before = {str(path): digest(path) for path in metadata}
    calls = load_scanner()

    names = """assert(true)
helpers.assert_eq(1, 1)
helpers.assert_not_nil(value)
assert_clean_diagnostics(buffer)
assert_not_nil_extra(value)
helpers.deep.assert_overlapping_prefix_lifecycle(buffer)
ensure_not_nil(value)
reassert_not_nil(value)
"""
    found = calls(names)
    assert [item["call"].split("(", 1)[0] for item in found] == [
        "assert", "helpers.assert_eq", "helpers.assert_not_nil",
        "assert_clean_diagnostics", "assert_not_nil_extra",
        "helpers.deep.assert_overlapping_prefix_lifecycle",
    ], found
    assert [item["line"] for item in found] == [1, 2, 3, 4, 5, 6]

    multiline = 'helpers.assert_not_nil(\n  (value),\n  "closing ) inside a string"\n)'
    assert calls(multiline) == [{"line": 1, "call": multiline}]
    masked = '''-- assert_not_nil(comment)
local first = "assert_not_nil(string)"
local second = 'helpers.assert_not_nil(string)'
local third = [=[assert_clean_diagnostics(long_string)]=]
--[=[ helpers.assert_not_nil(long_comment)
assert_clean_diagnostics(long_comment) ]=]
local function assert_not_nil(value) return value end
function helpers.assert_not_nil(value) return value end
function assert_eq(value) return value end
assert_not_nil(visible)
'''
    assert calls(masked) == [{"line": 10, "call": "assert_not_nil(visible)"}]
    assert calls('check("original case", function() end)', False) == [
        {"line": 1, "call": 'check("original case", function() end)'}
    ]
    assert before == {str(path): digest(path) for path in metadata}
    print(json.dumps({
        "passed": True,
        "checks": ["plain/qualified/multiple-underscore names", "multiline calls",
                   "masked comments/strings", "excluded function declarations",
                   "unchanged case-call scanning", "metadata bytes unchanged"],
        "scannerSha256": digest(SCANNER),
        "generatorImported": False,
    }, indent=2))


if __name__ == "__main__":
    main()
