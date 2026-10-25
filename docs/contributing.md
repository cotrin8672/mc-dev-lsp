# Contributing

This branch replaces the JDT LS implementation with Rust. Use the
[migration record](rust-migration.md) for current acceptance; the older Kotlin
design dossier describes the preserved legacy implementation.

## Boundaries

| Concern | Owner |
|---|---|
| Descriptors, mappings, bytecode interpretation, Mixin/Extras, AW/AT and MC project rules | `mcdev-core` |
| Existing command payloads, response shapes and conversion | `mcdev-protocol` |
| Extension process, cancellation, command/core composition and host adapters | `mcdev-companion` |
| Java/Kotlin parsing, project/classpath/SDK selection, owned source/resource facts, opaque identities and final read validation | Stock ktlsp |
| Neovim buffers, client selection, completion presentation and user commands | `mcdev-nvim` |

Use the existing core facades and converters. Minecraft policy stays in Rust;
the host provides generic authoritative inputs. Lua must not resolve mappings,
scan classpaths, interpret bytecode, validate Mixin targets or choose handler
signatures. Keep labels separate from insertion text and preserve additional
edits, snippets and UTF-16 ranges.

Unknown facts cannot become empty lists, missing members, a fabricated project
session or zero-filled generations. Check availability before calling an API
that assumes complete data. Source overlays must remain current and source
authority must avoid unnecessary binary reads. Keep host borrows out of child
I/O and let the host validate positive and negative reads before publication.

## Tests

Add domain tests in `mcdev-core`, conversion tests in `mcdev-protocol`, process
and whole-command integration in `mcdev-companion`, and client-only tests in
`mcdev-nvim`. Reuse existing fixtures and helpers. A fake generic fact provider
is appropriate; a fake MC result does not prove that a command reaches the real
core services.

Every migrated legacy case keeps its original input, final assertion behavior,
fixture and identity in the [test ledger](rust-test-migration.json). Grouped
tables may share a test function, but each original case must execute. New or
intentional platform differences need a concrete per-case explanation and an
independent regression. Preserve golden bytes and their provenance; native
verification must not invoke a Java compiler or JVM fixture helper.

```powershell
rtk proxy pwsh -NoProfile -File scripts/run-rust-tests-compact.ps1
```

The runner retains full logs, enforces finite deadlines, stops only owned
process trees, and rejects debug filters that would turn a full suite into a
subset. Run affected checks after an edit and the complete suite once after
changes are stable. Real host/editor checks are separate acceptance evidence;
their pinned binaries and observed scope must be recorded explicitly.

When changing a public command, update its DTO/converter, callers, tests and
[host integration requirements](mcdev-command-host-contract.md). Keep
[native installation](native-distribution.md) and
[Neovim setup](lazy-nvim.md) consistent with capabilities actually advertised.
