# Native companion distribution

The native package is a separate `mcdev-companion` executable plus the
production `mcdev-nvim` runtime. It does not bundle ktlsp, start a host, invoke
Java/JVM helpers, or rebuild ktlsp with an extension.

This is an experimental `semantic-v1` distribution. The current production
capability is only the bounded Mixin/Shadow policy slice: the child exchanges
versioned stdio JSON-RPC frames with a host that owns project facts, parsing,
acceptance and publication. The full eleven-command
surface, project/resource/class-byte wire, Java SDK facts, AW/AT, mappings,
MixinExtras and the remaining MC host integration are not connected by this
package. A successful package build is not a full-replacement claim.

## Install

1. Download the archive for the existing release tag or workflow-dispatch
   version and extract it.
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

The archive name uses the existing release tag or `workflow_dispatch` input
(without its leading `v`). The current script checks that value against the
existing root Gradle text version (`0.7.7`) while the native migration is being
staged. That Gradle reference is temporary; a future native-only release should
make a dedicated native version field the canonical source instead of inferring
it from the legacy build file. The companion Cargo package currently retains
its independent `0.1.0` package version; the package manifest records both
values and does not silently bump or rename either version namespace.

Verify an extracted payload with the included `manifest.json`; its `files`
entries contain each payload path, byte count and SHA-256. The manifest excludes
itself from that list to avoid a self-referential hash.

The local verification path is Windows-capable. Linux packaging is prepared in
the release workflow using the existing Ubuntu/Windows matrix, but a local
Windows run does not constitute Linux execution evidence.
