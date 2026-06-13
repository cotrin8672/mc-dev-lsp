# Lazy.nvim Setup

This is a complete Lazy.nvim example for mcdev with Mason `jdtls`, the mcdev Mason registry, and `nvim-jdtls`.

Install the extension bundle first:

```lua
require("mason").setup({
  registries = {
    "github:cotrin8672/mc-dev-lsp",
    "github:mason-org/mason-registry",
  },
})
```

```vim
:MasonInstall mcdev-jdtls-extension
```

If Nix, a system package, or a local build owns the jar, set `jdtls.extension_jar` explicitly in `require("mcdev").setup()`.

Adjust the plugin checkout path to match your machine. On Windows, use forward slashes or escaped backslashes in Lua strings.

Completion is enabled by default. mcdev registers available blink.cmp / nvim-cmp adapters automatically and falls back to omnifunc. Navigation and code actions remain user keymap choices.

## Full spec

```lua
local mcdev_root = "C:/Users/you/ghq/github.com/cotrin8672/mc-dev-lsp"

return {
  -- mcdev Neovim plugin (local checkout)
  {
    name = "mcdev-nvim",
    dir = mcdev_root .. "/mcdev-nvim",
    lazy = false,
    config = function()
      require("mcdev").setup({
        mappings = {
          preferred_at_target = "descriptor",
          mixin_class_insert = "import",
          inject_method_descriptor = "auto",
        },
      })
    end,
  },

  -- JDT LS client
  {
    "mfussenegger/nvim-jdtls",
    dependencies = { "mcdev-nvim" },
    ft = "java",
    config = function()
      require("mcdev.jdtls").start_or_attach()
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

## nvim-cmp instead of blink.cmp

mcdev tries to register nvim-cmp automatically when it is available. If your completion manager loads after mcdev and you need explicit registration:

```lua
{
  "hrsh7th/nvim-cmp",
  dependencies = { "mcdev-nvim" },
  config = function()
    local cmp = require("cmp")
    cmp.setup({
      sources = {
        { name = "nvim_lsp" },
        { name = "mcdev" },
      },
    })
    cmp.register_source("mcdev", require("mcdev.cmp").new())
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
2. The buffer filetype or extension is recognized by `mcdev.buffer`.

Recommended filetype settings:

```lua
vim.filetype.add({
  extension = {
    accesswidener = "accesswidener",
    aw = "accesswidener",
    at = "accesstransformer",
  },
  pattern = {
    [".-_at%.cfg"] = "accesstransformer",
    ["accesstransformer%.cfg"] = "accesstransformer",
  },
})
```

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
