# Mason Setup

mcdev uses Mason for the standard `jdtls` executable when available.

The mcdev JDT LS extension bundle is not installed from a Mason registry in this
repo layout. Build or install the jar separately, then let mcdev add it
to JDT LS through `init_options.bundles`.

## Registry

Keep Mason on the official registry:

```lua
require("mason").setup({
  registries = {
    "github:mason-org/mason-registry",
  },
})
```

Install `jdtls` through your Mason setup. That can be Mason UI,
`:MasonInstall`, or an ensure-installed plugin. Do not add
`github:cotrin8672/mc-dev-lsp` to Mason `registries`.

## Neovim

After the repository build, the versioned extension jar is discovered automatically:

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
3. newest repo-built `mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-*.jar`
4. legacy Mason bundle lookup, only when `jdtls.mason.enabled = true`
