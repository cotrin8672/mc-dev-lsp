# Lazy.nvim Setup

This is a complete Lazy.nvim example for mcdev with Mason `jdtls`, `nvim-jdtls`, and blink.cmp.

Adjust paths to match your checkout. On Windows, use forward slashes or escaped backslashes in Lua strings.

`mcdev-nvim` does not enable editor behavior by default. The example below explicitly enables completion and wires blink.cmp. Navigation and code actions remain user keymap choices.

## Full spec

```lua
local mcdev_root = "C:/Users/you/ghq/github.com/cotrin8672/mc-dev-lsp"
local mcdev_jar = mcdev_root .. "/mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar"

return {
  -- mcdev Neovim plugin (local checkout)
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
          source = "blink",
        },
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
      local jdtls = require("jdtls")
      local mcdev = require("mcdev")

      local mason_jdtls = vim.fn.stdpath("data") .. "/mason/bin/jdtls"
      local jdtls_cmd = vim.fn.executable(mason_jdtls) == 1 and mason_jdtls or "jdtls"
      local data_dir = vim.fn.stdpath("cache") .. "/mcdev-jdtls"

      local config = {
        cmd = { jdtls_cmd, "-data", data_dir },
        root_dir = jdtls.setup.find_root({ "build.gradle", "build.gradle.kts", "pom.xml", ".git" }),
        init_options = {
          bundles = {
            mcdev.extension_jar(),
          },
        },
      }

      jdtls.start_or_attach(config)
    end,
  },

  -- blink.cmp with mcdev source
  {
    "Saghen/blink.cmp",
    dependencies = { "mcdev-nvim" },
    opts = {
      sources = {
        default = { "lsp", "mcdev" },
        providers = {
          mcdev = {
            name = "mcdev",
            module = "mcdev.blink",
            enabled = function()
              return require("mcdev.buffer").is_mcdev_buffer(0)
            end,
          },
        },
      },
    },
  },
}
```

## Alternative: mcdev.jdtls helper

The repository includes a thin `mcdev.jdtls` helper that picks Mason `jdtls`, validates the extension jar, and injects `init_options.bundles`:

```lua
require("mcdev").setup({
  jdtls = {
    extension_jar = mcdev_jar,
  },
})

require("mcdev.jdtls").start_or_attach({
  root_dir = vim.fn.getcwd(),
})
```

Use this for E2E workspaces or minimal configs. For normal mod development, prefer `nvim-jdtls` with `root_dir` detection as shown above.

## nvim-cmp instead of blink.cmp

If you use nvim-cmp, set `completion.source = "cmp"` in `require("mcdev").setup()` and register the mcdev source after cmp loads:

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

Blink and cmp share the same `mcdev.completion` command path. Do not implement separate semantic behavior in each adapter.

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
