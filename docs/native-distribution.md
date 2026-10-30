# Native companion distribution

The native package is a separate `mcdev-companion` executable plus the
production `mcdev-nvim` runtime. It does not bundle ktlsp, start a host, invoke
Java/JVM helpers, or rebuild ktlsp with an extension.

This experimental distribution requires `semantic-v1`. It also negotiates the
published project/class, source snapshot, owned resource, runtime class-byte,
JVM catalog and provider-command capabilities. The host owns project facts,
parsing, acceptance and publication. All eleven commands have routes, with
typed unavailable results where complete project or semantic facts are
missing. Full original editor behavior remains incomplete; a successful
package build is not a full-replacement claim. See the
[integration checkpoint](ipc-integration-status.md) for measured scope.

## Install

1. Extract an archive built with the command below or published by the native
   release workflow. On Linux, mark the extracted companion executable with
   `chmod +x /absolute/path/to/mcdev-companion`.
2. Add the extracted `mcdev-nvim` directory to Neovim's runtime path.
3. Configure the separately installed ktlsp and the extracted companion with
   explicit argv arrays:

   ```lua
   require("mcdev").setup({
     ktlsp = { cmd = { "/absolute/path/to/ktlsp" } },
     companion = { cmd = { "/absolute/path/to/mcdev-companion" } },
   })
   ```

   On Windows, use the `.exe` path. Keep each argument as a separate array
   entry; neither executable is discovered through a shell command string.

`setup` only stores these options; it does not start or register a host. If you
already own a ktlsp configuration, merge the provider into that configuration
and start the same client. This preserves existing manual settings and does not
create a second LSP:

```lua
local config = require("my.ktlsp").config()
local merged, err = require("mcdev.ktlsp").extend_config(config)
assert(merged, err)
vim.lsp.start(merged)
```

The archive contains the executable, the currently present production runtime
directories (currently `mcdev-nvim/lua`), the existing third-party notices,
this usage text as `README-native.md`, and `manifest.json`. It deliberately
does not contain tests, test corpora, Kotlin/JDT jars, a host executable, or
build output other than the companion binary.

## Build metadata

`BuildIdentity::current()` records the companion executable path and
compile-time build metadata. The `mcdev.info` route still needs an authoritative
complete project summary before it can return its success DTO. Release
packaging injects the existing product version, the UTC ISO build time, and the
Git commit; a dirty checkout is marked with `+dirty`. A source archive or
checkout without Git may provide `MCDEV_BUILD_COMMIT`, `MCDEV_BUILD_TIME`, or
`MCDEV_BUILD_VERSION` explicitly at compile time. Missing commit/time values
are reported as `unknown`; the version otherwise falls back to the companion's
Cargo package version. No metadata is inferred from an unrelated artifact.

## Build and verify locally

From the repository root on PowerShell 7 with Rust and Neovim available:

```powershell
pwsh -NoProfile -File scripts/build-native-release.ps1 -Version 0.7.7
```

The script runs the standalone command
`cargo build --release --locked -p mcdev-companion --bin mcdev-companion`
through `Invoke-RustCheck`, packages only the whitelisted production files,
creates a SHA-256/size manifest, checks the archive contents, and reuses the
existing `mcdev-nvim/tests/native_helper.lua` entry as a finite
`--stdio-completion` smoke test. Full stdout/stderr and result records remain
under `build/rust-test-logs/native-release-*`; the archive is written under
`build/release/native/`.

The archive name uses the release tag or `workflow_dispatch` input without its
leading `v`. The existing product version remains `0.7.7`; its canonical source
is now `[workspace.package].version` in the root `Cargo.toml`, inherited by all
three native crates. The packager uses Cargo metadata to validate the requested
version. Building or packaging the companion does not read a Gradle build file
or fetch ktlsp source.

Verify an extracted payload with the included `manifest.json`; its `files`
entries contain each payload path, byte count and SHA-256. The manifest excludes
itself from that list to avoid a self-referential hash.

The manifest's `capabilities` field lists the required `semantic-v1` base.
`optionalCapabilities` and `supportedOps` describe supported negotiation, not
facts that every host supplies. Packaging checks both semantic-only fallback
and optional negotiation against the packaged executable. Provider commands
also require the configured command allowlist.

The CI workflow builds and checks Ubuntu and Windows packages. The
[test ledger](rust-test-migration.json) pins the revisions and artifacts actually
verified on each platform; a local Windows run alone does not prove Linux
execution.
