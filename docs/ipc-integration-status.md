# Published host integration checkpoint

The complete replacement objective is unchanged. This records work in progress
on 2026-10-30; routing coverage is not complete command or editor parity.

## Published contract

The current integration pin is ktlsp
[`ea8be671ed4a876a5d5514fb63daa043e953c08f`](https://github.com/cotrin8672/ktlsp/blob/ea8be671ed4a876a5d5514fb63daa043e953c08f/docs/architecture/extension-ipc.md).
It publishes source snapshots, owned resources, selected runtime bytes, the
physical JVM catalog, and explicit provider command routing. The companion
negotiates capabilities and individual operations; configured command IDs are
intersected with the implemented domain routes.

The host's evaluation lifetime, accumulated reads and final validation govern
these inputs. They do not provide the earlier prototype's eight generation
counters. The evaluation command entry therefore uses no synthetic generation
identity and bypasses cross-evaluation semantic/expression cache reuse.

Physical catalog coverage certifies the selected physical inputs only. Raw ABI,
source editing members and executable runtime bytes remain distinct. Header-only
SDK input cannot supply Code/LVT, and `sourceCurrent: Unknown` does not certify
compiled output against unsaved source.

## Follow-up acceptance checkpoints

The stable native pipeline in
`build/rust-test-logs/native-pipeline-proof-20261030-090912525/` passed 1,777
functions across 76 targets, all 2,604 original JUnit case locators, doc tests,
the companion build and the independent Neovim suite. All 848 input files kept
fingerprint `6466d3e5116c38b64a725bfabf8630e9351f0ffdc8781afb5e5366fc66ecb6ff`.

The real-host receipt in
`build/rust-test-logs/host-commands-20261030-090521312/receipt.json` passes all
101 checks and 23 scenarios on Neovim 0.11.5 in 114 seconds. Host SHA-256 is
`1c08c7769a0fd810a6c8b28f98aefd042dd4f1dda35ca9aa99c7727988d8bf07`;
the independently built companion is
`c5d5bbab4fb5438096c23356c501d303f89ed89393ccdbfdb3a3c7bd3a3ddeba`.
Input hashes are stable and owned processes exit cleanly.

- All eleven configured command IDs are acknowledged. Completion,
  diagnostics/context, hover and definition retain the bounded full-DTO checks.
- AW-header references and Kotlin code actions match the original empty DTOs.
  Java/Kotlin empty references require normalized, high-confidence facts;
  fallback emptiness cannot certify absence.
- The six project-dependent cases match exact typed incomplete envelopes.
  Both refresh commands complete their real host refresh before returning that
  result. This does not certify the missing project summary or index state.
- Public definition navigation follows an unsaved Unicode `@Definition` through
  the real provider and `show_document`, checking the URI and actual byte cursor
  obtained from UTF-16 positions. Stale versions, cancellation, disable/re-enable
  and restart retain their assertions.
- The unchanged original `e2e/minimal_init.lua` supplies the actual runtime path
  and swapfile settings. Its frozen and Windows input differ only by LF/CRLF.
  This verifies its support role, with zero invented original assertion sites.

Earlier failed runs remain recorded: the first used an untrusted mise Gradle
shim; the second abandoned cold indexing and refresh requests too early. The
runner now requires completed indexing and accepted source hashes, cancels a
timed-out request and aborts subsequent cases. Indexing has a 180-second limit,
refresh requests 120 seconds, and the editor process a 600-second outer limit.
It never extends a timed-out run.

The Windows optimized package in
`build/rust-test-logs/native-release-20261030-075035279/` verifies all 24 payload
hashes, stdio completion, semantic-only fallback and exact optional negotiation.
The required base remains `semantic-v1`; six optional capabilities describe
eleven input operations plus configured provider commands. Its archive SHA-256
is `8af213217c9e4d57fea09855f0f1009ba3c7244c12e1812cb5e71122d7fb8858`.
The real-host receipt above uses the debug executable, not this optimized binary.

The separate runtime-input proof in
`build/rust-test-logs/runtime-bytes-probe-20261030-084946767/receipt.json` passes
12 checks using a test-only Rust provider and the production command actor and
typed input reader. All 837 bytes match the debug Gradle output, SHA-256
`af0aa4ea131c3dacacb7b3f7ce0b9d8bc0bb5d929694502d6f50e5bdfeb390bd`.
Rust decodes the six instructions of `draw(Ljava/lang/String;FF)V`, its real
`String.length()I` invocation, and `text` at LVT slot 1 with descriptor
`Ljava/lang/String;`. `sourceCurrent` remains Unknown.

This run uses recognized Gradle 9.6.1. The earlier 9.8.0 run returned
`BYTECODE_UNAVAILABLE / IncompleteRuntimeModel` with absent SDK producer claims
and an unsupported Java-recipe adapter. The isolated distribution is checked
against [Gradle's official checksums](https://gradle.org/release-checksums/).
The probe does not exercise the product Local-completion command path or prove
the original Sugar workflow; nested annotations and source authority remain
required.

The four original CEM feature-snippet scenarios also pass through actual
provider responses in
`build/rust-test-logs/cem-snippets-20261030-085920361/receipt.json`: Inject,
Implements, Descriptors and Group. Their shared original group preserves 20
assertion sites, with 169 total assertion calls in the live entry. Complete
expanded buffers, every replacement/import edit, native tabstops, unsaved
state and unchanged disk are checked. A separate preflight establishes provider
readiness; the four cases each retain their own actual response. The fix also
clears an old loopback-transport error only after a valid, current provider
completion, preserving genuine failures and local-helper fallback behavior.
Method/field, Shadow and Blink assertions remain unverified. The original Lua
file record is therefore partial: 17 files are ported, three partial and seven
pending, with full original editor parity still false.

## Earlier focused boundary checks

These are separate proofs of individual boundaries, before the full integrated
pipeline and stock-host command/editor acceptance:

| Boundary | Evidence |
| --- | --- |
| Typed host input reader | 15 tests passed; fixed source/resource serializer frames, pagination, batching, opaque handle scopes, Unknown/absence, Unicode, byte offsets/lengths/base64/SHA and header-only byte refusal. Logs: `build/rust-test-logs/host-inputs-20261029-102054214/`. |
| Evaluation entry into the real core | 22 tests passed, including the existing fourteen command cases and eight evaluation cases; complete core DTOs/config edits, stale/cancellation, Unknown, native roots, host refresh authority and no fabricated generation keys or LSP document versions. Logs: `build/rust-test-logs/eval-semantic-20261029-104134693/`. |
| Provider command actor | Ten command tests and four server tests passed; configured/implemented ACK intersection, exact provider serializer frame, cancellation/late replies, fatal/stale evaluation closure and separate metadata/stream quotas. Existing lifecycle ten tests also passed. Logs: `build/rust-test-logs/provider-actor-acceptance2/` and `provider-actor-lifecyclefinal/`. |
| Real companion process with generic host stubs | Two framed process tests passed, including stale context refusal, buffer-only completion and scoped input reads. This runs the executable but does not run stock ktlsp. Logs: `build/rust-test-logs/ipc-command-process-20261029-103716402/`. |
| Host semantic projection | Eleven tests passed for checked host type/member identities, ranges, annotation values, current versions and cancellation. Unprovided annotation/full-declaration facts remain unavailable or explicitly low-confidence fallback. Logs: `build/rust-test-logs/eval-semantic-20261029-104134693/`. |
| Neovim provider command routing | The existing adapter entry plus new routing tests pass on Neovim 0.11.5 and 0.12.5. All eleven outer command shapes, real LSP text/version, unsaved didChange, raw resource didOpen, allowlists, disabled providers and workspace boundaries are exercised. Logs: `build/nvim-test-logs/ktlsp-provider011-final5.*` and `ktlsp-provider012-final5.*`. This is not the stock-host end-to-end run. |

These focused results do not replace the original complete editor workflows.
The follow-up host receipt above retains its narrower fixture scope.

## Concrete remaining input differences

An adapter issue remains open in addition to the producer gaps below:
configuration-dependent diagnostics still select a Mixin config from the
incomplete owner-scoped project projection. Other source-set resources visible
through a compile classpath can be misclassified as shared, so a known config
snapshot does not certify the original selection or its negative registration
diagnostic. This needs a selection-authority guard and a counterexample test;
the passing single-config fixture is not proof of general correctness. This
work was recorded, without implementation, when the user requested stopping.

Requests with these examples have been sent to the ktlsp task. Public operations
are used where available; missing facts remain typed unavailable.

- The current source/project/resource surfaces select the owner's module/source
  set. The original project dump and reference/config searches also consume other
  source sets. A main-owned request must not silently omit a test-only reference
  or advertise a complete project dump from the main inventory alone.
- Current normalized annotation values do not represent nested annotations,
  integer/enum values or escaped/folded string/source correspondence. For example,
  `@Inject(method="tick()V", at=@At(value="INVOKE", target="Lpkg/C;m()V", ordinal=1))`
  needs the nested annotation, typed ordinal and exact ranges. Authored full/header
  declaration ranges and source modifiers are also needed by existing consumers.
  A child parser must not turn missing host semantic authority into normalized facts.
- The additional injection-point index needs subtype annotations such as
  `@AtCode(value="CUSTOM", namespace="example")` from compile-only inputs too.
  Catalog ABI attribute names alone do not supply those values, and runtime-byte
  tokens cannot be used to read a compile-only class.
- Source ownership/current member coverage must stay distinct from physical SDK
  and dependency ABI. Unknown source freshness cannot erase a known unsaved
  override; it also must not prevent legitimate physical/runtime queries from
  consuming their independently authoritative inputs.
- A `typeJvm` direct-member list does not identify its representation as current
  authored source or certify that it matches the physical ABI. Independently
  issued opaque type tokens need not be identical even for the same internal
  type. Future adapters must preserve the selected host identity and obtain
  completeness/provenance authority before exposing a complete editing list.

All eleven commands remain required. A temporarily smaller negotiated command
set, typed unavailable responses, or a passing five-command smoke is not the
completion condition. Full original Neovim behavior, live applied edits,
unsaved-update/lifecycle tests, packaging and final legacy runtime retirement
remain required.

## Published-input cross-platform checkpoint

Commit `cc55f89335a070988838ca301ef094f73fe83462` passed
[GitHub Actions run 40886521996](https://github.com/cotrin8672/mc-dev-lsp/actions/runs/40886521996).
Downloaded logs and payloads independently resolve all 2,604 original JUnit
case locators: Windows has 1,765 native functions and 2,494 editor assertions;
Linux has 1,764 functions and 2,455 editor assertions. Both have 75 native
targets and zero failed, ignored or filtered native tests. The Windows-only
teardown process entry does not execute assertions on Linux.

Both optimized archives have all 24 payload hashes and the manifest verified,
correct PE/ELF executables and successful stdio smoke results. The exact archive
and executable hashes are retained in `rust-test-migration.json` under
`verification.nativeCi`; raw artifacts are in `build/ci-runs/40886521996/`.
This validates the published-input checkpoint, before the follow-up six command
routes, package capability metadata and editor-harness changes. It does not
establish full Minecraft host integration.
