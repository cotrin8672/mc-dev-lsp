# Core command composition audit

This is the design and implementation note for the Rust command boundary. It
adds only the concrete library host and focused whole-command proof; it does
not add a command wire, a product type, a DTO, or a second parser. The command
corpus and the facade corpus prove different things:

- `mcdev-companion/tests/fixtures/commands` proves all 127 legacy callers
  through `CommandDispatcher` with `RecordedHost` callback facts.
- `mcdev-core/tests/fixtures/mixin_facade` proves the real Rust facade and
  host-adapter composition with complete owned fixtures.
- The focused `core_commands.rs` proof now connects a concrete
  `CoreCommandHost` to the dispatcher and real MC core, but it is a generic
  fixture-host proof. It does not claim that the stock semantic-v1 child has
  eleven-command routing. `main.rs` still exposes only semantic-v1 or
  `--stdio-completion`.

The relevant public boundary is
[`CommandHost`](../mcdev-companion/src/commands/host.rs#L67-L162); the
dispatcher is already the canonical command composition in
[`runtime.rs`](../mcdev-companion/src/commands/runtime.rs).

## Conclusion

The public Rust library is sufficient to compose the minimum real
`CoreCommandHost` once a caller supplies the existing owned fact/provider
inputs. The implementation is in
[`core.rs`](../mcdev-companion/src/commands/core.rs), and its constructor keeps
the caller-owned `Arc<HostProjectSession>`, `CachedProjectSession`, semantic
provider, mixin source provider, source inventory and `BuildIdentity` for the
dispatcher lifetime. It is not sufficient to run the complete eleven-command
surface against the current published host. The current `projectContext`/
`typeJvm` slice is missing the identity, location, source-inventory, resource,
hierarchy/assignability, executable bytecode and refresh-control authority
required below.

Therefore the smallest honest production slice is:

1. buffer-only completion, which remains host-independent;
2. `CoreCommandHost` over complete caller-owned generic facts, with real core
   calls reaching final command DTO conversion;
3. live project-aware commands only after the host publishes the missing
   providers and control operations.

`reindex` and `reloadProjectContext` deliberately return typed
`IncompleteProjectContext`/Unavailable from this host: no refresh authority is
invented. Feature callbacks for specifiers and additional At values likewise
remain typed unavailable when requested.

Do not turn a partial `ProjectContext`/`typeJvm` answer into a complete
`HostProjectSession`, and do not encode `Unknown` as an empty list, `false`,
`Missing`, an old source, or stale class bytes.

## Eleven-command call graph

The arrows below name the existing dispatcher call, then the smallest Rust
core operation it can delegate to. `Host` means an authoritative owned fact or
host mutation is still required; it does not mean “read a path supplied by the
client”.

| Command | Dispatcher → `CommandHost` → Rust core / conversion | Required authority or mutation |
|---|---|---|
| `mcdev.completion` | Buffer-only: `buffer_only_completion` → `HostMixinFacade::try_complete_buffer_only` → `MixinServiceFacade::complete_with_debug` with `BUFFER_ONLY_INDEX` → `CompletionItemConverter`. Project-aware Java/Kotlin: `load_cached_session` → `semantic_model` → `complete` → `HostMixinServiceFacade::complete_with_debug` → `MixinServiceFacade`/At-value service → converter. AW/AT: `awat_complete` → existing AW/AT core conversion. | Current source/overlay, semantic provider, class/bytecode index, selected config/source callbacks, complete project identity, versioned caches. `has_project_facts` is a gate, not a semantic result. |
| `mcdev.diagnostics` | `load_session` → `diagnostics` → `HostMixinServiceFacade::diagnose` → `MixinServiceFacade::diagnose` and the AW/AT service → `DiagnosticConverter`. | Current source, semantic model, mappings/resources, exact class/member facts and bytecode where a diagnostic needs them. |
| `mcdev.context` | Same path as diagnostics; `DecodedCommand::Context` is an alias and returns the diagnostics envelope. | Exactly the diagnostics authority; it is not a cheaper context probe. |
| `mcdev.definition` | `load_session` → `definitions` → `HostMixinServiceFacade::definitions` → `MixinDefinitionService::definitions_at`; then `resolve_definitions` → `DefinitionResolutionService` backends (`HostProjectSession` and/or `SourceDefinitionBackend`/`SourceIndex`) → `DefinitionConverter`. | Target identity plus a current source declaration/location or a host `definition` fact. Unresolved/unknown locations must remain typed unresolved/unavailable, not fabricated URI/range data. |
| `mcdev.references` | `load_session` → `definitions`; if targets exist, `source_entries` for Java/Kotlin and AW/AT → `references` → `MixinReferenceService::find_references` → URI/range conversion and dispatcher deduplication. | Complete, current source inventory including unsaved overlays and negative inventory reads. The current public providers do not enumerate/read all Java/Kotlin source entries. |
| `mcdev.hover` | `load_session` → `definitions` → dispatcher `format_hover` → `McdevHoverResponse`. | Same target/project facts as definition. No second fake hover model is needed. |
| `mcdev.codeAction` | Kotlin is an explicit known-empty fast path. Otherwise `load_session` → `selected_mixin_config_content` → `HostMixinFacade::selected_mixin_config` plus a separate provider content read; `code_actions` → `HostMixinServiceFacade::code_actions` → core fix services; `resolve_fix` for fixes needing source/config materialization → `CodeActionConverter`. | Current source/diagnostics, selected config content, file versions and safe edit preconditions. Host owns actual source/resource reads and any editor-side mutation; Rust owns fix computation/edit shape. |
| `mcdev.info` | `load_session` + `build_identity` → `InfoService::format_lines` and `McdevInfoResponse`. | Owned project snapshot and real build identity. Advertise only commands actually connected. |
| `mcdev.dumpContext` | `load_session` → dispatcher `dump` → `InfoService` plus the existing project DTO projection. | Complete selected project/resource snapshot. Partial metadata cannot fill classpath, source sets, configs, AW/AT or index state. |
| `mcdev.reindex` | `reindex` → `InfoService::index_state_name` and classpath count → response. | A host-owned project/model refresh and corresponding MC/session invalidation. `HostProjectSession::reindex` only rebuilds Rust indexes from the same identity; it is not this host refresh. |
| `mcdev.reloadProjectContext` | `clear_project_cache` → `reindex` → `InfoService::reload_value`/response. | Scoped refresh of the selected workspace and a fresh identity. No existing generic `CommandHost` control API supplies this. |

`load_session` is intentionally shown on each project-aware path even when the
core call is pure: it is the lifetime and freshness gate for the borrowed
`ProjectContext`, class index and bytecode index.

## `CommandHost` method ownership

These are all twenty methods in the trait. The primary owner is shown first;
cross-cutting host inputs are listed after the arrow.

| Method | Primary classification | What the implementation must do |
|---|---|---|
| `now_millis` | cache/time | Return one monotonic-enough host clock value for document and negative-completion TTLs. Do not derive it from a request or source timestamp. |
| `build_identity` | build identity | Return an owned commit/build-time/version/artifact tuple. No guessed package metadata. |
| `has_project_facts` | generic host gate | Return true only when the selected project has the facts needed by the requested project-aware path. `Unknown` is false/error at the gate, never “no project”. |
| `load_session` | generic host fact + cache lifetime | Obtain a current `Arc<ProjectContext>` and validate the selected root/identity. It must not retain host handles in the context. |
| `load_cached_session` | generic host fact + cache | Return `CachedProjectSession` with a real version/fingerprint and a real cache-hit bit. It must be compatible with `HostProjectSessions::load`. |
| `reindex` | generic host control | Ask the host to refresh project facts/model/resources, then construct a new session. The core has no generic refresh callback. |
| `clear_project_cache` | generic host control + cache lifetime | Invalidate only the selected command-host project/session caches and close old `HostProjectSession` adapters. |
| `semantic_model` | MC-side compute after generic facts | Call `HostMixinServiceFacade::semantic_model` with a `HostSemanticModelProvider` and `SemanticModelCache`; stable fallback is explicit, transient fallback is not cacheable. |
| `buffer_only_completion` | MC-side compute | Call `HostMixinFacade::try_complete_buffer_only`; this path may use no project/session/source provider. Feature callbacks are only evaluated by the At-value service when requested. |
| `complete` | MC-side compute after generic facts | Build a short-lived `HostMixinServiceFacade::new` with private checked `ClassIndex`/`BytecodeIndex` adapters backed by the caller-owned session, attach the matching source/provider callbacks and shared expression cache, then call `complete_with_debug`; pass through `completion_from_core`. |
| `candidate_cache_metrics` | cache | Read the existing core candidate-cache debug state. It is observability, not a new candidate table. |
| `awat_complete` | MC-side compute after generic facts | Use the existing AW/AT core parser/index and the session mappings; do not replace missing source/resource authority with an empty result. |
| `diagnostics` | MC-side compute after generic facts | Delegate to the existing facade/service for the selected `SourceFamily`; map unavailable facts to `CommandFailure`. |
| `definitions` | MC-side compute after generic facts | Delegate target extraction to `HostMixinServiceFacade::definitions`/`MixinDefinitionService`. It returns targets, not locations. |
| `resolve_definitions` | generic host location + MC resolver | Run existing `DefinitionResolutionService` with `HostProjectSession` and/or `SourceIndex` backends. A missing host location is not permission to synthesize one. |
| `source_entries` | generic host source inventory | Return current `SourceScanEntry` values selected by the host. `HostMixinFacade::collect_source_entries` can merge/deduplicate them, but it cannot enumerate the workspace. |
| `references` | MC-side compute after generic facts | Delegate to `MixinReferenceService::find_references` over the exact entries supplied by `source_entries`. |
| `code_actions` | MC-side compute after generic facts | Delegate to `HostMixinServiceFacade::code_actions` and the existing Mixin/MixinExtras action services. |
| `selected_mixin_config_content` | MC selection + generic resource read | Use `HostMixinFacade::selected_mixin_config` against the immutable `ProjectContext`, then read the selected path through `HostMixinSourceProvider`. Selection and content read stay separate. |
| `resolve_fix` | MC fix/edit compute + generic location/precondition | Resolve only with current source/config and version checks; return the existing `(String, McFix)` shape. Do not write files or accept stale source as current. |

The three ownership buckets are therefore not three new abstractions: they are
the existing core facade, the existing host-provider traits, and the existing
dispatcher caches.

## Smallest real composition

No new wire or DTO is needed. The public core-side opt-ins are
`HostCheckedClassIndex` and `HostCheckedReferenceTypes`: they wrap the
existing lazy binary/source facts so command composition can preserve
`Unavailable` instead of observing compatibility `Option`/`Vec` APIs.
`CoreCommandHost` owns the following values for its whole
`CommandDispatcher` lifetime:

```text
Arc<HostProjectSession>
CachedProjectSession
Arc<dyn HostProjectQuery>
Arc<dyn HostMixinSourceProvider>
Arc<dyn HostSemanticModelProvider>
SemanticModelCache
HostFact<Vec<SourceScanEntry>>
Arc<ExpressionMemberCompletionCache> (per complete HostProjectKey)
private checked class/bytecode adapters backed by the same session/provider
BuildIdentity
standard clock
```

The concrete constructor is intentionally just the existing composition inputs:

```text
CoreCommandHost::new(
    Arc<HostProjectSession>,
    CachedProjectSession,
    Arc<dyn HostProjectQuery>,
    Arc<dyn HostSemanticModelProvider>,
    Arc<dyn HostMixinSourceProvider>,
    HostFact<Vec<SourceScanEntry>>,
    BuildIdentity,
    host_supports_kotlin_semantics: bool,
) -> Result<CoreCommandHost, CommandFailure>
```

The session construction it consumes is already public:

```text
HostProjectSession::with_class_index(
    Arc<ProjectContext>,
    HostSessionIdentity,
    Arc<dyn HostProjectQuery>,
    Arc<HostClassIndexOverlay>,
)
```

When the caller has an authoritative source query, the checked composition is:

```text
HostProjectSession::with_checked_source_index(
    Arc<ProjectContext>,
    HostSessionIdentity,
    Arc<dyn HostProjectQuery>,
    Arc<HostClassIndexOverlay>,
    Arc<dyn ProjectSourceQuery>,
)
```

`HostProjectSession::checked_class_index.owner_result` requires complete source
class/member facts or a real binary catalog plus readable/parseable class
bytes. A catalog-only `find_class(Some)` is never treated as proof. A
`SourceAuthority::Complete` owner is returned without touching binary bytes;
partial source falls back to the fallible binary path. Existing
`with_class_index`/`create`, legacy overlays, and compatibility methods are
unchanged.

The caller may supply a source-first class-index overlay through
`HostProjectSession::with_class_index`. `HostMixinServiceFacade::new` then
borrows the session-backed checked indexes only for one command.
`with_registered_sources` and
`with_expression_cache` attach the existing source and project-keyed caches;
the facade itself is not stored across host I/O. This composition does not use
`FileBasedProjectContextService::with_builder`: even its builder computes a
filesystem `context_fingerprint`, which would violate the host-owned context
boundary here.

The per-root load operation is the existing:

```text
HostProjectSessions::load(
    &CachedProjectSession,
    HostSessionIdentity,
    Arc<dyn HostProjectQuery>,
    Arc<HostClassIndexOverlay>,
) -> HostProjectResult<(Arc<HostProjectSession>, bool)>
```

It validates version/root/all generation dimensions, `HostProjectQuery::is_open`
and `validate`, reuses only an identical live session, and closes the replaced
session. `FileBasedProjectContextService` may cache the owned `ProjectContext`,
but it cannot invent the host identity.

### No-Unknown gate

Before calling `HostProjectSession::create/with_class_index` or entering a
complete-only core service, the host must have:

- a current workspace URI/root and project identity;
- `HostSessionIdentity` with every `HostGenerations` dimension known
  (`project`, `model`, `namespace`, `classpath`, `dependency`,
  `generated_source`, `config`, `provider`); zero is valid only when the host
  explicitly reports generation zero;
- a complete/current `HostProjectFacts` classpath and source-set projection;
- current `HostProjectQuery` validation, class catalog and exact class bytes
  when bytecode is required. Header/ABI facts do not replace Code/LVT or
  annotation-bearing bytes;
- a `HostSemanticModelProvider` answer that is `Known`, or an explicit stable
  fallback. `SemanticModelCache` already bypasses reuse without a complete
  `HostProjectKey` and never caches transient fallback;
- complete source/config/resource authority for the requested command. A
  `HostFact::Missing` means a checked absence; `Unavailable` must remain an
  error/unknown;
- location facts for navigation. The existing `HostSourceSnapshot` +
  `SourceIndex` and `DefinitionResolutionService` are safe providers, but the
  host must actually publish the snapshots and declaration ranges.

`HostFactCache` validates read stamps and generations on every reuse. Keep
`HostProjectSessions`, semantic/source caches and expression caches owned by
the host; keep `HostMixinServiceFacade` and `HostReferenceTypes` borrowed for
one command. The dispatcher already owns document snapshots, negative results
and in-flight locks. This is the minimum cache lifetime that prevents stale
class bytes, source, project identity or host handles from crossing a refresh.

`CoreCommandHost` passes private checked class/bytecode adapters into every
actual MC facade/AWAT access. Catalog, owner, member and bytecode failures
become typed `BytecodeUnavailable`; a source-complete owner is authoritative
and does not read binary bytes. There is no global catalog probe for a model
with no targets, so plain no-MC diagnostics can complete without catalog or
bytecode reads. Every project-aware compute path validates the session again after core computation,
so a refresh occurring during computation cannot be returned as a successful
DTO. Source inventory `Missing`/`Unavailable`, stale sessions, closed sessions,
root mismatches and cancellation remain typed failures. These checks only
preserve existing host facts; they do not manufacture class bytes, catalogs,
locations or source entries.

The same rule applies to reference facts. `checked_reference_types` uses the
existing fallible `reference_type_result`/`assignability_result` calls; a
known absence is `false`, but `Unavailable`, `Failure` and `Interrupted` are
raised through the facade's existing panic boundary and mapped to typed
unavailable/cancellation results. The compatibility `Option<bool>` adapter is
left unchanged for legacy callers and is never used as the production command
gate.

## What is and is not available now

### Safe before host publication

- Keep the existing dispatcher, decoder, DTO converters and `CommandHost`
  signatures unchanged.
- Exercise `HostProjectSession` + source-first index +
  `HostMixinServiceFacade` with complete generic fixture facts. This proves
  actual core output through the whole command DTO, while faking only the
  generic provider answers.
- Finish/retain buffer-only completion through
  `HostMixinFacade::try_complete_buffer_only` and `--stdio-completion`; it
  does not claim a project session.
- Keep `mcdev-companion/tests/core_commands.rs` as the fixture-host proof for
  completion, diagnostics, config edits, and definition resolution. It calls
  the real dispatcher, facade and converters; only generic semantic/source/
  index/definition facts are faked.
  Do not advertise this library proof as connected to semantic-v1.

### Must wait for host/provider/control authority

- live host publication of `has_project_facts`, `load_cached_session`, all
  eight generations and session validation;
- project refresh for `reindex`, scoped cache invalidation for
  `reloadProjectContext`, and complete source/resource inventory. The concrete
  host leaves those two control methods typed Unavailable until that authority
  exists;
- class catalogs, hierarchy/assignability, exact executable class bytes,
  source declaration ranges and definition locations;
- source text enumeration for `source_entries` (the current
  `HostProjectSourceProvider` returns class facts, while
  `HostMixinSourceProvider` reads selected resources/registered compilation
  units; neither is a complete Java/Kotlin source inventory);
- actual command routing/acceptance in the host. The executable's default
  framed server accepts only `initialize`, `analysis/evaluate` and the
  semantic-v1 lifecycle; it does not route the eleven commands.

The current two-operation `projectContext`/`typeJvm` adapter therefore remains
an explicit incomplete-facts boundary. No second parser, SDK table, child
scanner, hardcoded classpath, fabricated `HostProjectSession`, or guessed
wire is part of this composition.

## Implemented whole-command integration cases

`mcdev-companion/tests/core_commands.rs` calls the real `CommandDispatcher`,
real core facade and real protocol converter; only generic host facts/providers
are fake.

1. `dispatcher_reaches_core_for_completion_diagnostics_and_config_edit`
   compares the complete public completion, diagnostic and code-action DTOs
   (only explicitly measured timing/payload fields are excluded), then applies
   the returned protocol edit and compares the complete JSON config result.
2. `unavailable_catalog_and_class_bytes_are_typed_failures_but_source_overlay_is_authoritative`
   asserts `BytecodeUnavailable` for generic catalog and target-byte Unknown,
   including a real lazy binary session where `find_class(Some)` is true but
   bytes are unavailable, and success with zero binary reads when a complete
   source overlay owns the target.
3. `stale_root_unknown_source_and_cancellation_never_become_empty_success`
   asserts typed source-inventory, stale-root and cancellation boundaries.
4. `unavailable_reference_facts_are_strict_on_the_core_facade_path` asserts
   reference/assignability Unknown stays Unknown at the compatibility edge and
   becomes a preserved typed host error on the checked facade path.
5. `actual_facade_preserves_unavailable_bytecode_and_reference_facts` reaches
   the real completion/diagnostic facade paths: source-complete members do not
   preflight binary bytes, but an expression bytecode read and an unresolved
   handler reference fact still become typed command errors.
6. `stale_project_after_semantic_compute_is_a_typed_failure` changes the project
   during semantic computation and requires `IncompleteProjectContext` with no
   successful result.
7. `definition_direct_source_is_resolved_without_host_lookup` proves that a
   direct source URI/range is returned as `SOURCE` without querying host
   definition facts.
8. `definition_missing_host_location_is_an_unresolved_location` preserves one
   `UNRESOLVED` location, its source range, and the exact
   `no navigable definition found` message for a checked host absence.
9. `definition_invalid_direct_source_preserves_resolution_message` preserves
   the existing `direct source target missing uri or range` message and avoids
   host lookup for an invalid direct target.
10. `definition_unknown_host_location_is_a_typed_error` preserves an
    unavailable host definition answer as `IncompleteProjectContext`.
11. `missing_document_version_uses_one_semantic_provider_call_per_command`
    proves exactly one semantic provider call for `definition`, `diagnostics`,
    and `codeAction` when `documentVersion` is absent.

Definition navigation is now covered through `HostProjectQuery::definition`
and the existing `DefinitionResolutionService`. References remain a follow-up:
`referencesReturnsMixinUsages` still needs a complete owned
`SourceScanEntry` inventory, including the Java/AW/AT deduplication path.

Focused verification passes all eleven `core_commands` tests in
`build/rust-test-logs/core-command-p1p2-20261025/core_commands.retry3.stdout-stderr.log`.
The same focused command was also run through `Invoke-RustCheck`, with complete
stdout/stderr retained under the sibling `invoke/` directory. Directly affected
navigation and host-project checks were run separately through the raw cargo and
`Invoke-RustCheck` paths, with their complete logs under the same unique
`core-command-p1p2-20261025` directory. The actual facade regression still covers
source-complete expression bytecode and handler reference facts. The stable
workspace pipeline includes repository-wide formatting and editor checks.
The legacy callback proof and 189-case facade proof remain valid for their
stated scopes and are not live command-wire proof.
