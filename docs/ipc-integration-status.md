# Published host integration checkpoint

The complete replacement objective is unchanged. Integration resumed on
2026-10-31; routing coverage is not complete command or editor parity.

The resumed implementation now connects normalized callable parameters,
Constant/Slice selectors, URI-preserving cross-file Share, and separate workspace
config/mapping/reference discovery domains. The fresh whole native pipeline
passes; original live editor acceptance is still being updated.
The first fresh whole-suite run exposed six legacy callback-order regressions;
the evaluation-only optimization was corrected without changing those fixtures,
and all 135 command replay tests plus 34 evaluation tests then passed.

The host has also published `root-files-v1` at
`392076bc5e364e02426450c574c1227db2c3be8f` (release86, host SHA-256
`4d2bcbc10601628e59ea1640145cb3f8ddd2a13be8eaf1a73cf29b56919d2019`).
Its accepted serializer sample has SHA-256
`af49d91e7e42dada7314a953b710ebe2d998176b7ed00ee361b410db1c3a4c41`.
This is the host's file-input proof, separate from MC command/editor acceptance.
Readonly snapshot JVM names are published from release89. Release92 separates
semantic-index dependencies from metadata and rejects old resolved type handles
after index changes, preserving the source/model/provider validation guards.
Release93 adds independently negotiated written annotation sites, including
annotations whose declaration has not yet been written.

## Published contract

The current integration pin is ktlsp
[`ddba88af8c4dfc2b5c67397b465ea01c98129069`](https://github.com/cotrin8672/ktlsp/blob/ddba88af8c4dfc2b5c67397b465ea01c98129069/docs/architecture/extension-ipc.md).
It adds normalized authored facts, owning-project input records, exact source
locations and typed binary declaration annotations to source snapshots, owned
resources, selected runtime bytes, the physical JVM catalog and provider
commands. The companion negotiates capabilities and individual operations;
binary annotations are selected by capability alone. Configured command IDs
are intersected with the implemented domain routes.

The copied release93 host has SHA-256
`46ae70429778a834951ce0c191b08b334863441a82bf20c3394f8075fabfed4e`.
Its provenance is retained in
`build/host-artifacts/published-ddba88a-46ae70429778/receipt.json`.
This identifies the integration input; upstream API tests do not prove MC
feature or original editor parity. The acceptance results below retain their
historical host and companion pins.

The host's evaluation lifetime, accumulated reads and final validation govern
these inputs. They do not provide the earlier prototype's eight generation
counters. The evaluation command entry therefore uses no synthetic generation
identity and bypasses cross-evaluation semantic/expression cache reuse.

Physical catalog coverage certifies the selected physical inputs only. Raw ABI,
source editing members and executable runtime bytes remain distinct. Header-only
SDK input cannot supply Code/LVT, and `sourceCurrent: Unknown` does not certify
compiled output against unsaved source.

Authored declaration ranges use UTF-8 bytes; `sourceLocation` uses UTF-16 LSP
positions. Snapshot source tokens are read-only and retain their compiler view.
Source-set membership does not establish ownership. A complete selected Gradle
resource inventory does not by itself certify the original whole-project
shared Mixin configuration search. Config-dependent results remain unavailable
when that selection is unproven. Callable parameter facts are optional knowledge:
an omitted or unsupported list does not establish a zero-parameter callable.

## Follow-up acceptance checkpoints

CI for the assertion-census commit `6e9e206` also passes on Windows and Linux:
https://github.com/cotrin8672/mc-dev-lsp/actions/runs/41229469984.
Downloaded logs verify the same 2,604 original locators and all five editor
entries; Windows has 1,875 functions and Linux has 1,874 across 80 targets.
The independent proofs are
`build/rust-test-logs/ci-6e9e206/{native-cases-proof,packages-proof}.json`.
Both archives also verify all 24 payload sizes/hashes, with no extra regular
files. Their archive SHA-256 values are
`dfbe2dfe997d4c53fccf0db530e8fbeceac37975cdb159878e641f03d7f25cb5`
(Windows) and `d82e639b0f79151d1a5694171c0f2e9daec8c79f0e9cf341e533a8f210925802`
(Linux). The following archive record belongs to the preceding functional
commit `7e338e4`.

The full Windows/Linux CI for `7e338e4` passes:
https://github.com/cotrin8672/mc-dev-lsp/actions/runs/41228019825.
Downloaded logs independently verify Windows 1,875 / Linux 1,874 functions,
80 targets and all 2,604 original case locators on each platform, with no failed,
ignored or filtered native tests. All five editor entries pass, including the
real process suite. Both optimized archives have all 24 payload sizes/hashes
verified, 11 optional capabilities and 9 operation maps. Evidence is retained in
`build/rust-test-logs/ci-7e338e4/{native-cases-proof,packages-proof}.json`.
Archive SHA-256 values are
`f3f0b15c0014baf881c6a628cefb795a727429e6da99508ed20a65acadebb9d9`
(Windows) and `e073ce19066eadbdc86aed967610abe721dccfb567d764f6445b571bc1b45dec`
(Linux). These checks do not certify the unpassed live CEM workflow.

The latest native pipeline is
`build/rust-test-logs/native-pipeline-proof-20261101-025824638/`: 1,875 functions,
80 targets, all 2,604 original JUnit locators, doc/build checks and five editor
entries pass in 116.8 seconds. Its 872 inputs retain fingerprint
`2d7ec181ff68c15a878730b4f9af4fb41922e0427b4acde9a53da744144bb2fb`.
The copied debug companion is
`6c99f535a868c5bfd9f527e6829384a97737bad353007cd4f1f9a6d4fabae9e5`.
Info/Dump/Reindex/Reload now skip unused compiler source inventory/body reads;
their complete DTOs match the all-input baseline, and skipped source coverage
stays Unknown(NotRequested). All 67 affected checks pass.

The bounded readiness change also passes seven mock groups and the adapter
entry. It preserves the first Stale, requires a genuinely new accepted
evaluation/publication with the same instance/owner/SHA and unchanged editor
version, then sends one separate outer request. A second Stale fails. It neither
retries readFacts nor synthesizes a document change.

Actual candidate88 runs still fail before original CEM assertions. D3
(`cem-project-20261101-022439783-d5b5134f`) identifies sourceInventory/prepay
Stale; D4 (`cem-project-20261101-024118400-adb6cf5b`) records a new accepted
evaluation and both failed outer requests. D5
(`cem-project-20261101-030230338-e662aa1e`) proves the unused source reads are
gone, then fails after resourceInventory pages with host validation Stale.
All paths above are under `build/rust-test-logs/`. Those attempts execute no
original assertions or golden comparisons. The following diagnostic isolates
the first failed guard without relaxing admission.

Release90 D6 (`cem-project-20261101-031413365-96dd433e`) identifies that guard:
the first command failure is model generation 30 to 31, evaluation 8, directly
after index-changed invalidation. The fresh command evaluation 11 fails model
31 to 32. Source SHA/version and provider instance stay unchanged. Release92
separates project/resource dependencies from semantic-index updates while
retaining real model/source/host-input and semantic lookup invalidation.
This run still credits zero original assertions or member comparisons.

Release92 D7 (`cem-project-20261101-035520000-7b79286b`) passes Info on its first
request, with no command validation failures, child errors or readiness retry.
It reaches two original assertion-helper calls and one completion request.
That request fails with typed `INCOMPLETE_PROJECT_CONTEXT`: the full
`sourceFacts` declaration/parameter annotations do not cover the written,
ownerless `@Inject(method = "is")` before a method has been declared. The
sourceFacts read succeeds; the consumer rejects missing annotation coverage
before resolving annotation types. The exact unchanged 254-byte CRLF input is
saved as `build/live-fixture-inputs/d7-orphan-inject-crlf.java`, SHA-256
`7bd1004fdea0383b51e973c48f3a2e5162e359b9abc08eab559920e98e8df5a8`,
document version 3, command evaluation 11. Its accepted provider evaluation is
9, publication 8. Release93 publishes generic `annotationSites` facts for
this incomplete-edit state. The operation is independently negotiated
under `source-facts-v2`, retaining the existing `sourceFacts` shape. It returns
source URI/revision/SHA, written annotation sites and a coverage fact. A positive
site may support its specific completion even when syntax recovery makes total
coverage Unknown; that does not establish missing sites or diagnostic absence.
D7 shuts down cleanly. The 85-row member oracle comparison is still not reached.

The consumer connection passes 82 affected Rust tests and actual debug-binary
operation negotiation. The host's unmodified serializer fixture has SHA-256
`fa2007364b01b5cfc877085e0c7297cc0513bfcdaa17f3243ee5564a7183eb1a`.
It records positive ownerless sites with Unknown coverage, then an owned site
after an unsaved edit. Owner Declaration tokens are reused only within the
same evaluation/source view; ordinary and read-only snapshot provenance remain
distinct. TypeRef tokens may be reissued, so their positive same-view resolved
identities are compared. The sample captures only the inventory reference's
type resolution; it does not prove the declaration alias's resolution. That
pair remains a live MC check. Its percent-encoded Windows drive URI also exposed
a consumer string-comparison mismatch, now corrected using normalized file
paths while retaining exact source SHA/revision checks. Raw fixture conformance
and the URI regressions pass. The new full native pipeline and original CEM
rerun are pending.

Current-source metadata preserves the 2,604 cases, 17 ported / 3 partial / 7
pending editor-file states and historical four-snippet/169-call proof. Its
scanner has now been corrected to include repeated-underscore helper names.
Its census retains all 1,014 previous IDs and adds 183 frozen sites, with zero
ID collisions and no remaining lexer omissions (1,197 total). All additions
remain pending for semantic parity with no added test-case or full-E2E credit.
All 183 now have static correspondence; 57 have qualified recorded helper
invocations and 126 lack such direct observation. These counts do not certify
original input rows or semantic parity. Historical invocation collections remain
unchanged, with the new adapter collection recorded separately.
The original Kotlin Drill/Placement branch has now been restored with both
unchanged inputs and all 32 assertions, alongside physical Mixin 0.8.7 / Fabric
MixinExtras 0.5.5 prerequisites and the mandatory Info `jarLocation` field.
Fidelity, syntax and negative mock checks pass; real Drill and generated-source
compilation remain unverified. An optional broad metadata check
hit its finite 60-second deadline and is recorded as failed, separately from
the passing product/client checks. Full original editor parity remains false.

The standard native fixture matrix now requires
`pwsh -NoProfile -File scripts/run-osgi-e2e-matrix.ps1 -InputsReceipt <receipt.json>`
or `MCDEV_E2E_INPUTS_RECEIPT`. It validates the copied physical Mixin 0.8.7 and
Fabric MixinExtras 0.5.5 inputs before generated-source compilation. Missing
receipt data fails at launcher setup. The existing compact-runner support
suite passes after its routing mocks were updated to forward that prerequisite;
mock routing is separate from physical JAR or generated-source compile proof.

The resumed pipeline in
`build/rust-test-logs/native-pipeline-proof-20261101-003939212/` passes 1,871
functions across 80 targets, all 2,604 original JUnit case locators, doc tests,
the companion build and all five independent Neovim entries. All 872 input files
retain fingerprint `74eb3ef7ee4ddbc5f0d930c1c020056cd64d420f25f31f863f3d5579889792f3`.
The full pipeline takes 136.4 seconds with no timeout. The independently copied
debug companion has SHA-256
`2c6793521ee4371dd8d82d0f2c7ae40b623df847f67bd9b7432a5928d5c33afe`.
The Windows optimized package also passes its 24-payload hash checks, stdio
completion, semantic-only fallback and exact optional negotiation (11
capabilities / 9 operation maps, with annotations selected by capability alone).
Its archive SHA-256 is
`fdddf559c9fc0118216d7b76870363aa088a6bb4e86e5dcc9754f0fb0f27296c`;
logs are in `build/rust-test-logs/native-release-20261101-004324420/`.

The original CEM workflow was attempted with release86 and the copied debug
companion in `build/rust-test-logs/cem-project-20261101-004634121-336c13de/`.
Its isolated fixture uses the seven fixed inputs and the separately observed
legacy Kotlin 85-row member oracle. Indexing completed and the source SHA was
accepted, but the first `mcdev.info` readiness preflight returned outer
`-32801 / Stale`. The run failed before any original assertion or golden
comparison. Input hashes stayed stable and the host shut down cleanly. The
saved trace cannot identify which guard returned Stale; the host task has been
asked for bounded command/read diagnostics. No same-evaluation read retry or
empty-success fallback has been added.

A second diagnostic run uses the explicitly unpublished host88 candidate
(`6037dc8d613ffddcd6e4d270c9c9e85cb6b850189b625d1d41bc1cb52e9c6444`)
in `build/rust-test-logs/cem-project-20261101-020445745-1c42a509/`.
Its bounded trace proves projectContext/projectInputs/sourceInventory succeeded
and final host validation stayed valid. The child then rejected membership with
`-32602`, exposed as outer `CommandFailed`. The adapter incorrectly equated the
raw Gradle project path with a build-qualified semantic module. It now joins
issued source-set tokens/project/name and checks semantic identities against
each other, preserving membership/ownership and foreign compiler-view limits.
All 65 affected tests pass. This fixes the observed consumer error; it does not
identify the earlier release86 Stale or claim that CEM has passed yet.

CI `880f8a7` verifies all original case locators on both platforms (Windows
1,871 functions, Linux 1,870, 80 targets each). Linux's full job passes. Windows
passes native/doc/build checks but fails the unchanged teardown unit at process
snapshot acquisition. That unit now mocks subprocesses; the unchanged real
process suite still observes PowerShell, owned descendants and unrelated-process
survival. Focused unit/real-process checks and a fresh five-entry editor run
pass. The original six unit assertions remain mapped; this backend adaptation
is explicitly non-strict in the source-fidelity metadata.

The member oracle directly executes the old compiled Kotlin builder, adapter and
completion service twice with byte-identical results. Its 155 source files match
the frozen baseline and 698 core/fat-JAR class entries agree; the compiled JAR's
build provenance against that source is not proven. Its receipt is in
`build/legacy-member-reference/20261031T144315699851Z/receipt.json`. This is an
observed core-service oracle, not a complete old JDT editor-route proof.

The following historical results predate the resumed input adapters and retain
their own pins and narrower live acceptance scope.

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

The owning-project adapter now preserves all captured source sets, distinguishes
source/resource ownership from membership, and guards config-dependent results.
The client/shared configuration counterexample no longer becomes a successful
registration diagnostic. Root-file coverage now supplies the original config,
mapping and reference domains independently. Missing or unreadable domains still
prevent the affected commands from claiming full discovery.

The published pin resolves the previous transport gaps for nested annotations,
integer/enum values, decoded string maps, declaration ranges/modifiers, sibling
inventories, source locations and typed binary declaration annotations. Remaining
requirements have been checked with the host task:

- The original Mixin configuration walker also searches project-root and Java
  source directories, excluding `build`, `.gradle` and `node_modules`. Gradle
  resource coverage includes its own include/exclude selection. Even a resource
  root equal to the project root cannot prove the original candidate set.
  The separate root-bound filtered inventory/snapshot contract is published in
  release86. The companion connects its complete membership and body coverage
  to each original discovery policy; MC live selection still needs acceptance.
- Callable parameter annotations are published in `db0f080` as the optional
  knowledge-bearing `parameters` field of `source-facts-v2`. The transport
  distinguishes missing/unsupported facts from known zero parameters. Nominal
  generic class resolution does not expose its generic arguments; unsupported
  lowering and type-variable cases remain Unknown. Host parameter probes are
  separate from full product Sugar acceptance.
- Foreign snapshot types remain read-only. Their `typeJvm` fields, including
  `binaryName`, were `UnsupportedSource` before release89. That pin now supplies
  only the binary name from the same readonly source view; flags, ABI, member
  coverage and require authority remain unavailable. The producer's 36-check
  foreign-name proof is separate from MC editor acceptance. The child still
  cannot construct nested JVM names from qualified source names or substitute
  the evaluation-owner view.
- Typed `@AtCode` annotations are available for selected physical inputs,
  including compile-only inputs. Current source supertype/subtype relations are
  not public. A physical subtype scan alone does not certify the complete current
  injection-point index and grants no executable-byte authority.
- Standard host definition supports dependency source attachment, but extension
  `sourceLocation` remains Unknown for binary-only inputs. Those are separate
  paths; original Create/Drill and simulated Loom source-navigation acceptance
  still needs the corresponding extension authority and actual artifacts.
- The host has no extract-local action implementation. Materialized code-action
  support alone does not satisfy the original action-alias scenario.

Current authored source, selected physical ABI and executable runtime bytes remain
distinct. `sourceLocation` can connect an issued type/member to an accepted source
URI and digest; it does not establish that compiled bytes match that source.
Independently issued handle strings are not a type-equality test. Runtime Code/LVT
reads retain their own authority even when `sourceCurrent` is Unknown.

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
