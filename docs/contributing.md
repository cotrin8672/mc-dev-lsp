# Contributing

This guide explains where code belongs in the mcdev monorepo. Read it before adding features so semantic logic stays in Kotlin and Lua remains a thin transport layer.

For agent-oriented checklists, see [Agent Onboarding](08-agent-onboarding.md). For module layout, see [Repository Layout](01-repository-layout.md).

## Three layers

```text
mcdev-nvim            UI and transport (Lua)
mcdev-jdtls-extension JDT LS integration (Kotlin + OSGi)
mcdev-core            Minecraft modding semantics (Kotlin/JVM)
```

Shared command payloads live in `mcdev-protocol`.

## Where semantics live

All Minecraft modding rules belong in **`mcdev-core`**.

| Concern | Module | Examples |
|---|---|---|
| JVM descriptors | `mcdev-core` | Parse/render method signatures, member targets |
| Mappings | `mcdev-core` | Tiny v2, SRG, namespace remapping |
| Bytecode index | `mcdev-core` | ASM class reading, `@At` instruction candidates |
| Mixin / MixinExtras | `mcdev-core` | Completion, diagnostics, code actions, definitions |
| Access Widener / AT | `mcdev-core` | Parsers, slot detection, SRG-aware insertion |
| Project context | `mcdev-core` | Platform detection, config discovery |
| Protocol DTOs | `mcdev-protocol` | Request/response shapes, error codes, command names |

`mcdev-core` must **not** depend on Eclipse JDT, LSP4J, or Neovim.

When adding a new rule, start with a core service and unit tests against fixtures before touching JDT LS or Lua.

## Handler layer

**`mcdev-jdtls-extension`** connects JDT LS to core services.

Handlers own:

- OSGi activator and command registration (`plugin.xml`, `McdevDelegateCommandHandler`)
- Decoding Neovim command payloads (`ProtocolPayloadDecoder`)
- JDT project and classpath lookup (`FileBasedProjectContextService`)
- Java AST and binding integration where needed
- Converting domain results to protocol DTOs and LSP shapes (`CompletionItemConverter`, `DiagnosticConverter`, `CodeActionConverter`, `DefinitionConverter`)
- Routing AW/AT buffers by `languageId` or file extension (`AwAtServiceFacade`)

Handler classes follow the naming pattern `Mcdev*Handler`:

```text
McdevCompletionHandler
McdevDiagnosticsHandler   (serves mcdev.diagnostics, with mcdev.context as a compatibility alias)
McdevDefinitionHandler
McdevReferencesHandler
McdevCodeActionHandler
McdevInfoHandler
McdevReindexHandler
```

Integration rules stay here. Do not reimplement descriptor parsing, mapping resolution, or mixin validation in handlers — delegate to `mcdev-core` facades (`MixinServiceFacade`, `AwAtServiceFacade`).

## No Lua semantics rule

**`mcdev-nvim`** is a transport and UI adapter only.

Lua may:

- Find the active JDT LS client
- Collect buffer text, cursor position, and effective `languageId`
- Send `workspace/executeCommand` payloads
- Convert completion items for blink.cmp / nvim-cmp (preserve `label` vs `insertText`)
- Register `:McdevInfo`, `:McdevReindex`, and similar commands
- Detect AW/AT buffers (`mcdev.buffer`)

Lua must **not**:

- Parse Mixin annotations semantically
- Parse AW or AT lines beyond filetype detection
- Resolve mappings or namespaces
- Scan jars or classpath
- Interpret bytecode
- Decide handler signatures or mixin target validity
- Implement separate completion behavior for Blink vs cmp

If you are about to write descriptor logic in Lua, move it to `mcdev-core`.

## Decision checklist

For every change, ask:

```text
Is this a semantic rule, an integration rule, or a UI transport rule?
```

| Answer | Target module |
|---|---|
| Semantic rule | `mcdev-core` |
| Integration rule | `mcdev-jdtls-extension` |
| UI transport rule | `mcdev-nvim` |

## Completion item rule

Never collapse human-readable labels into insertion text.

```text
label       setScreen(Screen): void
insertText  m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V
```

Tests should assert label and insertText differ where mappings apply.

## Tests

| Layer | Test style |
|---|---|
| `mcdev-core` | JVM unit tests, fixture-backed E2E in core |
| `mcdev-jdtls-extension` | Handler tests with fixture workspaces |
| `mcdev-nvim` | Headless Lua adapter tests (`mcdev-nvim/tests/run.lua`) |
| Full stack | OSGi E2E (`scripts/run-osgi-e2e.ps1`) |

Add core tests first. Add handler tests when wiring protocol conversion. Add Lua tests only for payload shape and adapter behavior.

## Verification commands

```powershell
gradle test
gradle :mcdev-jdtls-extension:checkBundle
nvim --headless -u mcdev-nvim/tests/minimal_init.lua -c "luafile mcdev-nvim/tests/run.lua" -c qa
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
```

## Documentation

User-facing guides:

- [Installation](installation.md)
- [Lazy.nvim setup](lazy-nvim.md)
- [Troubleshooting](troubleshooting.md)

Design dossier (architecture and work packages):

- [docs/README.md](README.md)

When you add a command or change protocol fields, update `docs/03-protocol-and-neovim.md` and [Implementation Status](10-implementation-status.md).
