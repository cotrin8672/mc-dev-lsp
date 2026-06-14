# mcdev-kotlin

Minecraft modding semantic support for Neovim through a JDT LS extension.

The repository follows the design dossier in [docs](docs/README.md). Kotlin owns Minecraft modding semantics, the JDT LS bundle owns Java/project integration, and Lua owns only Neovim transport and UI adaptation.

## Features

- **Completion** — Mixin targets, inject methods, `@At` bytecode targets, MixinExtras handlers, Access Widener, and Access Transformer slots with readable labels and exact insertion text
- **Diagnostics** — Mixin, MixinExtras, AW, and AT issues with stable diagnostic codes via `mcdev.context`
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

- [Installation](docs/installation.md) — prerequisites, Mason, prebuilt jar, build from source
- [Mason setup](docs/mason.md) — install the JDT LS extension bundle through a custom Mason registry
- [Lazy.nvim setup](docs/lazy-nvim.md) — full Lazy spec with mcdev-nvim and jdtls bundles
- [Troubleshooting](docs/troubleshooting.md) — bundle loading, workspace root, AW/AT buffers, diagnostics
- [Contributing](docs/contributing.md) — architecture boundaries and the no-Lua-semantics rule

## Build

This project targets Java 21 and Kotlin JVM target 21.

```powershell
gradle test
gradle :mcdev-jdtls-extension:jar
gradle :mcdev-jdtls-extension:checkBundle
```

The JDT LS extension jar is produced as:

```text
mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar
```

## Neovim Setup

Mason owns the `jdtls` executable and the `mcdev-jdtls-extension` bundle. Add
the mcdev registry to Mason, then install `jdtls` and `mcdev-jdtls-extension`
from your Mason layer. mcdev does not run Mason installs by itself.

With lazy.nvim, a minimal setup looks like this:

```lua
return {
  {
    "mason-org/mason.nvim",
    opts = {
      registries = {
        "github:cotrin8672/mc-dev-lsp",
        "github:mason-org/mason-registry",
      },
    },
  },

  -- Optional example with mason-tool-installer. Use Mason UI or another
  -- ensure-installed layer if that is how your config manages tools.
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    dependencies = { "mason-org/mason.nvim" },
    opts = {
      ensure_installed = {
        "jdtls",
        "mcdev-jdtls-extension",
      },
    },
  },

  {
    name = "mcdev-nvim",
    -- Mason installs the JDT LS bundle. lazy.nvim loads this Lua plugin.
    dir = "/path/to/mc-dev-lsp/mcdev-nvim",
    lazy = false,
    opts = {
      insert = {
        at_target = "smart",
        mixin_class_import = true,
        inject_method_descriptor = "auto",
      },
    },
    config = function(_, opts)
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

`extend_config(config)` only appends the Mason-installed mcdev bundle to
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
        mcdev = require("mcdev.blink").source(),
      },
    },
  },
}
```

nvim-cmp:

```lua
{
  "hrsh7th/nvim-cmp",
  dependencies = { "mcdev-nvim" },
  config = function()
    local cmp = require("cmp")
    cmp.register_source("mcdev", require("mcdev.cmp").source())
    cmp.setup({
      sources = {
        { name = "nvim_lsp" },
        { name = "mcdev" },
      },
    })
  end,
}
```

Use your normal Neovim keymap layer for navigation and code actions. The current JDT LS bundle exposes mcdev navigation through `workspace/executeCommand` commands (`mcdev.definition`, `mcdev.references`); it does not contribute to JDT LS `textDocument/definition` directly.

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
