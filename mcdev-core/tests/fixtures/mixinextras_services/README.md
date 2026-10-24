# MixinExtras diagnostics / actions migration evidence

`services-coverage.json` maps all 65 diagnostic and 68 code-action cases to exact
native tests, original case bodies, and public capture rows. All 133 original
assertion bodies run during one-time capture. Native replay executes the final
public Rust service APIs for 249 full outputs across 128 cases; the remaining
five context-selector cases have direct native tests with every original input
and assertion. Ordered callback API/argument/count traces are compared exactly;
repeated reads consume their original sequential values. Concrete fix kinds,
metadata, snippets, imports, UTF16 ranges, and checked applied documents survive.

`caller-calls.jsonl` additionally exercises completeHandler and applyHandlerFix.
These are partial evidence for CoreHandlerSignatureCompletionTest callers;
their full Facade paths remain pending the separate Facade migration. Three new
ApplyParityTest cases record the legacy materializer's pass-through behavior.
The test-only snapshot/application guard rejects stale source, overlap, invalid
UTF8 boundaries and out-of-bounds edits using the shared `apply_text_edits`.
Runtime generation/version guards remain the host adapter's responsibility.

`host-reference-facts.json` records actual frozen VM known-reference and
reflection fallback assignability results, including negative lookup outcomes.
It is not a production JDK type table. Production uses the shared authoritative
HandlerReferenceTypeResolver; unavailable facts remain Unknown. Diagnostics and
coercion-preserving fixes do not invent incompatibility from unavailable facts.

`generate_capture.py` compiles only instrumented frozen copies into ignored
build/extras-services-oracle and records constructor bytecode availability.
Pure native tests require no JVM or Gradle. `generate_coverage.py` is also a
one-time provenance tool. Source scalar offsets are bytes in Rust; legacy
Kotlin UTF16 scalar offsets are checked and converted at the fixture boundary.
LSP text ranges retain UTF16 units. Engine cancellation propagates unchanged.
