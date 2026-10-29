# Host project/type facts: historical two-operation slice

## Status and primary contract

This document preserves candidate37's isolated project/type proof. Current
production IPC negotiation and the additional source/resource/JVM readers are
recorded in [the companion contract](mcdev-companion-contract.md) and
[integration checkpoint](ipc-integration-status.md). The two-operation reader
alone still cannot construct a complete project/session.

At that checkpoint, published host `f76a43fbeddf4fbdc69768fd0f7544e5a4a3610f` implemented semantic-v1
and accepted-contribution tracing. Real MC/editor acceptance is recorded in
`mcdev-companion-contract.md`. It does not implement the optional operations below.
The earlier `f5953350928eb8af1ab34bb39db23ba7370d14af` interop checkpoint also
lacks these two operations; the new host pin does not enable their advertisement.

The host's **candidate 37 implements these two operations**. Its fixed
serializer handoff is
`C:/Users/gummy/.codex/ktlsp-runs/01a12550-7d56-71f2-8c34-8ab8c5fe5dda/candidate37-semantic-handoff/manifest.json`,
SHA256 `517f3f3906e46345854cccec8b94e37ea3b88d235ecfcf1c5f2f29485eb0b2af`.
The copied `crates/ktlsp/src/extension_ipc.rs` is the actual wire authority,
SHA256 `ecfc4d8bfbd7b5fe35dfa9ede4c3859c0f2535a6c429c948adae8d6526f9f7a5`.
All five copied sources match the producer's pre-test and post-release guards.
The candidate executable SHA256 is
`a3f1248d6aa39cf582f87c88477ca175964f3e55d11f45348b8f353a0675e1b6`;
this is an artifact pin, not a build of a single published Git revision.

The API source is now published in
[`16580db`](https://github.com/cotrin8672/ktlsp/commit/16580dbbb7ccb3dbbb42bee3afa90ee6aeef0e3b).
All five source blobs were checked against GitHub at that exact commit: three
match the frozen bytes directly and two match after Git's CRLF-to-LF normalization.
The candidate executable remains a combined working-tree build; the API commit
must not be used as its complete binary source identity.

The actual serializer and recorded frames agree with the isolated MC `host_facts`
adapter. The producer's `project-facts143-37-final.json`, SHA256
`dc09e9a59770fd859c7ede28426307e4ee5e329bd92387f7934687b7262ff096`,
records 46 matched optional checks with no mismatch or unmeasured checks.
That proves the producer's optional reader, not its integration into the MC
command host. At that checkpoint the MC production child negotiated only semantic-v1.
Its 112-check/29-request editor smoke also passes against this candidate in
`build/rust-test-logs/host-smoke-20261025-125351932/`.

`mcdev-companion/tests/fixtures/host_facts/candidate37-fresh.json` preserves the
producer's recorded fresh fact subtree, with its input pointer and hashes in
the adjacent provenance record. It retains opaque classpath/SDK/member IDs and
the exact Unknown fields. The fixture is a recorded payload, not a complete
RPC frame or a live MC command connection.

The earlier MC proposal for an eight-counter project identity, external
`validateReads`, catalog/blob queries and a full project adapter was not accepted
for slice 143. Its code/checks were saved under ignored build history
`build/host-facts143-superseded/`. They are not a current production contract.

## Optional negotiation

```json
{"protocol":{"major":1,"minor":0},"provider":"mcdev","capabilities":["semantic-v1","project-class-facts-v1"],"supportedOps":{"project-class-facts-v1":["projectContext","typeJvm"]}}
```

The worker/instance retains the supported-operation intersection. Missing optional
capability or map means no optional operations. Unknown names do not become
supported. The existing semantic-v1-only child reply remains valid. Unnegotiated
queries follow JSON-RPC `-32602`, never DefinitelyAbsent. The capability name
denotes only the negotiated two-operation subset.

`analysis/evaluate` stays unchanged: evaluation, owner, source, configuration and
declarations. No project model/classpath JSON is cloned into each file payload.

## Project metadata

```json
{"evaluation":"e1","queries":[{"op":"projectContext"}]}
```

The query has no child-selected path/project parameter. The host selects its
subject from the owner and accepted model. Independent Facts describe
`workspaceUri`, `scope:{module,sourceSet}`, `sourceRoots`, `outputRoots`,
`classpath:{entries:[{id,displayPath}],coverage}`, and
`sdk:{identity,api:{kind:"full"}|{kind:"release",release:17}}`.
Observed entries retain order when coverage is Unknown. Entry IDs/display paths
give no filesystem authority. Unknown SDK differs from a known full SDK API.
An oversize projection cannot silently claim complete coverage.

These fields cannot construct `HostSessionIdentity`, `HostProjectSession`,
`HostGenerations` or a complete ClassIndex. The adapter reports a full session as
Unavailable even when all metadata fields are Found. It adds no fabricated zero
generation, source-set/SDK scan or child read validator.

## JVM facts and raw ABI

```json
{"evaluation":"e1","queries":[{"op":"resolveType","name":"fixture.Target"}]}
```
```json
{"evaluation":"e1","queries":[{"op":"typeJvm","target":"<returned host type token>"}]}
```

The target must be a type handle issued in this evaluation. `typeJvm` contains
`binaryName`, `classFlags`, `declaredMembers`, `abi` and `jvmAbiCoverage`. Knowledge
uses found, unknown-with-reasons and definitelyAbsent. The existing editing member
surface retains member/signature handles, dispatch, visibility, synthetic facts,
mismatches and coverage. Found instance proves static=false; Unknown dispatch does not.

Raw `abi` retains exact identity, versions, flags, superclass/interfaces, generic
signature, source-file hint, attributes, inner classes, **all fields and methods**.
Constructors, clinit, overloads, bridges, synthetic and Kotlin-generated entries
retain exact descriptors and flags. Raw entries have no source MemberId,
declaration or fabricated source range. A Synthetic attribute does not rewrite
raw flags. Complete headers project into core method/field DTOs using the existing
Rust descriptor parser and renderer.

Header projection requires Found raw ABI, Found JVM ABI coverage, and checked
binary identity matching the ABI owner. Editing-member coverage/counts cannot
establish raw JVM coverage. Source-only, dirty, unsupported-lowering, wrong-owner
and oversize ABI remain Unknown while other positive facts survive.
The host confirmed `contentSha256` as a lowercase 64-digit hexadecimal JSON
string without `0x`; the child preserves that lossless identity without hashing
bytes. The fixed candidate serializer above is the current wire source; a later
published revision must be checked before replacing that pin.

Complete headers do not provide Code, LVT, annotation payloads, callability or
source location authority. `executable_bytecode()` reports BYTECODE_UNAVAILABLE
even for a complete header. No ClassBytesProvider or BytecodeIndex is fabricated.

## Lifetime and implementation boundary

The host associates shared accepted model/scope/SDK selection with its existing
evaluation and pending ReadSet. Every batch resumes that transaction; finish
validates all prior reads. Source, namespace, compiled/model, SDK/classpath and
provider changes use existing host invalidation hooks. Workspace/ReadSession
borrows are released before child I/O. The child has no counter authority or
external validateReads contract.

The adapter borrows the evaluation registry and RPC reader. It retains no handles
in persistent caches. Existing transport owns cancellation, stale/foreign routing,
1 MiB frames, 25-second lifetime, 64 batches, 256 queries, queue bounds and provider
failure. Typed Unavailable reaches INCOMPLETE_PROJECT_CONTEXT or
BYTECODE_UNAVAILABLE envelopes; Cancelled remains cancellation. Capability absence
or incomplete ABI cannot create empty success or missing-target diagnostics.

## Deferred capabilities

Not supplied: catalogs/prefix pagination and negative inventory authority; raw
bytes, Code/LVT/annotation payloads; hierarchy/assignability; resources/document
snapshots; all eleven commands; complete MC sessions and mapping/runtime policy.
Full MC runtime wiring depends on those later contracts. Operations from the
earlier broader draft are not advertised by this slice.
