# AW/AT Rust port verification map

Authoritative scope: frozen `build/migration-baseline/current-20261023`, including its Kotlin test additions. All offsets in Rust are UTF-8 bytes; external LSP positions use the shared checked UTF-16 conversion.

Each row names an original `@Test` case and the Rust test/table whose assertions cover its behavior. Closely related cases are consolidated into tables; the Rust function count is not a count of legacy cases. Tests apply quickfix edits and reparse the resulting buffer.

## Preserved baseline boundaries

- AW accepts `accessWidener v2` and `accessible`, `extendable`, `mutable`, `natural`; v1 and transitive directives remain invalid. `extendable` is class-only, `mutable` field-only. No new AW grammar is introduced.
- AW owner resolution and insertion are mapping-aware; literal member names/descriptors for diagnostics/definitions/references retain the baseline behavior. AT owner text stays named; member insertion/resolution uses project-selected namespaces and exact descriptor ambiguity rules.
- AT descriptor slots offer no completion, matching baseline. Missing mappings retain named insertion fallback and the existing `at.srgMappingNotFound` code.
- AW member reference locations cover the entry, as in baseline; AT class references include class-only entries. Duplicate AW diagnostics cover every duplicate occurrence; AT diagnostics cover subsequent occurrences.
- Legacy malformed AT entry failures contain the word `modifier` and therefore use `at.invalidModifier`; the exposed `at.parseError` constant is retained without inventing an unreachable test.

## Intentional data-preservation corrections

- On `public |`, legacy `valueStartOffset` fell back to line start; using it for replacement could yield just `net.minecraft.client.MinecraftClient`. Rust inserts at the caret and yields `public net.minecraft.client.MinecraftClient`. The same check covers a missing member after the owner (`missing_slot_replacement_never_overwrites_modifier_or_owner`).
- For `  public\t<owner>\ttick # keep\r\n`, legacy descriptor/remap actions rebuilt the whole line, losing indentation/tabs/comment and potentially adding blank lines. Rust changes the member token only, yielding `  public\t<owner>\ttick()V # keep\r\n`. AW remap changes only changed tokens, retaining tabs, comments and line terminators (`at_actions_missing_descriptor_duplicate_generate_and_stale_check`, `add_aw_descriptor_fix_and_full_whitespace_remapping`).
- Byte offsets and actual token spans avoid the legacy single-space range approximation. Unicode/UTF-16/CRLF checks reject interior code-point/surrogate boundaries.

## Fixture provenance

`fixtures/awat-simple-target.class.bin` is the existing `mcdev-test-fixtures/src/fixture-java/com/example/target/SimpleTarget.java`, compiled once with JDK 21 `javac --release 8 -g:none`. The test suite consumes embedded class bytes through the Rust bytecode index; it does not launch Java or require a JDK/JVM. Original `fabric-aw-at` AW, AT, and Tiny mapping fixtures are included directly. Fixture generation log: `build/rust-test-logs/awat-fixture-20261023-221952130`.

Class SHA-256: `7f0fdaf1433dfbccf7178347f632df7afe096328087572b9ff7267d34885efc6`.
Source SHA-256: `58826b1d037f58bef6ecf5530be1c8d3ddc7de955ffa679a16511b74ef4f76d4`.

## Original cases → Rust assertions

### `aw/AccessWidenerCodeActionServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `offersRemoveDuplicateFix` | [`aw_actions_duplicate_descriptor_generate_apply_and_reparse`](awat.rs#L723) |
| `offersRemapNamespaceFix` | [`aw_namespace_remap_action_applies_and_reparses`](awat.rs#L784) |
| `applyRemoveDuplicateFixRemovesLine` | [`aw_actions_duplicate_descriptor_generate_apply_and_reparse`](awat.rs#L723) |
| `generateEntryFixUsesEditorFormatting` | [`aw_actions_duplicate_descriptor_generate_apply_and_reparse`](awat.rs#L723) |
| `applyGenerateEntryFixInsertsLine` | [`aw_actions_duplicate_descriptor_generate_apply_and_reparse`](awat.rs#L723) |
| `offersFixDescriptorForMismatch` | [`aw_actions_duplicate_descriptor_generate_apply_and_reparse`](awat.rs#L723) |
| `applyDescriptorFixReplacesWrongMethodDescriptor` | [`aw_actions_duplicate_descriptor_generate_apply_and_reparse`](awat.rs#L723) |

### `aw/AccessWidenerCompletionServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `completesDirectives` | [`aw_completion_keywords_kinds_namespaces`](awat.rs#L340) |
| `completesAllDirectives` | [`aw_completion_keywords_kinds_namespaces`](awat.rs#L340) |
| `completesKinds` | [`aw_completion_keywords_kinds_namespaces`](awat.rs#L340) |
| `mutableKindCompletionOnlyOffersField` | [`aw_completion_keywords_kinds_namespaces`](awat.rs#L340) |
| `extendableKindCompletionOnlyOffersClass` | [`aw_completion_keywords_kinds_namespaces`](awat.rs#L340) |
| `completesClasses` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `classCompletionShowsInternalNameDetail` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `completesMethods` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `completesFields` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `completesDescriptorsForMethods` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `completesHeaderNamespaces` | [`aw_completion_keywords_kinds_namespaces`](awat.rs#L340) |
| `mappingAwareOwnerInsertionUsesTargetNamespace` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `methodCompletionInsertsNameOnlyAtNameSlot` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `methodCompletionIncludesDescriptorAtDescriptorSlot` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `descriptorCompletionUsesValueKind` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `returnsEmptyForUnknownOwnerMembers` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |
| `completesOwnerFromPartialInternalClassPath` | [`aw_completion_owner_member_descriptor_display_and_mapping`](awat.rs#L372) |

### `aw/AccessWidenerDefinitionServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `resolvesOwnerToClassDefinition` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |
| `resolvesMethodNameToMethodDefinition` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |
| `methodDefinitionDoesNotFallbackWhenDescriptorMismatches` | [`overload_policy_exact_later_overload_missing_and_mismatch`](awat.rs#L597) |
| `resolvesFieldNameToFieldDefinition` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |
| `returnsEmptyOnDirectiveSlot` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |

### `aw/AccessWidenerDiagnosticsServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `validFileProducesNoErrors` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsInvalidDirective` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsInvalidKind` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsUnresolvedClass` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsUnresolvedMember` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsInvalidDescriptor` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsDescriptorMismatch` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsDuplicateEntry` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsMutableOnNonField` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsExtendableOnNonClass` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `detectsNamespaceMismatch` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `duplicateDiagnosticsIncludeLineMetadata` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `parseFailureUsesInvalidDirectiveCodeForBadDirectiveLine` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |

### `aw/AccessWidenerEditorTest.kt`

| Original case | Rust test/table |
|---|---|
| `formatsClassEntryDeterministically` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `formatsMethodEntryDeterministically` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `formatsFieldEntryDeterministically` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `insertEntryAppendsFormattedLine` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |
| `removeLineDeletesTargetEntry` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `entryKeyIgnoresClassMemberParts` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `entryKeyIncludesMemberParts` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |

### `aw/AccessWidenerParserTest.kt`

| Original case | Rust test/table |
|---|---|
| `parsesHeaderAndEntries` | [`aw_parser_header_entries_and_invalid_inputs`](awat.rs#L181) |
| `rejectsInvalidDescriptor` | [`aw_parser_header_entries_and_invalid_inputs`](awat.rs#L181) |

### `aw/AccessWidenerReferenceServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `findsClassReferencesInAccessWidenerFile` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |
| `findsMethodReferencesInAccessWidenerFile` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |
| `findsFieldReferencesInAccessWidenerFile` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |
| `returnsEmptyWhenNoMatches` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |

### `aw/AwContextExtractorTest.kt`

| Original case | Rust test/table |
|---|---|
| `extractsDirectiveSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `extractsKindSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `extractsOwnerSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `extractsMemberNameSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `extractsDescriptorSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `extractsHeaderNamespaceSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `parseFileNamespaceReadsHeader` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `toOffsetRoundTripsLineCharacter` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `blankEntryLineCompletesDirective` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `trailingWhitespaceAdvancesToTheNextSlot` | [`aw_context_all_slots_trailing_whitespace_and_replacement`](awat.rs#L253) |
| `extractUsesLineCharacter` | [`unicode_offsets_and_crlf_are_checked`](awat.rs#L321) |

### `at/AccessTransformerCodeActionServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `offersAddDescriptorFixForMissingDescriptor` | [`at_actions_missing_descriptor_duplicate_generate_and_stale_check`](awat.rs#L810) |
| `appliesAddDescriptorFix` | [`at_actions_missing_descriptor_duplicate_generate_and_stale_check`](awat.rs#L810) |
| `offersRemapFixForWrongNamespace` | [`at_remap_actions_exact_overload_and_missing_mapping`](awat.rs#L845) |
| `appliesRemapFixToSrgNames` | [`at_remap_actions_exact_overload_and_missing_mapping`](awat.rs#L845) |
| `remapActionIsNotOfferedForOverloadedMethodWithoutDescriptor` | [`at_remap_actions_exact_overload_and_missing_mapping`](awat.rs#L845) |
| `remapActionDoesNotUseFirstOverloadWhenDescriptorMismatches` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `remapActionUsesExactDescriptorWhenDescriptorIsPresent` | [`at_remap_actions_exact_overload_and_missing_mapping`](awat.rs#L845) |
| `remapActionUsesSingleCandidateOnlyWhenDescriptorIsOmitted` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `offersRemoveDuplicateFix` | [`at_actions_missing_descriptor_duplicate_generate_and_stale_check`](awat.rs#L810) |
| `appliesRemoveDuplicateFix` | [`at_actions_missing_descriptor_duplicate_generate_and_stale_check`](awat.rs#L810) |
| `generatesEntryFixFromContext` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |
| `appliesGenerateEntryFix` | [`at_actions_missing_descriptor_duplicate_generate_and_stale_check`](awat.rs#L810) |

### `at/AccessTransformerCompletionServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `completesAllModifiers` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `filtersModifiersByPrefix` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `completesClassesWithNamedFqnInsert` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `completesMethodsWithNamedInsertOnFabric` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `completesFieldsWithNamedInsertOnFabric` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `completesMethodsWithSrgInsertOnForge` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `completesFieldsWithSrgInsertOnForge` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `separatesDisplayLabelFromInsertText` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `returnsEmptyForDescriptorSlot` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |
| `returnsEmptyWhenOwnerMissingForMemberCompletion` | [`at_completion_modifiers_owners_mapped_members_and_empty_slots`](awat.rs#L428) |

### `at/AccessTransformerDefinitionServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `resolvesOwnerToClassDefinition` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |
| `resolvesFieldMemberToFieldDefinition` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |
| `resolvesMethodMemberToMethodDefinition` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |
| `methodDefinitionDoesNotFallbackWhenDescriptorMismatches` | [`overload_policy_exact_later_overload_missing_and_mismatch`](awat.rs#L597) |
| `returnsEmptyOnModifierSlot` | [`definitions_class_field_method_directive_and_wrong_namespace`](awat.rs#L627) |

### `at/AccessTransformerDiagnosticsServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `reportsInvalidModifier` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsUnresolvedClass` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `noUnresolvedForValidClassOnlyEntry` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsUnresolvedMember` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsMissingMethodDescriptor` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `noMissingDescriptorForFieldEntry` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsInvalidDescriptor` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsDuplicateEntry` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsWrongNamespaceForNamedMemberOnForge` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `acceptsSrgMemberOnForge` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsSrgMappingNotFoundWhenMethodMappingMissing` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `acceptsNamedMemberOnFabric` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `reportsInvalidDescriptorMismatch` | [`at_diagnostics_all_codes_and_missing_mapping`](awat.rs#L545) |
| `acceptsDescriptorMatchingLaterOverload` | [`overload_policy_exact_later_overload_missing_and_mismatch`](awat.rs#L597) |
| `reportsInvalidDescriptorWhenOverloadedMethodsExistButDescriptorMatchesNone` | [`overload_policy_exact_later_overload_missing_and_mismatch`](awat.rs#L597) |
| `reportsUnknownMethodOnlyWhenMethodNameItselfDoesNotExist` | [`overload_policy_exact_later_overload_missing_and_mismatch`](awat.rs#L597) |
| `reportsMissingMethodDescriptorWhenDescriptorIsOmittedAndOverloadsExist` | [`overload_policy_exact_later_overload_missing_and_mismatch`](awat.rs#L597) |

### `at/AccessTransformerEditorTest.kt`

| Original case | Rust test/table |
|---|---|
| `formatsClassEntry` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |
| `formatsFieldEntry` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |
| `formatsMethodEntryWithDescriptor` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |
| `appendEntryIsStable` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |
| `replaceLineNoOpWhenUnchanged` | [`editors_format_keys_insert_remove_replace_preserve_terminators`](awat.rs#L683) |
| `entryKeyIncludesDescriptor` | [`remaining_legacy_helper_and_action_cases`](awat.rs#L1131) |

### `at/AccessTransformerParserTest.kt`

| Original case | Rust test/table |
|---|---|
| `parsesClassFieldAndMethodEntries` | [`at_parser_all_modifiers_class_field_method_and_errors`](awat.rs#L219) |
| `rejectsInvalidModifier` | [`at_parser_all_modifiers_class_field_method_and_errors`](awat.rs#L219) |

### `at/AccessTransformerReferenceServiceTest.kt`

| Original case | Rust test/table |
|---|---|
| `findsClassOnlyReferences` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |
| `findsFieldReferences` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |
| `findsMethodReferences` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |
| `returnsEmptyWhenNoMatches` | [`references_crossfile_owner_member_descriptor_and_missing`](awat.rs#L880) |

### `at/AtContextExtractorTest.kt`

| Original case | Rust test/table |
|---|---|
| `detectsModifierSlot` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `detectsOwnerSlot` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `detectsMemberNameSlot` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `detectsMemberDescriptorSlot` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `detectsMemberSlotAfterOwnerOnTrailingCursor` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `detectsOwnerSlotAfterModifierOnTrailingCursor` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `ignoresCommentSuffix` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `parsesLineTokens` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |
| `returnsNullOutsideSource` | [`at_context_all_slots_trailing_comment_and_partial_descriptor`](awat.rs#L298) |

### `at/AtMemberInsertFormatterTest.kt`

| Original case | Rust test/table |
|---|---|
| `insertsNamedMethodOnFabric` | [`at_insert_formatter_mapping_missing_and_owner`](awat.rs#L467) |
| `insertsSrgMethodOnForge` | [`at_insert_formatter_mapping_missing_and_owner`](awat.rs#L467) |
| `reportsMissingMappingForUnknownMethod` | [`at_insert_formatter_mapping_missing_and_owner`](awat.rs#L467) |
| `formatsOwnerAsFqn` | [`at_insert_formatter_mapping_missing_and_owner`](awat.rs#L467) |

### `awat/AwAtBufferSupportTest.kt`

| Original case | Rust test/table |
|---|---|
| `detectsAccessWidenerByLanguageId` | [`routing_and_facade_completion_diagnostics_definitions_actions`](awat.rs#L917) |
| `detectsAccessTransformerByLanguageId` | [`routing_and_facade_completion_diagnostics_definitions_actions`](awat.rs#L917) |
| `detectsAccessWidenerByExtension` | [`routing_and_facade_completion_diagnostics_definitions_actions`](awat.rs#L917) |
| `detectsAccessTransformerByExtension` | [`routing_and_facade_completion_diagnostics_definitions_actions`](awat.rs#L917) |
| `returnsNullForJavaBuffers` | [`routing_and_facade_completion_diagnostics_definitions_actions`](awat.rs#L917) |

### `awat/AwAtServiceFacadeE2ETest.kt`

| Original case | Rust test/table |
|---|---|
| `completesAccessWidenerDirectives` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessWidenerKinds` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessWidenerClassesFromFixture` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessWidenerMethodsFromFixture` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessWidenerFieldsFromFixture` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessTransformerModifiers` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessTransformerClassesFromFixture` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessTransformerMembersWithIntermediaryNames` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessWidenerMethodDescriptorSlotFromFixture` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `completesAccessTransformerFieldWithIntermediaryInsertText` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `codeActionsOffersRemoveDuplicateFixForAccessWidener` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `codeActionsOffersAddDescriptorFixForMissingMethodDescriptor` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `diagnosesUnresolvedAccessWidenerClass` | [`aw_diagnostics_all_codes_partial_lines_and_duplicates`](awat.rs#L489) |
| `diagnosesValidFixtureAccessWidenerWithoutUnresolvedTargets` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |
| `diagnosesValidFixtureAccessTransformerWithoutUnresolvedTargets` | [`fixture_facade_bytecode_index_mapped_completions_and_diagnostics`](awat.rs#L986) |

Total mapped original cases: **155** across **19** Kotlin suites.

Additional Rust-only regression tables cover invalid/missing input, absent mappings, strict overloads, stale member fixes, descriptor addition, actual bytecode-index/static-member integration, Unicode, replacement boundaries and exact comment/whitespace/CRLF preservation.

Host/editor IPC lifecycle, Minecraft runtime integration, rename orchestration and full workspace suite are outside this bounded domain task and are not claimed by these tests.

## Strict complete: exact original cases and whole results

All **155 original cases across 19 suites** now execute independently in `awat_legacy.rs`. The earlier table above remains a supplementary behavior map; it is not the evidence for literal case coverage.

The shared one-time `mixin_source/Capture.kt` and `Run.kt` harness invoked every frozen original test method and its original assertions unchanged. Disposable AW/AT service wrappers recorded **549 complete public outputs** with exact arguments and observed class/mapping facts. Rust replays all 549 outputs; each row below identifies its own native test. This includes **17 final `AwAtServiceFacade` outputs** (10 complete, 5 diagnose, 2 codeActions), not only underlying calls.

The committed `legacy_calls.jsonl` and `legacy_cases.json` are raw oracle records. Cargo reads them directly and launches no JVM. `generate_capture.py` only regenerates fixtures on demand, uses the shared finite-deadline runtime runner, and never modifies frozen sources. Full one-time logs: `build/awat-oracle/{compile,capture}.{stdout,stderr}.log`.

**27 explicitly approved observations in 17 cases** use `intentional_deltas.json`: every selector includes exact case/API/arguments, field, original/new value, reason and an executed range/document regression. No case is skipped. Descriptor/remap token edits are applied alongside the old edits to the same source and must produce the same whole document; title, kind, type, URI and metadata remain compared. Two duplicate-removal cases adapt their original assertion on full-document replacement `newText` to count entries in the applied, reparsed document instead (bug-corrected adaptation). Newline differences and corrected token ranges are exact expectations, not ignored fields.

The only schema conversion is snake/camel DTO field names and enum spelling; arbitrary metadata dictionary keys are preserved exactly, including colliding `owner_name`/`ownerName` keys. Kotlin fix subtypes are reconstructed from their unique retained kind and workspace-edit shape and compared against the Rust type tag; type/kind/range metadata is not removed. The manifest check compares the exact frozen 155-case ID set against independently executable native function calls and rejects duplicate IDs/functions.

Strict replay also exposed and corrected genuine compatibility omissions: overlong columns now clamp to the line end through context/facade/navigation APIs without splitting UTF-16 surrogates; AT descriptor-mismatch metadata matches legacy; remapping avoids an unnecessary fields lookup once the exact method is found. Original 26 supplementary tests are retained, with one additional Unicode/CRLF/clamping regression.

| Original path and method | Native independent test | Complete outputs | Explicit adaptations |
|---|---|---:|---:|
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::applyDescriptorFixReplacesWrongMethodDescriptor` | [`access_widener_code_action_service_test_apply_descriptor_fix_replaces_wrong_method_descriptor`](awat_legacy.rs#L10) | 8 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::applyGenerateEntryFixInsertsLine` | [`access_widener_code_action_service_test_apply_generate_entry_fix_inserts_line`](awat_legacy.rs#L15) | 1 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::applyRemoveDuplicateFixRemovesLine` | [`access_widener_code_action_service_test_apply_remove_duplicate_fix_removes_line`](awat_legacy.rs#L20) | 2 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::generateEntryFixUsesEditorFormatting` | [`access_widener_code_action_service_test_generate_entry_fix_uses_editor_formatting`](awat_legacy.rs#L25) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::offersFixDescriptorForMismatch` | [`access_widener_code_action_service_test_offers_fix_descriptor_for_mismatch`](awat_legacy.rs#L30) | 7 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::offersRemapNamespaceFix` | [`access_widener_code_action_service_test_offers_remap_namespace_fix`](awat_legacy.rs#L35) | 8 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCodeActionServiceTest.kt::offersRemoveDuplicateFix` | [`access_widener_code_action_service_test_offers_remove_duplicate_fix`](awat_legacy.rs#L40) | 11 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::classCompletionShowsInternalNameDetail` | [`access_widener_completion_service_test_class_completion_shows_internal_name_detail`](awat_legacy.rs#L45) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesAllDirectives` | [`access_widener_completion_service_test_completes_all_directives`](awat_legacy.rs#L50) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesClasses` | [`access_widener_completion_service_test_completes_classes`](awat_legacy.rs#L55) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesDescriptorsForMethods` | [`access_widener_completion_service_test_completes_descriptors_for_methods`](awat_legacy.rs#L60) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesDirectives` | [`access_widener_completion_service_test_completes_directives`](awat_legacy.rs#L65) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesFields` | [`access_widener_completion_service_test_completes_fields`](awat_legacy.rs#L70) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesHeaderNamespaces` | [`access_widener_completion_service_test_completes_header_namespaces`](awat_legacy.rs#L75) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesKinds` | [`access_widener_completion_service_test_completes_kinds`](awat_legacy.rs#L80) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesMethods` | [`access_widener_completion_service_test_completes_methods`](awat_legacy.rs#L85) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::completesOwnerFromPartialInternalClassPath` | [`access_widener_completion_service_test_completes_owner_from_partial_internal_class_path`](awat_legacy.rs#L90) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::descriptorCompletionUsesValueKind` | [`access_widener_completion_service_test_descriptor_completion_uses_value_kind`](awat_legacy.rs#L95) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::extendableKindCompletionOnlyOffersClass` | [`access_widener_completion_service_test_extendable_kind_completion_only_offers_class`](awat_legacy.rs#L100) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::mappingAwareOwnerInsertionUsesTargetNamespace` | [`access_widener_completion_service_test_mapping_aware_owner_insertion_uses_target_namespace`](awat_legacy.rs#L105) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::methodCompletionIncludesDescriptorAtDescriptorSlot` | [`access_widener_completion_service_test_method_completion_includes_descriptor_at_descriptor_slot`](awat_legacy.rs#L110) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::methodCompletionInsertsNameOnlyAtNameSlot` | [`access_widener_completion_service_test_method_completion_inserts_name_only_at_name_slot`](awat_legacy.rs#L116) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::mutableKindCompletionOnlyOffersField` | [`access_widener_completion_service_test_mutable_kind_completion_only_offers_field`](awat_legacy.rs#L121) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerCompletionServiceTest.kt::returnsEmptyForUnknownOwnerMembers` | [`access_widener_completion_service_test_returns_empty_for_unknown_owner_members`](awat_legacy.rs#L126) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::methodDefinitionDoesNotFallbackWhenDescriptorMismatches` | [`access_widener_definition_service_test_method_definition_does_not_fallback_when_descriptor_mismatches`](awat_legacy.rs#L131) | 7 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::resolvesFieldNameToFieldDefinition` | [`access_widener_definition_service_test_resolves_field_name_to_field_definition`](awat_legacy.rs#L137) | 7 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::resolvesMethodNameToMethodDefinition` | [`access_widener_definition_service_test_resolves_method_name_to_method_definition`](awat_legacy.rs#L142) | 7 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::resolvesOwnerToClassDefinition` | [`access_widener_definition_service_test_resolves_owner_to_class_definition`](awat_legacy.rs#L147) | 7 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDefinitionServiceTest.kt::returnsEmptyOnDirectiveSlot` | [`access_widener_definition_service_test_returns_empty_on_directive_slot`](awat_legacy.rs#L152) | 7 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsDescriptorMismatch` | [`access_widener_diagnostics_service_test_detects_descriptor_mismatch`](awat_legacy.rs#L157) | 5 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsDuplicateEntry` | [`access_widener_diagnostics_service_test_detects_duplicate_entry`](awat_legacy.rs#L162) | 10 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsExtendableOnNonClass` | [`access_widener_diagnostics_service_test_detects_extendable_on_non_class`](awat_legacy.rs#L167) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsInvalidDescriptor` | [`access_widener_diagnostics_service_test_detects_invalid_descriptor`](awat_legacy.rs#L172) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsInvalidDirective` | [`access_widener_diagnostics_service_test_detects_invalid_directive`](awat_legacy.rs#L177) | 6 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsInvalidKind` | [`access_widener_diagnostics_service_test_detects_invalid_kind`](awat_legacy.rs#L182) | 6 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsMutableOnNonField` | [`access_widener_diagnostics_service_test_detects_mutable_on_non_field`](awat_legacy.rs#L187) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsNamespaceMismatch` | [`access_widener_diagnostics_service_test_detects_namespace_mismatch`](awat_legacy.rs#L192) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsUnresolvedClass` | [`access_widener_diagnostics_service_test_detects_unresolved_class`](awat_legacy.rs#L197) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::detectsUnresolvedMember` | [`access_widener_diagnostics_service_test_detects_unresolved_member`](awat_legacy.rs#L202) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::duplicateDiagnosticsIncludeLineMetadata` | [`access_widener_diagnostics_service_test_duplicate_diagnostics_include_line_metadata`](awat_legacy.rs#L207) | 10 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::parseFailureUsesInvalidDirectiveCodeForBadDirectiveLine` | [`access_widener_diagnostics_service_test_parse_failure_uses_invalid_directive_code_for_bad_directive_line`](awat_legacy.rs#L212) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerDiagnosticsServiceTest.kt::validFileProducesNoErrors` | [`access_widener_diagnostics_service_test_valid_file_produces_no_errors`](awat_legacy.rs#L218) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::entryKeyIgnoresClassMemberParts` | [`access_widener_editor_test_entry_key_ignores_class_member_parts`](awat_legacy.rs#L223) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::entryKeyIncludesMemberParts` | [`access_widener_editor_test_entry_key_includes_member_parts`](awat_legacy.rs#L228) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::formatsClassEntryDeterministically` | [`access_widener_editor_test_formats_class_entry_deterministically`](awat_legacy.rs#L233) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::formatsFieldEntryDeterministically` | [`access_widener_editor_test_formats_field_entry_deterministically`](awat_legacy.rs#L238) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::formatsMethodEntryDeterministically` | [`access_widener_editor_test_formats_method_entry_deterministically`](awat_legacy.rs#L243) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::insertEntryAppendsFormattedLine` | [`access_widener_editor_test_insert_entry_appends_formatted_line`](awat_legacy.rs#L248) | 3 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerEditorTest.kt::removeLineDeletesTargetEntry` | [`access_widener_editor_test_remove_line_deletes_target_entry`](awat_legacy.rs#L253) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerParserTest.kt::parsesHeaderAndEntries` | [`access_widener_parser_test_parses_header_and_entries`](awat_legacy.rs#L258) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerParserTest.kt::rejectsInvalidDescriptor` | [`access_widener_parser_test_rejects_invalid_descriptor`](awat_legacy.rs#L263) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::findsClassReferencesInAccessWidenerFile` | [`access_widener_reference_service_test_finds_class_references_in_access_widener_file`](awat_legacy.rs#L268) | 9 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::findsFieldReferencesInAccessWidenerFile` | [`access_widener_reference_service_test_finds_field_references_in_access_widener_file`](awat_legacy.rs#L273) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::findsMethodReferencesInAccessWidenerFile` | [`access_widener_reference_service_test_finds_method_references_in_access_widener_file`](awat_legacy.rs#L278) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AccessWidenerReferenceServiceTest.kt::returnsEmptyWhenNoMatches` | [`access_widener_reference_service_test_returns_empty_when_no_matches`](awat_legacy.rs#L283) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::blankEntryLineCompletesDirective` | [`aw_context_extractor_test_blank_entry_line_completes_directive`](awat_legacy.rs#L288) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractUsesLineCharacter` | [`aw_context_extractor_test_extract_uses_line_character`](awat_legacy.rs#L293) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsDescriptorSlot` | [`aw_context_extractor_test_extracts_descriptor_slot`](awat_legacy.rs#L298) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsDirectiveSlot` | [`aw_context_extractor_test_extracts_directive_slot`](awat_legacy.rs#L303) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsHeaderNamespaceSlot` | [`aw_context_extractor_test_extracts_header_namespace_slot`](awat_legacy.rs#L308) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsKindSlot` | [`aw_context_extractor_test_extracts_kind_slot`](awat_legacy.rs#L313) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsMemberNameSlot` | [`aw_context_extractor_test_extracts_member_name_slot`](awat_legacy.rs#L318) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::extractsOwnerSlot` | [`aw_context_extractor_test_extracts_owner_slot`](awat_legacy.rs#L323) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::parseFileNamespaceReadsHeader` | [`aw_context_extractor_test_parse_file_namespace_reads_header`](awat_legacy.rs#L328) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::toOffsetRoundTripsLineCharacter` | [`aw_context_extractor_test_to_offset_round_trips_line_character`](awat_legacy.rs#L333) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/aw/AwContextExtractorTest.kt::trailingWhitespaceAdvancesToTheNextSlot` | [`aw_context_extractor_test_trailing_whitespace_advances_to_the_next_slot`](awat_legacy.rs#L338) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesAddDescriptorFix` | [`access_transformer_code_action_service_test_applies_add_descriptor_fix`](awat_legacy.rs#L343) | 3 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesGenerateEntryFix` | [`access_transformer_code_action_service_test_applies_generate_entry_fix`](awat_legacy.rs#L348) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesRemapFixToSrgNames` | [`access_transformer_code_action_service_test_applies_remap_fix_to_srg_names`](awat_legacy.rs#L353) | 6 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::appliesRemoveDuplicateFix` | [`access_transformer_code_action_service_test_applies_remove_duplicate_fix`](awat_legacy.rs#L358) | 2 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::generatesEntryFixFromContext` | [`access_transformer_code_action_service_test_generates_entry_fix_from_context`](awat_legacy.rs#L363) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::offersAddDescriptorFixForMissingDescriptor` | [`access_transformer_code_action_service_test_offers_add_descriptor_fix_for_missing_descriptor`](awat_legacy.rs#L368) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::offersRemapFixForWrongNamespace` | [`access_transformer_code_action_service_test_offers_remap_fix_for_wrong_namespace`](awat_legacy.rs#L373) | 8 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::offersRemoveDuplicateFix` | [`access_transformer_code_action_service_test_offers_remove_duplicate_fix`](awat_legacy.rs#L378) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionDoesNotUseFirstOverloadWhenDescriptorMismatches` | [`access_transformer_code_action_service_test_remap_action_does_not_use_first_overload_when_descriptor_mismatches`](awat_legacy.rs#L383) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionIsNotOfferedForOverloadedMethodWithoutDescriptor` | [`access_transformer_code_action_service_test_remap_action_is_not_offered_for_overloaded_method_without_descriptor`](awat_legacy.rs#L389) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionUsesExactDescriptorWhenDescriptorIsPresent` | [`access_transformer_code_action_service_test_remap_action_uses_exact_descriptor_when_descriptor_is_present`](awat_legacy.rs#L395) | 6 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCodeActionServiceTest.kt::remapActionUsesSingleCandidateOnlyWhenDescriptorIsOmitted` | [`access_transformer_code_action_service_test_remap_action_uses_single_candidate_only_when_descriptor_is_omitted`](awat_legacy.rs#L401) | 6 | 1 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesAllModifiers` | [`access_transformer_completion_service_test_completes_all_modifiers`](awat_legacy.rs#L407) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesClassesWithNamedFqnInsert` | [`access_transformer_completion_service_test_completes_classes_with_named_fqn_insert`](awat_legacy.rs#L412) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesFieldsWithNamedInsertOnFabric` | [`access_transformer_completion_service_test_completes_fields_with_named_insert_on_fabric`](awat_legacy.rs#L417) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesFieldsWithSrgInsertOnForge` | [`access_transformer_completion_service_test_completes_fields_with_srg_insert_on_forge`](awat_legacy.rs#L422) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesMethodsWithNamedInsertOnFabric` | [`access_transformer_completion_service_test_completes_methods_with_named_insert_on_fabric`](awat_legacy.rs#L427) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::completesMethodsWithSrgInsertOnForge` | [`access_transformer_completion_service_test_completes_methods_with_srg_insert_on_forge`](awat_legacy.rs#L432) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::filtersModifiersByPrefix` | [`access_transformer_completion_service_test_filters_modifiers_by_prefix`](awat_legacy.rs#L437) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::returnsEmptyForDescriptorSlot` | [`access_transformer_completion_service_test_returns_empty_for_descriptor_slot`](awat_legacy.rs#L442) | 4 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::returnsEmptyWhenOwnerMissingForMemberCompletion` | [`access_transformer_completion_service_test_returns_empty_when_owner_missing_for_member_completion`](awat_legacy.rs#L447) | 4 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerCompletionServiceTest.kt::separatesDisplayLabelFromInsertText` | [`access_transformer_completion_service_test_separates_display_label_from_insert_text`](awat_legacy.rs#L453) | 10 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::methodDefinitionDoesNotFallbackWhenDescriptorMismatches` | [`access_transformer_definition_service_test_method_definition_does_not_fallback_when_descriptor_mismatches`](awat_legacy.rs#L458) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::resolvesFieldMemberToFieldDefinition` | [`access_transformer_definition_service_test_resolves_field_member_to_field_definition`](awat_legacy.rs#L464) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::resolvesMethodMemberToMethodDefinition` | [`access_transformer_definition_service_test_resolves_method_member_to_method_definition`](awat_legacy.rs#L469) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::resolvesOwnerToClassDefinition` | [`access_transformer_definition_service_test_resolves_owner_to_class_definition`](awat_legacy.rs#L474) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDefinitionServiceTest.kt::returnsEmptyOnModifierSlot` | [`access_transformer_definition_service_test_returns_empty_on_modifier_slot`](awat_legacy.rs#L479) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::acceptsDescriptorMatchingLaterOverload` | [`access_transformer_diagnostics_service_test_accepts_descriptor_matching_later_overload`](awat_legacy.rs#L484) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::acceptsNamedMemberOnFabric` | [`access_transformer_diagnostics_service_test_accepts_named_member_on_fabric`](awat_legacy.rs#L489) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::acceptsSrgMemberOnForge` | [`access_transformer_diagnostics_service_test_accepts_srg_member_on_forge`](awat_legacy.rs#L494) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::noMissingDescriptorForFieldEntry` | [`access_transformer_diagnostics_service_test_no_missing_descriptor_for_field_entry`](awat_legacy.rs#L499) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::noUnresolvedForValidClassOnlyEntry` | [`access_transformer_diagnostics_service_test_no_unresolved_for_valid_class_only_entry`](awat_legacy.rs#L504) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsDuplicateEntry` | [`access_transformer_diagnostics_service_test_reports_duplicate_entry`](awat_legacy.rs#L509) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidDescriptor` | [`access_transformer_diagnostics_service_test_reports_invalid_descriptor`](awat_legacy.rs#L514) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidDescriptorMismatch` | [`access_transformer_diagnostics_service_test_reports_invalid_descriptor_mismatch`](awat_legacy.rs#L519) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidDescriptorWhenOverloadedMethodsExistButDescriptorMatchesNone` | [`access_transformer_diagnostics_service_test_reports_invalid_descriptor_when_overloaded_methods_exist_but_descriptor_matches_none`](awat_legacy.rs#L524) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsInvalidModifier` | [`access_transformer_diagnostics_service_test_reports_invalid_modifier`](awat_legacy.rs#L530) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsMissingMethodDescriptor` | [`access_transformer_diagnostics_service_test_reports_missing_method_descriptor`](awat_legacy.rs#L535) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsMissingMethodDescriptorWhenDescriptorIsOmittedAndOverloadsExist` | [`access_transformer_diagnostics_service_test_reports_missing_method_descriptor_when_descriptor_is_omitted_and_overloads_exist`](awat_legacy.rs#L540) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsSrgMappingNotFoundWhenMethodMappingMissing` | [`access_transformer_diagnostics_service_test_reports_srg_mapping_not_found_when_method_mapping_missing`](awat_legacy.rs#L546) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsUnknownMethodOnlyWhenMethodNameItselfDoesNotExist` | [`access_transformer_diagnostics_service_test_reports_unknown_method_only_when_method_name_itself_does_not_exist`](awat_legacy.rs#L552) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsUnresolvedClass` | [`access_transformer_diagnostics_service_test_reports_unresolved_class`](awat_legacy.rs#L558) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsUnresolvedMember` | [`access_transformer_diagnostics_service_test_reports_unresolved_member`](awat_legacy.rs#L563) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerDiagnosticsServiceTest.kt::reportsWrongNamespaceForNamedMemberOnForge` | [`access_transformer_diagnostics_service_test_reports_wrong_namespace_for_named_member_on_forge`](awat_legacy.rs#L568) | 4 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::appendEntryIsStable` | [`access_transformer_editor_test_append_entry_is_stable`](awat_legacy.rs#L573) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::entryKeyIncludesDescriptor` | [`access_transformer_editor_test_entry_key_includes_descriptor`](awat_legacy.rs#L578) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::formatsClassEntry` | [`access_transformer_editor_test_formats_class_entry`](awat_legacy.rs#L583) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::formatsFieldEntry` | [`access_transformer_editor_test_formats_field_entry`](awat_legacy.rs#L588) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::formatsMethodEntryWithDescriptor` | [`access_transformer_editor_test_formats_method_entry_with_descriptor`](awat_legacy.rs#L593) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerEditorTest.kt::replaceLineNoOpWhenUnchanged` | [`access_transformer_editor_test_replace_line_no_op_when_unchanged`](awat_legacy.rs#L598) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerParserTest.kt::parsesClassFieldAndMethodEntries` | [`access_transformer_parser_test_parses_class_field_and_method_entries`](awat_legacy.rs#L603) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerParserTest.kt::rejectsInvalidModifier` | [`access_transformer_parser_test_rejects_invalid_modifier`](awat_legacy.rs#L608) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::findsClassOnlyReferences` | [`access_transformer_reference_service_test_finds_class_only_references`](awat_legacy.rs#L613) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::findsFieldReferences` | [`access_transformer_reference_service_test_finds_field_references`](awat_legacy.rs#L618) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::findsMethodReferences` | [`access_transformer_reference_service_test_finds_method_references`](awat_legacy.rs#L623) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AccessTransformerReferenceServiceTest.kt::returnsEmptyWhenNoMatches` | [`access_transformer_reference_service_test_returns_empty_when_no_matches`](awat_legacy.rs#L628) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsMemberDescriptorSlot` | [`at_context_extractor_test_detects_member_descriptor_slot`](awat_legacy.rs#L633) | 3 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsMemberNameSlot` | [`at_context_extractor_test_detects_member_name_slot`](awat_legacy.rs#L638) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsMemberSlotAfterOwnerOnTrailingCursor` | [`at_context_extractor_test_detects_member_slot_after_owner_on_trailing_cursor`](awat_legacy.rs#L643) | 3 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsModifierSlot` | [`at_context_extractor_test_detects_modifier_slot`](awat_legacy.rs#L648) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsOwnerSlot` | [`at_context_extractor_test_detects_owner_slot`](awat_legacy.rs#L653) | 3 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::detectsOwnerSlotAfterModifierOnTrailingCursor` | [`at_context_extractor_test_detects_owner_slot_after_modifier_on_trailing_cursor`](awat_legacy.rs#L658) | 3 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::ignoresCommentSuffix` | [`at_context_extractor_test_ignores_comment_suffix`](awat_legacy.rs#L663) | 3 | 2 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::parsesLineTokens` | [`at_context_extractor_test_parses_line_tokens`](awat_legacy.rs#L668) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtContextExtractorTest.kt::returnsNullOutsideSource` | [`at_context_extractor_test_returns_null_outside_source`](awat_legacy.rs#L673) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::formatsOwnerAsFqn` | [`at_member_insert_formatter_test_formats_owner_as_fqn`](awat_legacy.rs#L678) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::insertsNamedMethodOnFabric` | [`at_member_insert_formatter_test_inserts_named_method_on_fabric`](awat_legacy.rs#L683) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::insertsSrgMethodOnForge` | [`at_member_insert_formatter_test_inserts_srg_method_on_forge`](awat_legacy.rs#L688) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/at/AtMemberInsertFormatterTest.kt::reportsMissingMappingForUnknownMethod` | [`at_member_insert_formatter_test_reports_missing_mapping_for_unknown_method`](awat_legacy.rs#L693) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessTransformerByExtension` | [`aw_at_buffer_support_test_detects_access_transformer_by_extension`](awat_legacy.rs#L698) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessTransformerByLanguageId` | [`aw_at_buffer_support_test_detects_access_transformer_by_language_id`](awat_legacy.rs#L703) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessWidenerByExtension` | [`aw_at_buffer_support_test_detects_access_widener_by_extension`](awat_legacy.rs#L708) | 2 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::detectsAccessWidenerByLanguageId` | [`aw_at_buffer_support_test_detects_access_widener_by_language_id`](awat_legacy.rs#L713) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtBufferSupportTest.kt::returnsNullForJavaBuffers` | [`aw_at_buffer_support_test_returns_null_for_java_buffers`](awat_legacy.rs#L718) | 1 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::codeActionsOffersAddDescriptorFixForMissingMethodDescriptor` | [`aw_at_service_facade_e2_e_test_code_actions_offers_add_descriptor_fix_for_missing_method_descriptor`](awat_legacy.rs#L723) | 6 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::codeActionsOffersRemoveDuplicateFixForAccessWidener` | [`aw_at_service_facade_e2_e_test_code_actions_offers_remove_duplicate_fix_for_access_widener`](awat_legacy.rs#L729) | 13 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerClassesFromFixture` | [`aw_at_service_facade_e2_e_test_completes_access_transformer_classes_from_fixture`](awat_legacy.rs#L734) | 6 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerFieldWithIntermediaryInsertText` | [`aw_at_service_facade_e2_e_test_completes_access_transformer_field_with_intermediary_insert_text`](awat_legacy.rs#L739) | 6 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerMembersWithIntermediaryNames` | [`aw_at_service_facade_e2_e_test_completes_access_transformer_members_with_intermediary_names`](awat_legacy.rs#L745) | 7 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessTransformerModifiers` | [`aw_at_service_facade_e2_e_test_completes_access_transformer_modifiers`](awat_legacy.rs#L750) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerClassesFromFixture` | [`aw_at_service_facade_e2_e_test_completes_access_widener_classes_from_fixture`](awat_legacy.rs#L755) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerDirectives` | [`aw_at_service_facade_e2_e_test_completes_access_widener_directives`](awat_legacy.rs#L760) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerFieldsFromFixture` | [`aw_at_service_facade_e2_e_test_completes_access_widener_fields_from_fixture`](awat_legacy.rs#L765) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerKinds` | [`aw_at_service_facade_e2_e_test_completes_access_widener_kinds`](awat_legacy.rs#L770) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerMethodDescriptorSlotFromFixture` | [`aw_at_service_facade_e2_e_test_completes_access_widener_method_descriptor_slot_from_fixture`](awat_legacy.rs#L775) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::completesAccessWidenerMethodsFromFixture` | [`aw_at_service_facade_e2_e_test_completes_access_widener_methods_from_fixture`](awat_legacy.rs#L780) | 5 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::diagnosesUnresolvedAccessWidenerClass` | [`aw_at_service_facade_e2_e_test_diagnoses_unresolved_access_widener_class`](awat_legacy.rs#L785) | 6 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::diagnosesValidFixtureAccessTransformerWithoutUnresolvedTargets` | [`aw_at_service_facade_e2_e_test_diagnoses_valid_fixture_access_transformer_without_unresolved_targets`](awat_legacy.rs#L790) | 8 | 0 |
| `mcdev-core/src/test/kotlin/io/github/mcdev/core/awat/AwAtServiceFacadeE2ETest.kt::diagnosesValidFixtureAccessWidenerWithoutUnresolvedTargets` | [`aw_at_service_facade_e2_e_test_diagnoses_valid_fixture_access_widener_without_unresolved_targets`](awat_legacy.rs#L796) | 6 | 0 |

Fixture SHA-256:
- `legacy_calls.jsonl`: `c584d77d1ab8ff43624e8f4ba989cb090478bb5710f47dcef71da27b1c7edfb9`
- `legacy_cases.json`: `1ce6ccab8f6e5a58685d75549c4817291aa05fe1fe44396100d5eba48903063f`
- `intentional_deltas.json`: `c466bcd8de438c0cdef9785788aa7a7c8153de0ee52cd454b4652e7c78293705`

Verification: `cargo test -p mcdev-core --test awat --test awat_legacy --no-fail-fast`, through the shared compact runner with a finite 180-second deadline: **27/27 supplementary tests and 157/157 strict tests passed**, exit 0, no timeout, 42.3 seconds. Complete logs: `build/rust-test-logs/awat-strict-20261024-141117454/awat.{stdout,stderr}.log`. Full workspace/host/editor IPC execution remains the parent task’s responsibility.
