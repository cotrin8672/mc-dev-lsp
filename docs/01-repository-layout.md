# Repository Layout

## Monorepo Decision

The project should start as a monorepo.

Reason:

- `mcdev-core`, the JDT LS extension, the Neovim wrapper, and fixtures are tightly coupled by protocol and release version.
- The Neovim plugin is not useful without a compatible JDT LS extension jar.
- The JDT LS extension needs fixtures that also exercise the Neovim-facing protocol.
- Splitting repositories early creates version skew before there is a stable protocol.

The repository is not a single artifact. It is a single source of truth for multiple artifacts.

## Target Layout

```text
mcdev-kotlin/
├─ settings.gradle.kts
├─ build.gradle.kts
├─ gradle/
├─ mcdev-core/
├─ mcdev-protocol/
├─ mcdev-jdtls-extension/
├─ mcdev-nvim/
├─ mcdev-test-fixtures/
├─ docs/
└─ scripts/
```

`mcdev-protocol` may initially be folded into `mcdev-core`, but it should be split once request/response DTOs become stable enough to be consumed by multiple modules.

## Module Responsibilities

### mcdev-core

Pure Kotlin/JVM logic.

Owns:

- JVM descriptor parsing and rendering
- mapping model and remapping services
- Tiny v2 parsing
- SRG parsing
- bytecode class reading through ASM
- bytecode instruction candidate extraction
- ordinal calculation
- Mixin semantic models
- MixinExtras signature service
- Access Widener parser/completion/diagnostics
- Access Transformer parser/completion/diagnostics
- completion item domain model
- diagnostic domain model
- code action domain model independent from LSP

Must not depend on:

- Eclipse JDT
- Eclipse Resources
- LSP4J
- Neovim
- file watching APIs specific to JDT LS

### mcdev-protocol

DTOs shared between Neovim-facing command payloads and the JDT LS extension.

Owns:

- request parameter models
- response models
- serialization-safe enums
- command names
- protocol version constants
- structured error payloads

Must remain small.

The purpose is not to duplicate LSP. The purpose is to define the project-specific command payloads that are sent through JDT LS.

### mcdev-jdtls-extension

OSGi bundle loaded by JDT LS.

Owns:

- bundle activator
- command handler registration
- JDT project model integration
- Eclipse resource lookup
- classpath lookup
- AST parsing
- binding resolution
- Java descriptor extraction from JDT bindings
- LSP conversion
- diagnostic publication
- workspace edit generation
- index lifecycle
- logging through Eclipse/JDT LS facilities

Must not:

- implement domain rules that belong in `mcdev-core`
- parse AW/AT differently from `mcdev-core`
- duplicate descriptor parsing
- duplicate mapping resolution rules

### mcdev-nvim

Lua Neovim plugin.

Owns:

- user configuration
- extension jar path configuration
- attaching the JDT LS bundle through `nvim-jdtls` config examples
- Blink/cmp/omnifunc source adapters
- commands such as `:McdevInfo`, `:McdevReindex`
- collecting unsaved buffer text and cursor position
- sending command payloads to JDT LS
- rendering user-facing status/errors

Must not:

- scan Java classpaths
- read Minecraft jars
- parse bytecode
- parse JVM descriptors except for trivial display fallback
- resolve mappings
- decide if a Mixin target is valid
- decide handler signatures

### mcdev-test-fixtures

Realistic test projects.

Owns:

- Fabric fixture
- Forge fixture
- NeoForge fixture
- MixinExtras fixture
- mapping fixture files
- compiled class fixtures
- intentionally broken examples for diagnostics
- expected output snapshots for completion/code actions

Fixtures should be small but realistic. They must include enough project structure to validate project detection, mapping context, classpath, and config file edits.

## Release Shape

The monorepo should use one version for all modules until a stable protocol exists.

Release tag:

```text
v0.x.y
```

Release artifacts:

```text
io.github.mcdev.jdtls-0.x.y.jar
mcdev-nvim source files from the same tag
```

Recommended user configuration:

```lua
require("mcdev").setup({
  jdtls = {
    extension_jar = "/path/to/io.github.mcdev.jdtls-0.x.y.jar",
  },
})
```

Build policy:

- the repository may provide Gradle tasks to build the extension jar
- Neovim plugin configuration decides whether to use an existing jar or invoke a build command
- semantic logic must not depend on how the jar was obtained

## Version Compatibility

The protocol version should be explicit.

Example:

```kotlin
object McdevProtocol {
    const val VERSION = 1
}
```

Every request should include:

```text
protocolVersion
clientName
clientVersion
```

Every response should include:

```text
protocolVersion
serverVersion
capabilities
```

If protocol versions are incompatible, the extension should return a structured error that Neovim can display.

## When To Split Repositories

Do not split repositories until one of these is true:

- the JDT LS extension is useful to non-Neovim editors independently
- `mcdev-core` is published as a library for external tools
- Neovim plugin release cadence diverges from extension release cadence
- clone/build size becomes a concrete user problem
- the command protocol is stable across multiple minor versions

Until then, splitting repositories increases coordination cost without improving product quality.

