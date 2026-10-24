# Lazy.nvim setup

mcdev uses your existing ktlsp LSP client and an independent Rust companion.
Configure both executables as argv lists. The companion is registered under
`init_options.analysisProviders["mcdev.mixin"]`; it is not a second LSP server
and does not create another project index. No JDT LS extension, Mason bundle,
extension JAR, Java subprocess helper, or repository executable discovery is used.

Diagnostics remain disabled by default. Completion sources, mappings, navigation,
and code-action keymaps are opt-in. Loading the plugin does not start either
executable. Supply executables you installed or built explicitly.

```lua
{
  "cotrin8672/mc-dev-lsp",
  name = "mcdev-nvim",
  init = function(plugin)
    vim.opt.rtp:prepend(plugin.dir .. "/mcdev-nvim")
  end,
  config = function()
    require("mcdev").setup({
      ktlsp = { cmd = { "/absolute/path/to/ktlsp" } },
      companion = { cmd = { "/absolute/path/to/mcdev-companion" } },
      insert = {
        at_target = "smart", mixin_class_import = true,
        inject_method_descriptor = "auto",
      },
      diagnostics = {
        enabled = false, debounce_ms = 500, insert_mode = true,
        events = { "TextChanged", "TextChangedI", "TextChangedP", "InsertLeave", "BufWritePost" },
        in_flight_policy = "latest", stale_result_policy = "drop",
      },
    })
  end,
}
```

Windows argv entries can be absolute `.exe` paths. Pass arguments as separate
array entries rather than shell command strings. Build the Rust companion with
`cargo build --release --bin mcdev-companion`; install/build ktlsp separately.

## Keep an existing ktlsp setup

Before starting your existing client, merge the provider into its configuration:

```lua
local config = require("my.ktlsp").config()
local merged, err = require("mcdev.ktlsp").extend_config(config)
assert(merged, err)
vim.lsp.start(merged)
```

This preserves your command, root, settings, capabilities, initialization options,
other providers, and an explicitly configured `mcdev.mixin` command,
configuration, or disabled state. To override the companion for this config,
pass `{ companion = { cmd = { "/explicit/companion" } } }` as the second argument.
Repeated merges are idempotent and do not share mutable provider tables between
workspace configs.

Alternatively, explicitly call `require("mcdev.ktlsp").start_or_attach({
root_dir = "/project" })` after setup. It uses the configured host argv and a
normalized workspace root. No JVM `-data` directory is needed. ktlsp owns all
workspace sessions and indexing.

## Capability and readiness

Indexed MC commands are used only when the selected host advertises them through
`executeCommandProvider.commands`. An unadvertised command returns an explicit
unavailable error. Readiness notifications and the authenticated loopback
completion endpoint resume requests only for the selected workspace client.
The standalone `--stdio-completion` mode provides the existing no-index subset
while the project is unavailable. It does not claim target-member or project
index completion. Standard LSP preference/fallback remains unchanged.

The full indexed MC command/resource wire is still being connected in ktlsp;
older hosts can provide standard language features and the no-index subset.
`McdevHealth` reports the actual advertised/available behavior rather than
reporting an extension loaded successfully.

## Completion Sources

Register the mcdev source in your completion UI.

Blink:

```lua
{
  "saghen/blink.cmp",
  dependencies = { "mcdev-nvim" },
  opts = {
    sources = {
      default = function()
        return require("mcdev.blink").route_sources({ "lsp", "path", "snippets", "mcdev" })()
      end,
      providers = {
        mcdev = {
          name = "mcdev",
          module = "mcdev.blink",
          score_offset = 100,
          async = true,
          timeout_ms = 5000,
        },
      },
    },
  },
}
```

Do not restrict this provider to `vim.bo.filetype == "java"`: the source already checks its context, and a Java-only predicate disables Access Widener and Access Transformer completion. Use `route_sources` to select the sources for the current input position; setting a score offset alone does not exclude unrelated candidates.

nvim-cmp:

```lua
{
  "hrsh7th/nvim-cmp",
  dependencies = { "mcdev-nvim" },
  config = function()
    local cmp = require("cmp")
    local mcdev_cmp = require("mcdev.cmp")
    cmp.register_source("mcdev", mcdev_cmp.new())
    cmp.setup({
      sources = {
        { name = "mcdev", priority = 1100 },
        mcdev_cmp.with_exclusive_filter({ name = "nvim_lsp", priority = 1000 }),
      },
    })
  end,
}
```

Wrap other generic sources, such as `buffer` and `path`, with `with_exclusive_filter` as well. The wrapper preserves their existing options and entry filters, excludes their candidates in Mixin-only input positions, and restores them in ordinary Java positions. The `mcdev` source already checks its position and does not need this wrapper.

Blink, cmp, and omnifunc share the same `mcdev.completion` command path. Do not implement separate semantic behavior in each adapter.

## Navigation and code actions

mcdev does not install `gd`, `gr`, or code-action keymaps. If you want mcdev-specific navigation before the standard LSP fallback, wire it explicitly:

```lua
vim.keymap.set("n", "gd", function()
  require("mcdev.navigation").definition(0, nil, function(locations, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
      return
    end
    if locations and locations[1] then
      vim.lsp.util.show_document(locations[1], "utf-8", { focus = true })
      return
    end
    vim.lsp.buf.definition()
  end)
end)
```

The client prefers standard LSP navigation and falls back to advertised
`mcdev.definition` / `mcdev.references` commands when needed.

When `navigation.enable` is true, `K` uses standard hover first when
`standard_lsp.prefer` is enabled, then the advertised MC hover fallback.

## AW and AT filetypes

The host need not attach its ordinary language providers to AW/AT buffers. MC
commands can select the running ktlsp client from the matching workspace when:

1. A ktlsp client is running for the mod project workspace and advertises MC commands.
2. The file extension is recognized by `mcdev.buffer`: `.accesswidener`, `.aw`, `_at.cfg`, `accesstransformer.cfg`, or `.at`.

You do not need to add global Neovim filetype rules for mcdev itself. If you want syntax highlighting or other plugins to see custom filetypes, configure that separately in your editor layer.

Open AW/AT files from the same workspace root ktlsp uses so `workspaceRoot` resolves correctly.

## Commands

After setup, these user commands are available:

| Command | Protocol command | Purpose |
|---|---|---|
| `:McdevInfo` | `mcdev.info` | Show project platform, mappings, configs, index state |
| `:McdevReindex` | `mcdev.reindex` | Request classpath/bytecode index rebuild |
| `:McdevReloadProjectContext` | `mcdev.reloadProjectContext` | Reload the selected project context |
| `:McdevDumpContext` | `mcdev.dumpContext` | Inspect the project context |
| `:McdevDiagnosticsRefresh` | `mcdev.diagnostics` | Refresh this buffer's MC diagnostics |
| `:McdevDiagnosticsStart` / `Stop` / `Status` | Local scheduler | Control or inspect diagnostic updates |
| `:McdevHealth` | Advertised MC probes | Show real backend availability |
| `:McdevDebugCompletion` / `DebugDiagnostics` | Advertised MC probes | Inspect actual requests and results |

## Updating the companion

Rebuild/reinstall the executable at the configured argv path, then restart its
ktlsp provider/client. `:McdevReloadProjectContext` refreshes project context
when that command is advertised; it does not hot-load executable code. Run
`:McdevInfo` to inspect the actual backend artifact/build metadata.

## Related docs

- [Installation](installation.md)
- [Troubleshooting](troubleshooting.md)
- [Native Neovim host-contract migration](neovim-host-contract.md)
