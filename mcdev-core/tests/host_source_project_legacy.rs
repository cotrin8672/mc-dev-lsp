use mcdev_core::{host_source::*, *};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
#[allow(dead_code)]
#[path = "fixtures/mixin_source/replay.rs"]
mod shared;
struct Provider {
    fact: HostFact<HostClassFact>,
    failure: Option<HostSourceError>,
    complete: bool,
    search: Vec<ClassIndexEntry>,
    reads: Mutex<Vec<(String, String)>>,
}
impl HostProjectSourceProvider for Provider {
    fn search_classes(&self, prefix: &str, limit: usize) -> HostSourceResult<Vec<ClassIndexEntry>> {
        self.reads
            .lock()
            .unwrap()
            .push(("search".into(), format!("{prefix}:{limit}")));
        Ok(HostAnswer::known(self.search.clone(), vec![]))
    }
    fn class_by_internal_name(&self, owner: &str) -> HostSourceResult<HostClassFact> {
        self.reads
            .lock()
            .unwrap()
            .push(("internal".into(), owner.into()));
        if let Some(e) = &self.failure {
            Err(e.clone())
        } else {
            Ok(HostAnswer {
                fact: self.fact.clone(),
                reads: vec![],
            })
        }
    }
    fn class_by_fqn(&self, fqn: &str) -> HostSourceResult<HostClassFact> {
        self.reads.lock().unwrap().push(("fqn".into(), fqn.into()));
        if let Some(e) = &self.failure {
            Err(e.clone())
        } else {
            Ok(HostAnswer {
                fact: self.fact.clone(),
                reads: vec![],
            })
        }
    }
    fn search_is_complete(&self) -> bool {
        self.complete
    }
}
#[test]
fn normalized_host_project_query_replays_every_final_captured_query() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/host_source/project-normalized.json")).unwrap();
    let mut cases = std::collections::BTreeSet::new();
    let mut errors = 0;
    for row in &rows {
        let provider = Arc::new(Provider {
            fact: serde_json::from_value(row["classFact"].clone()).unwrap(),
            failure: serde_json::from_value(row["error"].clone()).unwrap(),
            complete: false,
            search: vec![],
            reads: Mutex::new(vec![]),
        });
        let query = HostProjectSourceQuery::new(provider.clone());
        let args = &row["args"];
        let api = row["api"].as_str().unwrap();
        let actual: Result<(Value, SourceAuthority), HostSourceError> = match api {
            "findClass" => query
                .find_class(args["internalName"].as_str().unwrap())
                .map(|r| (json!(r.value), r.authority)),
            "findClassByFqn" => query
                .find_class_by_fqn(args["fqn"].as_str().unwrap())
                .map(|r| (json!(r.value), r.authority)),
            "getMethods" => query
                .get_methods(args["ownerInternalName"].as_str().unwrap())
                .map(|r| (json!(r.value), r.authority)),
            "getFields" => query
                .get_fields(args["ownerInternalName"].as_str().unwrap())
                .map(|r| (json!(r.value), r.authority)),
            "findClasses" => query
                .find_classes(
                    args["prefix"].as_str().unwrap(),
                    usize::try_from(args["limit"].as_i64().unwrap()).unwrap_or(0),
                )
                .map(|r| (json!(r.value), r.authority)),
            _ => panic!("unknown {api}"),
        };
        if let Some(error) = &provider.failure {
            assert_eq!(actual.unwrap_err(), *error);
            errors += 1;
        } else {
            let (value, authority) = actual.unwrap();
            assert_eq!(
                shared::canonical(value, ""),
                shared::canonical(row["expected"].clone(), ""),
                "{} capture row {}",
                row["legacyCase"],
                row["captureRow1Based"]
            );
            if matches!(provider.fact, HostFact::Unavailable { .. }) && api != "findClasses" {
                assert_eq!(authority, SourceAuthority::Unavailable);
            }
        }
        let expected_reads = match api {
            "findClass" => vec![(
                "internal".into(),
                args["internalName"].as_str().unwrap().into(),
            )],
            "findClassByFqn" => vec![("fqn".into(), args["fqn"].as_str().unwrap().into())],
            "getMethods" | "getFields" => vec![(
                "internal".into(),
                args["ownerInternalName"].as_str().unwrap().into(),
            )],
            "findClasses" if args["limit"].as_i64().unwrap() <= 0 => vec![],
            "findClasses" => vec![(
                "search".into(),
                format!("{}:{}", args["prefix"].as_str().unwrap(), args["limit"]),
            )],
            _ => unreachable!(),
        };
        assert_eq!(
            *provider.reads.lock().unwrap(),
            expected_reads,
            "{}",
            row["legacyCase"]
        );
        cases.insert(row["legacyCase"].as_str().unwrap());
    }
    assert_eq!(rows.len(), 38);
    assert_eq!(cases.len(), 25);
    assert_eq!(errors, 2);
}
fn fact(simple: &str) -> HostClassFact {
    HostClassFact {
        internal_name: format!("com/example/{simple}"),
        simple_name: simple.into(),
        package_name: "com.example".into(),
        methods: vec![],
        fields: vec![],
        methods_complete: true,
        fields_complete: true,
    }
}
#[test]
fn normalized_search_preserves_both_legacy_prefix_requests_and_sort_order() {
    for (legacy_prefix, legacy_package_chars, legacy_type_chars) in [
        ("com/example/Sample", Some("com.example"), "Sample"),
        ("Sample", None, "Sample"),
    ] {
        let provider = Arc::new(Provider {
            fact: HostFact::Missing,
            failure: None,
            complete: true,
            search: vec![],
            reads: Mutex::new(vec![]),
        });
        let query = HostProjectSourceQuery::new(provider.clone());
        query.find_classes(legacy_prefix, 50).unwrap();
        assert_eq!(
            *provider.reads.lock().unwrap(),
            [("search".into(), format!("{legacy_prefix}:50"))]
        );
        // Obsolete SearchEngine's CharArray transport is fixture data only. The
        // host receives the complete original prefix, with no child re-parser.
        let explicit_fixture = match legacy_prefix {
            "com/example/Sample" => (Some("com.example"), "Sample"),
            "Sample" => (None, "Sample"),
            _ => unreachable!(),
        };
        assert_eq!(explicit_fixture, (legacy_package_chars, legacy_type_chars));
    }
    let first = fact("Alpha").entry();
    let second = fact("Beta").entry();
    let third = fact("Gamma").entry();
    let query = HostProjectSourceQuery::new(Arc::new(Provider {
        fact: HostFact::Missing,
        failure: None,
        complete: true,
        search: vec![third.clone(), first.clone(), second.clone()],
        reads: Mutex::new(vec![]),
    }));
    assert_eq!(
        query.find_classes("com", 50).unwrap().value,
        [first, second, third]
    );
}
#[test]
fn normalized_host_names_preserve_all_four_requestor_entry_cases() {
    let rows = [
        (
            "requestorTypeToClassIndexEntryMapsTopLevelClass",
            "Sample",
            vec![],
            "Sample",
            "com/example/Sample",
        ),
        (
            "requestorTypeToClassIndexEntryMapsNestedTypesWithDollarSeparators",
            "Inner",
            vec!["Outer"],
            "Outer$Inner",
            "com/example/Outer$Inner",
        ),
        (
            "requestorTypeToClassIndexEntryJoinsMultipleEnclosingTypesWithDollarSeparators",
            "Deep",
            vec!["Outer", "Middle"],
            "Outer$Middle$Deep",
            "com/example/Outer$Middle$Deep",
        ),
        (
            "requestorTypeToClassIndexEntryMapsInterfacesSameAsClasses",
            "Service",
            vec![],
            "Service",
            "com/example/Service",
        ),
    ];
    for (id, legacy_type, legacy_enclosing, normalized_name, expected_internal) in rows {
        // These owned host names replace a JDT requestor's fragmented arrays.
        // The original fragments remain explicit fixture inputs for traceability.
        assert_eq!(
            legacy_type,
            normalized_name.rsplit('$').next().unwrap(),
            "{id}"
        );
        assert_eq!(
            legacy_enclosing.len(),
            normalized_name.matches('$').count(),
            "{id}"
        );
        let class = fact(normalized_name);
        let provider = Arc::new(Provider {
            fact: HostFact::Known(class.clone()),
            failure: None,
            complete: true,
            search: vec![],
            reads: Mutex::new(vec![]),
        });
        let query = HostProjectSourceQuery::new(provider);
        let entry = query.find_class(expected_internal).unwrap().value.unwrap();
        assert_eq!(
            entry,
            ClassIndexEntry {
                simple_name: normalized_name.into(),
                package_name: "com.example".into(),
                internal_name: expected_internal.into()
            },
            "{id}"
        );
        if legacy_type == "Service" {
            assert_eq!(entry, class.entry());
        }
    }
}
#[test]
fn search_capability_is_explicit_and_each_operation_keeps_its_own_authority() {
    for available in [false, true] {
        let query = HostProjectSourceQuery::new(Arc::new(Provider {
            fact: HostFact::Missing,
            failure: None,
            complete: available,
            search: vec![],
            reads: Mutex::new(vec![]),
        }));
        assert_eq!(query.covers_project_dependencies(), available);
    }
    struct Mixed;
    impl HostProjectSourceProvider for Mixed {
        fn search_classes(&self, _: &str, _: usize) -> HostSourceResult<Vec<ClassIndexEntry>> {
            Ok(HostAnswer::known(vec![], vec![]))
        }
        fn class_by_internal_name(&self, owner: &str) -> HostSourceResult<HostClassFact> {
            if owner == "com/example/Missing" {
                Ok(HostAnswer::unavailable("source type not resolved"))
            } else {
                Ok(HostAnswer::known(fact("Found"), vec![]))
            }
        }
        fn class_by_fqn(&self, fqn: &str) -> HostSourceResult<HostClassFact> {
            self.class_by_internal_name(&fqn.replace('.', "/"))
        }
        fn search_is_complete(&self) -> bool {
            true
        }
    }
    let query = Arc::new(HostProjectSourceQuery::new(Arc::new(Mixed)));
    let missing = query.find_class("com/example/Missing").unwrap();
    assert_eq!(missing.value, None);
    assert_eq!(missing.authority, SourceAuthority::Unavailable);
    let found = query.find_class("com/example/Found").unwrap();
    assert_eq!(found.value.unwrap().internal_name, "com/example/Found");
    assert_eq!(found.authority, SourceAuthority::Complete);
    std::thread::scope(|scope| {
        let a = query.clone();
        let b = query.clone();
        let first = scope.spawn(move || a.find_class("com/example/Missing").unwrap());
        let second = scope.spawn(move || b.find_class("com/example/Found").unwrap());
        assert_eq!(
            first.join().unwrap().authority,
            SourceAuthority::Unavailable
        );
        assert_eq!(second.join().unwrap().authority, SourceAuthority::Complete);
    });
    assert_eq!(
        missing.authority,
        SourceAuthority::Unavailable,
        "later successful query cannot mutate a previous operation"
    );
}
#[test]
fn incomplete_normalized_method_and_field_results_preserve_actual_bytecode_fallback() {
    struct Binary {
        methods: Vec<MethodIndexEntry>,
        fields: Vec<FieldIndexEntry>,
    }
    impl ClassIndex for Binary {
        fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
            vec![]
        }
        fn find_class(&self, _: &str) -> Option<ClassIndexEntry> {
            None
        }
        fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
            None
        }
        fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
            self.methods.clone()
        }
        fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
            self.fields.clone()
        }
    }
    let owner = "com/example/BlockItem";
    let mut class = fact("BlockItem");
    class.methods = vec![
        HostMethodFact {
            name: "keep".into(),
            is_constructor: Some(false),
            access_flags: Some(0),
            parameter_types: vec![],
            return_type: HostTypeFact::Primitive(HostPrimitive::Void),
        },
        HostMethodFact {
            name: "getBlock".into(),
            is_constructor: Some(false),
            access_flags: Some(0),
            parameter_types: vec![],
            return_type: HostTypeFact::Unresolved,
        },
    ];
    class.fields = vec![
        HostFieldFact {
            name: "keep".into(),
            access_flags: Some(0),
            field_type: HostTypeFact::Primitive(HostPrimitive::Int),
        },
        HostFieldFact {
            name: "block".into(),
            access_flags: Some(0),
            field_type: HostTypeFact::Unresolved,
        },
    ];
    let query = Arc::new(HostProjectSourceQuery::new(Arc::new(Provider {
        fact: HostFact::Known(class),
        failure: None,
        complete: true,
        search: vec![],
        reads: Mutex::new(vec![]),
    })));
    let methods = query.get_methods(owner).unwrap();
    assert_eq!(
        methods
            .value
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["keep()V"]
    );
    assert_eq!(methods.authority, SourceAuthority::Partial);
    let fields = query.get_fields(owner).unwrap();
    assert_eq!(
        fields
            .value
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["keepI"]
    );
    assert_eq!(fields.authority, SourceAuthority::Partial);
    let block = "Lnet/minecraft/world/level/block/Block;";
    let index = SourceBackedClassIndex::new(
        Arc::new(Binary {
            methods: vec![MethodIndexEntry {
                name: "getBlock".into(),
                descriptor: format!("(){block}"),
                is_static: false,
                readable_signature: "getBlock(): Block".into(),
            }],
            fields: vec![FieldIndexEntry {
                name: "block".into(),
                descriptor: block.into(),
                is_static: false,
                readable_type: "Block".into(),
            }],
        }),
        query.clone(),
    );
    assert_eq!(
        index
            .get_methods(owner)
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        [
            "keep()V",
            "getBlock()Lnet/minecraft/world/level/block/Block;"
        ]
    );
    assert_eq!(
        index
            .get_fields(owner)
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["keepI", "blockLnet/minecraft/world/level/block/Block;"]
    );
    let source_only = SourceBackedClassIndex::new(
        Arc::new(Binary {
            methods: vec![],
            fields: vec![],
        }),
        query,
    );
    assert_eq!(
        source_only
            .get_methods(owner)
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["keep()V"]
    );
    assert_eq!(
        source_only
            .get_fields(owner)
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["keepI"]
    );
}
