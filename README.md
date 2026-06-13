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

- [Installation](docs/installation.md) — prerequisites, prebuilt jar, build from source
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

Build the JDT LS extension jar first:

```powershell
gradle :mcdev-jdtls-extension:jar
```

Then add two pieces to Neovim:

1. load `mcdev-nvim`
2. pass the extension jar to JDT LS through `init_options.bundles`

`mcdev-nvim` is opt-in. Calling `setup()` records configuration and creates the `:McdevInfo` / `:McdevReindex` commands, but it does not install keymaps and does not enable completion or diagnostics unless configured.

Minimal local-checkout setup:

```lua
vim.opt.runtimepath:prepend("/path/to/mc-dev-lsp/mcdev-nvim")

require("mcdev").setup({
  jdtls = {
    extension_jar = "/path/to/mc-dev-lsp/mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar",
  },
  completion = {
    enable = true,
    source = "blink", -- blink | cmp | omnifunc
  },
})

local mcdev = require("mcdev")

require("jdtls").start_or_attach({
  cmd = { "jdtls" },
  root_dir = require("jdtls.setup").find_root({ "build.gradle", "build.gradle.kts", "pom.xml", ".git" }),
  init_options = {
    bundles = {
      mcdev.extension_jar(),
    },
  },
})
```

With Lazy.nvim, the same setup usually looks like this:

```lua
local mcdev_root = "/path/to/mc-dev-lsp"
local mcdev_jar = mcdev_root .. "/mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar"

return {
  {
    name = "mcdev-nvim",
    dir = mcdev_root .. "/mcdev-nvim",
    lazy = false,
    config = function()
      require("mcdev").setup({
        jdtls = {
          extension_jar = mcdev_jar,
        },
        completion = {
          enable = true,
          source = "blink", -- blink | cmp | omnifunc
        },
        diagnostics = {
          enable = true,
        },
      })
    end,
  },
  {
    "mfussenegger/nvim-jdtls",
    dependencies = { "mcdev-nvim" },
    ft = "java",
    config = function()
      local jdtls = require("jdtls")
      local mcdev = require("mcdev")

      jdtls.start_or_attach({
        cmd = { "jdtls" },
        root_dir = jdtls.setup.find_root({ "build.gradle", "build.gradle.kts", "pom.xml", ".git" }),
        init_options = {
          bundles = {
            mcdev.extension_jar(),
          },
        },
      })
    end,
  },
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
