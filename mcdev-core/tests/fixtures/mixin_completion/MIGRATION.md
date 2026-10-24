# Project and completion/edit migration evidence

`legacy_coverage.json` is the exact path/class/method ledger. It records frozen source SHA256, the native Rust test, evidence log, and pending assertions. Current result: **288 ported, one pending host integration**, from 289 assigned original tests. This does not count caller facade/handler tests assigned to other domains.

## Completion and edits

All 162 assigned original tests ran with their original assertions against disposable instrumented Kotlin source copies. `legacy_calls.jsonl` contains 655 API events: 361 direct completion/edit calls, including expected exceptions and callback reads, plus 294 source helper calls. `mixin_completion_legacy.rs` executes one native test per original case. The native replay compares whole results, including completion metadata, edits, import references, snippets, configuration deltas, and callback order/count. Source helper rows also check concrete result variants and UTF16-to-UTF8 offset conversion before canonicalizing serialization.

Covered services: AtTargetCompletionService, AtTargetInsertFormatter, AtValueCompletionService, ConstantAtHintFormatter, InjectMethodCompletionService, MixinAttributeCompletionService, MixinTargetCompletionService, MixinDeclarationSnippetService, MixinImportEditBuilder, MixinConfigEditor, and MixinSideResolver. The selected MixinEdgeCasesTest cases directly assert those services; no facade/handler result is inferred from a helper capture.

Native source-helper replay is shared with `mixin_source/replay.rs`. The source module remains the authority for annotations, imports, source scopes and semantic models. No independent whole-language parser or Extras engine was introduced.

## Project context

All 106 original core project tests ran their original assertions during capture. There are 568 captured API events. `project_legacy.rs` replays the 100 service cases with original filesystem bytes materialized in a unique disposable directory for every operation. The six constructor/model cases run directly in `project_legacy_stateful.rs`. Mapping resolver calls are executed against a resolver rebuilt from the original mapping files, rather than comparing serialized trait handles.

The 21 FileBasedProjectContextServiceTest cases have 20 native ports: 16 fixture/resource/source-set cases in `project_legacy_stateful.rs` and four stateful concurrency cases in `project.rs`. The latter retain the original 32 concurrent callers, identity/version/cache-miss assertions, independent-workspace gates, cold-load/reindex ordering, and eight callers with exactly one build failure and seven successes. Fixtures are copied from the frozen baseline, including the compiled SimpleTarget class.

`sessionIndexesClasspathClasses` remains **pending_host_integration**. Its original assertions require a single session to expose both `classBytesProvider.classCount() >= 1` and a READY context. The new host owns project/classpath and classbytes; independently asserting context readiness and a bytecode count would not prove that connection. The exact fixture and required assertions are retained in `project_capture_metadata.json` for HostProjectSession/HostClassBytesProvider integration.

Host-supplied project facts are authoritative. The file fallback discovers Minecraft resources/configuration and explicit filesystem outputs but adds no Java/Kotlin source resolver. Resource contents and host generations invalidate cached context. Project discovery and lossless config editing share the same permissive JSON value parser, including unquoted properties and single-quoted values.

## Validation

Final affected run: `build/rust-test-logs/project-completion-final/affected.stdout.log` and `.stderr.log`, exit 0, no timeout (25.4 seconds). Seven suites passed: 20 completion regressions, 12 declaration regressions, 162 exact completion/edit cases, 18 edit regressions, 28 project regressions/stateful checks, 100 exact project service cases, and 22 model/fallback cases: **362 passed**.

Owned Rust files were formatted. No host/editor/E2E suite was run by this domain agent. Windows native tests passed; Linux was not executed. The capture-platform adapter is documented in `project_capture_metadata.json`: only the three APIs explicitly returning native sorted paths adapt Windows-captured ordering to Unix native ordering. No semantic output arrays are reordered to hide differences. Wall-clock capture time and trait handles are serialization-only adaptations; explicit file timestamps and recorded resolver operations remain asserted.

The one-time generators require the read-only frozen baseline/JVM compiler; running the saved native tests requires neither JVM nor the original checkout. No commits were created.
