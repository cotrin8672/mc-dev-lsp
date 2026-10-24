# MixinExtras source/expression migration evidence

All **316 original tests** in the frozen current baseline run their unchanged
assertions during one-time oracle generation. The checked-in corpus contains
**6,650 successful API calls and two cancellation events**. The native replay
executes the actual Rust production APIs and checks every full result, concrete
business enum variant, case status, and call count. Unsupported API rows fail.
It also includes the original tests' calls into the seven shared source parser
objects and the handler annotation-site discovery functions.

`coverage.json` is the complete original `path::case` to native replay/table-test
mapping. It contains 41 additional direct assertion entries for constructor,
index, class-literal resolution, cache, callback, cancellation, and helper cases.
`cases.json` records the unchanged original test execution and capture counts.

| Original file | Cases |
| --- | ---: |
| ClassLiteralTypeNameResolverTest.kt | 10 |
| DefinitionAnnotationParseIssueTest.kt | 4 |
| DefinitionAnnotationParserTest.kt | 48 |
| DefinitionAnnotationSingleQuoteRecoveryTest.kt | 1 |
| DefinitionsAnnotationParserTest.kt | 28 |
| ExpressionAnnotationParserTest.kt | 24 |
| ExpressionCompletionLexerTest.kt | 16 |
| ExpressionContextResolverTest.kt | 40 |
| ExpressionDefinitionEditPlannerTest.kt | 17 |
| ExpressionMemberCompletionServiceTest.kt | 19 |
| ExpressionReceiverParserTest.kt | 18 |
| ExpressionsAnnotationParserTest.kt | 24 |
| ExpressionSupportTest.kt | 44 |
| ExpressionTypeImportCacheTest.kt | 1 |
| KotlinExpressionContextTest.kt | 7 |
| MixinExtrasDefinitionIndexTest.kt | 3 |
| MixinExtrasExpressionIndexTest.kt | 6 |
| MixinExtrasAnnotationContextTest.kt | 6 |

Stateful tests use the original request sequences with live Rust caches and
matcher callbacks. They check the invocation counter after every call, shared
cache reuse across service instances, prefix filtering, receiver/bytecode/type
binding/definition/context invalidation, LRU eviction, and rejection of cached
unavailable/cancelled results. Cancellation checks retain the same `Arc` error
identity. A separate native callback computes candidates from the real resolved
type pool and checks the import-change sequence produces `one`, `two`, `two`
with two invocations. Callback laziness and the branch-merge common-superclass
resolver side effect are tested directly, as are valid pool bindings and source
ID slices in the Kotlin context case.

Fixtures normalize only representation differences: struct field spelling,
known enum spelling, sets, half-open ranges, and JVM UTF-16 string offsets versus
Rust UTF-8 byte offsets. `sourceHint` preserves the outer document for converting
absolute ranges in nested parser calls. Import aliases and arbitrary dictionary
keys are preserved; sealed variants and `Empty`/`Unavailable` remain distinct.

The production implementation is `src/mixinextras_expression/`. It uses the
single `expression_engine` parser, AST, flow, pool, matcher, receiver validation,
and completion implementation. `feature_snippets.json` contains the baseline's
46 static snippets; its one-time copier is `generate_snippets.py`. Cargo tests
and the production library do not start Java or invoke the fixture generators.

The optional generators reuse the source oracle runtime helper and pinned
dependency cache. Full generation logs are under `build/expression-source-oracle`.
After restoring the original dependency cache, the source oracle was rerun:
all 184 original assertions passed and its 11,139 captured calls were byte-for-byte
identical to the saved corpus (`build/source-oracle-restored`). The unavailable
Mixin artifact is supplied only to the one-time JVM oracle by the frozen bundle
recorded in `build/migration-dependencies/bundle-fallback.json`; it is appended
after the restored pinned dependencies and instrumented classes.

Final affected verification: `build/rust-test-logs/mixinextras-expression-joined-final-2/`.
The suite passes seven expression replay/state tests, 27 direct model/helper
tests, 31 shared source tests, and three source replay/regression tests.
After delegating receiver validation to the engine's common implementation,
the seven expression tests passed again with all 316 cases and 6,650 outputs
(`build/rust-test-logs/mixinextras-expression-engine-validation`).

The former global isolated-surrogate gap is closed by the facade migration.
`MixinExtrasExpression.values_utf16` and `AnnotationContext`'s UTF-16 prefix are
authoritative when scalar strings cannot represent a JVM value; checked scalar
projection is explicit. The existing decoder/raw maps feed the single engine's
UTF-16 APIs and lossless member-cache keys. Normal strings keep the original
serialized form. `mixin_facade_utf16.rs` exercises actual Java/Kotlin CRLF source,
the public facade, and applied definition edits. The additional official JVM
oracle in `../mixin_facade/utf16-calls.jsonl` compares 30 whole outputs using
numeric UTF-16 units for high/low surrogates, NUL, pairs, and U+FFFD. This extra
fidelity proof is separate from the unchanged original 316 cases.
