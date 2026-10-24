# Protocol And Neovim Integration

## Design Principle

Neovim is the UI and transport layer. Rust core provides MC semantics. The existing ktlsp client owns workspace indexing and registers an independent Rust companion under `analysisProviders["mcdev.mixin"]`.

The protocol must allow Neovim to send enough context for unsaved buffers, AW/AT files, and Java annotations without making Lua understand Minecraft semantics.

## Transport Choice

Default transport:

```text
workspace/executeCommand
```

Commands:

```text
mcdev.completion
mcdev.definition
mcdev.references
mcdev.hover
mcdev.codeAction
mcdev.diagnostics
mcdev.reindex
mcdev.reloadProjectContext
mcdev.dumpContext
mcdev.context (compatibility alias for diagnostics)
mcdev.info
```

The client sends MC commands only when ktlsp advertises them in
`executeCommandProvider.commands`. The full indexed command/resource wire is
under integration; absent capabilities are reported as unavailable. The native
line helper (`mcdev-companion --stdio-completion`) retains the no-index fallback,
and an authenticated loopback endpoint is used only after a validated readiness
notification. Neither path creates a separate LSP or authoritative index.

The Neovim adapter uses the first successful standard `textDocument/definition`,
`textDocument/references`, or `textDocument/hover` response once, falling back
once only when all results are empty or failed. Code actions merge standard and
mcdev candidates, preserve distinct effects, and resolve/execute standard
actions on their originating client. Before applying edits, the client checks
the captured buffer URI, changedtick, text and LSP version; changes while a
resolve request is pending also invalidate its edit.

Cursor positions passed to the Lua helpers use Neovim's one-based rows and byte columns. Explicit code-action ranges and mcdev protocol locations use zero-based rows and UTF-16 columns. Standard LSP requests, edits and navigation use each client's negotiated encoding; definition/reference callbacks expose that encoding as their fourth argument, defaulting to UTF-16 for mcdev results.

`mcdev.hover` is a custom `workspace/executeCommand` request. The Neovim adapter binds `K` to this command when navigation support is enabled, so the current hover UI is mcdev-specific rather than the standard LSP hover provider. A future standard hover provider should keep `mcdev.hover` as a compatibility and debug command.

## Registration and ownership

Set explicit `ktlsp.cmd` and `companion.cmd` argv, then merge the provider into
your existing ktlsp configuration with `mcdev.ktlsp.extend_config`. Loading
mcdev does not launch a server or scan repositories for executables. Existing
manual settings and other providers are preserved. There is no JDT bundle/JAR
installation path. All MC rules remain in Rust core.

`mcdev.info` v1 retains the legacy `jarLocation` JSON slot for wire compatibility;
the native runtime places its executable artifact path there. Neovim displays it
as an executable location, also accepting `executableLocation` if supplied.
Native authoritative syntax provenance is `HOST_NORMALIZED`, with semantic
context/confidence and project session versions. It does not imply use of JDT
compilation-unit or Java-project objects.

## Request Context

Every request should include:

```json
{
  "protocolVersion": 1,
  "workspaceRoot": "file:///path/to/project",
  "documentUri": "file:///path/to/file.java",
  "languageId": "java",
  "position": { "line": 10, "character": 20 },
  "bufferText": "...",
  "client": {
    "name": "mcdev.nvim",
    "version": "0.1.0"
  }
}
```

`bufferText` is important. It allows completion and diagnostics for unsaved Java, AW, AT, and JSON files.

Large buffers may later use document version synchronization instead of sending full text every time, but the first stable protocol should favor correctness and simpler debugging.

## Completion Request

```json
{
  "protocolVersion": 1,
  "workspaceRoot": "file:///project",
  "documentUri": "file:///project/src/main/java/example/Mixin.java",
  "languageId": "java",
  "position": { "line": 8, "character": 19 },
  "bufferText": "...",
  "trigger": {
    "kind": "manual",
    "character": null
  },
  "options": {
    "preferredAtTarget": "descriptor",
    "mixinClassInsert": "import",
    "injectMethodDescriptor": "auto"
  }
}
```

Completion response:

```json
{
  "protocolVersion": 1,
  "items": [
    {
      "label": "draw(String, float, float, int): int",
      "detail": "TextRenderer",
      "documentation": null,
      "filterText": "draw TextRenderer String float int",
      "insertText": "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
      "insertTextFormat": "plainText",
      "kind": "method",
      "sortKey": "0200_draw",
      "edit": {
        "range": {
          "start": { "line": 8, "character": 19 },
          "end": { "line": 8, "character": 19 }
        },
        "newText": "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I"
      },
      "additionalEdits": [],
      "metadata": {
        "source": "mixin.atTarget",
        "owner": "net/minecraft/client/font/TextRenderer",
        "name": "draw",
        "descriptor": "(Ljava/lang/String;FFI)I",
        "namespace": "NAMED"
      }
    }
  ],
  "warnings": []
}
```

## Code Action Request

```json
{
  "protocolVersion": 1,
  "workspaceRoot": "file:///project",
  "documentUri": "file:///project/src/main/java/example/Mixin.java",
  "languageId": "java",
  "range": {
    "start": { "line": 4, "character": 0 },
    "end": { "line": 4, "character": 20 }
  },
  "bufferText": "...",
  "diagnosticCodes": [
    "MIXIN_CLASS_NOT_LISTED_IN_CONFIG"
  ]
}
```

Response should be convertible to LSP `CodeAction`.

Code action examples:

```text
Add class to mixins.client.json
Add descriptor to method target
Fix WrapOperation handler signature
Generate Access Widener entry
Generate Access Transformer entry
Remap AT entry to SRG
```

## Neovim Setup Shape

The user selects installed/built ktlsp and companion executables explicitly.
The Neovim plugin should be usable with a minimal `setup()` call. It exposes completion adapters as sources for the user's completion UI, but it must not install completion UI configuration, navigation keymaps, or code-action keymaps.

Recommended config:

```lua
require("mcdev").setup({
  ktlsp = { cmd = { "/absolute/path/to/ktlsp" } },
  companion = { cmd = { "/absolute/path/to/mcdev-companion" } },
  insert = {
    at_target = "smart",
    mixin_class_import = true,
    inject_method_descriptor = "auto", -- auto | always | never
  },
})
```

The plugin does not discover or execute repository artifacts automatically.
`ktlsp.cmd` and `companion.cmd` are argv lists, not shell strings.

Defaults do not publish diagnostics. Enable debounced diagnostics explicitly for updates while editing and after saving; completion sources, navigation, and code actions remain explicit user choices around the thin `mcdev.*` adapters.

Merge into an existing manual ktlsp configuration before starting it:

```lua
local mcdev = require("mcdev")

local config = require("my.ktlsp").config()

local merged, err = require("mcdev.ktlsp").extend_config(config)
assert(merged, err)
vim.lsp.start(merged)
```

## Blink.cmp Source

Configure Blink's dynamic default source list with
`require("mcdev.blink").route_sources(your_default_sources)`. It keeps `lsp`
and `mcdev` for an annotation-name prefix or an open MCDev annotation value or selector, and returns the
caller-provided standard sources everywhere else. The fallback list may still
contain `mcdev`; the helper removes that entry outside an owned context.

The Blink source should:

- check if the active buffer is Java, Kotlin, AW, AT, or relevant JSON
- find the ktlsp client for this workspace
- send advertised `mcdev.completion` or use the no-index native helper subset
- convert response items into Blink items
- preserve `label`, `detail`, `filterText`, `insertTextFormat`, and exact text edit
- expose warnings through logs or `:McdevInfo`, not as noisy completion items

It should not:

- parse Mixin annotations
- parse AW or AT lines semantically
- infer descriptors
- cache classpath data

## cmp Source

The cmp source should be a thin adapter over the same Lua request function used by Blink.

Do not implement separate semantic behavior for Blink and cmp.

## omnifunc

Omnifunc support can be a fallback adapter. It should use the same command and response model.
It is disabled by default because omnifunc is synchronous; the opt-in adapter
uses a short configurable timeout instead of blocking editing for seconds.

## AW/AT Buffer Handling

The host's ordinary language provider may not attach to AW/AT buffers.

Therefore Neovim must pass:

- workspace root
- document URI
- buffer text
- cursor position
- detected or configured language ID

The companion must use the supplied unsaved snapshot instead of assuming disk is current.

## Commands

### :McdevInfo

Should call `mcdev.info` and display:

```text
Project: fabric
Mappings: named <-> intermediary loaded
Minecraft jar: found
Mixin config: 2 files
Access Widener: 1 file
Access Transformer: none
Bytecode index: ready
Companion: 0.1.0
Protocol: 1
```

### :McdevReindex

Should call `mcdev.reindex`.

Expected behavior:

- clear project indexes
- rebuild asynchronously
- report progress if possible
- avoid blocking the editor

### Health and scheduling

`:McdevHealth`, `:McdevDebugCompletion`, and `:McdevDebugDiagnostics` inspect
actual advertised backend results. Diagnostic scheduling remains disabled by
default; `:McdevDiagnosticsStart`, `Stop`, `Status`, and `Refresh` control it.

## Failure UX

User-facing failure should be precise:

```text
mcdev: no active ktlsp client for this buffer
mcdev: ktlsp has not advertised command mcdev.completion
mcdev: companion executable is not configured or available
mcdev: protocol mismatch, client=1 server=2
mcdev: mappings are not loaded for this project
mcdev: bytecode index is not ready
```

Do not say generic `completion failed` unless no structured error is available.
