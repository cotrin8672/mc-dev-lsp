"""Capture every original completion/edit test through disposable service wrappers.

Uses the source migration's shared recorder/runtime; it does not modify the JVM baseline.
"""
from pathlib import Path
import importlib.util
import json
import os
import re

HERE = Path(__file__).resolve().parent
SHARED = HERE.parent / "mixin_source"
spec = importlib.util.spec_from_file_location("source_capture", SHARED / "generate_capture.py")
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)
capture.OUT = capture.ROOT / "build/completion-oracle"
CONFIG = json.loads((HERE / "capture_targets.json").read_text(encoding="utf-8"))


def instrument(name, names, group="mixin"):
    path = capture.MAIN / group / (name + ".kt")
    text = path.read_text(encoding="utf-8")
    replacements = []
    wrappers = []
    for match in re.finditer(r"(?m)^    fun (\w+)\s*\(", text):
        method = match.group(1)
        if method not in names:
            continue
        op = text.index("(", match.start())
        end = capture.closing(text, op)
        parameters = text[op + 1:end]
        at = end + 1
        while text[at].isspace():
            at += 1
        assert text[at] == ":", (name, method, "explicit return type required")
        return_start = at + 1
        depth = 0
        at = return_start
        while at < len(text):
            char = text[at]
            if char in "<(":
                depth += 1
            elif char in ">)":
                depth -= 1
            if depth == 0 and char in "={":
                break
            at += 1
        returns = text[return_start:at].strip()
        argument_names = [p.split(":", 1)[0].strip() for p in capture.split_params(parameters)]
        entries = [f'"{arg}" to Capture.arg({arg})' for arg in argument_names]
        signature = text[match.start():end + 1].strip()
        wrappers.append(f'    {signature}: {returns} = Capture.capture("{name}.{method}", mapOf({", ".join(entries)})) {{ capture_impl_{method}({", ".join(argument_names)}) }}\n')
        replacements.append((match.start(1), match.end(1), "capture_impl_" + method))
    assert wrappers, name
    for start, end, replacement in reversed(replacements):
        text = text[:start] + replacement + text[end:]
    package_end = text.index("\n", text.index("package "))
    text = text[:package_end + 1] + "\nimport oracle.Capture\n" + text[package_end + 1:]
    declaration = re.search(r"(?:class|object) " + re.escape(name) + r"\b", text)
    at = declaration.end()
    while text[at].isspace():
        at += 1
    if text[at] == "(":
        at = capture.closing(text, at) + 1
    body = text.index("{", at) + 1
    extra = ""
    if "private val classIndex: ClassIndex" in text[:body]:
        text = text[:body].replace("private val classIndex: ClassIndex", "classIndex: ClassIndex") + text[body:]
        body -= len("private val ")
        extra += "    private val classIndex: ClassIndex = Capture.index(classIndex)!!\n"
    if name == "AtValueCompletionService":
        original_body = body
        header = text[:body]
        for parameter in ("injectionPointSpecifierSupported", "additionalAtValues"):
            old = "private val " + parameter
            header = header.replace(old, parameter)
        text = header + text[original_body:]
        # Locate the constructor again after shortening both val parameters.
        decl = re.search(r"class AtValueCompletionService\b", text)
        opening = text.index("(", decl.end())
        body = text.index("{", capture.closing(text, opening) + 1) + 1
        extra += '    private val injectionPointSpecifierSupported = { Capture.read("callback.specifier", emptyList<String>(), injectionPointSpecifierSupported()) }\n'
        extra += '    private val additionalAtValues = { Capture.read("callback.additional", emptyList<String>(), additionalAtValues()) }\n'
    text = text[:body] + "\n" + extra + "".join(wrappers) + text[body:]
    output = capture.OUT / "src" / (name + ".kt")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(text, encoding="utf-8")
    return output


def main():
    runtime = capture.prepare_runtime()
    sources = [instrument(name, methods) for name, methods in CONFIG["services"].items()]
    sources += [capture.instrument(capture.MAIN/path,names) for path,names in capture.APIS.items()]
    sources += [SHARED / "Capture.kt", HERE / "Run_completion.kt"]
    destination = capture.OUT / "classes"
    destination.mkdir(exist_ok=True)
    capture.run_java(runtime, runtime["compiler_classpath"], "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler", [
        "-no-stdlib", "-no-reflect", "-module-name", "mcdev-core", "-jvm-target", "21", "-Xfriend-paths=" + str(capture.BASE / "mcdev-core/build/classes/kotlin/main"),
        "-classpath", runtime["classpath"], "-d", str(destination), *map(str, sources),
    ], "compile", 180)
    cases = []
    for name in CONFIG["testClasses"]:
        file = "MixinEdgeCasesTest.kt" if name in {"MixinImportEditBuilderTest", "AtTargetMappingTest"} else name + ".kt"
        cases.append("io.github.mcdev.core.mixin." + name + "|mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/" + file)
    for name, methods in CONFIG["selectedTestMethods"].items():
        cases.append("io.github.mcdev.core.mixin." + name + "|mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/" + name + ".kt|" + ",".join(methods))
    capture.run_java(runtime, str(destination) + os.pathsep + runtime["classpath"], "oracle.Run_completionKt", [
        str(capture.OUT / "calls.jsonl"), str(capture.OUT / "cases.json"), *cases,
    ], "capture", 180)
    rows = json.loads((capture.OUT / "cases.json").read_text(encoding="utf-8"))
    events = [json.loads(line) for line in (capture.OUT / "calls.jsonl").read_text(encoding="utf-8").splitlines()]
    assert len(rows) == 162, len(rows)
    assert all(row["status"] == "passed" and any(event["legacyCase"] == row["legacyCase"] for event in events) for row in rows)
    (HERE / "legacy_calls.jsonl").write_text((capture.OUT / "calls.jsonl").read_text(encoding="utf-8"), encoding="utf-8")
    (HERE / "legacy_cases.json").write_text((capture.OUT / "cases.json").read_text(encoding="utf-8"), encoding="utf-8")
    write_rust_replay(rows)
    print(f"captured legacy completion/edit cases={len(rows)}")


def rust_name(row):
    cls = row["legacyClass"].rsplit(".", 1)[-1]
    method = row["legacyCase"].split("::")[-1]
    return re.sub(r"(?<!^)(?=[A-Z])", "_", cls + "_" + method).lower()


def write_rust_replay(rows):
    tests = ['#[path="fixtures/mixin_completion/replay.rs"] mod legacy_replay;\n']
    names = set()
    for row in rows:
        name = rust_name(row)
        assert name not in names
        names.add(name)
        tests.append(f'#[test]\nfn {name}() {{ legacy_replay::replay_case({json.dumps(row["legacyCase"])}); }}\n')
    (HERE.parents[1] / "mixin_completion_legacy.rs").write_text("\n".join(tests), encoding="utf-8")


if __name__ == "__main__":
    main()
