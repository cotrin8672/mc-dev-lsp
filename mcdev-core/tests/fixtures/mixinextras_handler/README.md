# Handler migration evidence

`handler-coverage.json` maps every one of the 394 original case names to its
native test, complete original case body, and exact rows of `handler-calls.jsonl`.
The native replay compares full typed results for all 4,772 recorded calls,
including nested calls. Ordered lists, nullable results, ranges, generic types,
sugar metadata, target/index reads and operation typing hooks are retained.
The capture records actual optional-bytecode constructor presence. Runtime
capabilities are never inferred from whether a read happened.

390 cases have complete final-entrypoint replay coverage; `mixinExtrasAnnotationMapsFromSimpleName` has a direct
native test. Three cases remain **pending Facade integration**, rather than being
counted as completed by testing a leaf service:

- `CoreHandlerSignatureCompletionTest::facadeRoutesAccessorInvokerAndOverwriteToDeclarationSnippets`
- `CoreHandlerSignatureCompletionTest::facadeNormalizesDeclarationSnippetIndentationAtBothInsertionBoundaries`

The one-time frozen Kotlin capture ran all 394 original assertion bodies. Rust
test execution uses only checked-in JSON and has no JVM/Gradle dependency.
`generate_capture.py`, `ReferenceFacts.kt`, and `generate_reference_facts.py`
document the one-time provenance. Reference facts contain actual VM
`Class.forName` / `isAssignableFrom` results; they are test data, not a production
list of recognized JDK classes.

Production SDK/classpath integration must supply authoritative
`HandlerReferenceTypeResolver` facts (or a complete ClassIndex/BytecodeIndex).
`None` means unavailable; it is distinct from `Some(false)`. Use
`enrich_handler_types_with_reference_types` for reflection fallback enrichment.
The three-argument compatibility entrypoint retains unresolved descriptors as
`None`. Validation avoids declaring a mismatch from missing ordinary parameter
descriptors or unavailable reference coercion facts. Boolean match predicates
only mean “proven match”; diagnostic consumers need `type_match_knowledge`.

Core scalar source offsets are UTF8 byte offsets. Kotlin String scalar offsets
in the legacy capture are checked and converted from UTF16 by the replay.
Text ranges remain UTF16. Cancellation from expression/local analysis propagates
as the engine's typed `Cancellation` panic, preserving the throwing boundary.

`mixinextras_handler_knowledge.rs` adds native coverage for unavailable versus
incompatible reference facts, SDK enrichment, unavailable ordinary parameters,
primitive/array coercion, and exact binary32/binary64 hexadecimal rounding.
