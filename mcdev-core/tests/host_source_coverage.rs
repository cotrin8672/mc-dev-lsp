use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[test]
fn every_one_of_170_original_cases_retains_assertions_and_an_existing_native_entrypoint() {
    let ledger: Value =
        serde_json::from_str(include_str!("fixtures/host_source/coverage.json")).unwrap();
    let cases = ledger["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 170);
    let sources = [
        ("host_source_types.rs", include_str!("host_source_types.rs")),
        (
            "host_source_legacy.rs",
            include_str!("host_source_legacy.rs"),
        ),
        ("host_source_cache.rs", include_str!("host_source_cache.rs")),
        (
            "host_source_completion_cache.rs",
            include_str!("host_source_completion_cache.rs"),
        ),
        ("host_source_query.rs", include_str!("host_source_query.rs")),
        (
            "host_source_snapshot.rs",
            include_str!("host_source_snapshot.rs"),
        ),
        (
            "host_source_semantic.rs",
            include_str!("host_source_semantic.rs"),
        ),
        (
            "host_source_project_legacy.rs",
            include_str!("host_source_project_legacy.rs"),
        ),
    ];
    let mut ids = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut assertions = 0;
    for case in cases {
        let id = case["legacyCase"].as_str().unwrap();
        assert!(ids.insert(id));
        assert_eq!(case["originalExecution"], "passed");
        assert!(!case["originalCaseBody"].as_str().unwrap().is_empty());
        let original_assertions = case["originalAssertionExpressions"].as_array().unwrap();
        assert_eq!(
            original_assertions.len() as u64,
            case["originalAssertionCount"].as_u64().unwrap()
        );
        assertions += original_assertions.len();
        for a in original_assertions {
            assert!(a["callee"].as_str().unwrap().starts_with("assert"));
            assert!(case["originalCaseBody"]
                .as_str()
                .unwrap()
                .contains(a["expression"].as_str().unwrap()));
        }
        let path = case["nativePath"].as_str().unwrap();
        let source = sources
            .iter()
            .find(|(name, _)| path.ends_with(name))
            .unwrap()
            .1;
        assert!(
            source.contains(&format!("fn {}(", case["nativeTest"].as_str().unwrap())),
            "{id}"
        );
        if let Some(test) = case["additionalNativeTest"].as_str() {
            assert!(source.contains(&format!("fn {test}(")));
        }
        if let Some(table_id) = case["nativeTableCase"].as_str() {
            assert!(source.contains(table_id), "{id}");
        }
        *counts
            .entry(
                case["legacyClass"]
                    .as_str()
                    .unwrap()
                    .rsplit('.')
                    .next()
                    .unwrap(),
            )
            .or_insert(0usize) += 1;
    }
    assert_eq!(assertions, 417);
    assert_eq!(
        counts,
        BTreeMap::from([
            ("CompletionIndexCachesTest", 9),
            ("JdtAnnotationConstantExtractorTest", 16),
            ("JdtBindingDescriptorConverterTest", 4),
            ("JdtInjectionPointAtCodeIndexTest", 18),
            ("JdtInjectionPointFeatureProbeTest", 12),
            ("JdtMixinExtrasResolvedContextExtractorTest", 25),
            ("JdtMixinSemanticModelParserTest", 11),
            ("JdtProjectSourceQueryTest", 34),
            ("JdtTypeSignatureConverterTest", 14),
            ("SemanticModelCacheTest", 2),
            ("SourceBackedClassIndexTest", 25)
        ])
    );
    assert_eq!(ledger["bugCorrections"].as_array().unwrap().len(), 1);
    assert_eq!(ledger["bugCorrections"][0]["new"], "123");
    assert!(ledger["wireIntegration"]
        .as_str()
        .unwrap()
        .starts_with("pending:"));
}
