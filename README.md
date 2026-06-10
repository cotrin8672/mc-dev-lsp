# mcdev-kotlin

Minecraft modding semantic support for Neovim through a JDT LS extension.

The repository follows the design dossier in [docs](docs/README.md). Kotlin owns Minecraft modding semantics, the JDT LS bundle owns Java/project integration, and Lua owns only Neovim transport and UI adaptation.

## Modules

- `mcdev-core`: editor-independent JVM descriptor, mapping, bytecode, Mixin, MixinExtras, Access Widener, and Access Transformer semantics.
- `mcdev-protocol`: command payload and response DTOs shared by Neovim and the JDT LS extension.
- `mcdev-jdtls-extension`: OSGi bundle entry point and JDT LS command integration.
- `mcdev-nvim`: Neovim plugin that sends `workspace/executeCommand` requests to JDT LS.
- `mcdev-test-fixtures`: future fixture projects for semantic and packaging tests.

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

```lua
require("mcdev").setup({
  jdtls = {
    extension_jar = "/path/to/io.github.mcdev.jdtls.jar",
  },
})

local mcdev = require("mcdev")

require("jdtls").start_or_attach({
  cmd = { "jdtls" },
  init_options = {
    bundles = {
      mcdev.extension_jar(),
    },
  },
})
```

## Local OSGi Bundle E2E

Verify the extension bundle loads in real Mason `jdtls` and answers `mcdev.info` / `mcdev.completion`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
```

See [docs/local-osgi-e2e.md](docs/local-osgi-e2e.md) for manual Neovim setup.

## Current Implementation Status

See [docs/10-implementation-status.md](docs/10-implementation-status.md) for the current feature and test matrix.
