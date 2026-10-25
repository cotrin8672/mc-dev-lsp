# mc-dev-lsp

Minecraft development support implemented in Rust, with a separate
`mcdev-companion` executable connected to stock ktlsp over stdio JSON-RPC and a
thin Neovim client. Building the companion does not build or link ktlsp.

This is the `codex/ktlsp-rust` replacement branch. The original JDT LS checkout
and its uncommitted work remain on `main`. Full replacement is still in
progress: all 2,604 original JUnit cases have native replacements, but the full
MC command surface and original editor workflows are not yet connected to the
host. See the [migration record](docs/rust-migration.md) for verified scope and
remaining work.

The current real-host integration supports the bounded Mixin/Shadow slice:
Java/Kotlin target bindings, definition/references/rename, diagnostics, and
shared completion. The 112-check editor proof covers that slice. Mappings,
AW/AT, MixinExtras, all eleven MC commands, and source/resource/class-byte/SDK
integration remain outside the published live capability.

## Modules

- `mcdev-core`: descriptors, mappings, bytecode, project/resource models, Mixin,
  MixinExtras, Access Widener and Access Transformer services.
- `mcdev-protocol`: existing command payloads, response DTOs and conversions.
- `mcdev-companion`: standalone extension process and command/core composition.
- `mcdev-nvim`: Neovim transport and UI; Minecraft rules stay in Rust.
- `mcdev-test-fixtures`: preserved source/resource inputs for regression and
  editor tests.

The earlier statically linked `mcdev-ktlsp` prototype has been retired. Its
thirteen scenario families execute in the native companion process tests, and
its original test source is preserved with provenance in the fixtures.

## Build and verify

Rust builds the companion. The complete verification runner additionally needs
PowerShell 7 and Neovim 0.11.5 or newer. A JVM is not required for native core,
protocol, process, or client tests.

```powershell
rtk cargo build --release --locked -p mcdev-companion
rtk proxy pwsh -NoProfile -File scripts/run-rust-tests-compact.ps1
```

The runner checks formatting, all workspace targets and doc tests, builds the
companion, and runs the finite Neovim suite. Full logs are retained under
`build/rust-test-logs` and `build/nvim-test-logs`. The Rust CI workflow uses the
same path on Ubuntu and Windows. Real stock-host tests require a separately
provided, pinned ktlsp executable; the native suite alone does not prove full
editor compatibility.

For that separate proof, `scripts/run-host-smoke-compact.ps1` accepts explicit
`-HostBinary`, `-HostSha256`, `-CompanionBinary` and `-CompanionSha256` arguments.
It verifies both binaries, executes the 112-check fixture with a 120-second
deadline, and records input hashes and the observed scope.

The existing product version is `0.7.7`, now defined by the native Cargo
workspace. Build a native archive with:

```powershell
rtk proxy pwsh -NoProfile -File scripts/build-native-release.ps1 -Version 0.7.7
```

## Setup and development

- [Native distribution and installation](docs/native-distribution.md)
- [Neovim configuration](docs/lazy-nvim.md)
- [ktlsp integration](docs/ktlsp.md)
- [Process protocol and verified host pins](docs/mcdev-companion-contract.md)
- [Remaining host capabilities](docs/mcdev-command-host-contract.md)
- [Contributing](docs/contributing.md)
- [Exact test migration ledger](docs/rust-test-migration.json)

The Kotlin/OSGi sources and older design documents remain migration references;
they are not part of the native build or release.
