# Host project/type facts: limited slice 143

## Status and primary contract

Current published host `f76a43fbeddf4fbdc69768fd0f7544e5a4a3610f` implements semantic-v1
and accepted-contribution tracing. Real MC/editor acceptance is recorded in
`mcdev-companion-contract.md`. It does not implement the optional operations below.
The earlier `f5953350928eb8af1ab34bb39db23ba7370d14af` interop checkpoint also
lacks these two operations; the new host pin does not enable their advertisement.

The authoritative **proposed, not implemented or advertised** contract is
`C:/Users/gummy/.codex/ktlsp-runs/01a12550-7d56-71f2-8c34-8ab8c5fe5dda/project-type-facts143-wire.md`,
SHA256 `3951de07529737cef9df3a66610c163fe6f2baa8f47dc7bf367d3237911dd8e2`.
The isolated `host_facts` adapter models its two operations. Production negotiation
remains semantic-v1 until the host implementation and fixed published checkpoint
are available. A compiled DTO is not live host proof.

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
bytes. The implemented serializer and fixed commit will remain the final source.

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
