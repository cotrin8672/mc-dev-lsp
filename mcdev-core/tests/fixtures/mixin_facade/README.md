# Facade migration proof

`coverage.json` maps all **189 exact original IDs** and frozen test-file hashes to
their native Rust assertion, input/output corpus, final API rows, and call counts.
The one-time JVM run executed every original assertion unchanged: core 159 and
host/JDT boundary 30. Cargo tests invoke production Rust APIs; no JVM is used at
test/runtime. `generate_capture.py` uses disposable source copies, the shared
source recorder, restored pinned dependencies, and finite logged JVM runners.

The corpus includes whole `complete`, `completeWithDebug`, `diagnose`,
`codeActions`, host semantic/environment, and `tryCompleteBufferOnly` results.
It also includes **22 navigation final outputs** from the composite Kotlin case
and **3 actual apply-fix outputs**. Native replay materializes those document
edits. Capturing a facade's source callee alone does not close a caller case.
The original zero-call `overwriteAnnotationIsRecognized` assertion is direct.
Joined handler-site outputs retain the original helper assertions as well.

The post-review audit retains all 189 frozen test bodies' hashes and 437 static
assertion expressions in `assertion-audit.json`. This count is not expanded by
loops. It checks direct result-producing calls as well as calls following an
already captured facade result. Three original action cases now have independent
native `fixes_for_diagnostics` tests with the complete frozen `FakeClassIndex`
facts, exact diagnostic metadata, and the original accessor/invoker construction.
The Shadow routing case also directly asserts `complete_fields`. These final
assertions were missing from the earlier callee-only replay and are now named
explicitly in `coverage.json`; their old source traces remain supplemental.
The three cases which directly assert only a source API identify those whole
source returns as their final API rows. The zero-call enum case points to its
actual native enum test.

The Kotlin host source case now asserts the original selected config ref's full
value and the complete merged source-entry set, besides its registered sequence.
It exposed a production parity error: selection re-read JSON and returned a
different parsed ref. Selection now uses the original host context refs, with
source-set priority and shared fallback, and performs zero provider reads.
`build_request` still reads selected content separately. The two additional
computed edit tails execute actual native workspace edits/snippet insertion
and retain all original post-application substring assertions. These repairs
belong to the original 189 cases and do not add to the legacy case count.

Class/index/bytecode inputs and external callback outputs are complete typed
fixtures. Native replay checks order, count, arguments, and sequential return
values, including lazy project resolution, feature probes, custom At values,
factories, source providers, and override callbacks. Cancellations retain the
same error `Arc`; the next request runs on the **same** facade. Saved/updated/
deleted registered sources also run against real native files through a single
live provider. An authoritative `SourceBackedClassIndex` test makes every binary
delegate operation panic, proving all five original requests stay source-only.

Only three exact `MixinServiceFacadeBufferOnlyTest` cases normalize private
no-op singleton method observations: `bufferOnlyWrapCompletionOmitsUnknownWrapWithConditionVariants`
(2 empty calls), `definitionIdCompletionDoesNotLeakAdjacentHandlerDefinitions`
(8), and `definitionIdCompletionWorksWithoutJavaProjectResolver` (8). Their
original tests assert final items/availability and external provider laziness,
not these internal singleton calls. Every observed singleton value is checked
to be null/empty; all external callbacks and every final output remain strict.
The adapter rejects this normalization for any other case or nonempty value.
This bounded representation change was reviewed by the parent task. General
ClassIndex reads and unavailable host facts are never filtered.

The shared debug/result DTOs live in `src/mixin_facade.rs`. The host adapter
accepts the canonical host semantic provider/cache, already composed source-first
indices, and owned project identities with all eight generation dimensions.
It stores no opaque evaluation handles. Negative read validation and generation,
document-version, source-hash, transient-answer, unknown, cancellation, and
recovery behavior run through the public host pipeline in
`mixin_host_facade_state.rs`. Kotlin uses the legacy fallback for a Java-only
provider; an integrated host explicitly enables normalized Kotlin semantics.
Shared expression caches require a matching complete project/generation key;
otherwise the request gets a fresh cache.

`Utf16FacadeOracle.kt` is an additional official JVM oracle. It crosses JSON
using **numeric UTF-16 units**, retaining lone high/low surrogates, NUL, pairs,
and U+FFFD. `utf16-calls.jsonl` has 30 whole context/prefix/public facade outputs
for Java and Kotlin CRLF source. Native tests compare all of them, call the
single official Rust engine, and apply definition/import edits without changing
the original raw literal. Further tests distinguish U+FFFD, raw Kotlin strings,
and cancellation from Option-returning inference. This proof closes the former
global source/context scalar-string gap; normal-string serialization is unchanged.

Host source/semantic **wire capabilities remain a separate integration task**.
These native tests supply complete owned host facts. Missing real generation
dimensions must bypass caches or remain unknown; they are never asserted to be
known generation zero. The pure buffer-only path builds no host session, reads
no filesystem/classpath/source provider, and only evaluates feature callbacks
when requested by At-value completion.

Finite verification and source hashes are recorded in `verification.json`.
