# Testing Strategy

## Testing Goal

The test suite must prove that `mcdev-kotlin` behaves like a semantic Minecraft modding assistant, not a text completion plugin.

Tests must cover:

- pure Kotlin domain logic
- mapping-aware insertion
- descriptor parsing/rendering
- bytecode candidate extraction
- Mixin and MixinExtras semantics
- AW/AT parsing, completion, diagnostics, and code actions
- JDT LS integration behavior
- Neovim protocol adaptation
- OSGi packaging smoke checks

## Approximate Test Count

A sufficient mature suite should have roughly:

```text
Core unit tests:                 280-360
JDT LS integration tests:        120-180
Fixture/e2e semantic tests:       40-70
Neovim Lua adapter tests:         30-50
Packaging/protocol smoke tests:   10-20
Total:                           480-680
```

This is not a quota. It is a rough size for enough confidence given the surface area.

If the suite is much smaller than 300 tests at goal state, it is probably missing important cases. If it is much larger than 800 tests, check whether tests are duplicating implementation details rather than covering behavior.

## Test Layers

### Core Unit Tests

Fast tests for `mcdev-core`.

Target count:

```text
280-360
```

Breakdown:

```text
descriptor:        45-65
mapping:           45-70
bytecode:          45-65
mixin:             55-75
mixinextras:       35-55
access widener:    25-40
access transformer:25-40
domain fixes:      20-35
```

Core tests should not launch JDT LS or Neovim.

### JDT LS Integration Tests

Tests for `mcdev-jdtls-extension`.

Target count:

```text
120-180
```

Should cover:

- project detection
- classpath extraction
- AST annotation context extraction
- binding to descriptor conversion
- source range calculation
- completion conversion
- diagnostic publication conversion
- workspace edit generation
- command request/response behavior
- incomplete context handling

These tests can be slower than core tests but should still run in CI.

### Fixture/E2E Semantic Tests

Target count:

```text
40-70
```

Fixture projects:

```text
fabric-basic
fabric-mixinextras
forge-basic
neoforge-basic
broken-diagnostics
multi-source-set
```

Each fixture should validate realistic flows:

- completion item labels and insert text
- diagnostics
- code actions
- JSON/AW/AT workspace edits
- definition targets

### Neovim Lua Adapter Tests

Target count:

```text
30-50
```

Should cover:

- setup option normalization
- extension jar path handling
- active client lookup
- request payload creation
- Blink item conversion
- cmp item conversion
- error display behavior
- AW/AT buffer request behavior

These tests should use mocked LSP clients. They should not require a real JDT LS process.

### Packaging Smoke Tests

Target count:

```text
10-20
```

Should cover:

- extension jar contains OSGi manifest
- bundle symbolic name is correct
- Kotlin stdlib is included or otherwise resolvable
- `mcdev-core` classes are included/resolvable
- forbidden dependencies are absent, such as kotlin-reflect unless explicitly approved
- protocol version is reported
- bundle activator class exists

## Descriptor Tests

Must cover:

- primitive descriptors
- object descriptors
- array descriptors
- method descriptors
- constructors
- invalid missing semicolon
- invalid method return
- invalid field descriptor
- readable rendering
- exact target rendering
- owner/name/descriptor target parsing

Example cases:

```text
I                                  -> int
Ljava/lang/String;                 -> String
[I                                 -> int[]
(Ljava/lang/String;FFI)I           -> (String, float, float, int): int
Lowner/Class;draw(Ljava/lang/String;)V
```

## Mapping Tests

Must cover:

- class remap
- method remap with descriptor
- field remap with descriptor
- missing method mapping
- descriptor remapping
- Tiny v2 parsing
- SRG parsing
- namespace mismatch
- ambiguous mapping entry
- remap from named to intermediary
- remap from named to SRG

## Bytecode Tests

Must cover:

- method invocation extraction
- static invocation
- interface invocation
- special invocation
- constructor invocation
- field get/put extraction
- static field extraction
- object creation
- constants
- return ordinals
- field ordinals
- invoke ordinals
- method not found
- class bytes missing

Use small compiled fixture classes where possible. Avoid overusing handcrafted byte arrays.

## Mixin Tests

Must cover:

- `@Mixin(Class.class)` target completion
- `@Mixin(targets = "")` completion
- import edit generation
- FQN insertion mode
- unresolved target diagnostic
- missing mixin config diagnostic
- add to mixin config code action
- `@Inject(method = "")` completion
- overload descriptor insertion
- `always` descriptor mode
- ambiguous method diagnostic
- `@At(value = "")` completion
- `@At(target = "")` invoke completion
- field target completion
- ordinal out of range
- shadow field validation
- shadow method validation
- accessor getter/setter validation
- invoker validation

## MixinExtras Tests

Must cover:

- `@ModifyExpressionValue` method completion
- `@ModifyReturnValue` method completion
- `@WrapOperation` `@At` target completion
- `@WrapWithCondition` signature
- `@WrapMethod` signature
- `Operation<T>` parameter placement
- instance method receiver handling
- static target handling
- wrong return type diagnostic
- generated handler method code action
- fix handler signature code action
- `MIXINEXTRAS:EXPRESSION` completion

## Access Widener Tests

Must cover:

- directive completion
- kind completion
- class completion
- method completion
- field completion
- descriptor insertion
- invalid directive diagnostic
- unresolved class diagnostic
- unresolved member diagnostic
- invalid descriptor diagnostic
- duplicate entry diagnostic
- mutable on non-field diagnostic
- namespace mismatch diagnostic
- remap code action

## Access Transformer Tests

Must cover:

- modifier completion
- class completion
- method completion with SRG insertion
- field completion with SRG insertion
- descriptor insertion
- invalid modifier diagnostic
- unresolved class diagnostic
- missing descriptor diagnostic
- invalid descriptor diagnostic
- duplicate entry diagnostic
- SRG mapping not found diagnostic
- remap AT code action

## Code Action Tests

Every code action must have tests for:

- action title
- applicability condition
- generated edit
- stable formatting
- no duplicate edit when target already exists
- behavior with unsaved buffer text when applicable

Workspace edits must be snapshot-tested or structurally compared.

## Regression Policy

Every bug fix must add at least one regression test in the lowest practical layer.

Examples:

- descriptor parsing bug -> core descriptor test
- wrong AT insertion namespace -> core mapping or AT completion test
- missing Mixin config edit -> JDT LS integration or fixture test
- Blink item conversion bug -> Neovim adapter test

## CI Expectations

Recommended CI tasks:

```text
./gradlew test
./gradlew :mcdev-jdtls-extension:jar
./gradlew :mcdev-jdtls-extension:checkBundle
nvim --headless -u tests/minimal_init.lua -c "lua require('mcdev.tests').run()" -c qa
```

CI should not require downloading Minecraft dependencies during every test if fixtures can provide stable minimal jars/classes.

