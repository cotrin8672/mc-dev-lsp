# Frozen command caller corpus

The frozen Kotlin baseline is `build/migration-baseline/current-20261023`.
`original_cases.json` records every original path, method and source SHA256;
its `pending` field is the untouched initial inventory, not a native result.
`legacy_cases.json` records the successful one-time execution of all 127 original
methods and their original assertions. `legacy_calls.jsonl` stores 1,015 outputs
and callback read traces from that execution. Of these, 474 are top-level caller
observations; nested outputs are not counted as extra cases.

`commands_legacy.rs` contains one independently named Rust test per original ID.
The manifest test compares its exact executable ID/function set with the frozen
127 IDs and the successful capture set. `coverage.json`, generated only from a
completed successful Rust test log, gives each original ID's native function,
strategy and observation count.

The native replay executes the Rust dispatcher, delegate or handler path through
the existing protocol decoder and converters. `RecordedHost` supplies the
original domain callback results, verifies each callback's source, URI, position,
options and versions, and checks the exact high-level callback sequence. Runtime
instances and their negative/document caches persist for the original handler
identity and original request order. Symbolic AW/AT/Mixin fixes are materialized
by the real Rust core editors, then converted into the final handler DTO.
This is caller proof with authoritative fake host facts; actual core facades and
host adapters have their own corpus. It does not claim a connected command wire.

All output fields remain compared, including error envelopes, diagnostics,
metadata, edits, imports, snippets, capabilities, debug facts and cache flags.
Capabilities are sets. Gson's integral `1.0` and Rust's integral `1` are compared
as equal JSON numbers, without string coercion or field removal. Exactly 1,299
named elapsed-duration observations in `timing_assertions.json` execute the
original nonnegative-integer predicate instead of matching wall-clock duration.
Forwarded metrics and constant-zero branches remain exact. The single approved
AW duplicate-editor adaptation in `explicit_deltas.json` preserves original and
new DTO edits, compares both applications to the same complete original text,
and compares every remaining field strictly.

The HTTP cases run actual loopback TCP connections with original request bodies,
authentication, two concurrent clients, deadline, disconnect and teardown
assertions. Rust callbacks observe cooperative cancellation instead of JVM
`Thread.interrupt`; the deadline and cancellation are still observed at the
actual transport boundary. The stdio case runs the actual executable with
`--stdio-completion`, sends the original five-request sequence, and checks every
original response and EOF assertion. Normal semantic-v1 mode remains separate.

One-time capture uses the shared source oracle's `Capture.kt` and runtime
helpers, with the shared runner's test lifecycle/temporary-directory hooks.
`generate_capture.py` prepends frozen protocol/JDT main/test classes and Eclipse
test stubs. LSP4j 0.23.1, its JSONRPC artifact and OSGi core 6.0.0 are compiler-only
and are absent from the capture runtime, preserving reflection fallback branches.
Disposable generated classes and complete compile/capture logs are under
`build/commands-oracle/`. Tests execute unchanged original assertions; monitor
queries and callback arguments are never evaluated an extra time. No JVM,
reflection, original classpath scanner or oracle generator runs under `cargo test`.

## Verified checkpoint

All 127 original cases and seven supplemental checks passed through the finite
compact runner: exit 0, no timeout, 62.3 seconds against a 180-second deadline.
Full logs: `build/rust-test-logs/commands-20261024-154739853/commands.*.log`.
The subsequent complete companion-crate run passed all 163 tests, including the
previous 29 semantic/framing/lifecycle tests: exit 0, no timeout, 43.9 seconds
against a 180-second deadline. Full logs:
`build/rust-test-logs/companion-20261024-154945827/companion.*.log`.
Both runs execute the real no-index child. Its exact request/response lines and
stderr are saved under `build/command-line-process-logs/`.

| Original class | Cases |
|---|---:|
| McdevCommandDispatcherTest | 16 |
| McdevDelegateCommandHandlerTest | 3 |
| KotlinMixinRequestHandlerTest | 3 |
| McdevAwAtCodeActionHandlerTest | 2 |
| McdevAwAtCompletionHandlerTest | 6 |
| McdevAwAtDiagnosticsHandlerTest | 6 |
| McdevCodeActionHandlerTest | 13 |
| McdevCompletionHandlerTest | 35 |
| McdevDefinitionHandlerTest | 10 |
| McdevDiagnosticsHandlerTest | 9 |
| McdevInfoHandlerTest | 5 |
| McdevProjectContextHandlerTest | 3 |
| McdevReferencesHandlerTest | 5 |
| McdevReindexHandlerTest | 3 |
| CompletionTruncationTest | 2 |
| McdevCompletionTransportTest | 5 |
| McdevStdioMainTest | 1 |

The supplemental checks cover the exact ID/function set, collision-safe document
and negative caches with expiry/eviction, in-flight request sequencing, malformed
line recovery and ID retention, UTF-8 HTTP byte lengths and validation recovery,
runtime-error versus cancellation responses, and overlapping authenticated
computations. A detected Windows unread-body TCP reset was fixed with a response
FIN and bounded (100 ms / 64 KiB) rejection drain; both final runs include the
original unauthenticated-request assertions.

No original caller case remains pending. Live command/resource/class-byte wire
capabilities and real host/editor command acceptance remain pending parent-owned
integration and are not advertised by this library or helper.
