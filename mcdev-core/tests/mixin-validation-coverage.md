# Mixin validation, diagnostics and actions: exact legacy cases

Scope is the frozen `current-20261023` baseline, not whichever examples happened to pass. Tests retain original case IDs, original input values/source snippets, FakeClassIndex/FakeBytecodeIndex facts, and original assertions. Closely related supplemental regressions remain separate from the legacy corpus.

## Evidence

- `mixin_validation_legacy.rs` directly ports every `ShadowAccessorInvokerTest` case by its original method name and arguments/range.
- `mixin_actions_legacy.rs` directly ports every `MixinCodeActionServiceTest` case by its original method name, sources, JSON/JSON5 content, fix fields and assertions. Generated fixes are additionally applied to the complete document.
- `fixtures/mixin-diagnostics-legacy.json` contains each original `MixinDiagnosticsServiceTest` input plus all six relevant `MixinEdgeCasesTest` inputs; original test bodies are retained beside executable code/count/metadata/message/range expectations. `mixin_diagnostics.rs::every_original_mixin_diagnostics_case_uses_exact_input_and_assertions` executes each row and reports failures with the original ID. The custom foo/bar method and return-count fixture is also reproduced.
- `mixin_validation_support::LegacyIndex/LegacyBytecode` retain the complete original default fixture facts. Supplemental `Index/Bytecode` fixtures are used only for extra branch/Unicode/static/Kotlin checks.

## Original cases -> executable assertions

### `ShadowAccessorInvokerTest.kt`

| Exact original case | Rust test |
|---|---|
| `shadowFieldValidationPassesForValidTarget` | [`shadowFieldValidationPassesForValidTarget`](mixin_validation_legacy.rs#L74) |
| `shadowFieldValidationFailsForMissingTarget` | [`shadowFieldValidationFailsForMissingTarget`](mixin_validation_legacy.rs#L85) |
| `shadowFieldStaticMismatchProducesDiagnostic` | [`shadowFieldStaticMismatchProducesDiagnostic`](mixin_validation_legacy.rs#L98) |
| `shadowFieldValidationFailsForDescriptorMismatch` | [`shadowFieldValidationFailsForDescriptorMismatch`](mixin_validation_legacy.rs#L111) |
| `shadowMethodValidationPassesForValidTarget` | [`shadowMethodValidationPassesForValidTarget`](mixin_validation_legacy.rs#L124) |
| `shadowMethodValidationFailsForMissingMethod` | [`shadowMethodValidationFailsForMissingMethod`](mixin_validation_legacy.rs#L130) |
| `shadowPrefixResolutionStripsPrefix` | [`shadowPrefixResolutionStripsPrefix`](mixin_validation_legacy.rs#L143) |
| `accessorInfersGetterFieldName` | [`accessorInfersGetterFieldName`](mixin_validation_legacy.rs#L154) |
| `accessorInfersSetterFieldName` | [`accessorInfersSetterFieldName`](mixin_validation_legacy.rs#L163) |
| `accessorValidationPassesForValidGetter` | [`accessorValidationPassesForValidGetter`](mixin_validation_legacy.rs#L172) |
| `accessorValidationFailsForMissingField` | [`accessorValidationFailsForMissingField`](mixin_validation_legacy.rs#L181) |
| `accessorValidationFailsForSignatureMismatch` | [`accessorValidationFailsForSignatureMismatch`](mixin_validation_legacy.rs#L192) |
| `accessorFieldCompletionReturnsCandidates` | [`accessorFieldCompletionReturnsCandidates`](mixin_validation_legacy.rs#L203) |
| `accessorGeneratesGetterStub` | [`accessorGeneratesGetterStub`](mixin_validation_legacy.rs#L211) |
| `invokerInfersTargetName` | [`invokerInfersTargetName`](mixin_validation_legacy.rs#L218) |
| `invokerValidationPassesForValidTarget` | [`invokerValidationPassesForValidTarget`](mixin_validation_legacy.rs#L226) |
| `invokerValidationFailsForMissingMethod` | [`invokerValidationFailsForMissingMethod`](mixin_validation_legacy.rs#L235) |
| `invokerValidationFailsForDescriptorMismatch` | [`invokerValidationFailsForDescriptorMismatch`](mixin_validation_legacy.rs#L246) |
| `invokerMethodCompletionReturnsCandidates` | [`invokerMethodCompletionReturnsCandidates`](mixin_validation_legacy.rs#L257) |
| `shadowMethodCompletionExcludesClassInitializers` | [`shadowMethodCompletionExcludesClassInitializers`](mixin_validation_legacy.rs#L264) |
| `invokerGeneratesMethodStub` | [`invokerGeneratesMethodStub`](mixin_validation_legacy.rs#L285) |

### `MixinCodeActionServiceTest.kt`

| Exact original case | Rust test |
|---|---|
| `producesAddMixinConfigFix` | [`producesAddMixinConfigFix`](mixin_actions_legacy.rs#L51) |
| `addMixinConfigFixUsesClientArrayForFabricClient` | [`addMixinConfigFixUsesClientArrayForFabricClient`](mixin_actions_legacy.rs#L82) |
| `addMixinConfigFixUsesServerArrayForNeoForgeDedicatedServer` | [`addMixinConfigFixUsesServerArrayForNeoForgeDedicatedServer`](mixin_actions_legacy.rs#L88) |
| `addMixinConfigFixUsesMixinsArrayWhenNoSideAnnotationPresent` | [`addMixinConfigFixUsesMixinsArrayWhenNoSideAnnotationPresent`](mixin_actions_legacy.rs#L93) |
| `addMixinConfigFixUsesMixinsArrayForConflictingSideAnnotations` | [`addMixinConfigFixUsesMixinsArrayForConflictingSideAnnotations`](mixin_actions_legacy.rs#L98) |
| `applyMixinConfigFixTargetsClientArrayFromFix` | [`applyMixinConfigFixTargetsClientArrayFromFix`](mixin_actions_legacy.rs#L103) |
| `applyMixinConfigFixTargetsServerArrayFromFix` | [`applyMixinConfigFixTargetsServerArrayFromFix`](mixin_actions_legacy.rs#L119) |
| `applyMixinConfigFixAddsEntryDeterministically` | [`applyMixinConfigFixAddsEntryDeterministically`](mixin_actions_legacy.rs#L135) |
| `applyMixinConfigFixUsesLosslessDeltaForJson5Comments` | [`applyMixinConfigFixUsesLosslessDeltaForJson5Comments`](mixin_actions_legacy.rs#L171) |
| `applyMixinConfigFixUsesLosslessDeltaForCrlf` | [`applyMixinConfigFixUsesLosslessDeltaForCrlf`](mixin_actions_legacy.rs#L175) |
| `applyMixinConfigFixSkipsDuplicate` | [`applyMixinConfigFixSkipsDuplicate`](mixin_actions_legacy.rs#L179) |
| `doesNotGuessDescriptorForAmbiguousMethod` | [`doesNotGuessDescriptorForAmbiguousMethod`](mixin_actions_legacy.rs#L188) |
| `producesGenerateAccessorFix` | [`producesGenerateAccessorFix`](mixin_actions_legacy.rs#L216) |
| `producesGenerateInvokerFix` | [`producesGenerateInvokerFix`](mixin_actions_legacy.rs#L236) |
| `doesNotOfferQuickfixForUnresolvedHandlerDescriptor` | [`doesNotOfferQuickfixForUnresolvedHandlerDescriptor`](mixin_actions_legacy.rs#L256) |

### Shared Kotlin composite case

`KotlinMixinSemanticModelTest.mappedShadowPropertiesAndPrivateFunctionsMatchTargetDescriptorsWithoutLosingSourceNames`: the exact Kotlin source and target member facts are retained in `fixtures/mixin-kotlin-shadow-legacy.kt` and `mixin_validation_legacy.rs` under the same original name. This executes the declaration-name mapping, hidden name-range mapping, validation-result-empty, and declaration-diagnostics-empty assertions. Complete model assertions are owned by the source parser oracle; this shared validation tail is not double-counted below.

### `MixinDiagnosticsServiceTest.kt`

| Exact original case | Executed fixture row |
|---|---|
| `reportsUnresolvedMixinClassTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedMixinClassTarget` |
| `reportsUnresolvedMixinStringTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedMixinStringTarget` |
| `noUnresolvedForValidClassTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.noUnresolvedForValidClassTarget` |
| `reportsMissingMixinConfigEntry` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsMissingMixinConfigEntry` |
| `acceptsRelativeEntryForMixinSubpackage` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.acceptsRelativeEntryForMixinSubpackage` |
| `rejectsSimpleEntryForMixinSubpackage` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.rejectsSimpleEntryForMixinSubpackage` |
| `missingSubpackageEntryUsesConfigRelativeName` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.missingSubpackageEntryUsesConfigRelativeName` |
| `duplicateMixinConfigEntryRangesConfigEntry` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.duplicateMixinConfigEntryRangesConfigEntry` |
| `reportsUnresolvedInjectMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedInjectMethod` |
| `reportsAmbiguousInjectMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsAmbiguousInjectMethod` |
| `reportsUnresolvedJavaTypeInInvokerDeclaration` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedJavaTypeInInvokerDeclaration` |
| `analyzesInjectMethodArray` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.analyzesInjectMethodArray` |
| `reportsDescriptorMismatchForInjectMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsDescriptorMismatchForInjectMethod` |
| `reportsUnresolvedAtTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedAtTarget` |
| `analyzesAtArraySeparately` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.analyzesAtArraySeparately` |
| `reportsUnresolvedAtTargetForDescriptorBearingInjectMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedAtTargetForDescriptorBearingInjectMethod` |
| `reportsInvalidAtTargetDescriptor` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsInvalidAtTargetDescriptor` |
| `diagnosticsUseCompleteConcatenatedAtTargetValue` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.diagnosticsUseCompleteConcatenatedAtTargetValue` |
| `reportsOrdinalOutOfRange` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsOrdinalOutOfRange` |
| `noDiagnosticForValidAtTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.noDiagnosticForValidAtTarget` |
| `injectMethodDiagnosticUsesAbsoluteSourceOffset` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.injectMethodDiagnosticUsesAbsoluteSourceOffset` |
| `injectMethodDiagnosticUsesAbsoluteSourceOffsetWithCrlf` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.injectMethodDiagnosticUsesAbsoluteSourceOffsetWithCrlf` |
| `atTargetDiagnosticRangesTargetValueInSource` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.atTargetDiagnosticRangesTargetValueInSource` |
| `returnOrdinalValidationUsesContainingInjectorMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.returnOrdinalValidationUsesContainingInjectorMethod` |
| `reportsUnresolvedFullyQualifiedMixinClassTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedFullyQualifiedMixinClassTarget` |
| `reportsUnresolvedFullyQualifiedMixinArrayTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinDiagnosticsServiceTest.reportsUnresolvedFullyQualifiedMixinArrayTarget` |

### `MixinEdgeCasesTest.kt`

| Exact original case | Executed fixture row |
|---|---|
| `overwriteValidationPassesForMatchingTargetMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinEdgeCasesTest.overwriteValidationPassesForMatchingTargetMethod` |
| `overwriteValidationReportsMissingTargetMethod` | `fixtures/mixin-diagnostics-legacy.json`: `MixinEdgeCasesTest.overwriteValidationReportsMissingTargetMethod` |
| `reportsDuplicateMixinClassTargetInArrayForm` | `fixtures/mixin-diagnostics-legacy.json`: `MixinEdgeCasesTest.reportsDuplicateMixinClassTargetInArrayForm` |
| `reportsDuplicateMixinStringTarget` | `fixtures/mixin-diagnostics-legacy.json`: `MixinEdgeCasesTest.reportsDuplicateMixinStringTarget` |
| `reportsDuplicateMixinConfigEntry` | `fixtures/mixin-diagnostics-legacy.json`: `MixinEdgeCasesTest.reportsDuplicateMixinConfigEntry` |
| `reportsOrdinalOutOfRangeForReturnAt` | `fixtures/mixin-diagnostics-legacy.json`: `MixinEdgeCasesTest.reportsOrdinalOutOfRangeForReturnAt` |

Total exact original cases in this bounded task: **68**.

## Supplemental branch checks and intentional corrections

`mixin_validation.rs`, `mixin_diagnostics.rs`, and `mixin_actions.rs` add all ambiguity policies, exact later overloads, static mismatches, empty/multiple owners, getters/setters/explicit names, constructor exclusion, invalid descriptor paths, byte/UTF-16 boundaries, CRLF, stale edits, aliases/known Kotlin source, lossless config deltas, and language-aware method generation. No diagnostic code/kind is renamed.

The standalone `InvokerService::generate_method_stub` keeps legacy readable-signature formatting. Actual generated edits use descriptor-derived valid Java/Kotlin signatures instead of inserting the old display signature (`setScreen(Screen): void`) into Java source. They use the existing import edit builder for generated annotation references and checked text edits; unknown descriptors and ambiguous target overloads do not get guessed fixes.

## Explicitly pending outside this task

The full facade, navigation/reference routing, MixinExtras diagnostics, JDT-specific adapters, host-normalized identity/snapshot lifecycle, and editor IPC E2E are owned by their corresponding tasks. Their original tests are not counted as ported here. `MixinDiagnosticsE2ETest`, `MixinCodeActionE2ETest`, facade routing and cross-language Kotlin facade assertions need the complete facade. Kotlin source/parser corpus belongs to `mixin_source`; pure validation is ready for its parsed declarations.

This file records scope and test provenance; actual pass/fail evidence is the compact runner output and complete logs, not this table.
