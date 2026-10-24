# Neovim migration: concrete host observations

This records outstanding real-host observations, not completed E2E results.
The immutable original test bodies and assertion IDs are inventoried in
`mcdev-nvim/tests/migration_inventory.json` (27 files, 1,014 assertion call
sites). Client unit and native helper/process checks have separate evidence.

## Existing ktlsp surface checked at f595

`crates/ktlsp/src/lsp.rs:2955` advertises `quickfix`, `refactor.rewrite`,
`source.organizeImports`, `source.fixAll`, and `source.fixAll.ktlsp`, with
`codeAction/resolve` disabled. The handler at 3543 produces materialized edits;
conversion at 4551 sets `edit`, with no `command` or resolve `data`.

`execute_command` at 3790 accepts `ktlsp.compile`, `ktlsp.reindex`,
`ktlsp.tracePath`, `ktlsp.explainResolution`, `ktlsp.explainCompletion`, and
`ktlsp.dumpSymbol`. It does not expose JDT refactoring commands. The URI helpers
at 2638/2643 accept/produce file URIs; main registers a normal `LspService`, with
no separate virtual-document content method. Selected dependency sources are
extracted to real files, so an existing source definition needs no custom content
request. A bytecode-only symbol must not masquerade as available source text.

## Original observation and native counterpart

| Original test / method | Exact input and observable | Existing native counterpart / remaining gap |
|---|---|---|
| `create_drill_definition_diagnostics_e2e.lua`, `mcdev.definition` then `java/classFileContents` | Position in the `useItemOn` selector in `DrillBlockMixin`; metadata owner `com/simibubi/create/content/kinetics/drill/DrillBlock`, full descriptor in `expected_descriptor`; original resolution `jdt`, `jdt://contents/create-1.21.1-6.0.10-280.jar/...DrillBlock.java`; text must contain `useItemOn` on the returned range line. | Standard `textDocument/definition` returns file locations for selected source files. MC selector-to-member resolution remains in MC core and needs the pending command/host query bridge. Once that returns the selected source file URI, read that file and assert the same owner/descriptor, declaration text and range. No JDT content method is needed. Missing selected source/resource remains unavailable. |
| `jdt_action_aliases_e2e.lua`, full-document `textDocument/codeAction` | In-memory Java `class DrillBlockMixinActionAlias { private int value; private int sum(int left, int right) { return left + right; } }` (with original package). Original raw response must contain at least one equal-title `quickassist`/`source.generate.constructors`, `.toString`, `source.overrideMethods` or `source.sortMembers` pair; merged picker shows each title once without resolving. No constructor/toString/sort edit is executed by this test. | Native materialized actions have no lazy alias/resolve data. Client unit tests retain all four original alias inputs and exact lazy/dedup assertions, including distinct effects and clients. Real E2E should observe a native materialized action/effect and its unique merged entry; the host should not fabricate JDT aliases merely to duplicate obsolete wire shapes. The original plain fixture offers none of the available import/interface actions; a native fixture adaptation must be explicit. |
| Same file, `textDocument/codeAction` followed by `java.action.applyRefactoringCommand` | Selection is exactly the ASCII `left + right` in `return left + right;`; original action title contains `Extract` and `local variable`. After application: a local assignment `= left + right;`, `return <identifier>;`, and absence of `return left + right;`. | Desired generic operation is a standard code action returning a `WorkspaceEdit` for Java extract-local. f595 has no Java extract-local implementation/advertised `refactor.extract` kind. This is a genuine host editor-surface gap, not an MC rule; do not recreate `java.action.applyRefactoringCommand` or a client prompt API. |
| `lsp_regressions.lua`, JDT alias unit case | Four known alias families with differing resolve `data`; same-client equal effects collapse, different edits/command arguments/disabled state remain distinct; two clients remain separately routable. | Already verified in the native client against all original fake inputs. The alias comparison is generic and client-identity-bound, with no JDT dependency. These inputs stay as compatibility test fixtures, not advertised product commands. |

The approved native real-action fixture adds `java.util.Set` followed by
`java.util.List` imports to the same in-memory class, making ktlsp's existing
organize-imports materialized action observable. Its exact effect must appear
once in the merged picker and retain the originating client ID. This does not
claim a new alias pair from the host. The extract-local input remains exactly
the original `left + right` selection and all three post-edit assertions remain
mandatory; it now requires a real `WorkspaceEdit` rather than a JDT client
command. That real-host step is still pending implementation.

The host's existing interface-implementation action and import quickfix/organize
actions can be used for their actual supported workflows. They are not proof of
extract-local or of source-less decompilation. Kotlin function-body rewriting is
also not the Java extract-local operation above.

## Other explicit backend conversions

- `parseSource = JDT_AST` becomes the existing `HOST_NORMALIZED` provenance.
  `usedCompilationUnit = true` becomes `parseConfidence = HIGH`, and
  `usedJavaProject = true` becomes a nonzero `projectSessionVersion`, alongside
  existing binding/semantic-context/target assertions. This checks authoritative
  syntax and project snapshots without claiming JDT object use.
- `info.jarLocation` is a legacy v1 JSON slot populated from the native runtime's
  artifact location. UI labels say executable and accept
  `info.executableLocation or info.jarLocation`; no new schema is required.
- Extension-JAR/Mason discovery and JVM `-data` isolation are obsolete. Their
  client configuration tests now observe explicit executable argv, idempotent
  provider registration, preserved manual settings, distinct normalized roots,
  and independently owned provider tables.
- `java.reloadBundles` becomes the existing MC project-context reload workflow;
  native executables are rebuilt/restarted explicitly. No hot JAR loader is used.
- The Java `TeardownProcess` fixture is replaced by the std-only Rust example
  `mcdev-companion/examples/teardown_process.rs`; its normal/hung, inherited-pipe,
  owned-descendant, unrelated-process and snapshot-failure observations execute
  as real processes. No Java helper runs in the native client test suite.
