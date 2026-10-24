# Remaining command and resource integration

This is a requirements record, not an implemented or advertised wire API.
Stock host `f76a43fbeddf4fbdc69768fd0f7544e5a4a3610f` provides the verified
semantic-v1 source Shadow slice. The proposed projectContext/typeJvm operations
are a separate, unpublished two-operation slice. Neither surface provides the
eleven commands below.

The Rust implementation already contains the exact command decoder, response
DTOs, dispatcher, domain services and transport library. All 127 original
command cases execute natively. Real integration still needs command routing,
an implementation of the core-facing `CommandHost`, and authoritative generic
host facts. A mock CommandHost returning captured domain results does not prove
that composition or its live connection.

Primary implementation references:

- [Command IDs and DTOs](../mcdev-protocol/src/dto.rs),
  [decoder](../mcdev-protocol/src/decoder.rs) and
  [protocol conversion](../mcdev-protocol/src/convert.rs).
- [Dispatcher](../mcdev-companion/src/commands/runtime.rs) and
  [required host boundary](../mcdev-companion/src/commands/host.rs).
- [Original command evidence](../mcdev-companion/tests/fixtures/commands/README.md).
- [Current host fact subset](mcdev-host-capabilities.md) and
  [concrete original editor observations](neovim-host-contract.md).

## Existing request and result contract

Every command carries `McdevRequestContext`: protocolVersion, workspaceRoot,
documentUri, languageId, position, bufferText, optional documentVersion,
bufferTextFallbackUsed and client. Positions/ranges use UTF-16. Completion also
carries trigger and the preferredAtTarget/mixinClassInsert/injectMethodDescriptor
options. Code actions also carry range and diagnosticCodes. The existing decoder
owns accepted legacy payload forms; integration must preserve their tested
meaning rather than silently discarding fields.

Results retain the existing protocolVersion/serverVersion/capabilities and
result-or-error envelope. Full DTOs, concrete variants, metadata, callbacks and
cache sequences have exact original coverage. Definition serialization must use
the appropriate existing typed encoder; arbitrary metadata strings must not be
lowercased. Cancellation remains cancellation. Unavailable facts preserve typed
INCOMPLETE_PROJECT_CONTEXT, MISSING_MAPPING_DATA or BYTECODE_UNAVAILABLE where
applicable, rather than successful empty lists or generic internal failures.

| Existing command | Core behavior and result | Generic host inputs still required |
|---|---|---|
| `mcdev.completion` | Mixin/Extras or AW/AT services; full items, edits, snippets, additional edits, warnings, incomplete state and debug metadata | Current normalized source/annotation context, selected project/resources, exact JVM catalog/ABI, SDK facts and executable bytecode where the feature requires it. Pure no-index helper eligibility remains separate. |
| `mcdev.definition` | MC selector/target resolution followed by location conversion | Exact selected type/member identity and source location; bytecode-only and unresolved remain explicit. Existing standard definition/file URIs should be reused when available. |
| `mcdev.references` | Resolve target, obtain Java/Kotlin and AW/AT source entries, then run both reference services with deduplication | Host-selected complete source/resource inventory, current unsaved overlays, location/version facts and invalidation of negative inventory reads. |
| `mcdev.hover` | MC target/project information rendered into contents and optional range | The same authoritative target/project inputs; standard hover reuse remains the client's first path where configured. |
| `mcdev.codeAction` | Domain fixes and concrete multi-document edits, including selected Mixin config materialization | Current source/diagnostics, selected resource contents and edit preconditions; file selection remains scoped to the host project. |
| `mcdev.diagnostics` | Mixin/Extras and AW/AT diagnostics with exact codes, ranges, severities and metadata | Current source/semantic context, mapping/resource facts, class/member authority, SDK and Code/LVT as needed. |
| `mcdev.context` | Compatibility alias of diagnostics | Identical validation, inputs and result semantics to diagnostics. |
| `mcdev.info` | Build identity, supported commands and project summary | Real executable/build identity plus selected project and MC resource-derived context. Unsupported commands must not be advertised as connected. |
| `mcdev.reindex` | Scoped reindex followed by status, index state and classpath count | An actual host-owned refresh of the selected workspace/model and corresponding MC cache invalidation, without child filesystem indexing. |
| `mcdev.reloadProjectContext` | Invalidate MC project context, refresh and return the new context summary | The same scoped refresh plus fresh model/resource selection; no other workspace or provider instance may be cleared. |
| `mcdev.dumpContext` | Project ID/root/platform, mappings, classpath, source sets, Mixin configs, AW/AT resources, Minecraft inputs and index state | Owned selected model metadata and resource snapshots with completeness. Partial two-operation metadata cannot fabricate the full response. |

## Source, resource and binary authority

Minecraft platform detection, mapping parsing, config selection and all MC rules
stay in Rust core. The host supplies scoped owned inputs and validates reads.
The extension must not rediscover workspaces, walk a classpath/JDK, or read a
display path as a filesystem grant.

The existing core consumers need these observations, regardless of the eventual
operation names chosen by the host:

- Selected source sets and their source/resource/output associations, ordered
  classpath entries and SDK selection. Partial coverage stays partial.
- Scoped resource inventory/content for the existing platform/config/mapping
  parsers: Mixin JSON, Fabric/Forge/NeoForge metadata, AW/AT files, mapping files
  and the existing build-property inputs. Relative paths are resolved in the
  selected scope; classpath resources use selected entries, not arbitrary paths.
- Unsaved Java, Kotlin, AW, AT and JSON contents with document identity/version.
  Resource additions, removal, replacements and previously absent lookups must
  invalidate their dependent results. Saved text cannot hide a newer overlay.
- Exact JVM names/descriptors/static facts and complete member inventory for
  complete-only core indexes. Catalog truncation cannot prove absence. Raw
  generated/bridge members remain separate from source-callable editing members;
  any identity used for shared LSP binding must actually be issued by the host.
- Authoritative class bytes on demand for the existing Rust reader, including
  Code/LVT and annotation payloads, with freshness against source/output changes.
  Complete header metadata alone does not provide executable bytecode.
- SDK-inclusive hierarchy/reference assignability, including arrays, interfaces
  and boxed classes. Unknown differs from known false; no small JDK fallback
  table or source-name-to-binary-name guess can establish authority.

Existing normalized core providers, source-first indexes, semantic models and
bytecode services should be composed directly. A missing capability must be
rejected before entering an Option/Vec API that assumes complete data.

## Request lifetime and editor acceptance

Routing must use the enabled provider instance and the host's selected workspace
and owner. Client workspaceRoot/documentUri are context hints to validate, not
authority to access another project. The command's source snapshot must match
the requested unsaved version/text or produce a typed stale/unavailable result.
No workspace/read-session borrow remains held across child I/O.

The existing host read transaction and final validation remain authoritative for
positive and negative reads. Disable, restart, configuration, source, resource,
model, classpath and SDK changes must reject or withdraw obsolete work. Opaque
handles remain evaluation scoped. Missing generation dimensions bypass child
caches; they must not be filled with zero or inferred from paths. Any future
operation negotiation/routing must be bounded and explicit, not an arbitrary
LSP/executeCommand proxy.

Editor proof must exercise the actual advertised command and current provider,
then compare the complete response and applied buffers. Completion proof includes
real partial-name/snippet insertion and additional edits; the existing reduced
Shadow smoke only checks identity edits at fully typed names. Code action proof
includes config edits and version checks on every affected document. References
and diagnostics must update after unsaved edits, deletion and re-enable/restart.
All original MC E2E assertions remain pending until these live paths execute.

The next useful integration boundary after the two-operation slice is scoped
owned source/resource snapshots plus explicit provider command routing. A small
first command may exercise validation, cancellation and typed unavailability,
but that alone does not complete any project-aware MC feature or the whole
eleven-command surface. The host's implemented serializer and fixed commit will
define the actual protocol.
