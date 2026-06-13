# mcdev-kotlin Design Dossier

This directory is an onboarding dossier for building `mcdev-kotlin`, a Kotlin/JVM semantic extension for Minecraft modding support in Neovim through JDT LS.

The goal is not to build a small Neovim plugin. The goal is to provide an IDEA MinecraftDev-like semantic editing experience for the selected Minecraft modding surfaces:

- Mixin
- MixinExtras
- Access Widener
- Access Transformer
- Mixin config JSON quick fixes

The explicit non-goals are:

- NBT tooling
- project creator
- GUI wizard
- debugger position manager
- run configuration generator

## Document Map

- [00 Product Goals](00-product-goals.md)
  Defines the product target, completion experience, code action expectations, and non-goals.
- [01 Repository Layout](01-repository-layout.md)
  Defines the monorepo structure, module responsibilities, release artifact shape, and ownership boundaries.
- [02 Architecture](02-architecture.md)
  Defines the Kotlin/JVM architecture, OSGi bundle model, core domain model, service boundaries, and failure handling.
- [03 Protocol And Neovim](03-protocol-and-neovim.md)
  Defines how Neovim talks to the JDT LS extension, how Blink/cmp integration should work, and what must stay out of Lua.
- [04 Domain Features](04-domain-features.md)
  Defines the concrete Mixin, MixinExtras, Access Widener, Access Transformer, diagnostics, definitions, references, and code actions to implement.
- [05 Project Context And Indexing](05-project-context-and-indexing.md)
  Defines project detection, mapping context, classpath, indexing, cache keys, invalidation, and performance rules.
- [06 Testing Strategy](06-testing-strategy.md)
  Defines the required test layers, approximate test counts, fixture matrix, regression expectations, and packaging checks.
- [07 Work Packages](07-work-packages.md)
  Defines implementation work packages and acceptance criteria without treating the product as a staged partial rollout.
- [08 Agent Onboarding](08-agent-onboarding.md)
  A practical checklist for future agents so they can work without drifting from the goal.
- [09 Decisions And Risks](09-decisions-and-risks.md)
  Records architectural decisions, known risks, and the rules for changing the design.
- [10 Implementation Status](10-implementation-status.md)
  Records what is implemented, what remains, and verification commands.

## User Guides

- [Installation](installation.md)
  Prerequisites, prebuilt jar setup, and build from source.
- [Mason Setup](mason.md)
  Install the JDT LS extension bundle through the mcdev Mason registry.
- [Lazy.nvim Setup](lazy-nvim.md)
  Full Lazy.nvim spec with mcdev-nvim path and jdtls bundles.
- [Troubleshooting](troubleshooting.md)
  Bundle loading, workspace root, AW/AT buffers, McdevInfo, and diagnostics.
- [Contributing](contributing.md)
  Where semantics live, handler layer boundaries, and the no-Lua-semantics rule.
- [Local OSGi Bundle E2E](local-osgi-e2e.md)
  Verify the extension bundle in real Mason `jdtls`.

## One-Sentence Mission

Build a monorepo where Kotlin owns Minecraft modding semantics, JDT LS owns Java project/classpath/AST integration, and Neovim owns only transport, configuration, and editing UI.

## Critical Design Rule

Never move semantic logic into Lua.

Lua may:

- find the active Neovim LSP client
- collect the current buffer text and cursor position
- send commands or requests to the JDT LS extension
- adapt results into Blink/cmp/omnifunc UI
- expose commands such as `:McdevInfo` and `:McdevReindex`

Lua must not:

- parse JVM descriptors beyond trivial UI display
- resolve mappings
- scan jars
- interpret bytecode
- decide Mixin handler signatures
- remap Access Widener or Access Transformer entries
- decide whether a target is valid

## Completion Quality Rule

Every completion item must separate human-readable display from exact insertion text.

Example:

```text
Display: setScreen(Screen): void       named: setScreen
Insert:  m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V
```

This is the core of the intended IDEA-like experience.

## Quick Fix Quality Rule

Quick fixes are not limited to the current Java file. They must be able to update the correct workspace file when the context is known:

- add a mixin class to `mixins.json`
- add an Access Widener entry
- add an Access Transformer entry
- add descriptors to ambiguous targets
- remap names into the required namespace
- fix handler method signatures

All workspace edits must be deterministic, minimal, and test-covered.
