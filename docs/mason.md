# Mason Setup

mcdev publishes the JDT LS extension bundle through a Mason registry package.

The Mason package only installs the extension jar. It does not replace `jdtls`, `nvim-jdtls`, or your Java LSP configuration. mcdev still works by adding the jar to JDT LS through `init_options.bundles`.

## Registry

Add the mcdev registry before the core Mason registry:

```lua
require("mason").setup({
  registries = {
    "github:cotrin8672/mc-dev-lsp",
    "github:mason-org/mason-registry",
  },
})
```

Install `jdtls` and `mcdev-jdtls-extension` through your Mason setup. That can be Mason UI, `:MasonInstall`, or an ensure-installed plugin. mcdev does not trigger installation; it only resolves the installed jar.

The installed jar is resolved from:

```text
$MASON/share/mcdev-jdtls-extension/io.github.mcdev.jdtls.jar
```

When `$MASON` is not set, mcdev uses:

```text
vim.fn.stdpath("data") .. "/mason"
```

## Neovim

With Mason, `extension_jar` can usually be omitted:

```lua
require("mcdev").setup()
```

Add the bundle to your existing `nvim-jdtls` config:

```lua
local jdtls = require("jdtls")

local config = require("my.java.jdtls").config()

if require("mcdev.jdtls").extend_config(config) then
  jdtls.start_or_attach(config)
end
```

`extend_config(config)` only appends the mcdev jar to `config.init_options.bundles`. It does not change `cmd`, `root_dir`, `settings`, `capabilities`, or other JDT LS options.

For a minimal setup, mcdev can start JDT LS itself and infer the workspace root:

```lua
require("mcdev").setup()
require("mcdev.jdtls").start_or_attach()
```

## External Package Managers

If Nix, a system package, or a local build owns the jar, pass the path explicitly:

```lua
require("mcdev").setup({
  jdtls = {
    extension_jar = "/nix/store/.../share/java/io.github.mcdev.jdtls.jar",
  },
})
```

You can also set:

```text
MCDEV_JDTLS_EXTENSION_JAR=/absolute/path/to/io.github.mcdev.jdtls.jar
```

The resolution order is:

1. `jdtls.extension_jar`
2. `MCDEV_JDTLS_EXTENSION_JAR`
3. Mason `mcdev-jdtls-extension`
