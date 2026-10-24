# Host project adapter migration

All 42 original cases execute their assertions in `tests/host_project.rs`.
`coverage.json` lists each exact original path/method and native test. The count is
3 classpath bridge + 10 reflection bridge + 16 bytecode adapter + 7 bytes provider
+ 3 lazy index + 2 session + the previously pending FileBased session case.
The other 20 FileBased cases remain in their original project migration ledger.

The architecture changes the source of facts, rather than recreating JDT:

* The host selects classpath entries, their order/classification/timestamps, source
  sets, project/session identity, and all eight semantic generations.
* Host callbacks supply normalized catalogs and exact owner bytes. The original
  directory/JAR fixtures are represented explicitly as those selected entries.
  The child does not walk a classpath or load JVM classes.
* `select_host_method` consumes normalized signature strings, preserves first
  overload selection without a descriptor, qualified unresolved signatures,
  generic erasure, primitive/array parameters, ambiguity, and extraction absence.
  Reflective extraction failure and an absent signature capability both produce
  an unavailable signature fact and the original null result.
* Normalized reference hierarchy/access flags, assignability, and source
  locations are host facts. Missing capabilities remain unknown. There is no
  SDK-class table or reflection fallback.
* `HostProjectSessions` couples the resource context/version with one provider,
  binary index, optional source overlay, and bytecode adapter. Reindex creates
  new lazy adapters while retaining the source overlay factory. Session
  replacement/clear closes the old adapters.
* Persistent caches contain owned values and read stamps. Each use validates and
  registers fresh current-evaluation positive/negative reads. The host's opaque
  project/type/evaluation handles are never stored in these data structures.
  Unknown ClassIndex overload facts are not cached as an empty negative answer.

The 18 additional native tests cover mutation, negative-read invalidation,
precedence changes, all eight generations, workspace isolation, cancellation,
closed sessions, LRU recency, metadata/fingerprints, Unicode string ordering,
source-overlay recreation, host reference/location facts, typed corrupt/missing/unavailable member queries, zero-capacity disabled caches, and fixture integrity.
The full public candidate snapshot test also compares every field of 10 outputs
from the original adapter (including operation, constant kind, ordinal,
instruction occurrence, namespace, display label, and result classification).

The FileBased case copies the original `fabric-basic` resources, loads a real
`FileBasedProjectContextService` cached context with host-selected classpath
facts, and obtains one `HostProjectSession`. That same session has READY context,
`class_count >= 1`, and the original SimpleTarget class bytes. It does not combine
unrelated ready/index results.

## Fixture provenance

`HostProjectOracle.java` invokes the frozen original tests and their original
private Java compiler helpers. All 42 original tests passed. `original_assertions.json`
records their IDs; `source_inputs.json` retains all 12 Java source blocks/templates
and original source hashes. The 18 compiled fixtures and SHA-256/length manifest
are checked natively. `generate_fixtures.py` reuses the shared pinned dependency
runtime, logs finite compiler/oracle runs, and is a one-time migration utility.
Cargo tests require no Java, Kotlin, Gradle, JDT, network, or fixture compilation.

## Verification and remaining connection

`build/rust-test-logs/host-project-zero-capacity-final/affected.*`: all 60 host tests passed, exit 0, 23.7 seconds, no compiler warnings. The preceding `build/rust-test-logs/host-project-closed-final/affected.*` also passed all 59 then-current host tests and 195 affected completion tests (254 total), exit 0, 65.9 seconds. Only the zero-capacity cache regression changed afterward; the final affected host run passed all 60. The completion run includes the shared Java floating-point formatter regression for the smallest float/double subnormal hints.

The actual ktlsp host capability/wire adapter remains pending. Native fixture
tests prove the typed boundary and cache/session behavior; they do not prove
runtime reachability from ktlsp. A host must validate all generation dimensions
and register current-evaluation reads; unavailable semantic generations must not
be represented as an asserted known generation zero.
