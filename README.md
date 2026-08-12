# mc-dev-lsp

Minecraft modding semantic support for Neovim through a JDT LS extension.

The repository follows the design dossier in [docs](docs/README.md). Kotlin owns Minecraft modding semantics, the JDT LS bundle owns Java/project integration, and Lua owns only Neovim transport and UI adaptation.

## Features

- **Completion** — Mixin targets, inject methods, `@At` bytecode targets, MixinExtras handlers, Access Widener, and Access Transformer slots with readable labels and exact insertion text
- **Diagnostics** — Mixin, MixinExtras, AW, and AT issues with stable diagnostic codes via `mcdev.diagnostics`
- **Code actions** — Mixin config entries, descriptor fixes, handler signature generation, AW/AT entry generation and remapping
- **Definition and references** — Mixin target navigation through `mcdev.definition` and `mcdev.references`; Neovim keymaps are always user-defined
- **Project introspection** — `:McdevInfo` and `:McdevReindex` for platform, mappings, configs, and index state

## Modules

- `mcdev-core`: editor-independent JVM descriptor, mapping, bytecode, Mixin, MixinExtras, Access Widener, and Access Transformer semantics.
- `mcdev-protocol`: command payload and response DTOs shared by Neovim and the JDT LS extension.
- `mcdev-jdtls-extension`: OSGi bundle entry point and JDT LS command integration.
- `mcdev-nvim`: Neovim plugin that sends `workspace/executeCommand` requests to JDT LS.
- `mcdev-test-fixtures`: fixture projects for semantic and packaging tests.

## User documentation

- [Installation](docs/installation.md) — prerequisites, JDT LS, prebuilt jar, build from source
- [Mason setup](docs/mason.md) — use Mason for JDT LS while loading the mcdev bundle separately
- [Lazy.nvim setup](docs/lazy-nvim.md) — full Lazy spec with mcdev-nvim and jdtls bundles
- [Troubleshooting](docs/troubleshooting.md) — bundle loading, workspace root, AW/AT buffers, diagnostics
- [Contributing](docs/contributing.md) — architecture boundaries and the no-Lua-semantics rule

## Build

This project targets Java 21 and Kotlin JVM target 21.

```powershell
gradle test
gradle :mcdev-jdtls-extension:jar
gradle :mcdev-jdtls-extension:checkBundle
nvim --headless -u mcdev-nvim/tests/minimal_init.lua -c "luafile mcdev-nvim/tests/run.lua" -c qa
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-layout.ps1
```

The `CI` GitHub Actions workflow runs `gradle test --no-daemon`, the headless Neovim adapter tests, and `scripts/check-layout.ps1` on Linux and Windows for pull requests and pushes to `main`.

The JDT LS extension jar is produced as:

```text
mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-<version>.jar
```

## Neovim Setup

Mason should own the `jdtls` executable and ordinary formatter/linter tools.
Do not register this repository as a Mason registry; `mcdev-jdtls-extension`
is built from this repo and passed to JDT LS through `init_options.bundles`.

With lazy.nvim, a minimal setup looks like this:

```lua
return {
  -- Optional example with mason-tool-installer. Use Mason UI or another
  -- ensure-installed layer if that is how your config manages tools.
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    dependencies = { "mason-org/mason.nvim" },
    opts = {
      ensure_installed = {
        "jdtls",
      },
    },
  },

  {
    "cotrin8672/mc-dev-lsp",
    name = "mcdev-nvim",
    build = "gradle :mcdev-jdtls-extension:jar --no-daemon",
    init = function(plugin)
      vim.opt.rtp:prepend(plugin.dir .. "/mcdev-nvim")
    end,
    opts = {
      insert = {
        at_target = "smart",
        mixin_class_import = true,
        inject_method_descriptor = "auto",
      },
    },
    config = function(plugin, opts)
      vim.opt.rtp:prepend(plugin.dir .. "/mcdev-nvim")
      require("mcdev").setup(opts)
    end,
  },

  {
    "mfussenegger/nvim-jdtls",
    dependencies = { "mcdev-nvim" },
    ft = "java",
    config = function()
      local jdtls = require("jdtls")
      local config = require("my.java.jdtls").config()

      if require("mcdev.jdtls").extend_config(config) then
        jdtls.start_or_attach(config)
      end
    end,
  },
}
```

The plugin auto-discovers the newest repo-built jar under
`mcdev-jdtls-extension/build/libs`. Set `jdtls.extension_jar` only when the jar
is installed elsewhere. `extend_config(config)` only appends the resolved bundle to
`config.init_options.bundles`; it does not change your JDT LS `cmd`, `root_dir`,
`settings`, or `capabilities`.

### Completion Sources

mcdev completion is a separate source because it uses the `mcdev.completion`
command, not plain `textDocument/completion`.

Blink:

```lua
{
  "saghen/blink.cmp",
  dependencies = { "mcdev-nvim" },
  opts = {
    sources = {
      default = { "lsp", "path", "snippets", "mcdev" },
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

Keep the provider enabled for Java, Access Widener, and Access Transformer buffers; `mcdev.blink` performs its own context check. The score offset makes mcdev's complete snippets (for example `method = "…"`) win over JDT LS' incomplete `method = ` annotation item.

nvim-cmp:

```lua
{
  "hrsh7th/nvim-cmp",
  dependencies = { "mcdev-nvim" },
  config = function()
    local cmp = require("cmp")
    cmp.register_source("mcdev", require("mcdev.cmp").new())
    cmp.setup({
      sources = {
        { name = "mcdev", priority = 1100 },
        { name = "nvim_lsp", priority = 1000 },
      },
    })
  end,
}
```

The higher mcdev priority serves the same purpose as Blink's score offset: complete Mixin snippets should beat JDT LS' plain annotation-attribute stubs.

Use your normal Neovim keymap layer for navigation and code actions. The current JDT LS bundle exposes mcdev navigation through `workspace/executeCommand` commands (`mcdev.definition`, `mcdev.references`); it does not contribute to JDT LS `textDocument/definition` directly.

Diagnostics are off by default in `mcdev-nvim` to avoid sending JDT LS work on every edit. Enable them explicitly when you want on-save publication:

```lua
require("mcdev").setup({
  diagnostics = {
    enabled = true,
    events = { "BufWritePost" },
    debounce_ms = 1000,
    insert_mode = false,
  },
})
```

Use `:McdevDiagnosticsRefresh` for a manual refresh, `:McdevDiagnosticsStatus` to inspect the current state, `:McdevHealth` when completion does not appear, and `:McdevDebugCompletion` to see the raw completion response.

mcdev hover is currently implemented as a custom `workspace/executeCommand` request through the `mcdev.hover` command. When mcdev navigation support is enabled, the Neovim adapter binds `K` to that custom hover UI. It is not yet integrated into standard `textDocument/hover`.

Example navigation keymap:

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

See [docs/lazy-nvim.md](docs/lazy-nvim.md) for a complete Lazy.nvim configuration.

## Local OSGi Bundle E2E

Verify the extension bundle loads in real Mason `jdtls` and answers `mcdev.info` / `mcdev.completion`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
```

See [docs/local-osgi-e2e.md](docs/local-osgi-e2e.md) for manual Neovim setup.

## Current Implementation Status

See [docs/10-implementation-status.md](docs/10-implementation-status.md) for the current feature and test matrix.
