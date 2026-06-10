# Protocol And Neovim Integration

## Design Principle

Neovim is the UI and transport layer. Kotlin is the semantic layer.

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
mcdev.codeAction
mcdev.reindex
mcdev.context
mcdev.info
```

Do not design around arbitrary custom LSP methods unless JDT LS support is confirmed for the specific integration point.

## Why Commands First

JDT LS external bundles are commonly injected through `init_options.bundles`. That gives extension code a place to run, but it does not automatically guarantee that arbitrary custom LSP methods can be registered as normal language-server endpoints.

Using commands gives the project a practical and stable integration path:

- Neovim can call commands through the active JDT LS client.
- The extension can return structured payloads.
- The project avoids depending on unsupported custom protocol hooks.
- Blink/cmp can still render results as normal completion UI.

If a future JDT LS extension point supports standard completion contribution safely, the protocol can add that path later without changing core semantics.

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

The user controls whether to build the extension jar or point to an existing binary.

Recommended config:

```lua
require("mcdev").setup({
  jdtls = {
    extension_jar = "/path/to/io.github.mcdev.jdtls.jar",
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
```

Example `nvim-jdtls` integration:

```lua
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

## Blink.cmp Source

The Blink source should:

- check if the active buffer is Java, AW, AT, or relevant JSON
- find an active JDT LS client for the project
- send `mcdev.completion`
- convert response items into Blink items
- preserve `label`, `detail`, `filterText`, and exact text edit
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

## AW/AT Buffer Handling

JDT LS may not attach to AW/AT buffers as normal Java documents.

Therefore Neovim must pass:

- workspace root
- document URI
- buffer text
- cursor position
- detected or configured language ID

The extension must not assume it can read current unsaved AW/AT content from disk.

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
Extension: 0.1.0
Protocol: 1
```

### :McdevReindex

Should call `mcdev.reindex`.

Expected behavior:

- clear project indexes
- rebuild asynchronously
- report progress if possible
- avoid blocking the editor

### :McdevLog

Optional but useful.

Should open recent extension messages if the extension exposes them.

## Failure UX

User-facing failure should be precise:

```text
mcdev: no active JDT LS client for this buffer
mcdev: extension jar is not loaded by JDT LS
mcdev: protocol mismatch, client=1 server=2
mcdev: mappings are not loaded for this project
mcdev: bytecode index is not ready
```

Do not say generic `completion failed` unless no structured error is available.

