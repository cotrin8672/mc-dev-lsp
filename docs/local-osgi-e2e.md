# Local OSGi Bundle E2E

This guide verifies that the mcdev JDT LS extension bundle loads in a real `jdtls` process and answers `workspace/executeCommand` requests from Neovim.

## Prerequisites

- Java 21+
- Neovim 0.10+
- JDT LS installed locally (for example via Mason: `:MasonInstall jdtls`)
- Gradle

## One-command E2E

From the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
```

The script will:

1. Build the OSGi bundle jar (`:mcdev-jdtls-extension:jar`)
2. Validate bundle contents (`:mcdev-jdtls-extension:checkBundle`)
3. Materialize `build/e2e-workspace` from the `fabric-basic` fixture
4. Start headless Neovim with Mason `jdtls`
5. Load the bundle through `init_options.bundles`
6. Call `mcdev.info` and `mcdev.completion` against `ExampleMixin.java`

## Manual local setup in Neovim

Build the bundle:

```powershell
gradle :mcdev-jdtls-extension:jar :mcdev-jdtls-extension:checkBundle
```

Prepare the fixture workspace:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-e2e-workspace.ps1
```

Configure mcdev and start JDT LS with the bundle:

```lua
require("mcdev").setup({
  jdtls = {
    extension_jar = "C:/path/to/mc-dev-lsp/mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-0.1.0-SNAPSHOT.jar",
  },
})

local mcdev_jdtls = require("mcdev.jdtls")
mcdev_jdtls.start_or_attach({
  root_dir = "C:/path/to/mc-dev-lsp/build/e2e-workspace",
})
```

Open `src/main/java/com/example/mixin/ExampleMixin.java`, place the cursor inside `@Mixin(...)`, then run:

```vim
:McdevInfo
```

You should see Fabric project state and a ready bytecode index.

## Environment overrides

| Variable | Purpose |
|---|---|
| `JDTLS_CMD` | Path to `jdtls` executable |
| `JDTLS_PLUGINS_DIR` | Compile-time path to JDT LS plugins when building the extension |
| `MCDEV_BUNDLE_JAR` | Used by the headless E2E script |
| `MCDEV_E2E_WORKSPACE` | Used by the headless E2E script |

## What this proves

- `plugin.xml` registers `McdevDelegateCommandHandler`
- OSGi activator initializes `McdevCommandDispatcher`
- Bundle jar contains `mcdev-core`, `mcdev-protocol`, Gson, and Kotlin stdlib
- Real `jdtls` accepts `init_options.bundles`
- `mcdev.info` and `mcdev.completion` return structured responses through LSP

## Troubleshooting

```text
mcdev: extension jar is not configured or readable
```

Build the bundle jar first and point `extension_jar` to the absolute path.

```text
jdtls client did not initialize within 120s
```

Confirm Java 21 is available and Mason `jdtls` starts normally outside mcdev.

```text
Java language server doesn't support the command 'mcdev.info'
```

The bundle did not load. Check that `init_options.bundles` contains the absolute bundle jar path and that `checkBundle` passes.
