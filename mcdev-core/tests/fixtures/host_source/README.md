# Host source boundary migration evidence

`coverage.json` records all 170 original cases from the eleven legacy JDT
adapter test files. It retains the complete original bodies, every assertion
expression (including generic cancellation assertions), exact native test/table
mapping, capture row IDs, input transformations and the final finite test proof.
All 170 original assertion bodies also ran successfully in the frozen JVM once.
Native tests never run Java, Gradle or a JDT library.

`adapter-calls.jsonl` retains 340 records (339 successful calls and one thrown
constant-resolution cancellation) across 59 original cases. Native replay
compares 324 complete constant/binding/resolved-context results, the fourteen
signature cases use an explicit normalized-type table, and cancellation has a
stateful retry test. Concrete resolved/unavailable/not-handler outcomes remain
distinct. `project-calls.jsonl` retains 150 records (146 successful calls and four
throwing nested/public records). `normalize_project.py` uses the actual observed
resolver candidate arrays and fake getter storage to make typed host inputs;
it never uses the expected public descriptor as a fact. All 38 captured final
project-query calls, including two errors, execute against the native provider.
The other project cases and all cache/concurrency/scanner-replacement cases have
direct native tests. Prefix transport and JDT private signature syntax are
fixture compatibility data, rather than production utilities.

The only adapted malformed-definition `rawValue` is explicitly identified in
the ledger: the old fake IntLiteral's inherited `Object.toString()` leaked a
Java identity address; the native fixture supplies the actual snapshot slice
`123`. A separate regression checks the exact raw text and range. All other
raw values, messages, nullable fields, metadata and ordered results remain in
the comparison. Old JDT reflection/capability observations have explicit host
equivalents: HostNormalized success, visible stable/transient fallback, typed
failure/cancellation/interruption, and immutable per-operation authority.

Host-selected document stamps and normalized declarations replace the old disk
scanner. Tests retain the original one-file update, deletion, 30 overlays,
1,000-file reuse and bounded-cache assertions. “Read/parse” counters become
actual provider document reads/projections. The product contains no disk walk,
classpath scanner, reflection availability probe, private JDT signature parser
or additional Java/Kotlin parser.

The public implementation is `mcdev_core::host_source`: owned normalized type,
constant, annotation, declaration, semantic model and reference hierarchy facts;
ProjectSourceQuery/SourceBackedClassIndex/CompositeProjectSourceQuery; bounded
CompletionIndexCaches/SemanticModelCache/HostSourceSnapshotCache; and typed
InjectionPoint feature/AtCode providers. Existing MC source/descriptor/Extras
models and services remain the only domain model/parser implementations.

Caches retain no evaluation-scoped declaration/type/member handles. Every reuse
validates positive and negative read provenance and registers fresh reads in the
current evaluation. All eight *known* generation dimensions participate; source
hashes use SHA256. An unavailable generation identity bypasses reuse. Tests use
complete fake eight-counter snapshots, whose zero counters are explicitly known
initial values. The helper that used to fill three missing counters with zeros
was removed; the host-project fixtures now construct their complete stamps
locally. A missing or unavailable host fact never becomes a cached successful
empty legacy index result.

Actual host wire integration is **pending**. The current optional `projectContext`
and `typeJvm` draft does not provide a complete source environment, eight-counter
identity, source catalog, semantic snapshot, hierarchy or annotation constants.
These fixtures do not establish live capabilities, a complete HostProjectSession,
or editor success. Runtime adapters must negotiate real supported operations and
keep unknown facts and missing generation identities unavailable/bypassed.
