# Implementation Status

This file records what exists in the repository after the full product completion pass.

## Implemented

### Build and modules

- Gradle Kotlin DSL root project and module build files.
- Java 21 toolchain and Kotlin JVM target 21 configuration.
- `mcdev-core`, `mcdev-protocol`, `mcdev-jdtls-extension`, `mcdev-nvim`, and `mcdev-test-fixtures` modules.

### mcdev-core domain logic

- Descriptor model, parser, renderer, and member target parser.
- Tiny v2 parser, SRG parser, namespace-aware lookup model, `MappingResolver`, project mapping context model, and descriptor remapping service.
- Access Widener and Access Transformer parsers, context extractors, completion, diagnostics, code actions, definition, references, and unified `AwAtServiceFacade`.
- Structured diagnostic and completion domain models.
- Bytecode indexing, instruction extraction, ordinal calculation, and classpath-backed mixin indexes.
- Mixin target/method/@At completion, diagnostics, and code actions (config entry, descriptor, accessor, invoker, overwrite).
- `@Overwrite` annotation support with diagnostics, definition navigation, and references.
- MixinExtras completion, diagnostics (including full `MIXINEXTRAS:EXPRESSION` validation via `ExpressionContextResolver`), and handler signature code actions.
- Project context construction helpers, platform detection, mapping/AW/AT/mixin config discovery.
- Mixin import edit builder for deterministic additional import text edits.
- Mapping-aware `@At` target insert formatting when a `MappingResolver` is available.
- `@Constant` string/int hint generation for CONSTANT `@At` completion items.
- Duplicate `@Mixin` target diagnostics (class array, string targets, and mixin config entries).
- Mixin definition and references services with AW/AT cross-reference scanning.

### mcdev-protocol

- Command names, protocol versioning, request context, completion/navigation/code action/info DTOs, and structured error payloads.

### mcdev-jdtls-extension

- OSGi bundle manifest configuration, bundle activator, service holder, and command dispatcher.
- `plugin.xml` + `McdevDelegateCommandHandler` registered through `org.eclipse.jdt.ls.core.delegateCommandHandler`.
- Fat bundle jar embedding `mcdev-core`, `mcdev-protocol`, Gson, and Kotlin stdlib.
- File-based project context loading (including main + client source sets), classpath indexing, completion/diagnostics/info/reindex handlers.
- AW/AT buffer routing in completion, diagnostics, code action, definition, and references handlers via `AwAtServiceFacade` (languageId or `.accesswidener`/`.aw`/`_at.cfg` extension).
- Protocol payload decoding and completion/diagnostic DTO conversion (including text edits for mixin class completion).
- Mixin and AW/AT definition and references handlers (`mcdev.definition`, `mcdev.references`) with tiered `DefinitionResolutionService` (`SourceIndex` then `JdtReflectionBridge` fallback, `resolution=source|jdt|bytecode_only|unresolved`).
- JDT classpath merge via `JdtClasspathBridge` and Loom remapped jar discovery under `.gradle/loom-cache/remapped_working`.
- Local OSGi bundle E2E via `scripts/run-osgi-e2e.ps1` and fixture matrix via `scripts/run-osgi-e2e-matrix.ps1` (real Mason `jdtls` + headless Neovim), including definition URI assertions (`SimpleTarget.java` line 2), references, and code actions.
- Loom-style OSGi E2E via `scripts/run-osgi-loom-e2e.ps1` with Eclipse `.classpath` source attachment to remapped jar and `mapped-sources/` (asserts `resolution=jdt` when JDT resolves attached sources).
- End-to-end completion wiring: `preferredAtTarget` and mapping-aware `@At` insert formatting via `AtTargetInsertFormatter`, mixin import `additionalEdits` via `MixinImportEditBuilder`, and MixinExtras completions through the core `MixinServiceFacade`.
- Packaging smoke test via `checkBundle` Gradle task (runs as part of `gradle test`): manifest, `plugin.xml` command IDs, handler class, embedded core/protocol/Gson, Kotlin stdlib.

### mcdev-nvim

- Setup API, extension jar accessor, active JDT LS client detection, request wrapper.
- Completion payload construction matching `McdevCompletionRequest` shape.
- Blink, nvim-cmp, and omnifunc adapter shims preserving label vs insertText.
- Commands for `:McdevInfo` and `:McdevReindex`.
- AW/AT buffer detection (`mcdev.buffer`) and languageId routing for all protocol requests.
- Definition and references navigation helpers via `mcdev.definition` / `mcdev.references`; no keymaps are installed by default.
- Code action helper with workspace edit application; no `<leader>ca` keymap is installed by default.
- Diagnostics publication via `mcdev.context` with debounced autocmds and `mcdev` diagnostic namespace, enabled by default.
- Shared DTO converters (`mcdev.convert`) for locations, diagnostics, and code actions.
- `mcdev.jdtls` helper for Mason `jdtls` startup with bundle injection.
- Headless Lua adapter tests with mocked JDT LS client absence handling, AW/AT payload checks, navigation, diagnostics, and code action conversion.

### Documentation (WP-14)

- [installation.md](installation.md) — prerequisites, prebuilt jar, build from source.
- [lazy-nvim.md](lazy-nvim.md) — full Lazy.nvim spec with mcdev-nvim path and jdtls bundles.
- [troubleshooting.md](troubleshooting.md) — bundle loading, workspace root, AW/AT buffers, McdevInfo, diagnostics namespace.
- [contributing.md](contributing.md) — semantic vs handler vs Lua boundaries.

### mcdev-test-fixtures

- Fixture projects: `fabric-basic`, `fabric-mixinextras`, `fabric-aw-at`, `multi-source-set`, `forge-basic`, `neoforge-basic`, `broken-diagnostics`.
- Compiled `SimpleTarget` class bytes and fabric-basic Tiny v2 mappings.
- Fixture resource loader and mixin config validator tests.

### Test coverage (approximate)

| Module | Tests |
|--------|------:|
| mcdev-core | ~500+ |
| mcdev-jdtls-extension | ~115+ |
| mcdev-test-fixtures | ~14 |
| mcdev-protocol | 2 |
| mcdev-nvim (Lua adapter) | ~40 assertions |
| OSGi E2E (real jdtls) | mixin + AW/AT completion/diagnostics/definition/references/code actions |

Gradle `test` total: **~726** tests across Kotlin/Java modules (includes `checkBundle`).

Core tests cover descriptor/mapping/bytecode edge cases, mixin and MixinExtras semantics (including overwrite, expression context validation, shadow static mismatch and ambiguous inject overload), AW/AT completion/diagnostics/code-action/definition/reference flows, project context (including multi-source-set discovery), fixture-backed mapping regression, and mixin E2E flows. JDT LS handler tests cover mixin diagnostics, mixin/MixinExtras/AW/AT code actions, definition/references routing for Java and AW/AT buffers, and completion without facade overrides. Neovim adapter tests cover protocol payloads, LSP conversion, explicit navigation helpers, diagnostics publish, code actions, Blink/cmp/omnifunc adapters, default non-interference, and missing-client errors.

## Product Goal Alignment

All nine acceptance conditions from [00-product-goals.md](00-product-goals.md) are implemented end-to-end:

1. `@At(target)` readable bytecode candidates with exact JVM descriptor insertion.
2. `@Inject(method)` completion with descriptor insertion when overloads require it.
3. `@Mixin`, `@Shadow`, `@Accessor`, `@Invoker`, and `@Overwrite` completion with definition navigation.
4. MixinExtras annotations at the same quality level as core Mixin (including expression validation).
5. Access Widener mapping-aware completion.
6. Access Transformer readable display with SRG/namespace insertion.
7. Diagnostics for unresolved targets, ambiguous methods, ordinals, duplicates, and handler mismatches.
8. Code actions updating Java, AW, AT, and Mixin config JSON through workspace edits.
9. Neovim contains no semantic logic beyond transport and UI adaptation.

## Still To Implement

- Optional live `gradlew genSources` Loom E2E (`MCDEV_LOOM_RUN_GEN_SOURCES=1`) against real downloaded Minecraft sources.

## Verification Commands

```powershell
gradle test
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-layout.ps1
nvim --headless -u mcdev-nvim/tests/minimal_init.lua -c "luafile mcdev-nvim/tests/run.lua" -c qa
gradle :mcdev-jdtls-extension:checkBundle
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e-matrix.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-loom-e2e.ps1
```
