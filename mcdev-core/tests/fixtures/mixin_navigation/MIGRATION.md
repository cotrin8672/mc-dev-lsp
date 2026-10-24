# Definition/reference navigation and source-location boundary

`mixin_navigation.rs` implements MixinDefinitionService, AtTargetParser, MixinReferenceService and source-location resolution over host-supplied declaration facts. Existing root McDefinitionTarget, McReferenceLocation, SourceScanEntry, source semantic/declaration models, Accessor/Invoker inference, selector parsers and the MixinExtras expression context resolver remain the single authorities. No second language parser, classfile reader, JDT reflection or project/classpath source scanner is introduced.

## Exact scope

The 96 assigned cases are recorded individually in `coverage.json`: 59 MixinDefinitionService, ten MixinReferenceService, eight DefinitionResolutionService, five SourceIndex and 14 FixtureIntegration cases. Original paths, classes, methods, source SHA256 and native test names are preserved.

The core 69 tests first executed all their original assertions against disposable instrumented copies of the frozen Kotlin implementation. `legacy_calls.jsonl` contains 3,236 API events, including each final definition/reference result and the source helper inputs, variants and ranges. Native tests compare all target fields, namespaces, direct source URI, ranges and reference metadata. The shared source replay also checks concrete result variants and converts captured UTF16 absolute offsets to Rust UTF8 byte offsets.

The 13 former JDT source-location tests run at the explicitly relocated boundary. Their original Java/source URI/target inputs and expected resolution metadata, ranges, overload selection and backend callback assertions are retained. The host supplies normalized HostSourceSnapshot and HostSourceDeclaration facts. SourceIndex consumes those facts without reading files or reparsing source text; DefinitionResolutionService checks direct source locations first, then invokes backends in order. A source result suppresses later callbacks; malformed direct targets fail closed. The compatibility Jdt enum value is metadata, not a JVM backend.

These 13 domain tests establish the host-fact contract. Runtime host capability/session integration remains a separate parent-owned task; the fixture tests do not claim that a real host connection is reachable. Host snapshot generations and Unicode/UTF16 location preservation are independently tested. Discovered core targets preserve the baseline NAMED namespace; source-location results retain the original request target and namespace.

## Primary resources and provenance

`../resources/source_manifest.json` maps all 49 original input fixture paths to exact native copies and source SHA256: nine bytecode Java inputs, one fixture Java input and 39 primary resources. Twenty-two existing copies were reused and 27 missing copies preserved. No duplicate byte copy was added when an identical fixture already existed.

`../resources/manifest.json` and `catalog.rs` expose 40 named resources: the 39 primary resources plus the existing compiled SimpleTarget class. `derived_class_provenance.json` ties that class's original bytes/SHA to its Java source. All cargo tests use saved resources/class bytes and require neither javac nor a JVM. The resource suite checks all 49 original fixture hashes and all 40 resource hashes, preserves the 39 required resource assertions, and executes the original mapping/config/source/platform cases.

Native fixture closure: `build/rust-test-logs/fixture-integration-final/fixtures.stdout.log` and `.stderr.log`, exit 0, no timeout, 3.2 seconds, all 14 original cases passed. Root `.gitattributes` marks fixture files `-text` so checkout line-ending conversion cannot alter recorded fixture hashes.

The core navigation final validation and any remaining shared-source differences are recorded in `coverage.json` and the completion report. Linux and host/editor/E2E were not executed by this domain agent. No commits were created.

Final navigation run: `build/rust-test-logs/mixin-navigation-final/affected.stdout.log` and `.stderr.log`, exit 0, no timeout, no warnings, 22.0 seconds. All 103 tests passed: 69 exact core cases, 13 exact relocated source-boundary cases, 14 exact fixture cases, and seven Unicode/snapshot/backend regression cases. The seven initially observed source extractor/declaration-span differences were fixed in the shared source owner module and the complete original outputs then passed.

A final identifier-category review retained Kotlin Char semantics for definition IDs: Letter and DecimalDigit categories, UTF16 code units, underscore and dollar. Rust alphabetic/alphanumeric predicates would additionally accept Roman numerals, superscript digits and some combining marks. The native regression rejects these and supplementary-character IDs as the original Char predicate did. `build/rust-test-logs/mixin-navigation-identifier-final/affected.*` passed the new unit case plus the 69 exact core and three navigation regressions, exit 0, 47.6 seconds, no timeout/warnings. Together the domain has 104 passing tests and no pending original case. The 13 location-boundary cases still carry the explicit runtime host-connection handoff.
