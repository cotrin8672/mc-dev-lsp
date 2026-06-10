# Decisions And Risks

## Architectural Decisions

### Decision: Use A Monorepo

Status: accepted.

Rationale:

- protocol compatibility matters
- fixtures should validate all layers together
- release artifacts should share one tag
- early repository splitting creates version skew

Revisit when:

- `mcdev-core` becomes an externally consumed library
- the JDT LS extension is used by multiple editors independently
- Neovim plugin release cadence diverges
- repository size becomes a concrete user problem

### Decision: Kotlin/JVM For Semantic Logic

Status: accepted.

Rationale:

- JDT LS is JVM-based
- Eclipse/JDT APIs are Java APIs
- bytecode/mapping/descriptor logic benefits from strong types
- semantic behavior can be tested without Neovim

### Decision: Lua Is UI And Transport Only

Status: accepted.

Rationale:

- Neovim should stay lightweight
- mapping and bytecode logic in Lua would duplicate Kotlin behavior
- tests for semantic rules are easier on JVM

### Decision: Commands First For Neovim <-> JDT LS Extension

Status: accepted as default.

Rationale:

- external bundles can be loaded by JDT LS
- arbitrary custom LSP method registration should not be assumed
- command transport is practical and debuggable

Revisit when:

- a stable JDT LS extension point is confirmed for standard completion contribution
- the project can safely merge results into native `textDocument/completion`

### Decision: Completion Uses Display/Insertion Separation

Status: accepted and non-negotiable.

Rationale:

- modding names are namespace-dependent
- display should be readable
- insertion must be exact
- IDEA-like experience depends on this separation

## Major Risks

### Risk: OSGi Classloader Failures

Symptoms:

- extension fails to load
- Kotlin runtime class missing
- ASM version conflict
- service registration fails silently

Mitigation:

- packaging smoke tests
- explicit Kotlin stdlib embedding
- avoid kotlin-reflect
- keep dependencies private
- document bundle manifest

### Risk: JDT LS Internal API Drift

Symptoms:

- extension breaks after JDT LS update
- command registration changes
- AST/binding service behavior changes

Mitigation:

- isolate JDT LS integration behind service classes
- keep core independent
- test against supported JDT LS versions
- document minimum and maximum tested versions

### Risk: Mapping Context Is Too Weak

Symptoms:

- wrong namespace inserted
- AT uses named instead of SRG
- AW namespace mismatch ignored
- diagnostics contradict completion

Mitigation:

- centralize `ProjectMappingContext`
- never pass namespace-sensitive raw strings
- test Fabric/Forge/NeoForge separately

### Risk: Completion Becomes Slow

Symptoms:

- typing stalls
- completion triggers jar scans
- background indexing blocks UI

Mitigation:

- immutable project indexes
- no full scan in completion
- cache bytecode instruction extraction
- expose index state through `:McdevInfo`

### Risk: Code Actions Edit Wrong Files

Symptoms:

- mixin config entry added to wrong JSON
- AW/AT entry generated in wrong source set
- duplicate entries introduced

Mitigation:

- index config files with ranges
- include source set in context
- structural workspace edit tests
- require deterministic target selection or ask user through UI when ambiguous

### Risk: MixinExtras Signature Rules Are Under-modeled

Symptoms:

- generated handler compiles incorrectly
- `Operation<T>` type is wrong
- instance/static receiver handling wrong

Mitigation:

- dedicated `HandlerSignatureService`
- fixture tests with compile checks
- model handler generation from bytecode target and annotation type

### Risk: Lua Adapter Grows Semantic Logic

Symptoms:

- Blink and cmp behave differently
- Lua parses descriptors
- Lua remaps names
- Kotlin and Lua disagree

Mitigation:

- single request wrapper
- thin source adapters
- review checklist
- Lua tests focused on payload and UI conversion only

## Rules For Changing This Design

Design changes are allowed when:

- a real implementation constraint is discovered
- a JDT LS integration point requires adjustment
- tests show the model cannot represent a valid modding case
- performance measurements prove a different indexing strategy is needed

Design changes should not be made because:

- a quick Lua parser is easier
- a string replacement works for one fixture
- one platform is easier if namespaces are ignored
- a feature is easier if code actions edit only the current file

Any major change should update:

- relevant design document
- tests
- agent onboarding checklist if boundaries change

