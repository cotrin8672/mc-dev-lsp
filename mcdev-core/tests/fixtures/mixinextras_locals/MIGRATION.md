# Local capture and Local/Share completion

This domain owns the complete native implementations of LocalCaptureExtractor, LocalCaptureTypeResolver, LocalCaptureValidationService, LocalDiscriminatorResolver, LocalCompletionService, ShareCompletionService and MixinExtrasCompletionService, with LocalCaptureModels appended to the shared `extras_model.rs`.

## Native APIs

`LocalCaptureExtractor::extract` and `extract_with_cancellation` accept optional class bytes, method name/descriptor and a set of real instruction indices, returning `Result<LocalCaptureResult, Cancellation>`. A shared decoder validates the class, method and indices before analysis; an empty request bypasses analysis. The existing expression engine's `AnalysisPolicy::ExactLocalCapture` reuses its classfile reader and CFG. Pre-instruction local frames, LVT live ranges and declared argument descriptors determine candidates, wide slots, argument flags and descriptor ordinals. Receiver locals are excluded from candidates while retaining their ordinal contribution.

`LocalCaptureTypeResolver::{resolve_parameter,resolve_definition}` use the shared Java/Kotlin type resolver and host ClassIndex. `LocalDiscriminatorResolver::resolve` preserves ordinal/index/name/implicit precedence. `LocalCaptureValidationService::validate` uses the shared occurrence resolver and handler type enrichment, distinguishing unavailable, not found, ambiguous and resolved results while retaining raw snapshots. Cancellation remains a typed error with the original shared signal identity.

`LocalCompletionService::complete` validates each proposed selector across every snapshot. `ShareCompletionService::complete` takes a lazy additional-source iterator and filters IDs by target owner/method/descriptor, effective namespace and reference value type. `MixinExtrasCompletionService::{complete,complete_at_target_for_wrap_operation}` delegate to the shared method/At completion services and host bytecode index.

## Exact legacy cases

`legacy_coverage.json` records the exact original test path/class/case, source SHA256 and native test name. The direct set is 79 cases: Extractor 13, TypeResolver 14, Validation 4, LocalCompletion 12, Discriminator 20, MixinExtrasCompletion 11, RuntimeSemantics 5.

All original assertions were first executed against disposable instrumented copies of the frozen Kotlin baseline. The saved fixture contains 2,313 API events, including full service outputs, typed discriminator/validation results, index reads, class bytes, source helper results and cancellation phases. Native tests replay the original service inputs and whole results. RuntimeSemantics also executes the final native expression matcher and identifier-pool builder, checking pool issues and cancellation signal identity; it is not counted from extractor helper traces alone.

Two backend-specific error detail strings (ASM versus the native reader/analyzer) are not compared verbatim. The original corrupt/analysis tests assert the specific error types, not those detail strings. The native replay preserves these typed variants and all stable method/index/count fields, and supplemental tests assert the public message prefixes and payload formatting. No semantic arrays or candidate details are dropped.

## Caller handoff

`caller_handoff.json` explicitly retains the 23 Local/Share Facade routing cases as pending caller integration. Four Share caller tests were executed successfully during the one-time oracle capture. Four observed Share service results from three of those tests are replayed natively as callee evidence, including additional-source iteration count. These do not mark the enclosing Facade test cases complete. In particular, the provider-laziness assertion belongs to the Facade and still requires its native final behavior.

The earlier project `sessionIndexesClasspathClasses` case remains pending HostProjectSession integration in the separate project migration ledger.

## Verification

The saved Rust fixtures require no JVM, JNI or Kotlin compiler. The optional generators reuse the shared migration capture runtime and compile only disposable test sources. No production classpath/source resolver or duplicate classfile reader/CFG was added.

Completed checkpoint: `build/rust-test-logs/mixinextras-locals-legacy-6/replay.stdout.log` and `.stderr.log`: 79 exact cases and nine supplemental cases passed, exit 0, no timeout, 9.9 seconds. The final affected log and optional branch/debug-frame oracle verification are recorded in the completion report. Linux and host/editor/E2E were not executed by this domain agent. No commits were created.

Final affected verification: `build/rust-test-logs/mixinextras-locals-final/affected.stdout.log` and `.stderr.log`, exit 0, no timeout, 22.7 seconds, no warnings. All **273 tests passed**: 79 exact Local cases, ten Local/Share/error/UTF16 supplemental tests, two additional frame cases, plus 182 affected completion regressions/exact cases. Owned rustfmt check also passed.

The two additional frame cases execute all **194 instruction snapshots** of `LocalFlowSamples.java`, compiled once with and without debug information. A disposable JVM oracle ran the existing baseline extractor at each real instruction; the native replay compares every candidate field. Covered paths include equal/different reference merges, loops, exception handlers, constructors, wide slots, arrays/null, switches and Unicode LVT names. Inputs and expected results are saved in `additional_calls.jsonl`; `additional_cases.json` records the successful oracle assertions. The compiler-output class bytes are embedded in those exact JSON request arguments, so no generated `.class` file is required by cargo.
