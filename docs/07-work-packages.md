# Work Packages

This document describes work packages, not a staged product plan.

The product target is the complete design described in this dossier. Work packages only divide implementation so agents can make coherent changes without drifting.

## WP-01 Repository And Build Foundation

Deliverables:

- Gradle Kotlin DSL root project
- `mcdev-core`
- `mcdev-protocol`
- `mcdev-jdtls-extension`
- `mcdev-nvim`
- `mcdev-test-fixtures`
- Java 21 toolchain
- Kotlin JVM target 21
- OSGi bundle build for extension
- packaging smoke test

Acceptance:

- root build runs
- extension jar has OSGi manifest
- Kotlin stdlib resolution strategy is explicit
- `mcdev-core` does not depend on JDT or LSP4J
- `mcdev-nvim` has no generated Kotlin artifacts committed

## WP-02 Protocol Foundation

Deliverables:

- protocol version model
- command names
- completion request/response DTOs
- code action request/response DTOs
- definition request/response DTOs
- info request/response DTOs
- structured error payloads

Acceptance:

- Neovim can construct protocol payloads without semantic parsing
- JDT LS extension can decode payloads
- protocol mismatch returns structured error
- completion response preserves label/detail/filterText/insertText/edit/metadata

## WP-03 Descriptor Core

Deliverables:

- descriptor parser
- descriptor renderer
- member target parser
- readable signature renderer
- exact insertion renderer
- parse error model

Acceptance:

- method/field descriptors parse into structured models
- invalid descriptors produce useful errors
- descriptor rendering is stable
- test suite covers primitive/object/array/method/constructor cases

## WP-04 Mapping Core

Deliverables:

- namespace model
- mapping resolver interface
- Tiny v2 parser
- SRG/TSRG parser as needed
- descriptor remapper
- missing mapping diagnostics

Acceptance:

- class/method/field remap works with namespace metadata
- descriptor remapping works
- missing SRG mapping is distinguishable from unresolved member
- tests cover named/intermediary/SRG paths

## WP-05 ProjectContext

Deliverables:

- project context model
- platform detector
- classpath snapshot model
- mapping discovery
- mixin config discovery
- AW/AT discovery
- `mcdev.info` command

Acceptance:

- `mcdev.info` can explain project state
- partial context is represented explicitly
- completion services can access project state without re-detecting platform

## WP-06 Bytecode Index

Deliverables:

- class provider
- ASM class reader integration
- class/member index
- instruction extraction
- ordinal calculator
- cache keys and invalidation model

Acceptance:

- completion does not scan jars on demand
- invoke/field/new/constant/return candidates are extracted from fixtures
- ordinals are stable
- stale classpath entries invalidate affected bytecode

## WP-07 Mixin Semantics

Deliverables:

- annotation model
- annotation context extraction
- target class resolver
- injector method resolver
- `@At` resolver
- shadow/accessor/invoker services
- diagnostics
- code actions

Acceptance:

- Mixin target completion works
- injector method completion works
- descriptor insertion rules work
- `@At(target)` bytecode completion works
- missing mixin config diagnostic and code action work

## WP-08 MixinExtras Semantics

Deliverables:

- MixinExtras annotation model
- `MIXINEXTRAS:EXPRESSION` support
- handler signature service
- generate/fix handler code actions
- diagnostics

Acceptance:

- `@WrapOperation` signature generation works
- `@ModifyExpressionValue` signature generation works
- `@ModifyReturnValue` signature generation works
- wrong handler signatures produce diagnostics
- MixinExtras completions use the same target model as Mixin

## WP-09 Access Widener

Deliverables:

- parser
- syntax slot detection
- completion service
- diagnostics
- definition/references support
- code actions

Acceptance:

- directive/kind/class/member/descriptor completion works
- namespace mismatch is detected
- duplicate entries are detected
- generated entries are stable and formatted

## WP-10 Access Transformer

Deliverables:

- parser
- syntax slot detection
- completion service
- SRG-aware insertion
- diagnostics
- definition/references support
- code actions

Acceptance:

- modifier/class/member completion works
- readable display and SRG insertion are separated
- missing descriptor diagnostic works
- SRG mapping not found diagnostic works
- generated entries are stable and formatted

## WP-11 JDT LS Integration

Deliverables:

- bundle activator
- command registration
- JDT project lookup
- AST parse service
- binding service
- descriptor conversion from bindings
- source range conversion
- LSP/command response conversion
- diagnostic publication

Acceptance:

- extension loads in JDT LS
- command calls return structured responses
- Java annotation contexts are identified from buffer position
- workspace edits point to correct files and ranges

## WP-12 Neovim Plugin

Deliverables:

- setup API
- extension jar config
- active JDT LS client detection
- command request wrapper
- Blink source
- cmp source
- omnifunc fallback
- `:McdevInfo`
- `:McdevReindex`

Acceptance:

- Neovim sends buffer text and cursor position
- Blink/cmp display labels and insert exact text edits
- Lua contains no semantic resolver logic
- errors are actionable

## WP-13 Fixtures And E2E

Deliverables:

- Fabric fixture
- Forge fixture
- NeoForge fixture
- MixinExtras fixture
- broken diagnostics fixture
- expected completion/code action snapshots

Acceptance:

- fixture tests cover goal-state examples
- e2e tests validate realistic project context
- fixture updates are intentional and reviewed

## WP-14 Documentation

Deliverables:

- user installation guide
- Lazy.nvim example
- nvim-jdtls example
- build-from-source guide
- troubleshooting guide
- contributor architecture guide

Acceptance:

- user can configure a prebuilt jar
- user can configure a local build artifact
- contributor can identify where semantic logic belongs
- common failures are documented

