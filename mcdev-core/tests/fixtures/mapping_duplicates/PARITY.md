# Ordered duplicate mapping lookup

The frozen original `mapping/MappingModel.kt` uses `firstOrNull` for class,
field and method lookups. It first matches the complete source key, then reads
the destination namespace from that row. It neither requires equal duplicate
destinations nor skips a matching row whose destination is absent.

The Rust-only `unique_name` policy required agreement among all matching rows.
It returned None on conflicting destinations or on any later missing destination.
`MappingSetResolver` translated that to `MappingLookupResult::Missing`; completion
callers then kept named spelling or reported `mapping_found = false`. This was
not a `HostFact::Unknown` result or a host-authority rule.

Class destinations illustrate the exact behavior (member names follow the same
row selection; their full values and metadata are in `original_results.json`):

| Ordered matching destinations | Original Kotlin | Previous Rust | Corrected Rust |
| --- | --- | --- | --- |
| B, B | Found B | Found B | Found B |
| B, C | Found B | Missing | Found B |
| C, B | Found C | Missing | Found C |
| absent, C | Missing | Missing | Missing |
| B, absent | Found B | Missing | Found B |
| empty string, C | Found empty string | Missing | Found empty string |

The fix uses `.find(source_key).and_then(destination)` in the three common
`MappingSet` lookup methods and removes `unique_name`. No caller-specific
selection or sorting was introduced. Tiny/SRG parsing already preserved row
order. SRG's current-class metadata still tracks the latest CL record for later
members; lookup still selects the first member matching the original source key.
Composite delegates still use first Found, fall through Missing, and stop on
InvalidDescriptor. Project mapping files retain the caller-supplied file order.

`MappingDuplicateOracle.java` invoked the frozen original compiled Kotlin classes
once. It recorded six duplicate scenarios, 18 direct-name results and 24 full
class/member/descriptor lookup results. Native Cargo tests replay all those
results, including the Found/Missing kinds and all Missing metadata. Java/Kotlin
is not invoked by Cargo tests.

The nine supplemental native tests also cover inverse many-to-one lookup,
descriptor/owner/name/namespace filtering, missing first member destinations,
Tiny/SRG duplicate retention, composite fallback, actual project mapping-file
precedence, and user-visible Mixin/AW/AT insertion text and mapping_found values.
The first duplicate now produces the original remapped owner/name/descriptor
instead of silently falling back to named text.

All original mapping cases remain unchanged: MappingParserTest's four cases and
MappingFixtureRegressionTest's five cases run in `foundation_legacy.rs`. Their
case ledger needs no status/input/expectation changes. One Rust-only auxiliary
assertion in `foundation.rs` was corrected from conflicting→None to first B;
the nine new tests are supplemental and do not inflate legacy coverage counts.

`build/rust-test-logs/mapping-duplicates-final/affected.*`: 578 affected native
test functions passed across mapping/foundation, project, completion and AW/AT,
exit 0, 15.6 seconds, with a finite 300-second deadline. `receipt.json` records
source hashes, the original oracle evidence, test summaries and the unchanged
legacy-case accounting. No host_facts or host Unknown/authority code was changed.
