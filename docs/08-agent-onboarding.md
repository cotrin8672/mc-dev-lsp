# Agent Onboarding

This document is for future agents taking over implementation work.

## First Read

Read these files in order:

1. `README.md`
2. `00-product-goals.md`
3. `02-architecture.md`
4. `03-protocol-and-neovim.md`
5. the work package relevant to the task in `07-work-packages.md`

Do not start by writing Lua completion logic. Most features belong in Kotlin.

## Core Mission

Build a Minecraft modding semantic extension for JDT LS with a Neovim frontend.

The implementation must support:

- Mixin
- MixinExtras
- Access Widener
- Access Transformer
- Mixin config JSON quick fixes

The implementation must not drift into:

- NBT tools
- project creator
- GUI wizard
- debugger tooling
- generic Java IDE features already handled by JDT LS

## Architecture Boundaries

Before changing code, identify which layer owns the work.

### If the task involves descriptors

Use `mcdev-core`.

Do not implement descriptor parsing in:

- JDT LS extension
- Lua
- tests as ad hoc string splitting

### If the task involves mappings

Use `mcdev-core.mapping` and `ProjectMappingContext`.

Do not infer namespace from string shape alone.

### If the task involves bytecode

Use `mcdev-core.bytecode`.

Do not read jars during completion.

### If the task involves Java AST or bindings

Use `mcdev-jdtls-extension`.

JDT-specific types must not enter `mcdev-core`.

### If the task involves UI completion display

Use `mcdev-nvim` only as an adapter.

Do not put semantic decisions in Lua.

## Required Question For Every Change

Ask:

```text
Is this a semantic rule, an integration rule, or a UI transport rule?
```

Then place it accordingly:

```text
semantic rule     -> mcdev-core
integration rule  -> mcdev-jdtls-extension
UI transport rule -> mcdev-nvim
```

## Completion Item Rule

Do not collapse label and insert text.

Every completion feature must keep:

```text
label       human-readable
detail      owner/signature/context
filterText  searchable words
insertText  exact inserted value
metadata    structured target identity
```

If the implementation cannot explain both display and insertion namespaces, it is incomplete.

## Code Action Rule

Do not generate workspace edits from diagnostic message strings.

Diagnostics must carry structured metadata. Code actions should use metadata, target refs, and project context.

## Mapping Rule

Every member target must know:

```text
owner
name
descriptor
namespace
kind
```

Raw strings are allowed only at parsing boundaries and rendering boundaries.

## Performance Rule

Completion is a read path.

Allowed:

- read current AST context
- read latest ProjectContext
- read indexes
- filter candidates
- render results

Forbidden:

- Gradle invocation
- dependency download
- full jar scan
- full project reindex
- blocking file-system crawl

## Test Rule

Any bug fix requires a regression test.

Choose the lowest practical layer:

- parser bug -> core unit test
- mapping bug -> core mapping test
- wrong JDT range -> JDT LS integration test
- wrong Blink item -> Lua adapter test
- wrong fixture behavior -> fixture/e2e test

## Common Drift Patterns

Avoid these:

- adding a Lua parser for AW/AT because it is quick
- adding string replacements for descriptors instead of using descriptor models
- treating Forge and Fabric namespaces as the same
- assuming all method names are named namespace
- assuming all target classes are compiled
- hiding missing mappings as empty completion results
- generating edits from formatted diagnostic text
- making completion build or refresh the whole index

## Definition Of Done For A Feature

A feature is not done until:

- core model exists
- completion/diagnostic/code action behavior is tested
- mapping namespace behavior is explicit
- JDT integration has range and conversion coverage where applicable
- Neovim adapter is thin
- `:McdevInfo` can explain missing context if the feature depends on project state

## Practical Start Checklist

When starting a task:

1. Locate the owning module.
2. Read the relevant design document.
3. Add or inspect tests first.
4. Implement in the lowest correct layer.
5. Add conversion code in JDT LS only if needed.
6. Add Lua adapter code only if UI transport changes.
7. Run focused tests.
8. Run packaging smoke tests if the extension bundle changed.

## Review Checklist

For every PR or patch, check:

- no semantic logic was added to Lua
- `mcdev-core` does not import JDT or LSP4J
- completion display and insertion are separate
- namespace is explicit
- no completion path performs expensive indexing
- diagnostics have structured codes
- code actions do not parse human-readable messages
- tests cover behavior, not only implementation details

