# Installation

This guide covers prerequisites, installing the extension jar with Mason, configuring an external jar path, and building the JDT LS bundle from source.

mcdev runs as an OSGi bundle inside JDT LS. Neovim talks to it through `workspace/executeCommand` requests. See [Protocol And Neovim](03-protocol-and-neovim.md) for the integration model.

## Prerequisites

| Requirement | Notes |
|---|---|
| **Java 21** | Runtime and toolchain for JDT LS and the extension bundle. |
| **Neovim 0.10+** | Uses `vim.lsp` APIs used by `mcdev-nvim`. |
| **JDT LS** | Install through Mason (`:MasonInstall jdtls`) or provide your own `jdtls` on `PATH`. |
| **nvim-jdtls** | Recommended for starting and attaching JDT LS to Gradle/Maven mod projects. |
| **Completion plugin** | Optional. [blink.cmp](https://github.com/Saghen/blink.cmp), [nvim-cmp](https://github.com/hrsh7th/nvim-cmp), or omnifunc can be wired explicitly. |

Optional for building from source:

- Gradle (wrapper included in the repository)
- Git

## Quick start with Mason

Add the mcdev registry before the core Mason registry:

```lua
require("mason").setup({
  registries = {
    "github:cotrin8672/mc-dev-lsp",
    "github:mason-org/mason-registry",
  },
})
```

Then install the extension bundle:

```vim
:MasonInstall mcdev-jdtls-extension
```

Configure mcdev and append the resolved jar to your existing `nvim-jdtls` config:

```lua
require("mcdev").setup({
  completion = {
    enable = true,
    source = "blink", -- blink | cmp | omnifunc
  },
  mappings = {
    preferred_at_target = "descriptor",
    mixin_class_insert = "import", -- import | fqn
    inject_method_descriptor = "auto", -- auto | always | never
  },
})

local jdtls = require("jdtls")

local config = {
  cmd = { vim.fn.stdpath("data") .. "/mason/bin/jdtls" },
  root_dir = jdtls.setup.find_root({ "gradlew", "build.gradle", "build.gradle.kts", "pom.xml", ".git" }),
}

if require("mcdev.jdtls").extend_config(config) then
  jdtls.start_or_attach(config)
end
```

`extend_config(config)` only appends the mcdev bundle to `config.init_options.bundles`; it does not change your JDT LS command, root detection, settings, or capabilities.

## External jar path

Use this path for Nix, system packages, or local builds.

1. Obtain the extension jar. After a local build it is:

   ```text
   mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar
   ```

2. Add `mcdev-nvim` to your Neovim runtime path. For local development, point at the repository checkout:

   ```lua
   vim.opt.runtimepath:prepend("/path/to/mc-dev-lsp/mcdev-nvim")
   ```

3. Configure mcdev with the jar path and pass the jar to JDT LS through `init_options.bundles`.

   `mcdev-nvim` is opt-in. `setup()` alone does not install keymaps or enable completion/diagnostics. Enable only the pieces you want:

   ```lua
   require("mcdev").setup({
     jdtls = {
       extension_jar = "/absolute/path/to/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar",
     },
     completion = {
       enable = true,
       source = "blink", -- blink | cmp | omnifunc
     },
     mappings = {
       preferred_at_target = "descriptor",
       mixin_class_insert = "import", -- import | fqn
       inject_method_descriptor = "auto", -- auto | always | never
     },
   })

   local config = {
     cmd = { vim.fn.stdpath("data") .. "/mason/bin/jdtls" },
   }

   if require("mcdev.jdtls").extend_config(config) then
     require("jdtls").start_or_attach(config)
   end
   ```

4. Wire completion, diagnostics, navigation, and code actions through your own Neovim plugin config or keymaps. The Lazy.nvim example shows blink.cmp source wiring.

5. Open a Minecraft mod workspace (Fabric, Forge, or NeoForge Gradle project) and run `:McdevInfo` in a Java buffer.

   You should see project platform, mapping state, mixin config count, and bytecode index status.

For a full Lazy.nvim layout, see [lazy.nvim setup](lazy-nvim.md).

## Build from source

From the repository root:

```powershell
gradle test
gradle :mcdev-jdtls-extension:jar
gradle :mcdev-jdtls-extension:checkBundle
```

The bundle jar is written to:

```text
mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar
```

`checkBundle` verifies the OSGi manifest, `plugin.xml`, delegate command handler class, embedded `mcdev-core` classes, and Kotlin stdlib.

Point `extension_jar` at the absolute path of that file. Rebuild and restart JDT LS after Kotlin changes.

### Environment variables (optional)

| Variable | Purpose |
|---|---|
| `JDTLS_CMD` | Override the `jdtls` executable path (used by E2E scripts). |
| `JDTLS_PLUGINS_DIR` | Compile-time path to JDT LS plugins when building the extension. |
| `MCDEV_JDTLS_EXTENSION_JAR` | Runtime extension jar path for Neovim when Mason is not used. |
| `MCDEV_BUNDLE_JAR` | Bundle jar path for headless OSGi E2E. |
| `MCDEV_E2E_WORKSPACE` | Fixture workspace path for headless OSGi E2E. |

## Verify the installation

### Gradle and bundle checks

```powershell
gradle test
gradle :mcdev-jdtls-extension:checkBundle
```

### Headless Neovim adapter tests

```powershell
nvim --headless -u mcdev-nvim/tests/minimal_init.lua -c "luafile mcdev-nvim/tests/run.lua" -c qa
```

### Local OSGi bundle E2E

This starts real Mason `jdtls`, loads the bundle, and exercises `mcdev.info` and `mcdev.completion`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
```

See [Local OSGi Bundle E2E](local-osgi-e2e.md) for manual Neovim steps.

## Supported buffers

mcdev completion and diagnostics apply to:

- Java (`java`) — Mixin, MixinExtras, and related annotations
- Access Widener (`.accesswidener`, `.aw`, or `accesswidener` filetype)
- Access Transformer (`_at.cfg`, `accesstransformer.cfg`, `.at`, or `accesstransformer` filetype)
- Mixin config JSON (`json`)

AW and AT buffers may not attach to JDT LS as normal Java documents. The Neovim plugin sends unsaved buffer text and a detected `languageId` with every request. See [troubleshooting](troubleshooting.md#aw-and-at-buffers).

## Next steps

- [Mason setup](mason.md) — custom registry and Mason package details
- [Lazy.nvim setup](lazy-nvim.md) — full plugin spec with blink.cmp and jdtls bundles
- [Troubleshooting](troubleshooting.md) — common setup failures
- [Contributing](contributing.md) — where to put semantic vs integration code
