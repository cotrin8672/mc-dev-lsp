# ktlsp integration

The extension and host are separate executables. Stock ktlsp owns Java/Kotlin
parsing, project models, source snapshots, symbol identities, read validation
and LSP publication. `mcdev-companion` computes Minecraft rules in Rust and
exchanges versioned JSON-RPC frames over its standard streams.

No ktlsp/ktcore Rust dependency is part of this workspace. The former linked
prototype was retired after its thirteen scenario families passed in companion
process tests and the independent stock-host/editor integration passed. Its
original test source is preserved at
`mcdev-companion/tests/fixtures/semantic_v1/linked-prototype-cases.rs.txt`, with
the hash and native mappings in the adjacent `linked-retirement.json`.

## Build and configure

```powershell
rtk cargo build --release --locked -p mcdev-companion
```

Use a separately installed ktlsp binary that implements the agreed extension
protocol. [Native installation](native-distribution.md) and
[Neovim configuration](lazy-nvim.md) show explicit executable argv arrays and
`mcdev.ktlsp.extend_config` integration with an existing client configuration.
The MC setup function does not start a second host.

The provider ID is `mcdev.mixin`. The currently verified production capability
is only `semantic-v1`; it is not a claim that every MC command is connected.
The [process contract](mcdev-companion-contract.md) records published host
revisions, wire sources, artifact hashes, accepted trace barriers and exact
editor evidence.

## Current scope

The standalone process handles the bounded Mixin/Shadow contribution policy.
The Rust library also contains all migrated domain services and command DTOs.
Real project-aware commands still require the host-owned generic facts and
command routing described in the [remaining integration contract](mcdev-command-host-contract.md).

The optional `projectContext`/`typeJvm` operations are implemented in the fixed,
host 37 candidate, and their API source is published as `16580db`. Its actual serializer matches the isolated MC
adapter; [the capability record](mcdev-host-capabilities.md) pins the evidence.
The MC production child still negotiates semantic-v1. These limited metadata
operations do not establish a complete project session, class catalog, source/resource
inventory, executable bytecode, Java SDK hierarchy or refresh authority. Unknown
facts must stay unknown; display paths do not grant filesystem access.

## Verification

```powershell
rtk proxy pwsh -NoProfile -File scripts/run-rust-tests-compact.ps1
```

This runs the native workspace, process and Neovim checks with finite deadlines
and full logs. The companion process tests fake only explicit host facts; they
do not contain a host language parser. The separate real-host editor proof
checks applied buffers and fresh accepted evaluations. The latest proof scope
and incomplete original editor workflows remain in the
[migration record](rust-migration.md).
