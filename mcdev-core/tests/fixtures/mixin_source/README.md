# Source oracle coverage

`source-cases.json` maps every original path and test name to its captured call count.
`source-calls.jsonl` retains those calls' exact inputs, lookup responses, full outputs,
and concrete sealed output classes. `mixin_source_legacy.rs` calls the Rust production
APIs for each row, compares full payloads and variants, and checks every call count.
Only representation differences (UTF-16 source indices versus UTF-8 byte indices,
struct field spelling, known enum spelling, and set order) are normalized. Import
aliases and arbitrary dictionary keys are preserved. The alias collision regression
also checks that normalization cannot erase their differences.

All 184 original tests passed their unchanged assertions during one-time execution
against disposable instrumented copies of the frozen baseline. This produced 11,139
source API calls. Cargo tests require no Java installation, compiler, or subprocess.
The baseline is read only; generator products and full logs live in
`build/source-oracle`. `generate_capture.py`, `Capture.kt`, and `Run.kt` are test-only
fixture generation utilities, not production dependencies.

| Original test file under `mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/` | Cases | Final source API assertions complete | Caller assertions outside source coverage |
| --- | ---: | ---: | ---: |
| AnnotationContextExtractorTest.kt | 117 | 117 | 0 |
| KotlinAnnotationContextExtractorTest.kt | 17 | 17 | 0 |
| KotlinTypeDescriptorResolverTest.kt | 8 | 8 | 0 |
| MixinMemberDeclarationParserTest.kt | 11 | 11 | 0 |
| MixinSemanticModelParserTest.kt | 5 | 5 | 0 |
| KotlinMixinSemanticModelTest.kt | 6 | 2 | 4 |
| MixinTargetResolverTest.kt | 8 | 8 | 0 |
| SemanticCompletionContextExtractorTest.kt | 12 | 3 | 9 |

The zero-call constructor case
`MixinSemanticModelParserTest.kt::retainsExplicitResolvedMixinExtrasContexts` is
ported directly as `mixin_source_models.rs::retains_explicit_resolved_mixin_extras_contexts`.
The four original `ResolvedMixinExtrasContextTest` selector cases are directly ported
in that file as `selector_prefers_unique_annotation_containment`,
`selector_falls_back_to_unique_handler_overlap`,
`selector_returns_null_when_annotation_matches_are_ambiguous`, and
`selector_returns_null_when_handler_overlap_is_ambiguous`.

Of the thirteen composite cases below, the mapped-shadow case is now jointly
complete: its source/model outputs are covered here and the final validator
assertion runs in `mixin_validation_legacy.rs` using the exact original Kotlin
fixture and index. The other twelve are now complete through the actual public
facade and navigation APIs in `mixin_facade_legacy.rs`. Their exact inputs,
complete final outputs, callback order/counts, and applicable materialized edits
are retained in `../mixin_facade/core-calls.jsonl`. Source traces alone were not
used to close those cases.

`KotlinMixinSemanticModelTest.kt`:

- `rawAndQuotedKotlinMethodSelectorsKeepTheRealValueAndHaveNoFalseDiagnostics`
- `originalKotlinKeepsAliasedTargetsArraysAndRangesSeparateAcrossClasses`
- `mappedShadowPropertiesAndPrivateFunctionsMatchTargetDescriptorsWithoutLosingSourceNames` (jointly complete)
- `facadeDiagnosticsAndDefinitionsUseEachKotlinClassScopeIncludingMappedMembers`

`SemanticCompletionContextExtractorTest.kt`:

- `completesInjectMethodFromSemanticContextWithoutFallback`
- `semanticMemberContextDoesNotStealShadowAttributes`
- `shadowAndOverwriteCompletionUseDeclaredMemberNames`
- `overwriteCompletionExcludesConstructorsAndClassInitializersFromEmptyPrefix`
- `shadowCompletionMatchesTheDeclaredMemberKind`
- `shadowCompletionKeepsMethodKindWhenDescriptorIsUnavailable`
- `memberCompletionStopsAtNextMemberAndMethodBody`
- `completesAtTargetFromRecoveredSemanticContextWithoutFallback`
- `completesAtTargetFromLaterConcatenatedLiteralAndKeepsWholeReplacementRange`

Supplemental Rust tests cover UTF-8/UTF-16 boundaries, raw strings, interpolation
conservatism, typed resolved-context retention, and the Java phase-1 Unicode to
phase-2 escape pipeline with raw source offset preservation.
