# Troubleshooting

Common setup and runtime failures for mcdev in Neovim with JDT LS.

Error messages are designed to be actionable. If you see a generic failure, check `:messages` and run `:McdevInfo` after confirming JDT LS is attached.

## Extension bundle not loading

### `mcdev: extension jar is not configured or readable`

mcdev could not resolve a readable JDT LS extension jar from explicit config, `MCDEV_JDTLS_EXTENSION_JAR`, or Mason.

Fix:

1. Add the mcdev Mason registry and ensure Mason installs `jdtls` and `mcdev-jdtls-extension`.
2. Or build the jar: `gradle :mcdev-jdtls-extension:jar`
3. If Mason does not own the jar, set an **absolute** path in `jdtls.extension_jar`.
4. Confirm readability: `:lua print(vim.fn.filereadable(require('mcdev').extension_jar()))` should print `1`.

### `Java language server doesn't support the command 'mcdev.info'`

The bundle did not load into JDT LS.

Checklist:

1. `init_options.bundles` includes the mcdev jar path (not just `require("mcdev").setup()`).
2. The jar path is absolute.
3. `gradle :mcdev-jdtls-extension:checkBundle` passes.
4. JDT LS initialized — wait for the `LspAttach` event before calling mcdev commands.

Example bundle injection:

```lua
if require("mcdev.jdtls").extend_config(config) then
  require("jdtls").start_or_attach(config)
end
```

### `jdtls client did not initialize within 120s`

JDT LS failed to start or attach.

Checklist:

1. Java 21 is on `PATH`: `java -version`
2. Mason `jdtls` runs outside mcdev: `:lua print(vim.fn.executable(vim.fn.stdpath("data") .. "/mason/bin/jdtls") == 1 or vim.fn.executable(vim.fn.stdpath("data") .. "/mason/bin/jdtls.cmd") == 1)`
3. The workspace root contains `build.gradle`, `build.gradle.kts`, or `pom.xml`
4. Clear a stale JDT LS data dir if needed: `vim.fn.stdpath("cache") .. "/mcdev-jdtls"`

## Workspace root problems

### `mcdev: no active JDT LS client for this buffer`

No `jdtls` LSP client is attached to the current buffer.

Common causes:

- JDT LS was never started for this project.
- You opened a file outside the detected `root_dir`.
- AW/AT buffer opened before JDT LS attached to the Java project.

Fix:

1. Open a Java file in the mod project and wait for JDT LS.
2. Confirm a client exists: `:lua print(vim.inspect(vim.lsp.get_clients({ name = "jdtls" })))`
3. For AW/AT files, ensure JDT LS is running for the same workspace even though it may not attach to that buffer.

### `workspace root is required` / incomplete project context

The extension received an empty `workspaceRoot` in the request payload.

mcdev resolves workspace root from the active JDT LS client's `config.root_dir`. If that is wrong:

- Set `root_dir` explicitly in `nvim-jdtls` config.
- Use `jdtls.setup.find_root({ "build.gradle", "build.gradle.kts", "pom.xml", ".git" })`.
- Open the mod project at its Gradle root, not a parent directory.

### Mappings or bytecode index not ready

`:McdevInfo` shows partial context.

| Symptom | Likely cause |
|---|---|
| `Mappings: not loaded` | Tiny/SRG mapping files missing or not discovered |
| `Bytecode index: not ready` | Minecraft jar not on classpath, or index still building |
| `Mixin config: none` | No `mixins.*.json` found in expected locations |

Run `:McdevReindex` after fixing classpath or mapping files, then check `:McdevInfo` again.

## AW and AT buffers

### Completion or diagnostics do nothing in `.accesswidener` / `_at.cfg`

AW and AT files are not Java. JDT LS may not attach to them as normal documents.

mcdev still requires:

1. A running JDT LS client for the **same workspace**.
2. Correct extension detection, or custom filetype detection if your editor layer changes these buffers.

Verify detection:

```lua
:lua print(require("mcdev.buffer").detect_file_type())
:lua print(require("mcdev.buffer").effective_language_id())
```

Expected `languageId` values sent to the extension:

- `accesswidener` for Access Widener files
- `accesstransformer` for Access Transformer files

mcdev recognizes the standard AW/AT extensions directly. Add Neovim filetype mappings only if you want syntax highlighting or other editor plugins to treat those buffers specially.

### Stale AW/AT content

The extension reads `bufferText` from Neovim, not from disk. Unsaved edits are included automatically. If results look stale, ensure the request is triggered after the buffer change (re-trigger completion or re-run the diagnostics fetch).

## McdevInfo

### `:McdevInfo` shows nothing or errors

1. Confirm JDT LS is attached (see above).
2. Confirm the bundle loaded (`mcdev.info` command must be registered).
3. Check `:messages` for protocol errors.

Expected output shape:

```text
Project: fabric
Mappings: named <-> intermediary loaded
Minecraft jar: found
Mixin config: 2 files
Access Widener: 1 file
Access Transformer: none
Bytecode index: ready
Extension: 0.1.0
Protocol: 1
```

### `protocol mismatch, client=1 server=2`

The Neovim plugin and extension jar are incompatible versions. Rebuild the extension jar and update `mcdev-nvim` from the same repository revision.

## Diagnostics

### How diagnostics work

Mixin and AW/AT diagnostics are computed by the extension through the `mcdev.diagnostics` command. `mcdev.context` remains as a compatibility alias. The response includes stable diagnostic codes such as:

- `MIXIN_CLASS_NOT_LISTED_IN_CONFIG`
- `UNRESOLVED_AT_TARGET`
- `AW_UNRESOLVED_CLASS`
- `AT_SRG_MAPPING_NOT_FOUND`

Each diagnostic has `code`, `severity`, `message`, and a `range`.

### Diagnostics namespace

When publishing mcdev diagnostics into Neovim's `vim.diagnostic` API, use a dedicated namespace so they do not collide with JDT LS Java diagnostics:

```lua
local mcdev_ns = vim.api.nvim_create_namespace("mcdev")

-- Example: fetch and publish after mcdev.diagnostics
local protocol = require("mcdev.protocol")
protocol.request(protocol.commands.diagnostics, { context = protocol.context() }, function(result)
  if not result or not result.result then return end
  local diags = {}
  for _, d in ipairs(result.result.diagnostics or {}) do
    table.insert(diags, {
      lnum = d.range.start.line,
      col = d.range.start.character,
      end_lnum = d.range["end"].line,
      end_col = d.range["end"].character,
      severity = d.severity == "error" and vim.diagnostic.severity.ERROR
        or d.severity == "warning" and vim.diagnostic.severity.WARN
        or vim.diagnostic.severity.INFO,
      message = d.message,
      code = d.code,
      source = "mcdev",
    })
  end
  vim.diagnostic.set(mcdev_ns, 0, diags)
end)
```

By default `mcdev-nvim` records configuration and creates commands such as `:McdevInfo`, `:McdevReindex`, `:McdevHealth`, and `:McdevDebugCompletion`; diagnostics publication is opt-in. Enable diagnostics with `events = { "BufWritePost" }` for on-save refresh, or use `:McdevDiagnosticsRefresh` manually. Do not put `TextChangedI` in the default diagnostics events unless you explicitly accept per-edit requests.

### Code actions

Code actions are available through `mcdev.codeAction` with `diagnosticCodes` from mcdev diagnostics. They are not bound to `<leader>ca` by default. Wire them through your own keymap, LSP code-action handler, or custom quick-fix UI.

## Still unsupported

These cases are expected to produce no mcdev results today:

- Definition/references from Neovim built-in `vim.lsp.buf.definition()` / `vim.lsp.buf.references()` without custom wiring to `mcdev.definition` / `mcdev.references`

See [Implementation Status](10-implementation-status.md) for the current matrix.

## Getting more help

1. Run the OSGi E2E script to isolate bundle vs editor issues:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
   ```

2. Read [Local OSGi Bundle E2E](local-osgi-e2e.md) for manual reproduction steps.

3. For architecture and ownership boundaries, see [Contributing](contributing.md).
