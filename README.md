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

The companion uses the published host operations for project/class facts,
source snapshots, owned resources, runtime class bytes, the JVM catalog and
provider commands. All eleven command IDs have evaluation routes; complete
project-wide results still need host facts that are not published. Missing
facts return typed unavailable results. The bounded Mixin/Shadow editor proof
and provider-command proof have separate scopes; see the
[integration checkpoint](docs/ipc-integration-status.md). Neither proves full
Minecraft editor parity.

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

For separate host proofs, `scripts/run-host-smoke-compact.ps1` runs the bounded
semantic fixture, and `scripts/run-host-commands-compact.ps1` runs provider
commands through Neovim. Both accept explicit `-HostBinary`, `-HostSha256`,
`-CompanionBinary` and `-CompanionSha256` arguments, verify the binaries, and
record input hashes and scope under finite deadlines. The command fixture also
requires Gradle and a Java toolchain.

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
