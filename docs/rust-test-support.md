# Rust test support audit (corrected)

Scope: the 11 Kotlin test-support sources still pending in the parent migration ledger. This is a support-role audit, not a case-count migration and not a waiver.

## Provenance and caller-ID boundary

The authoritative snapshot exists at `C:/Users/gummy/.codex/worktrees/ktlsp-rust/mc-dev-lsp/build/migration-baseline/current-20261023`. All 11 complete bodies were reread from that absolute target-worktree path; all 11 SHA-256 values match the parent ledger. The original main checkout remained read-only.

`confirmedDirectCallerCaseIds` means the support token occurs in the test function. `confirmedIndirectCallerCaseIds` means a bounded same-file test→helper/property→support chain was found. `candidateCallerCaseIds` is every case in a source file with an actual non-import support token and is only a file-scope upper bound. `unconfirmedCandidateCaseIds` are not exact support callers and remain pending/unknown.

| support | SHA prefix | candidate upper bound | confirmed direct/indirect | unconfirmed | status |
|---|---:|---:|---:|---:|---|
| `AtTestFixtures` | `705CF8D5F072…` | 43 | 43 | 0 | `complete_support_confirmed` |
| `AwTestFixtures` | `4FF918871051…` | 55 | 46 | 9 | `partial_pending` |
| `AwAtE2ETestSupport` | `489E2E5BB9D8…` | 33 | 33 | 0 | `complete_support_confirmed` |
| `BytecodeFixtureCompiler` | `50C63984E659…` | 731 | 610 | 121 | `partial_pending` |
| `CoreFixturePaths` | `8BE98FF414EC…` | 0 | 0 | 0 | `unused_support_native_role` |
| `CoreFixtureResourceLoader` | `AD92F04F7B8D…` | 0 | 0 | 0 | `unused_support_native_role` |
| `MixinTestFixtures` | `6F4914F0ED8A…` | 991 | 847 | 144 | `partial_pending` |
| `MixinE2ETestSupport` | `94114FF098B8…` | 219 | 217 | 2 | `partial_pending` |
| `MixinExtrasTestFixtures` | `FBD7D11F584C…` | 573 | 535 | 38 | `partial_pending` |
| `ClasspathCommonSuperClassResolverTest` | `7AAD5D335CE7…` | 0 | 0 | 0 | `tombstone_moved` |
| `JdtlsFixtureSupport` | `BD4E7C0FC294…` | 155 | 144 | 11 | `partial_pending` |

## AtTestFixtures

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtTestFixtures.kt`
- SHA-256: `705CF8D5F07295BF00BB4805CAE32C1ECFACA2D3630B39FB9CD61A47CF693780`
- Status: **complete_support_confirmed**
- Caller boundary: candidate upper bound 43; confirmed direct 31, confirmed indirect 12, unconfirmed 0. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: classIndex, fabricMappingContext, forgeMappingContext, srgMappingSet(), incompleteSrgMappingContext(). FakeClassIndex plus NAMED↔INTERMEDIARY Fabric and NAMED↔SRG Forge contexts. srgMappingSet() contains MinecraftClient→Minecraft, setScreen→m_91152_, tick→m_91398_, currentScreen→f_91074_, and player→f_91075_; incompleteSrgMappingContext() removes methods but keeps class/field mappings.
- Native replacement: `mcdev-core/tests/awat_legacy.rs`, `mcdev-core/src/awat.rs`. AT code-action, completion, diagnostics, and member-formatting legacy callers.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/resources/manifest.json` (19fa97d828e802b11a22418c20f8230061cde37551912431fe5ae2bd26c1e4c5), `mcdev-core/tests/fixtures/resources/source_manifest.json` (2c77ff92c7544304a982c97d30354d56d1f27854bf995b358049a1d3c5e56d2f), `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f), `mcdev-core/tests/fixtures/awat/legacy_cases.json` (1ce6ccab8f6e5a58685d75549c4817291aa05fe1fe44396100d5eba48903063f), `mcdev-core/tests/fixtures/awat/legacy_calls.jsonl` (c584d77d1ab8ff43624e8f4ba989cb090478bb5710f47dcef71da27b1c7edfb9).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/awat-strict-20261024-141117454/awat.stdout.log: support/facade 27-pass block and strict AT/AW 157-pass block; all 43 candidate cases have confirmed direct/indirect support reachability and ported status.`.
- Remaining: No unconfirmed caller mapping in the bounded graph. The 43 is a support-role candidate upper bound, not a new master case count.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No Java compiler/runtime fixture generation.

## AwTestFixtures

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwTestFixtures.kt`
- SHA-256: `4FF918871051596C3AA8D1EE8275849047C1C46228545F734407B76C0B197F3E`
- Status: **partial_pending**
- Caller boundary: candidate upper bound 55; confirmed direct 11, confirmed indirect 35, unconfirmed 9. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: VALID_AW, INTERMEDIARY_AW, mappingContext(), classIndex, FakeAwMappingResolver (separate object: namespaces/remapClass/remapField/remapMethod/remapDescriptor). VALID_AW is named AW text for MinecraftClient/setScreen/currentScreen; INTERMEDIARY_AW is intermediary class_310/method_91152 text. mappingContext() uses NAMED→INTERMEDIARY and FakeAwMappingResolver maps class, currentScreen↔field_1755, setScreen↔method_91152, with descriptor identity.
- Native replacement: `mcdev-core/tests/awat_legacy.rs`, `mcdev-core/src/awat.rs`. AW code-action, completion, diagnostics, editor, and context-extractor legacy callers.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/resources/manifest.json` (19fa97d828e802b11a22418c20f8230061cde37551912431fe5ae2bd26c1e4c5), `mcdev-core/tests/fixtures/resources/source_manifest.json` (2c77ff92c7544304a982c97d30354d56d1f27854bf995b358049a1d3c5e56d2f), `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f), `mcdev-core/tests/fixtures/awat/legacy_cases.json` (1ce6ccab8f6e5a58685d75549c4817291aa05fe1fe44396100d5eba48903063f), `mcdev-core/tests/fixtures/awat/legacy_calls.jsonl` (c584d77d1ab8ff43624e8f4ba989cb090478bb5710f47dcef71da27b1c7edfb9).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/awat-strict-20261024-141117454/awat.stdout.log: strict AT/AW pass; 46 candidate cases have confirmed direct/indirect support reachability, while 9 file-scope candidates remain unconfirmed.`.
- Remaining: 55 file-scope candidate cases are an upper bound; 46 have confirmed direct/indirect reachability and 9 remain unconfirmed. Candidate statuses are ported 55. Do not close this support role from file-level presence or a passing subset.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No Java compiler/runtime fixture generation; do not infer the 9 candidate cases from import/file scope.

## AwAtE2ETestSupport

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtE2ETestSupport.kt`
- SHA-256: `489E2E5BB9D80D7E09647A382D1F6CCE3006762718C4D344686CC26988E9DED5`
- Status: **complete_support_confirmed**
- Caller boundary: candidate upper bound 33; confirmed direct 32, confirmed indirect 1, unconfirmed 0. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: SIMPLE_TARGET_INTERNAL, loadFixtureText(), simpleTargetClassIndex(), fabricAwAtMappingContext(), facade(), indexedFacade(), requestAt(), requestAtOffset(), offsetToLineCharacter(). SimpleTarget index with draw/staticMethod/counter; TinyV2 mappings loaded from FixturePaths.FABRIC_AW_AT_MAPPINGS; class bytes through InMemoryClassBytesProvider and ProjectContextMixinIndex; AW/AT request buffers, offset conversion, and file:///mod.accesswidener/file:///mod_at.cfg defaults.
- Native replacement: `mcdev-core/tests/awat_legacy.rs`, `mcdev-core/tests/fixtures/awat/replay.rs`, `mcdev-core/tests/fixtures/awat-simple-target.class.bin`. AT/AW definition/reference and AwAtServiceFacade E2E legacy callers.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/resources/manifest.json` (19fa97d828e802b11a22418c20f8230061cde37551912431fe5ae2bd26c1e4c5), `mcdev-core/tests/fixtures/resources/source_manifest.json` (2c77ff92c7544304a982c97d30354d56d1f27854bf995b358049a1d3c5e56d2f), `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f), `mcdev-core/tests/fixtures/awat-simple-target.class.bin` (7f0fdaf1433dfbccf7178347f632df7afe096328087572b9ff7267d34885efc6), `mcdev-core/tests/fixtures/awat/legacy_cases.json` (1ce6ccab8f6e5a58685d75549c4817291aa05fe1fe44396100d5eba48903063f), `mcdev-core/tests/fixtures/awat/legacy_calls.jsonl` (c584d77d1ab8ff43624e8f4ba989cb090478bb5710f47dcef71da27b1c7edfb9).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/awat-strict-20261024-141117454/awat.stdout.log: all 33 candidate cases have confirmed direct/indirect support reachability and are in the existing strict pass.`.
- Remaining: No unconfirmed caller mapping in the bounded graph. The 33 is a support-role candidate upper bound, not a new master case count.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: SimpleTarget is consumed from preserved class bytes; no runtime Java compiler.

## BytecodeFixtureCompiler

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/bytecode/BytecodeFixtureCompiler.kt`
- SHA-256: `50C63984E659A2307657BD0DB2631B6D834371F554A4431D37A6409EAFCC73F1`
- Status: **partial_pending**
- Caller boundary: candidate upper bound 731; confirmed direct 37, confirmed indirect 573, unconfirmed 121. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: compileFixtures(), provider(), classBytes(), internalName(). Legacy Java bytecode fixture sources (InvokeSamples and eight peers), generated class-byte map, internal-name/provider metadata, and classpath/jar lookup behavior.
- Native replacement: `mcdev-core/tests/fixtures/bytecode_generate.py`, `mcdev-core/tests/fixtures/bytecode_classes.json`, `mcdev-core/tests/fixtures/bytecode_classes.provenance.json`, `mcdev-core/tests/fixtures/bytecode_legacy.rs`, `mcdev-core/tests/bytecode_legacy.rs`, `mcdev-core/tests/fixtures/expression_engine_replay.rs`. Bytecode/index/resolver plus MixinExtras expression/handler/locals native suites.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/bytecode_classes.json` (8fb442c4ac16a31b42aec1cf6e567fae603cb200e8ff272c3ad05a7869baea77).
- Derived compiled provenance: `mcdev-core/tests/fixtures/bytecode_classes.provenance.json` (595bbdad48a8c511e4bcf5c91a602b6957398f851218d349989381bd35c7f9a4); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/expression-engine-20261024/owned-final-parity.stdout.log: 153 bytecode/index/resolver tests passed, including exact provider bytes/common-superclass checks.`, `build/rust-test-logs/extras-services-handoff-20261024/services.stdout.log`, `build/rust-test-logs/handler-handoff-20261024/handler.stdout.log`, `build/rust-test-logs/mixinextras-expression-engine-validation/affected.stdout.log`, `build/rust-test-logs/mixinextras-expression-joined-final-2/affected.stdout.log`, `build/rust-test-logs/mixinextras-locals-final/affected.stdout.log`.
- Remaining: 731 file-scope candidate cases are an upper bound; 610 have confirmed direct/indirect reachability and 121 remain unconfirmed. Candidate statuses are ported 685, pending_caller_integration 3, pending 43. Do not close this support role from file-level presence or a passing subset.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: bytecode_generate.py is one-time JDK21 --release 21 export with -proc:none. Rust execution loads saved class bytes; it does not invoke javac, ToolProvider, or a Java fixture compiler.

## CoreFixturePaths

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/fixtures/CoreFixturePaths.kt`
- SHA-256: `8BE98FF414EC0B858DE7E5B03327BA8441419F962E2653EC42942024B735E91E`
- Status: **unused_support_native_role**
- Caller boundary: candidate upper bound 0; confirmed direct 0, confirmed indirect 0, unconfirmed 0. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: ROOT, FABRIC_BASIC, FABRIC_MIXINEXTRAS, FORGE_BASIC, NEOFORGE_BASIC, BROKEN_DIAGNOSTICS, FABRIC_AW_AT, SHARED_CLASSES, FABRIC_BASIC_MAPPINGS, FABRIC_AW_AT_MAPPINGS, FABRIC_AW_AT_ACCESS_WIDENER, FABRIC_AW_AT_ACCESS_TRANSFORMER, FABRIC_BASIC_MIXINS_JSON, SIMPLE_TARGET_CLASS. Canonical path data for six fixture roots, shared/classes, Tiny mappings, AW/AT files, mixins.json, and SimpleTarget.class. This is real fixture data even though no original test source references the object name.
- Native replacement: `mcdev-core/tests/fixtures/resources/mod.rs`, `mcdev-core/tests/fixtures/resources/catalog.rs`, `mcdev-core/tests/fixtures/resources/manifest.json`, `mcdev-core/tests/fixtures/resources/source_manifest.json`, `mcdev-core/tests/fixture_integration.rs`. Resource catalog and fixture integration invariants; no original caller cases found.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/resources/manifest.json` (19fa97d828e802b11a22418c20f8230061cde37551912431fe5ae2bd26c1e4c5), `mcdev-core/tests/fixtures/resources/source_manifest.json` (2c77ff92c7544304a982c97d30354d56d1f27854bf995b358049a1d3c5e56d2f), `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/fixture-integration-final/fixtures.stdout.log: 14 fixture integration tests passed, including resource listing, mappings/config, and compiled SimpleTarget checks.`.
- Remaining: Original caller candidate count and assertion count are 0. This is unused support with its role moved to the native catalog, not an empty support body or moved-test completion claim.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No Java-shaped constants wrapper added.

## CoreFixtureResourceLoader

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/fixtures/CoreFixtureResourceLoader.kt`
- SHA-256: `AD92F04F7B8D7633A9A21506A99226D6C29357AEA678D2D80E876C9C0A2F9F05`
- Status: **unused_support_native_role**
- Caller boundary: candidate upper bound 0; confirmed direct 0, confirmed indirect 0, unconfirmed 0. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: exists(path), openStream(path): InputStream, loadText(path), loadBytes(path), private normalize(path). Real 22-line/801-byte ClassLoader implementation: trim/trimStart/replace normalization, getResource/getResourceAsStream, buffered text, and byte loading.
- Native replacement: `mcdev-core/tests/fixtures/resources/mod.rs`, `mcdev-core/tests/fixtures/resources/catalog.rs`, `mcdev-core/tests/fixture_integration.rs`. Native catalog/include_str!/include_bytes! resource integration and exact manifest SHA checks; no original caller cases reference this object name.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/resources/manifest.json` (19fa97d828e802b11a22418c20f8230061cde37551912431fe5ae2bd26c1e4c5), `mcdev-core/tests/fixtures/resources/source_manifest.json` (2c77ff92c7544304a982c97d30354d56d1f27854bf995b358049a1d3c5e56d2f), `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/fixture-integration-final/fixtures.stdout.log: 14 fixture integration tests passed; 49 resource SHA entries and 40 logical resources are checked by native integration.`.
- Remaining: Original caller candidate count and assertion count are 0. The loader API role is represented by native catalog/resource access; it is not a tombstone and not independently claimed as a moved test.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No broad Java/Kotlin classloader shim introduced.

## MixinTestFixtures

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinTestFixtures.kt`
- SHA-256: `6F4914F0ED8A4B287F8F6BA924DEBE717C129D052960E83576B1A60BEDDFC1F6`
- Status: **partial_pending**
- Caller boundary: candidate upper bound 991; confirmed direct 102, confirmed indirect 745, unconfirmed 144. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: FakeClassIndex, FakeClassIndex.defaultClasses(), FakeClassIndex.defaultMethods(), FakeClassIndex.defaultFields(), FakeBytecodeIndex, FakeBytecodeIndex.defaultCandidates(), FakeBytecodeIndex.defaultReturnCounts(). Default MinecraftClient/GameRenderer/TextRenderer/DrawContext classes; tick/setScreen/render overloads; currentScreen/player fields; INVOKE/FIELD candidates and return count 2.
- Native replacement: `mcdev-core/tests/fixtures/mixin_completion/replay.rs`, `mcdev-core/tests/fixtures/mixin_facade/replay.rs`, `mcdev-core/tests/fixtures/mixin_source/replay.rs`, `mcdev-core/tests/fixtures/mixin_navigation/replay.rs`, `mcdev-core/tests/mixin_source_legacy.rs`, `mcdev-core/tests/mixin_navigation_legacy.rs`, `mcdev-core/tests/mixin_validation_legacy.rs`. Mixin source/navigation/validation, completion/project, and facade replay callers using explicit Rust indexes or captured callbacks.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/mixin_completion/project_files/SimpleTarget.class.bin` (4e3b9137c518f0449e9360875173d9dd516e48c58d0005a3a57c9ed5818a47f7).
- Captured callback/input files: `mcdev-core/tests/fixtures/mixin_completion/legacy_cases.json` (e31a3e13c11b918786237ace15dd4bac284c4cee9e52ad0c988aadbca04d2d0f), `mcdev-core/tests/fixtures/mixin_completion/legacy_calls.jsonl` (3e091e77d77d6b1516439d86b00540ccaa85c4a7b63eebd2448729157b9770de), `mcdev-core/tests/fixtures/mixin_completion/project_legacy_cases.json` (e7cc5696d24a225d16a3ce5384ad7aa2027f08f1d18eaeae8b7e3af028d335c3), `mcdev-core/tests/fixtures/mixin_completion/project_legacy_calls.jsonl` (f426c95760cc5623b5279aa5f52764ee4a681a97742b509cd8dfaad66ea2775e), `mcdev-core/tests/fixtures/mixin_facade/core-cases.json` (05172b3072c5a25e7e9a0f24ded7ec6563c4f9193b91ac03afe5aac2b1fa7fdf), `mcdev-core/tests/fixtures/mixin_facade/core-calls.jsonl` (1a0eb1af76ba44ba40c1f5c020e625383f847bf82e3bd7353ca559efc78d71d6), `mcdev-core/tests/fixtures/mixin_facade/host-cases.json` (c3f1675452e488fd8ed65bc0d7a813f95525ced0daa61406820e3fc7177b3c05), `mcdev-core/tests/fixtures/mixin_facade/host-calls.jsonl` (1ff18a9638de13a5956b5fd232337f9095dd75c6e74cdcbe53a5886e2d19fd27), `mcdev-core/tests/fixtures/mixin_source/source-cases.json` (826b7dedf9de506760ecd9994ab3bc1fe67d072d5d53c08e66d84b18342afec6), `mcdev-core/tests/fixtures/mixin_source/source-calls.jsonl` (1b7b8de352961ae8cbc845706867230e1b588983559f9b2b07ebedbaefa5a275), `mcdev-core/tests/fixtures/mixin_navigation/legacy_cases.json` (917665ed80d8c471950e2b36ba788a2a37d9e308b991ff9acb340b2f78e52c79), `mcdev-core/tests/fixtures/mixin_navigation/legacy_calls.jsonl` (edc6392d7cfe2ab97f2fd1af27a0a03f92e6362113f444495351920cf8f0b2a2).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/mixin-navigation-identifier-final/affected.stdout.log`, `build/rust-test-logs/mixin-source-phase1-final/affected.stdout.log`, `build/rust-test-logs/mixin-validation-20261023-230147260/validation.stdout.log`, `build/rust-test-logs/project-completion-final/affected.stdout.log`, `build/rust-test-logs/expression-engine-20261024/owned-final-parity.stdout.log`.
- Remaining: 991 file-scope candidate cases are an upper bound; 847 have confirmed direct/indirect reachability and 144 remain unconfirmed. Candidate statuses are ported 893, pending 83, pending_caller_integration 15. Do not close this support role from file-level presence or a passing subset.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: Do not create a Java-shaped FakeClassIndex wrapper; finish pending callers using existing native indexes/replays.

## MixinE2ETestSupport

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/e2e/MixinE2ETestSupport.kt`
- SHA-256: `94114FF098B81A27CA587CE8EB9D3AEFBB5E2C481F5B6C45D7C9C62EF965D8F8`
- Status: **partial_pending**
- Caller boundary: candidate upper bound 219; confirmed direct 169, confirmed indirect 48, unconfirmed 2. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: SIMPLE_TARGET_INTERNAL, loadFixtureText(), simpleTargetProvider(), fakeFacade(), simpleTargetFacade(), requestAt(), requestInAnnotationValue(), requestAtOffset(), offsetToLineCharacter(). SimpleTarget.class provider, FakeClassIndex/FakeBytecodeIndex facade, ProjectContextMixinIndex facade, annotation-value/request buffers, line/character conversion, and file:///Mixin.java default URI.
- Native replacement: `mcdev-core/tests/fixtures/mixin_completion/replay.rs`, `mcdev-core/tests/fixtures/mixin_completion/project_replay.rs`, `mcdev-core/tests/fixtures/mixin_facade/replay.rs`, `mcdev-core/tests/fixtures/mixin_source/replay.rs`, `mcdev-core/tests/fixtures/mixin_navigation/replay.rs`, `mcdev-core/tests/mixin_navigation_legacy.rs`, `mcdev-core/tests/mixin_source_legacy.rs`. Mixin definition/navigation, completion/project, facade, and bytecode callers with preserved class bytes and callback traces.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/mixin_completion/project_files/SimpleTarget.class.bin` (4e3b9137c518f0449e9360875173d9dd516e48c58d0005a3a57c9ed5818a47f7).
- Captured callback/input files: `mcdev-core/tests/fixtures/mixin_completion/legacy_cases.json` (e31a3e13c11b918786237ace15dd4bac284c4cee9e52ad0c988aadbca04d2d0f), `mcdev-core/tests/fixtures/mixin_completion/legacy_calls.jsonl` (3e091e77d77d6b1516439d86b00540ccaa85c4a7b63eebd2448729157b9770de), `mcdev-core/tests/fixtures/mixin_completion/project_legacy_cases.json` (e7cc5696d24a225d16a3ce5384ad7aa2027f08f1d18eaeae8b7e3af028d335c3), `mcdev-core/tests/fixtures/mixin_completion/project_legacy_calls.jsonl` (f426c95760cc5623b5279aa5f52764ee4a681a97742b509cd8dfaad66ea2775e), `mcdev-core/tests/fixtures/mixin_facade/core-cases.json` (05172b3072c5a25e7e9a0f24ded7ec6563c4f9193b91ac03afe5aac2b1fa7fdf), `mcdev-core/tests/fixtures/mixin_facade/core-calls.jsonl` (1a0eb1af76ba44ba40c1f5c020e625383f847bf82e3bd7353ca559efc78d71d6), `mcdev-core/tests/fixtures/mixin_facade/host-cases.json` (c3f1675452e488fd8ed65bc0d7a813f95525ced0daa61406820e3fc7177b3c05), `mcdev-core/tests/fixtures/mixin_facade/host-calls.jsonl` (1ff18a9638de13a5956b5fd232337f9095dd75c6e74cdcbe53a5886e2d19fd27), `mcdev-core/tests/fixtures/mixin_source/source-cases.json` (826b7dedf9de506760ecd9994ab3bc1fe67d072d5d53c08e66d84b18342afec6), `mcdev-core/tests/fixtures/mixin_source/source-calls.jsonl` (1b7b8de352961ae8cbc845706867230e1b588983559f9b2b07ebedbaefa5a275), `mcdev-core/tests/fixtures/mixin_navigation/legacy_cases.json` (917665ed80d8c471950e2b36ba788a2a37d9e308b991ff9acb340b2f78e52c79), `mcdev-core/tests/fixtures/mixin_navigation/legacy_calls.jsonl` (edc6392d7cfe2ab97f2fd1af27a0a03f92e6362113f444495351920cf8f0b2a2).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/expression-engine-20261024/owned-final-parity.stdout.log`, `build/rust-test-logs/mixin-navigation-identifier-final/affected.stdout.log`.
- Remaining: 219 file-scope candidate cases are an upper bound; 217 have confirmed direct/indirect reachability and 2 remain unconfirmed. Candidate statuses are ported 81, pending 138. Do not close this support role from file-level presence or a passing subset.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No runtime compiler and no standalone Kotlin-shaped request helper claimed as equivalent.

## MixinExtrasTestFixtures

- Original baseline source: `mcdev-core/src/test/kotlin/io/github/mcdev/core/mixinextras/MixinExtrasTestFixtures.kt`
- SHA-256: `FBD7D11F584C79A6D63E5F55064ABD1BE9A7B3440B30286613EBE3230F751039`
- Status: **partial_pending**
- Caller boundary: candidate upper bound 573; confirmed direct 105, confirmed indirect 430, unconfirmed 38. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: classIndex, bytecodeIndex, MODIFY_EXPRESSION_SOURCE, MODIFY_RETURN_VOID_SOURCE, MODIFY_RETURN_SOURCE, WRAP_OPERATION_SOURCE, WRAP_OPERATION_BAD_RETURN, WRAP_OPERATION_MISSING_OP, WRAP_OPERATION_OP_NOT_LAST, WRAP_WITH_CONDITION_SOURCE, WRAP_WITH_CONDITION_BAD_OPERATION, WRAP_METHOD_SOURCE, WRAP_METHOD_BAD_RECEIVER, WRAP_OPERATION_NO_HANDLER. FakeClassIndex extensions for String/SimpleTarget/shared targets/LocalRef/NestedLocalRef/CallbackInfo/CallbackInfoReturnable; SimpleTarget draw/compute/noop and label; String.length INVOKE candidate with immediately-popped result; 12 Java source snippets covering valid and invalid MixinExtras handlers.
- Native replacement: `mcdev-core/tests/mixinextras_expression_legacy.rs`, `mcdev-core/tests/mixinextras_handler_dependencies.rs`, `mcdev-core/tests/mixinextras_locals_legacy.rs`, `mcdev-core/tests/mixinextras_services_legacy.rs`, `mcdev-core/tests/fixtures/mixinextras_expression/cases.json`, `mcdev-core/tests/fixtures/mixinextras_handler/handler-cases.json`, `mcdev-core/tests/fixtures/mixinextras_locals/legacy_cases.json`, `mcdev-core/tests/fixtures/mixinextras_services/services-cases.json`. Expression, handler, locals, services, completion, diagnostics, and code-action replay suites using captured source/callback inputs.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/mixin_completion/project_files/SimpleTarget.class.bin` (4e3b9137c518f0449e9360875173d9dd516e48c58d0005a3a57c9ed5818a47f7).
- Captured callback/input files: `mcdev-core/tests/fixtures/mixinextras_expression/cases.json` (6d7d69b394ce53e9b06b46eae75cdc413e2de241e5050cf2b778c8b0ce760f81), `mcdev-core/tests/fixtures/mixinextras_expression/calls.jsonl` (3b75e6f26a3694c5e069ae3a009a92680e524969328c6b176925a2c676660590), `mcdev-core/tests/fixtures/mixinextras_handler/handler-cases.json` (b2ee91e2248ef37eb0e959fffef0a818014e58ce4092dbf6864f956d94e99969), `mcdev-core/tests/fixtures/mixinextras_handler/handler-calls.jsonl` (7e4b274130601a0892786b7374f392d45a1cdf519f90b04cf9297c7e9357b1cf), `mcdev-core/tests/fixtures/mixinextras_locals/legacy_cases.json` (f0e1180a9663628583517f5323842262bfbba13a92dc71a2a63542896562245f), `mcdev-core/tests/fixtures/mixinextras_locals/legacy_calls.jsonl` (f62a2fa69d277199152216c7f0bce550073b85e4df413e8bb266cc3897fa2119), `mcdev-core/tests/fixtures/mixinextras_services/services-cases.json` (ae790e3b8c3446c7f68d900949e1c7a778c4e76cf222b94d5f5e6143b28972b9), `mcdev-core/tests/fixtures/mixinextras_services/services-calls.jsonl` (8f5e3807aabc0fc95f4df0d658824ddb9c6d05c1bd883462f46b5b9579b8d40b).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/extras-services-handoff-20261024/services.stdout.log`, `build/rust-test-logs/handler-handoff-20261024/handler.stdout.log`, `build/rust-test-logs/mixinextras-expression-engine-validation/affected.stdout.log`, `build/rust-test-logs/mixinextras-expression-joined-final-2/affected.stdout.log`, `build/rust-test-logs/mixinextras-locals-final/affected.stdout.log`.
- Remaining: 573 file-scope candidate cases are an upper bound; 535 have confirmed direct/indirect reachability and 38 remain unconfirmed. Candidate statuses are ported 528, pending_caller_integration 3, pending 42. Do not close this support role from file-level presence or a passing subset.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: Use existing native fixture/callback data; do not add Java compiler or Java-shaped wrappers.

## ClasspathCommonSuperClassResolverTest

- Original baseline source: `mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/project/ClasspathCommonSuperClassResolverTest.kt`
- SHA-256: `7AAD5D335CE7E8A4412EEA488420BCFEB202DBD3C042FF8A3E6586AE3542DFC9`
- Status: **tombstone_moved**
- Caller boundary: candidate upper bound 0; confirmed direct 0, confirmed indirect 0, unconfirmed 0. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: . Six-line explicit unused tombstone/comment saying tests moved to mcdev-core; no fixture body or assertions.
- Native replacement: `mcdev-core/tests/bytecode_legacy.rs`, `mcdev-core/src/bytecode.rs`, `mcdev-core/tests/fixtures/bytecode_legacy.rs`. Native common-superclass/classfile resolver cases in the bytecode legacy suite.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/bytecode_classes.json` (8fb442c4ac16a31b42aec1cf6e567fae603cb200e8ff272c3ad05a7869baea77).
- Derived compiled provenance: `mcdev-core/tests/fixtures/bytecode_classes.provenance.json` (595bbdad48a8c511e4bcf5c91a602b6957398f851218d349989381bd35c7f9a4); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/expression-engine-20261024/owned-final-parity.stdout.log: common-superclass and exact class-provider cases are in the existing 153 passing bytecode/index/resolver block.`.
- Remaining: Original caller candidate and assertion counts are exactly 0. This is the only true empty tombstone in the 11-file set.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No Java wrapper created.

## JdtlsFixtureSupport

- Original baseline source: `mcdev-jdtls-extension/src/test/kotlin/io/github/mcdev/jdtls/support/JdtlsFixtureSupport.kt`
- SHA-256: `BD4E7C0FC2944170085120FB791B8F604FDF79EFB2CE61E12347CA49D1B29841`
- Status: **partial_pending**
- Caller boundary: candidate upper bound 155; confirmed direct 94, confirmed indirect 50, unconfirmed 11. Exact support invocation is not inferred for candidate-only IDs.
- Public helpers/data: copyFixture(), copyFixtureResource(), installClasspathClasses(), installLoomRemappedJar(), workspaceUri(), mixinCursorPosition(), memberCursorPosition(), offsetToPosition(). Fixture tree/resource copying, SimpleTarget.class classpath install, jar --create Loom remapped staging, workspace URI conversion, @Mixin/member cursor positions, and UTF-16 offset conversion.
- Native replacement: `mcdev-core/tests/host_project.rs`, `mcdev-core/tests/project_legacy_stateful.rs`, `mcdev-core/tests/host_source_project_legacy.rs`, `mcdev-companion/tests/commands_legacy.rs`, `mcdev-companion/tests/fixtures/commands/replay.rs`, `mcdev-core/tests/fixtures/host_project/source_inputs.json`, `mcdev-core/tests/fixtures/host_project/class_manifest.json`. Companion command/handler, host project/source, project context, and navigation legacy tests using preserved fixture snapshots/class bytes.
- Native fixture/provenance files: `mcdev-core/tests/fixtures/resources/manifest.json` (19fa97d828e802b11a22418c20f8230061cde37551912431fe5ae2bd26c1e4c5), `mcdev-core/tests/fixtures/resources/source_manifest.json` (2c77ff92c7544304a982c97d30354d56d1f27854bf995b358049a1d3c5e56d2f), `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f), `mcdev-core/tests/fixtures/host_project/source_inputs.json` (497c376fb1da1d6770c26adcf66b629e91746b0a451158753bdda5128d11ea11), `mcdev-core/tests/fixtures/host_project/class_manifest.json` (e033ec1ade121ee8e921b13834ab1e00507b2f496affe90765d26cb4db4f2d5b), `mcdev-core/tests/fixtures/mixin_completion/project_files/SimpleTarget.class.bin` (4e3b9137c518f0449e9360875173d9dd516e48c58d0005a3a57c9ed5818a47f7).
- Captured callback/input files: `mcdev-core/tests/fixtures/mixin_completion/project_legacy_cases.json` (e7cc5696d24a225d16a3ce5384ad7aa2027f08f1d18eaeae8b7e3af028d335c3), `mcdev-core/tests/fixtures/mixin_completion/project_legacy_calls.jsonl` (f426c95760cc5623b5279aa5f52764ee4a681a97742b509cd8dfaad66ea2775e).
- Derived compiled provenance: `mcdev-core/tests/fixtures/resources/derived_class_provenance.json` (015203e6c6dc30847c6344af47c92eaa2955b7d9e46bf20a6c069dcbed0ad72f); runtimeRequiresJava=false.
- Existing proof logs: `build/rust-test-logs/companion-20261024-154945827/companion.stdout.log: 134 companion command/handler cases passed.`, `build/rust-test-logs/host-project-zero-capacity-final/affected.stdout.log: 60 host-project/classpath cases passed, including exact compiler fixture hashes and provider bytes.`, `build/rust-test-logs/mixin-navigation-final/affected.stdout.log`, `build/rust-test-logs/project-completion-final/affected.stdout.log`.
- Remaining: 155 file-scope candidate cases are an upper bound; 144 have confirmed direct/indirect reachability and 11 remain unconfirmed. Candidate statuses are ported 153, pending 2. Do not close this support role from file-level presence or a passing subset.
- Exactness record: all confirmed and unconfirmed IDs are in JSON under `callerAudit`; candidate IDs are explicitly upper bounds.
- Restriction: No production helper edit and no Java compiler restoration.

## Parent merge proposal

Keep `docs/rust-test-migration.json` authoritative for the 2604 original cases. If desired, add only a `supportAudit` reference keyed by source SHA, support status, candidate upper bound, confirmed direct/indirect IDs, and unconfirmed IDs. Do not add candidate counts to the master, and do not promote partial/unused/tombstone entries.

## Audit log

- Verified the absolute baseline directory exists in the target worktree.
- Read all 11 full baseline support bodies and recomputed SHA-256; all 11 match the migration ledger.
- Corrected CoreFixturePaths/CoreFixtureResourceLoader: both contain real fixture data/loader implementation and have zero original caller cases; their role is represented by the native resource catalog, not an empty tombstone.
- Corrected AwTestFixtures public data to VALID_AW, INTERMEDIARY_AW, mappingContext, classIndex, and the separate FakeAwMappingResolver object.
- Replaced file-level exact caller claims with confirmed direct/indirect same-file links plus explicit candidate upper-bound and unconfirmed ID sets.
- Connected native evidence to fixture manifests, captured callback inputs, derived SimpleTarget provenance, bytecode class provenance, and existing bounded passing logs.
- No full suite, subagent, commit, production/test edit, parent-ledger edit, or original-main write was performed.
