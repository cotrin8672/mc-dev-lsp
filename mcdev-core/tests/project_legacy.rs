#[path = "fixtures/mixin_completion/project_replay.rs"]
mod legacy_replay;
#[test]
fn aw_at_discovery_service_test_discovers_access_transformer_files_in_tree() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::discoversAccessTransformerFilesInTree");
}
#[test]
fn aw_at_discovery_service_test_discovers_access_widener_files_in_tree() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::discoversAccessWidenerFilesInTree");
}
#[test]
fn aw_at_discovery_service_test_excludes_build_directory_aw_at_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::excludesBuildDirectoryAwAtFiles");
}
#[test]
fn aw_at_discovery_service_test_excludes_gradle_directory_aw_at_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::excludesGradleDirectoryAwAtFiles");
}
#[test]
fn aw_at_discovery_service_test_excludes_node_modules_directory_aw_at_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::excludesNodeModulesDirectoryAwAtFiles");
}
#[test]
fn aw_at_discovery_service_test_identifies_access_transformer_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::identifiesAccessTransformerFiles");
}
#[test]
fn aw_at_discovery_service_test_identifies_access_widener_extensions() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::identifiesAccessWidenerExtensions");
}
#[test]
fn aw_at_discovery_service_test_parses_invalid_access_transformer_without_parsed_model() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::parsesInvalidAccessTransformerWithoutParsedModel");
}
#[test]
fn aw_at_discovery_service_test_parses_invalid_access_widener_without_parsed_model() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::parsesInvalidAccessWidenerWithoutParsedModel");
}
#[test]
fn aw_at_discovery_service_test_parses_valid_access_transformer() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::parsesValidAccessTransformer");
}
#[test]
fn aw_at_discovery_service_test_parses_valid_access_widener() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/AwAtDiscoveryServiceTest.kt::parsesValidAccessWidener");
}
#[test]
fn info_service_test_format_joins_lines_with_newlines() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatJoinsLinesWithNewlines");
}
#[test]
fn info_service_test_formats_complete_fabric_project_info() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatsCompleteFabricProjectInfo");
}
#[test]
fn info_service_test_formats_discovered_mapping_source_and_source_set_count() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatsDiscoveredMappingSourceAndSourceSetCount");
}
#[test]
fn info_service_test_formats_index_states() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatsIndexStates");
}
#[test]
fn info_service_test_formats_partial_context_with_no_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatsPartialContextWithNoMappings");
}
#[test]
fn info_service_test_formats_partial_mappings_as_partial_status() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatsPartialMappingsAsPartialStatus");
}
#[test]
fn info_service_test_formats_unknown_platform() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::formatsUnknownPlatform");
}
#[test]
fn info_service_test_uses_classpath_minecraft_jars_when_context_list_empty() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/InfoServiceTest.kt::usesClasspathMinecraftJarsWhenContextListEmpty");
}
#[test]
fn mapping_discovery_service_test_build_mapping_context_from_explicit_file_list() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::buildMappingContextFromExplicitFileList");
}
#[test]
fn mapping_discovery_service_test_builds_fabric_mapping_context_from_tiny_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::buildsFabricMappingContextFromTinyFile");
}
#[test]
fn mapping_discovery_service_test_builds_forge_mapping_context_from_srg_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::buildsForgeMappingContextFromSrgFile");
}
#[test]
fn mapping_discovery_service_test_discovers_srg_mapping_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::discoversSrgMappingFiles");
}
#[test]
fn mapping_discovery_service_test_discovers_tiny_mapping_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::discoversTinyMappingFiles");
}
#[test]
fn mapping_discovery_service_test_empty_mapping_context_when_no_files_found() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::emptyMappingContextWhenNoFilesFound");
}
#[test]
fn mapping_discovery_service_test_excludes_build_directory_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::excludesBuildDirectoryMappings");
}
#[test]
fn mapping_discovery_service_test_excludes_unrelated_gradle_build_and_node_modules_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::excludesUnrelatedGradleBuildAndNodeModulesMappings");
}
#[test]
fn mapping_discovery_service_test_includes_build_loom_cache_and_forge_tmp_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::includesBuildLoomCacheAndForgeTmpMappings");
}
#[test]
fn mapping_discovery_service_test_includes_gradle_fabric_loom_cache_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::includesGradleFabricLoomCacheMappings");
}
#[test]
fn mapping_discovery_service_test_includes_gradle_loom_cache_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::includesGradleLoomCacheMappings");
}
#[test]
fn mapping_discovery_service_test_includes_gradle_module_cache_mappings() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::includesGradleModuleCacheMappings");
}
#[test]
fn mapping_discovery_service_test_includes_nested_whitelist_mapping_paths() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::includesNestedWhitelistMappingPaths");
}
#[test]
fn mapping_discovery_service_test_merges_multiple_tiny_mapping_files_for_lookup() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::mergesMultipleTinyMappingFilesForLookup");
}
#[test]
fn mapping_discovery_service_test_merges_tiny_and_srg_mapping_files_for_lookup() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::mergesTinyAndSrgMappingFilesForLookup");
}
#[test]
fn mapping_discovery_service_test_parses_srg_mapping_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::parsesSrgMappingFile");
}
#[test]
fn mapping_discovery_service_test_parses_tiny_mapping_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::parsesTinyMappingFile");
}
#[test]
fn mapping_discovery_service_test_returns_empty_list_for_nonexistent_root() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MappingDiscoveryServiceTest.kt::returnsEmptyListForNonexistentRoot");
}
#[test]
fn mixin_config_discovery_service_test_configs_for_source_set_excludes_paths_outside_project_root()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::configsForSourceSetExcludesPathsOutsideProjectRoot");
}
#[test]
fn mixin_config_discovery_service_test_configs_for_source_set_falls_back_to_root_shared_when_selected_source_set_has_no_config(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::configsForSourceSetFallsBackToRootSharedWhenSelectedSourceSetHasNoConfig");
}
#[test]
fn mixin_config_discovery_service_test_configs_for_source_set_prefers_local_resources_over_root_shared(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::configsForSourceSetPrefersLocalResourcesOverRootShared");
}
#[test]
fn mixin_config_discovery_service_test_configs_for_source_set_returns_empty_when_only_sibling_source_set_has_configs(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::configsForSourceSetReturnsEmptyWhenOnlySiblingSourceSetHasConfigs");
}
#[test]
fn mixin_config_discovery_service_test_configs_for_source_set_returns_selected_fabric_or_forge_resources(
) {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::configsForSourceSetReturnsSelectedFabricOrForgeResources");
}
#[test]
fn mixin_config_discovery_service_test_discovers_mixin_configs_in_project_tree() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::discoversMixinConfigsInProjectTree");
}
#[test]
fn mixin_config_discovery_service_test_excludes_build_directory_mixin_configs() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::excludesBuildDirectoryMixinConfigs");
}
#[test]
fn mixin_config_discovery_service_test_excludes_gradle_directory_mixin_configs() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::excludesGradleDirectoryMixinConfigs");
}
#[test]
fn mixin_config_discovery_service_test_excludes_node_modules_directory_mixin_configs() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::excludesNodeModulesDirectoryMixinConfigs");
}
#[test]
fn mixin_config_discovery_service_test_identifies_mixin_named_json5_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::identifiesMixinNamedJson5File");
}
#[test]
fn mixin_config_discovery_service_test_identifies_mixin_named_json_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::identifiesMixinNamedJsonFile");
}
#[test]
fn mixin_config_discovery_service_test_identifies_mixins_json5_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::identifiesMixinsJson5File");
}
#[test]
fn mixin_config_discovery_service_test_identifies_mixins_json_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::identifiesMixinsJsonFile");
}
#[test]
fn mixin_config_discovery_service_test_identifies_mod_mixins_json_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::identifiesModMixinsJsonFile");
}
#[test]
fn mixin_config_discovery_service_test_parses_json5_config_with_comments_and_trailing_commas() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::parsesJson5ConfigWithCommentsAndTrailingCommas");
}
#[test]
fn mixin_config_discovery_service_test_parses_mixin_config_content() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::parsesMixinConfigContent");
}
#[test]
fn mixin_config_discovery_service_test_parses_mixin_config_with_missing_optional_fields() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::parsesMixinConfigWithMissingOptionalFields");
}
#[test]
fn mixin_config_discovery_service_test_rejects_non_mixin_json_file() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::rejectsNonMixinJsonFile");
}
#[test]
fn mixin_config_discovery_service_test_returns_null_for_invalid_json() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::returnsNullForInvalidJson");
}
#[test]
fn mixin_config_discovery_service_test_returns_null_for_json_array_root() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::returnsNullForJsonArrayRoot");
}
#[test]
fn mixin_config_discovery_service_test_returns_null_when_equal_longest_package_prefix_is_ambiguous()
{
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::returnsNullWhenEqualLongestPackagePrefixIsAmbiguous");
}
#[test]
fn mixin_config_discovery_service_test_returns_null_when_multiple_configs_list_mixin_class() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::returnsNullWhenMultipleConfigsListMixinClass");
}
#[test]
fn mixin_config_discovery_service_test_returns_null_when_multiple_configs_match_package() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::returnsNullWhenMultipleConfigsMatchPackage");
}
#[test]
fn mixin_config_discovery_service_test_returns_null_when_no_selection_evidence() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::returnsNullWhenNoSelectionEvidence");
}
#[test]
fn mixin_config_discovery_service_test_selects_config_listing_mixin_class_over_earlier_config() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::selectsConfigListingMixinClassOverEarlierConfig");
}
#[test]
fn mixin_config_discovery_service_test_selects_config_listing_relative_subpackage_entry() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::selectsConfigListingRelativeSubpackageEntry");
}
#[test]
fn mixin_config_discovery_service_test_selects_more_specific_package_prefix_over_broader_one() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::selectsMoreSpecificPackagePrefixOverBroaderOne");
}
#[test]
fn mixin_config_discovery_service_test_selects_package_matching_config_when_class_is_not_listed() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::selectsPackageMatchingConfigWhenClassIsNotListed");
}
#[test]
fn mixin_config_discovery_service_test_selects_parent_config_package_for_subpackage_mixin() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::selectsParentConfigPackageForSubpackageMixin");
}
#[test]
fn mixin_config_discovery_service_test_shared_configs_returns_root_unowned_configs_only() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/MixinConfigDiscoveryServiceTest.kt::sharedConfigsReturnsRootUnownedConfigsOnly");
}
#[test]
fn platform_detector_test_combines_multiple_gradle_files_for_detection() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::combinesMultipleGradleFilesForDetection");
}
#[test]
fn platform_detector_test_detects_fabric_from_fabric_loom_coordinate() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsFabricFromFabricLoomCoordinate");
}
#[test]
fn platform_detector_test_detects_fabric_from_fabric_loom_plugin() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsFabricFromFabricLoomPlugin");
}
#[test]
fn platform_detector_test_detects_forge_from_forge_gradle() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsForgeFromForgeGradle");
}
#[test]
fn platform_detector_test_detects_forge_from_forge_gradle_legacy_marker() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsForgeFromForgeGradleLegacyMarker");
}
#[test]
fn platform_detector_test_detects_from_root_gradle_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsFromRootGradleFiles");
}
#[test]
fn platform_detector_test_detects_neo_forge_from_moddev_gradle() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsNeoForgeFromModdevGradle");
}
#[test]
fn platform_detector_test_detects_neo_forge_from_neoforged_gradle_coordinate() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::detectsNeoForgeFromNeoforgedGradleCoordinate");
}
#[test]
fn platform_detector_test_neoforge_takes_priority_over_forge_when_both_present() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::neoforgeTakesPriorityOverForgeWhenBothPresent");
}
#[test]
fn platform_detector_test_returns_unknown_for_empty_gradle_contents() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::returnsUnknownForEmptyGradleContents");
}
#[test]
fn platform_detector_test_returns_unknown_for_plain_java_project() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/PlatformDetectorTest.kt::returnsUnknownForPlainJavaProject");
}
#[test]
fn project_context_builder_test_builds_empty_context_with_explicit_unknown_platform() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectContextBuilderTest.kt::buildsEmptyContextWithExplicitUnknownPlatform");
}
#[test]
fn project_context_builder_test_builds_fabric_context_from_gradle_and_discovered_files() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectContextBuilderTest.kt::buildsFabricContextFromGradleAndDiscoveredFiles");
}
#[test]
fn project_context_builder_test_detects_platform_from_gradle_contents_when_root_files_missing() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectContextBuilderTest.kt::detectsPlatformFromGradleContentsWhenRootFilesMissing");
}
#[test]
fn project_context_builder_test_preserves_classpath_snapshot_in_built_context() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectContextBuilderTest.kt::preservesClasspathSnapshotInBuiltContext");
}
#[test]
fn project_context_builder_test_uses_explicit_platform_over_gradle_detection() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectContextBuilderTest.kt::usesExplicitPlatformOverGradleDetection");
}
#[test]
fn project_context_builder_test_uses_provided_mappings_without_rediscovery() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectContextBuilderTest.kt::usesProvidedMappingsWithoutRediscovery");
}
#[test]
fn project_tree_walker_test_includes_similarly_named_directories() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::includesSimilarlyNamedDirectories");
}
#[test]
fn project_tree_walker_test_mapping_walk_covers_all_mapping_whitelist_prefixes() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::mappingWalkCoversAllMappingWhitelistPrefixes");
}
#[test]
fn project_tree_walker_test_mapping_walk_descends_through_nested_whitelist_paths() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::mappingWalkDescendsThroughNestedWhitelistPaths");
}
#[test]
fn project_tree_walker_test_mapping_walk_prunes_irrelevant_build_and_gradle_subtrees() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::mappingWalkPrunesIrrelevantBuildAndGradleSubtrees");
}
#[test]
fn project_tree_walker_test_mapping_walk_returns_deterministic_sorted_order() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::mappingWalkReturnsDeterministicSortedOrder");
}
#[test]
fn project_tree_walker_test_mapping_walk_returns_empty_for_missing_root() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::mappingWalkReturnsEmptyForMissingRoot");
}
#[test]
fn project_tree_walker_test_returns_deterministic_sorted_order() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::returnsDeterministicSortedOrder");
}
#[test]
fn project_tree_walker_test_returns_empty_for_missing_root() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::returnsEmptyForMissingRoot");
}
#[test]
fn project_tree_walker_test_should_prune_mapping_discovery_directory_matches_whitelist_semantics() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::shouldPruneMappingDiscoveryDirectoryMatchesWhitelistSemantics");
}
#[test]
fn project_tree_walker_test_skips_excluded_subtrees_without_visiting_them() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::skipsExcludedSubtreesWithoutVisitingThem");
}
#[test]
fn project_tree_walker_test_skips_nested_excluded_directory_segments() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/ProjectTreeWalkerTest.kt::skipsNestedExcludedDirectorySegments");
}
#[test]
fn source_set_resolver_test_prefers_nested_most_specific_source_root() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/SourceSetResolverTest.kt::prefersNestedMostSpecificSourceRoot");
}
#[test]
fn source_set_resolver_test_resolves_main_and_client_source_sets() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/SourceSetResolverTest.kt::resolvesMainAndClientSourceSets");
}
#[test]
fn source_set_resolver_test_resolves_mapped_sources_to_main_source_set() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/SourceSetResolverTest.kt::resolvesMappedSourcesToMainSourceSet");
}
#[test]
fn source_set_resolver_test_returns_null_for_unknown_document_path() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/SourceSetResolverTest.kt::returnsNullForUnknownDocumentPath");
}
#[test]
fn source_set_resolver_test_returns_null_when_equal_specificity_tie() {
    legacy_replay::replay_case("mcdev-core/src/test/kotlin/io/github/mcdev/core/project/SourceSetResolverTest.kt::returnsNullWhenEqualSpecificityTie");
}
