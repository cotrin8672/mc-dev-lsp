# Native bytecode and official expression migration evidence

`verification.json` records the completed bounded domain. The finite final run
passed all 212 native test functions in nine binaries, including all 153 original
bytecode/index/resolver cases and all 82 original official expression cases.
Cargo test reads committed JSON/class fixtures; it never invokes a JVM, JNI,
ANTLR, a Java compiler, or an external expression implementation.

## Original case correspondence

- `../bytecode_legacy_coverage.json`: all 153 original Kotlin file/case IDs map
  individually to native tests in `tests/bytecode_legacy.rs`. The dispatcher
  `../bytecode_legacy.rs` preserves their complete inputs and assertions,
  including all common-superclass table pairs. It includes the three
  `ProjectContextMixinIndexTest` cases and two `BytecodeIndexDefaultTest` cases.
- `case_coverage.json`: all 82 original official engine file/case IDs map to
  exact corpus line numbers, API names, and native replay functions. All 432
  captured calls are replayed, including nested parser/builder calls, complete
  final match/completion results, result subclass tags, and thrown cancellation
  signals. The six additional oracle cases contribute another 5,276 calls.
- `legacy_calls.jsonl` is unchanged across dependency recovery and later edge
  additions: SHA-256
  `5731580a5f6b0de5e9d66e30dbafcb2a03864171728369e294fc3d7abf4b3a8f`.

The optional `generate_capture.py` executes the unchanged original Kotlin test
assertions against disposable instrumentation, then exports their API inputs,
typed outputs, class bytes, and resolver reads. One-time compilation/capture is a
migration tool, not a Rust runtime/test dependency. The original baseline test
source is authoritative. `--export-only` processes saved captures without Java.
`update_coverage.py` refreshes native case evidence only after the finite nine
binary check succeeds.

The parser failure cases preserve the original asserted contract: a syntax
failure with equal, nonempty message/cause messages. Native parser error wording
does not reproduce ANTLR's Java implementation diagnostics. Other available and
unavailable result variants, reasons, typed payloads, order, and decorations are
compared as required by the original cases. Cancellation remains an outer error
and preserves the same `Arc` signal, including null-message original markers.

## Explicit representation changes

The native candidate model directly contains the fields previously supplied by
`BytecodeIndexEntryMapper`; its six original tests assert those same final
values. Constructor identity is `name == "<init>"`. Java class-byte `assertSame`
tests compare the actual borrowed provider slice pointer through
`get_class_bytes_ref`, alongside the owned convenience API.

String constants retain JVM UTF-16 units through
`ConstantValue::StringUtf16Value` when a scalar `String` cannot represent them.
The shared reader handles NUL, supplementary characters, isolated high/low
surrogates, pairs, and mixed sequences. Labels escape isolated units, and ordinal
keys compare typed values, so colliding labels remain distinct. Engine parsing,
AST constants, matching, probes, and completion have corresponding UTF-16 APIs.
The shared source decoder supplies exact UTF-16 values and source offsets.

Float/double equality, hashing, JSON fixture bits, signed zero, and all NaN
payloads retain Java semantics. `../floating_java21.json` contains 2,193 bit-tagged
JDK 21 label samples, including minimum subnormals. A separate native test
verifies decimal expression matching of Float.MIN_VALUE.

## Two restricted additional-oracle adaptations

All 235 original cases remain under their strict assertions. The following
adaptations apply only to explicitly recorded additional inputs:

1. `identity_ambiguities.json` enumerates 17 input groups where the original
   `IdentityHashMap`-backed flow iteration makes physical-instruction deduplication
   select different virtual components. Native order is physical instruction
   index, then the original node before expansion insertion order. The complete
   virtual-component multiset (type, decorations, capture location) is still
   compared exactly through `matchComponents`; only these enumerated ordinary
   match groups accept their recorded legal deduplication alternatives. Other
   comparisons remain ordered.
2. `jsr_fatal_deltas.json` records 12 exact API/context rows for a single JSR
   fixture: original class SHA-256/version, opcode/index/offset, original
   `AssertionError` class/message, and native unavailable reason/cause. Official
   expression analysis has no defined JSR value, and the original throws outside
   its public result API. Native official analysis returns typed Unavailable for
   this JSR condition. `ExactLocalCapture` still supports JSR/RET, and
   cancellation bypasses this adaptation. Independent compatibility regressions
   verify all three behaviors.

## Shared production APIs

The single class decoder/CFG serves both official expression analysis and exact
local capture. `flow::decode_method` separates corrupt classes from missing
methods without premature analysis. `analyze_method_with_policy` selects
`OfficialExpression` or `ExactLocalCapture`, preserving their distinct merge,
boxing, frame, and max-stack policies. The StackMap scanner preserves ASM's
bounded prefetch behavior, including the captured malformed-count case; invalid
frame kinds, verification types, offsets, and constant-pool references fail.

The expression public surface includes typed AST/pools, `MatchContext`,
`MatchResult`, parser, builder, matching, receiver probes, receiver validation,
and member completion. Parsing/matching/probing/completion also accept UTF-16
unit slices. `codepoint_offset_to_utf16` bridges official parser spans to source
unit positions. General bytecode APIs expose providers, indexes, adapters,
instruction extraction, ordinals, cache invalidation, and common superclasses.

Native final logs:
`build/rust-test-logs/expression-engine-20261024/owned-final-parity.stdout.log`
and `.stderr.log` (exit 0, no timeout). The complete application suite is owned
by the parent integration task. No implementation gaps remain in this bounded
bytecode/official-engine domain; source/facade/host integrations are tracked by
their respective owners.
