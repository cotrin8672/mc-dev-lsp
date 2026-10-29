# Published host integration checkpoint

The complete replacement objective is unchanged. This records work in progress
on 2026-10-29; it is not an eleven-command or editor-parity acceptance report.

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

## Completed focused checks

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

Index, semantic, project/resource and Neovim integration are being assembled and
reviewed. Their focused results do not replace the original complete editor
workflows or the stock-host end-to-end gate.

## Concrete remaining input differences

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

All eleven commands remain required. A temporarily smaller negotiated command
set, typed unavailable responses, or a passing five-command smoke is not the
completion condition. Full original Neovim behavior, live applied edits,
unsaved-update/lifecycle tests, packaging and final legacy runtime retirement
remain required.

## Earlier stable cross-platform checkpoint

Before these IPC edits, commit `c1f0fd430e30bab2867a725ab1089fbd3efbf2a1`
passed [GitHub Actions run 40168190710](https://github.com/cotrin8672/mc-dev-lsp/actions/runs/40168190710).
The downloaded full logs independently resolve all 2,604 original JUnit case
locators: Windows has 1,671 native functions and 1,525 editor assertions; Linux
has 1,670 functions and 1,487 editor assertions. Both have 69 native targets and
zero failed, ignored or filtered native tests. The Windows-only teardown process
entry does not execute assertions on Linux.

Both optimized archives have all 24 payload hashes and the manifest verified,
correct PE/ELF executables and successful stdio smoke results. The exact archive
and executable hashes are retained in `rust-test-migration.json` under
`verification.nativeCi`; raw artifacts are in `build/ci-runs/40168190710/`.
This evidence is pinned to the earlier commit and does not validate the new IPC
working tree or full Minecraft host integration.
