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

Use a separately installed ktlsp binary built from
[`ea8be67`](https://github.com/cotrin8672/ktlsp/commit/ea8be671ed4a876a5d5514fb63daa043e953c08f)
or a compatible revision of its experimental extension protocol.
[Native installation](native-distribution.md) and
[Neovim configuration](lazy-nvim.md) show explicit executable argv arrays and
`mcdev.ktlsp.extend_config` integration with an existing client configuration.
The MC setup function does not start a second host.

```lua
local host = {
  cmd = { "/absolute/path/to/ktlsp" },
  root_dir = "/absolute/path/to/project",
}
assert(require("mcdev.ktlsp").extend_config(host, {
  companion = { cmd = { "/absolute/path/to/mcdev-companion" } },
}))
vim.lsp.start(host)
```

The provider ID is `mcdev.mixin`. When no explicit `executeCommands` list is
configured, `extend_config` registers the eleven MC command IDs. An existing
list and an explicit `enabled = false` are respected. Neovim sends those domain
commands through the host's advertised `ktlsp.providerCommand` and passes the
current LSP buffer text, document version and selected workspace. Reindex and
project reload request one host refresh before dispatch.

The companion must also acknowledge and implement each registered command; a
configuration entry alone does not establish feature availability.
The [process contract](mcdev-companion-contract.md) records published host
revisions, wire sources, artifact hashes, accepted trace barriers and exact
editor evidence.

## Current scope

The standalone process handles the bounded Mixin/Shadow contribution policy.
The Rust library also contains all migrated domain services and command DTOs.
Real project-aware commands still require the companion to consume host-owned
facts through the routing described in the
[remaining integration contract](mcdev-command-host-contract.md).

The pinned host also exposes scoped source and resource snapshots, selected
runtime bytes, a physical JVM catalog and provider-command routing. Coverage is
limited to the selected module and source set. Unavailable semantic facts,
unsaved-source authority, unsupported inputs and incomplete inventories remain
Unknown; a display path does not grant filesystem access. Companion adapters and
full editor parity require their own integration verification.

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
