# Native client migration checks

The original client has 20 Lua modules and 27 test/support files: 11 unit entry
files, 12 E2E files, three support files, and one Java process fixture. The Lua
suite mixes named checks, script assertions and table loops; it has no single
JUnit-style case count. There are 1,014 original assertion call sites.

`legacy_sources.json` preserves every original test/support byte sequence as
audit data, including all table inputs and the old Java fixture source. It is
never executed. `migration_inventory.json` validates each frozen source SHA-256
against the parent's original ledger and records every assertion, entry call,
replacement file/hash and execution status. Strict source/testcase fidelity also
records all 34 original input tables, 180 table rows, and 829 callback/state
markers; a repeated call string elsewhere is never accepted as loop proof.
Unchanged assertion bodies map to their exact current lines. `explicit_deltas.json`
contains per-original-assertion old/new IDs and equivalent observers for
backend-specific conversions; no file-scoped waiver is used.
`migration_evidence.json` records actual finite executions, per-site assertion
counters and logs. Loop evidence keeps static input rows separate from aggregate
runtime counts: row-level pass attribution is unknown unless explicitly
instrumented, and inferred loop counts are labeled as inference.

The independent native suite passed: Rust process fixture build, Lua syntax,
all 11 original unit entry files plus native guard checks, the actual no-index
helper process, and the original teardown unit/process scenarios. It records
1,521 executed client assertion calls. All 20 native client modules were loaded
by the unit entry. Complete stdout/stderr, per-site invocation counts and exit /
timeout / PID / duration records live under `build/nvim-test-logs/` with the
`native-client-final-*` prefixes. Neovim shutdown-autocmd errors are treated as
failures even if Neovim itself returns exit code zero.

```powershell
rtk cargo build -p mcdev-companion --bin mcdev-companion
rtk proxy pwsh -NoProfile -File scripts/run-nvim-tests-compact.ps1 -Name native-client -Suite
```

The suite compiles the std-only Rust `teardown_process` example with `rustc` and
does not require the companion library to compile merely to obtain that process
fixture. Its real process checks retain normal/hung shutdown, inherited pipes,
owned descendant cleanup, preservation of an unrelated Rust process, snapshot
exit/startup failures, and timeout failure reporting. Windows process identities
use PID plus creation time. The standalone completion check runs the actual
companion through Neovim job I/O, including all five request/response lines,
malformed recovery and EOF exit zero. Requests, stdout and stderr are saved.
No Java compiler, Java helper or JVM is used by this suite.

Additional actual-host proof uses the pinned f595 ktlsp artifact. The test
`e2e/native_materialized_action.lua` obtains a standard materialized organize-
imports action, observes one identical merged effect on its original client,
applies its real edit, compares the complete resulting buffer, rejects stale
reapplication, and verifies clean shutdown. Input SHA-256 remains unchanged
before/after. Evidence: `build/nvim-test-logs/native-materialized-f595.*` and
`build/nvim-test-logs/materialized-f595/proof.json`. This is standard host/UI
proof; it does not exercise an MC command bridge or Java extract-local.

## Pending real indexed editor workflows

The main MC E2E files remain pending until actual host MC commands/navigation
are connected and advertised. The four original JDT action-alias families stay
in unit coverage. Their approved real E2E adaptation uses a genuine native
materialized organize-imports effect; it does not ask ktlsp to fabricate JDT
aliases. The same original Java `left + right` selection still requires real
extract-local and all three post-edit observations. That host feature is under
development and has not been counted as passed. Selected dependency-source
file URIs replace `java/classFileContents`; no source-less virtual document or
decompilation success is invented. See `docs/neovim-host-contract.md`.

The optional real mod-project compile check uses that explicitly selected
project's Gradle wrapper/toolchain. This is separate from native client tests,
and is not a Java LSP/helper dependency.

Refresh evidence only after the recorded finite checks pass:

```powershell
rtk proxy python mcdev-nvim/tests/update_migration_evidence.py
```

The parent's master migration ledger remains owned by the integration task.
