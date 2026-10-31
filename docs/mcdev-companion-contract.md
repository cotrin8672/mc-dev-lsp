# mcdev standalone companion

`mcdev-companion` is a separate Rust executable. Its dependency graph contains
`mcdev-core`, `mcdev-protocol`, serde, serde_json, SHA-256 and the OS random-source helper;
it does not link ktlsp or ktcore. The linked `mcdev-ktlsp` prototype has been
retired; its thirteen scenario families execute in companion process tests,
and its original test source is preserved with provenance in the fixtures.

The original semantic-v1 stock-host checkpoint was
`f76a43fbeddf4fbdc69768fd0f7544e5a4a3610f` in cotrin8672/ktlsp.
`docs/architecture/extension-ipc.md`, `crates/ktlsp/src/extension_ipc.rs` and
`crates/ktlsp/src/analysis_extensions/ipc.rs` define the wire. The host's
`dev/external-extension/{src/bin/ipc-child.rs,ipc-manifest.json,ipc-probes.lua,README.md}`
contains the separate process/editor fixture. Host revisions must be pinned
alongside this experimental contract. Current command/input integration uses
[`2e4671f`](https://github.com/cotrin8672/ktlsp/blob/2e4671f085f199ea5bb970bbc2f9e15e2c07dff1/docs/architecture/extension-ipc.md).
The exact verified scope and remaining gaps are recorded in
[the integration checkpoint](ipc-integration-status.md); the historical traces
below retain their original artifact pins.

An explicit registration uses an executable/argv array, for example:

```json
{"analysisProviders":{"mcdev.mixin":{"enabled":true,"command":["C:/absolute/path/mcdev-companion.exe"],"configuration":{}}}}
```

The host spawns the process, retains the editor's sole LSP connection and owns
unsaved Java/Kotlin parsing, classpath/project selection, source hydration,
identity issuance, member solving, generations, acceptance and publication.
The child consumes owned normalized annotations/declarations and host-issued
opaque handles. It proposes requirements, sites and diagnostics. It creates no
language parser, workspace scanner, classpath index, JVM helper or second LSP.
Minecraft annotation identity and JVM descriptor validation reuse mcdev-core.

## Transport and lifecycle

stdin/stdout carry UTF-8 JSON-RPC 2.0 with byte-counted Content-Length frames;
stderr carries process errors only. Initialization negotiates major 1/minor 0
with `semantic-v1` as its base. Offered optional capabilities and their operations
are acknowledged by intersection. No native Rust ABI is exposed. The process
accepts repeated evaluations while retaining no previous evaluation's handles.

Frames are limited to 1 MiB, headers to 4 KiB and the reader queue to 32 frames.
The child limits one evaluation to 25 seconds, 64 metadata batches and 16,384 opaque
handles; fact batches contain at most 256 queries. Stream continuations have a
separate 256-query/64 MiB serialized-reply budget, so a class byte stream does not
consume one metadata batch per chunk. Invalid framing terminates
the child. Invalid source facts or failed host batches produce an evaluation
error without publishing partial contributions. Host acceptance remains the
final authority for stale generations and foreign handles.

Provider configuration is opaque JSON. Objects, arrays, scalars and null are
valid; Neovim's empty Lua table can encode as an empty array. A captured actual
host frame and native regressions preserve this boundary.

`analysis/evaluate` uses the evaluation token as its request ID.
`analysis/readFacts` and `analysis/require` requests carry that same evaluation
token and independent monotonically allocated request IDs. Cancellation while
waiting for a batch discards all accumulated sites/diagnostics and acknowledges
the evaluation with an empty contribution set. A late reply to that cancelled
batch is ignored, without being consumed by the next evaluation. `Stale` host
errors receive the same treatment. The host still rejects stale acceptance.
Foreign response IDs never become successful bindings or missing-target
diagnostics. EOF, shutdown and exit also interrupt an outstanding host read.

The published host currently shuts down by closing/reaping its owned child.
The companion additionally understands JSON-RPC shutdown/exit for a direct
client; this does not claim the host implements that graceful exchange.

## Provider commands and owned inputs

The production child also negotiates `project-class-facts-v1`,
`source-snapshots-v1`, `owned-resources-v1`, `runtime-class-bytes-v1`,
`jvm-catalog-v1`, and `provider-commands-v1` when the host offers them.
Typed readers preserve Unknown, incomplete coverage, origin/order and opaque
evaluation-scoped tokens. Reassembled resource/runtime bytes are checked against
their identities, offsets, lengths and SHA-256. Catalog ABI never grants runtime
Code access or certifies current unsaved source.

For commands, registration uses `analysisProviders.<id>.executeCommands`.
The configured IDs are intersected with the companion's implemented routes;
all eleven command IDs can be acknowledged. A route can return a typed
`INCOMPLETE_PROJECT_CONTEXT` result: acknowledgement does not certify complete
feature support. Project-wide references, Java code actions, info/dump and
refresh summaries still need complete host inputs. Resource-header references
and Kotlin code actions retain the original empty success DTOs where absence
can be established independently. Low-confidence Java/Kotlin source models
cannot certify empty reference targets.

The editor sends `workspace/executeCommand` with the outer command
`ktlsp.providerCommand`. The host validates its provider/document/workspace and
forwards the original domain arguments to `analysis/executeCommand`. The companion
keeps the existing decoder, complete result-or-error DTOs and typed converters.
Outer host context uses the selected native workspace path and exact current LSP
text/version; the original inner payload remains intact. Cancellation, stale text
and invalid/fatal reads close the evaluation. Late responses cannot satisfy a
later request, and at most eight command workers may remain draining.

`CoreCommandHost::with_evaluation` uses this lifetime instead of inventing the
legacy library's eight generation counters. Its semantic/expression caches do not
cross evaluations. SDK-dependent reads remain unavailable without SDK authority;
SDK-independent source/resource operations do not require a fabricated SDK.

Source-local definitions use host-resolved annotation identities, exact string
values/ranges and same-declaration ownership before invoking the existing Rust
definition services. Other unprovided semantic facts remain unavailable. This
does not declare the full eleven-command or original-editor migration complete.

## Current policy scope and pending contract

The bounded policy is the linked prototype's Mixin/Shadow slice: one known
target, exact annotation identity, class/string target values, descriptor and
dispatch facts, method prefixes, Kotlin backticks and field use-site limits.
Unknown target/signature/dispatch facts remain Unknown and cannot establish
absence. Aliases, unsupported target forms and unknown ABI remain incomplete.

The published checkpoint exposes an authoritative member-to-authored-declaration
association through
`{"op":"declarationMember","declaration":"<opaque declaration token>"}` to
`analysis/readFacts`, returning `Fact<MemberCandidate>`. The host selects the
exact indexed definition and unique containing type and returns Unknown for
missing/ambiguous/unsupported authored facts. The companion uses this operation
directly, with no inventory/name fallback. Its DTO was checked against the host's
`extension_ipc.rs`, `ReadSession::declaration_member` and updated reference child.
The operation and its actual stock-host/editor use are covered by the pinned
checkpoint and interoperability evidence below.

`mcdev-companion/tests/semantic.rs` runs all 13 linked-prototype scenario families
through the real child process, plus batch-size and late-cancellation regressions.
The test host supplies explicit normalized facts and opaque handles; it owns
binding decisions. These checks compare semantic proposals, binding knowledge,
diagnostics and editable source projection, rather than claiming the mock host
implements editor goto, reference or rename orchestration. See
`mcdev-companion/tests/fixtures/coverage.md` for the per-scenario mapping.

The transport does not advertise completion, navigation, rename, references,
AW/AT, mapping, injection, MixinExtras, code actions or the entire legacy
diagnostic/command catalog as implemented IPC surfaces. Standard LSP operations
will consume host-accepted contributions. Full Rust-core migration and this
small IPC policy slice have separate acceptance evidence.

Native framing/lifecycle process tests and the linked-equivalent corpus run
through the repository's finite compact runner with full logs on disk.
Actual MC policy also runs through stock ktlsp and real Neovim: 112 checks and
29 first-attempt RPCs pass, including applied rename, unsaved overload
rebinding/retraction and Java/Kotlin typed completions with checked applied
buffers. The generic accepted-publication trace is observed only by the test
harness; the production child contains no test barrier. The run completes in
16.7 seconds with the latest workspace-verified companion on this checkpoint,
within a 120-second deadline, and exits cleanly. The previous companion passed
in 22.0 seconds; the earlier f595 host passed in 25.7 seconds. Completion
application here uses already fully typed Shadow names: exact item fields,
suffix ranges and unchanged whole buffers prove prefix/backtick preservation;
partial-name insertion is outside this slice. See the exact
binary/source hashes and scope in
[`interop_receipt.json`](../mcdev-companion/tests/fixtures/semantic_v1/interop_receipt.json).
This does not establish full MC command/resource support.

The initial semantic-only check has 29 passing tests. Its full logs and copied binary
fingerprint are recorded in `mcdev-companion/tests/fixtures/coverage.md` and
`build/companion-artifacts/semantic-v1-20261024-143050633/receipt.json`. The
production child has no fixture filesystem barrier or `ipc-fixture:*` diagnostic
marker; the real fixture must exercise actual Minecraft annotations and policy
expectations, with any deterministic pause kept in its test adapter.

## Command runtime and explicit line mode

The same crate also exposes the eleven legacy command handlers as a library
through `commands::CommandDispatcher<H>` and `commands::CommandHost`. The host
supplies typed, immutable project/session/source facts, core semantic results,
definition resolution, references and fix materialization. Runtime caches own
document text and negative results; host adapters own project identity,
generations and authoritative fact validation. `completion_from_core` is the
checked core-debug-to-wire adapter. Core models do not depend on protocol DTOs.
The library does not discover files, read a classpath, invoke a JVM or retain
evaluation-scoped host handles. Command/resource/class-byte wire capabilities
are not advertised by semantic-v1; their host contract and real connection are
pending separate integration work.

`mcdev-companion --stdio-completion` explicitly selects the legacy provisional
buffer-only JSON-lines helper. Each line is `{ "id": ..., "payload": ... }`;
the response carries `id`, `handled`, and optional `response` or `error`. Invalid
JSON/payloads report `PARSE_ERROR`, then the process accepts the next line. EOF
exits cleanly. A false `handled` result requests the caller's project-aware path.
The shared core `HostMixinFacade` determines eligibility and computes actual
completion items; no project facts or extra feature support are assumed. This
mode does not use Content-Length framing or expose an LSP endpoint. The default
mode continues to use only the semantic-v1 framed protocol.

`completion_transport::CompletionTransport` is a separately instantiated library
transport for project-aware command executors. It binds loopback, generates a
32-byte OS-random bearer token, validates HTTP headers/body size, supports
concurrent clients, and applies an absolute computation deadline. Disconnect,
timeout and close signal the supplied cancellation checker. Executors must
observe cooperative cancellation; Rust does not interrupt arbitrary threads.
The executable does not start this transport or publish an endpoint by default.

The frozen 127-case command corpus, exact native case map, explicit timing
predicates, original/new edit equivalence and real process/TCP checks are
documented in `mcdev-companion/tests/fixtures/commands/README.md`. This proves
the isolated command runtime and provisional helper, not a completed command
wire connection to a stock host. The final affected run passes all 127 original
command cases plus seven supplemental checks; the complete companion-crate run
passes 165 tests after the actual-host opaque-configuration regressions,
including the previous 29 semantic/framing/lifecycle tests. Full logs are under
`build/rust-test-logs/companion-real-config-native-final/`.

Further project/class, resource and command capabilities are separate work.
The fixed unpublished host 37 candidate provides only optional `projectContext` and `typeJvm`
operations, with per-instance supported-operation negotiation and existing host
readset validation. It cannot establish a complete MC project session, class
catalog or bytecode provider. See [the capability record](mcdev-host-capabilities.md)
for the actual serializer pin, the still isolated MC adapter and what remains unavailable.
