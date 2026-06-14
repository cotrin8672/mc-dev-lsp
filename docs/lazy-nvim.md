# Lazy.nvim Setup

This is a complete Lazy.nvim example for mcdev with Mason `jdtls`, the mcdev Mason registry, and `nvim-jdtls`.

Register the mcdev Mason registry before the core Mason registry:

```lua
require("mason").setup({
  registries = {
    "github:cotrin8672/mc-dev-lsp",
    "github:mason-org/mason-registry",
  },
})
```

Install packages through your Mason layer, for example Mason UI, `:MasonInstall`, or an ensure-installed plugin. mcdev only resolves already-installed Mason packages; it does not install them.

If Nix, a system package, or a local build owns the jar, set `jdtls.extension_jar` explicitly in `require("mcdev").setup()`.

Diagnostics are enabled by default. Completion adapters are exposed as sources for your completion UI; mcdev does not register them globally. Navigation and code actions remain user keymap choices.

## Full spec

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

  -- Optional example. Use Mason UI or another ensure-installed layer if you
  -- already manage tools elsewhere.
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
    -- Local checkout layout. Mason installs the JDT LS bundle, not this Lua plugin.
    dir = "C:/Users/you/ghq/github.com/cotrin8672/mc-dev-lsp/mcdev-nvim",
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

## Alternative: mcdev.jdtls starter

The repository includes a thin `mcdev.jdtls` starter that picks Mason `jdtls`, validates the extension jar, and injects `init_options.bundles`:

```lua
require("mcdev").setup()
require("mcdev.jdtls").start_or_attach()
```

If you already have a detailed `nvim-jdtls` setup, keep it and add mcdev before `start_or_attach`:

```lua
local config = require("my.java.jdtls").config()

if require("mcdev.jdtls").extend_config(config) then
  require("jdtls").start_or_attach(config)
end
```

`extend_config(config)` only appends `init_options.bundles`; it does not change your command, root detection, settings, or capabilities.

## Completion Sources

Register the mcdev source in your completion UI.

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

The current JDT LS bundle exposes mcdev navigation through `mcdev.definition` / `mcdev.references` commands. It does not extend JDT LS `textDocument/definition` directly.

## AW and AT filetypes

JDT LS may not attach to Access Widener or Access Transformer buffers. mcdev still works when:

1. A JDT LS client is running for the mod project workspace.
2. The file extension is recognized by `mcdev.buffer`: `.accesswidener`, `.aw`, `_at.cfg`, `accesstransformer.cfg`, or `.at`.

You do not need to add global Neovim filetype rules for mcdev itself. If you want syntax highlighting or other plugins to see custom filetypes, configure that separately in your editor layer.

Open AW/AT files from the same workspace root JDT LS uses so `workspaceRoot` resolves correctly.

## Commands

After setup, these user commands are available:

| Command | Protocol command | Purpose |
|---|---|---|
| `:McdevInfo` | `mcdev.info` | Show project platform, mappings, configs, index state |
| `:McdevReindex` | `mcdev.reindex` | Request classpath/bytecode index rebuild |

## Reloading the bundle after rebuild

When iterating on the extension jar, reload bundles without restarting Neovim:

```vim
:lua vim.lsp.buf_request(0, 'workspace/executeCommand', {
\  command = 'java.reloadBundles',
\  arguments = { { require('mcdev').extension_jar() } },
\}, function() end)
```

Then run `:McdevInfo` to confirm the extension responds.

## Related docs

- [Installation](installation.md)
- [Troubleshooting](troubleshooting.md)
- [Local OSGi Bundle E2E](local-osgi-e2e.md)
