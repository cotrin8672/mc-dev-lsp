# Implementation Status

This file records what exists in the repository after the testing and adapter expansion pass.

## Implemented

### Build and modules

- Gradle Kotlin DSL root project and module build files.
- Java 21 toolchain and Kotlin JVM target 21 configuration.
- `mcdev-core`, `mcdev-protocol`, `mcdev-jdtls-extension`, `mcdev-nvim`, and `mcdev-test-fixtures` modules.

### mcdev-core domain logic

- Descriptor model, parser, renderer, and member target parser.
- Tiny v2 parser, SRG parser, namespace-aware lookup model, `MappingResolver`, project mapping context model, and descriptor remapping service.
- Access Widener and Access Transformer parsers, context extractors, completion, diagnostics, code actions, and unified `AwAtServiceFacade`.
- Structured diagnostic and completion domain models.
- Bytecode indexing, instruction extraction, ordinal calculation, and classpath-backed mixin indexes.
- Mixin target/method/@At completion, diagnostics, and code actions (config entry, descriptor, accessor, invoker).
- MixinExtras completion, diagnostics, and handler signature code actions.
- Project context construction helpers, platform detection, mapping/AW/AT/mixin config discovery.
- Mixin import edit builder for deterministic additional import text edits.
- Mapping-aware `@At` target insert formatting when a `MappingResolver` is available.
- `@Constant` string/int hint generation for CONSTANT `@At` completion items.
- Duplicate `@Mixin` target diagnostics (class array, string targets, and mixin config entries).

### mcdev-protocol

- Command names, protocol versioning, request context, completion/navigation/code action/info DTOs, and structured error payloads.

### mcdev-jdtls-extension

- OSGi bundle manifest configuration, bundle activator, service holder, and command dispatcher.
- `plugin.xml` + `McdevDelegateCommandHandler` registered through `org.eclipse.jdt.ls.core.delegateCommandHandler`.
- Fat bundle jar embedding `mcdev-core`, `mcdev-protocol`, Gson, and Kotlin stdlib.
- File-based project context loading, classpath indexing, completion/diagnostics/info/reindex handlers.
- AW/AT buffer routing in completion, diagnostics, and code action handlers via `AwAtServiceFacade` (languageId or `.accesswidener`/`.aw`/`_at.cfg` extension).
- Protocol payload decoding and completion/diagnostic DTO conversion (including text edits for mixin class completion).
- Local OSGi bundle E2E via `scripts/run-osgi-e2e.ps1` (real Mason `jdtls` + headless Neovim).
- End-to-end completion wiring: `preferredAtTarget` and mapping-aware `@At` insert formatting via `AtTargetInsertFormatter`, mixin import `additionalEdits` via `MixinImportEditBuilder`, and MixinExtras completions through the core `MixinServiceFacade`.

### mcdev-nvim

- Setup API, extension jar accessor, active JDT LS client detection, request wrapper.
- Completion payload construction matching `McdevCompletionRequest` shape.
- Blink and nvim-cmp adapter shims preserving label vs insertText.
- Commands for `:McdevInfo` and `:McdevReindex`.
- Headless Lua adapter tests with mocked JDT LS client absence handling.

### mcdev-test-fixtures

- Fixture projects: `fabric-basic`, `fabric-mixinextras`, `fabric-aw-at`, `forge-basic`, `neoforge-basic`, `broken-diagnostics`.
- Compiled `SimpleTarget` class bytes and fabric-basic Tiny v2 mappings.
- Fixture resource loader and mixin config validator tests.

### Test coverage (approximate)

| Module | Tests |
|--------|------:|
| mcdev-core | ~465 |
| mcdev-jdtls-extension | ~90 |
| mcdev-test-fixtures | ~14 |
| mcdev-protocol | 2 |
| mcdev-nvim (Lua adapter) | ~15 assertions |

Gradle `test` total: **~570** tests across Kotlin/Java modules.

Core tests cover descriptor/mapping/bytecode edge cases, mixin and MixinExtras semantics, AW/AT completion/diagnostics E2E flows, project context, fixture-backed mapping regression, and mixin E2E flows. JDT LS handler tests cover AW/AT buffer payloads by languageId and file extension. Neovim adapter tests cover protocol payload shape, LSP item conversion, Blink/cmp wrapping, command registration, and missing-client errors.

## Still To Implement

- `@Overwrite` annotation support (currently unsupported; covered by tests).
- AW/AT buffer request behavior in Neovim adapter.
- Full expression-context validation for `MIXINEXTRAS:EXPRESSION`.
- Multi-source-set fixture project.
- Packaging smoke tests (`checkBundle` CI task exists but is not part of default `gradle test`).

## Verification Commands

```powershell
gradle test
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-layout.ps1
nvim --headless -u mcdev-nvim/tests/minimal_init.lua -c "luafile mcdev-nvim/tests/run.lua" -c qa
gradle :mcdev-jdtls-extension:checkBundle
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-osgi-e2e.ps1
```
