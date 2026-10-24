use mcdev_core::{
    mapping::*,
    MappingNamespace::{self, Intermediary, Named, Official, Srg},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

fn names(source: &str, target: Option<String>) -> BTreeMap<MappingNamespace, String> {
    let mut names = BTreeMap::from([(Named, source.into())]);
    if let Some(target) = target {
        names.insert(Intermediary, target);
    }
    names
}
fn duplicates(destinations: &[Option<&str>]) -> MappingSet {
    let mut set = MappingSet {
        namespaces: vec![Named, Intermediary],
        ..Default::default()
    };
    for target in destinations {
        let owner = ClassMapping {
            names: names("A", target.map(str::to_owned)),
        };
        set.classes.push(owner.clone());
        set.fields.push(FieldMapping {
            owner: owner.clone(),
            descriptor: "LA;".into(),
            names: names("field", target.map(|v| format!("field_{v}"))),
        });
        set.methods.push(MethodMapping {
            owner,
            descriptor: "([LA;)LA;".into(),
            names: names("method", target.map(|v| format!("method_{v}"))),
        });
    }
    set
}
fn reference() -> ClassRef {
    ClassRef {
        internal_name: "A".into(),
        namespace: Named,
    }
}
fn field() -> FieldRef {
    FieldRef {
        owner: "A".into(),
        name: "field".into(),
        descriptor: "LA;".into(),
        namespace: Named,
    }
}
fn method() -> MethodRef {
    MethodRef {
        owner: "A".into(),
        name: "method".into(),
        descriptor: "([LA;)LA;".into(),
        namespace: Named,
    }
}
fn found<T: std::fmt::Debug>(result: MappingLookupResult<T>) -> T {
    match result {
        MappingLookupResult::Found(value) => value,
        other => panic!("expected Found: {other:?}"),
    }
}
fn legacy_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    (
                        if k == "internal_name" {
                            "internalName".into()
                        } else {
                            k
                        },
                        legacy_keys(v),
                    )
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(legacy_keys).collect()),
        other => other,
    }
}
fn snapshot<T: serde::Serialize>(result: MappingLookupResult<T>) -> Value {
    match result {
        MappingLookupResult::Found(value) => {
            json!({"kind":"Found","value":legacy_keys(serde_json::to_value(value).unwrap())})
        }
        MappingLookupResult::Missing {
            subject,
            owner,
            name,
            descriptor,
            from,
            to,
        } => {
            json!({"kind":"Missing","subject":subject,"owner":owner,"name":name,"descriptor":descriptor,"from":from,"to":to})
        }
        MappingLookupResult::InvalidDescriptor {
            descriptor,
            message,
            offset,
        } => {
            json!({"kind":"InvalidDescriptor","descriptor":descriptor,"message":message,"offset":offset})
        }
    }
}

#[test]
fn original_compiled_kotlin_duplicate_cases_preserve_first_row_and_all_lookup_metadata() {
    let rows: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/mapping_duplicates/original_results.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        let destinations = row["destinations"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_str)
            .collect::<Vec<_>>();
        let set = duplicates(&destinations);
        assert_eq!(
            json!(set.class_name("A", Named, Intermediary)),
            row["className"],
            "{} className",
            row["case"]
        );
        assert_eq!(
            json!(set.field_name("A", "field", "LA;", Named, Intermediary)),
            row["fieldName"],
            "{} fieldName",
            row["case"]
        );
        assert_eq!(
            json!(set.method_name("A", "method", "([LA;)LA;", Named, Intermediary)),
            row["methodName"],
            "{} methodName",
            row["case"]
        );
        let resolver = set.as_resolver();
        for (key, actual) in [
            (
                "classResult",
                snapshot(resolver.remap_class(&reference(), Intermediary)),
            ),
            (
                "fieldResult",
                snapshot(resolver.remap_field(&field(), Intermediary)),
            ),
            (
                "methodResult",
                snapshot(resolver.remap_method(&method(), Intermediary)),
            ),
            (
                "descriptorResult",
                snapshot(resolver.remap_descriptor("([LA;)LA;", Named, Intermediary)),
            ),
        ] {
            assert_eq!(actual, row[key], "{} {key}", row["case"]);
        }
    }
}
#[test]
fn first_matching_member_without_destination_does_not_fall_through_to_later_member() {
    let mut set = duplicates(&[Some("B"), Some("C")]);
    set.fields[0].names.remove(&Intermediary);
    set.methods[0].names.remove(&Intermediary);
    assert_eq!(
        set.field_name("A", "field", "LA;", Named, Intermediary),
        None
    );
    assert_eq!(
        set.method_name("A", "method", "([LA;)LA;", Named, Intermediary),
        None
    );
    let resolver = set.as_resolver();
    assert_eq!(
        resolver.remap_field(&field(), Intermediary),
        MappingLookupResult::Missing {
            subject: MappingSubject::Field,
            owner: Some("A".into()),
            name: Some("field".into()),
            descriptor: Some("LA;".into()),
            from: Named,
            to: Intermediary
        }
    );
    assert_eq!(
        resolver.remap_method(&method(), Intermediary),
        MappingLookupResult::Missing {
            subject: MappingSubject::Method,
            owner: Some("A".into()),
            name: Some("method".into()),
            descriptor: Some("([LA;)LA;".into()),
            from: Named,
            to: Intermediary
        }
    );
}
#[test]
fn matching_is_still_exact_for_owner_name_descriptor_and_namespace() {
    let mut set = duplicates(&[Some("B"), Some("C")]);
    let mut wrong_owner = set.methods[0].clone();
    wrong_owner.owner.names.insert(Named, "Other".into());
    wrong_owner.names.insert(Intermediary, "otherOwner".into());
    let mut overload = set.methods[0].clone();
    overload.descriptor = "(I)V".into();
    overload.names.insert(Intermediary, "intOverload".into());
    set.methods.insert(0, wrong_owner);
    set.methods.insert(0, overload);
    let mut overload = set.fields[0].clone();
    overload.descriptor = "I".into();
    overload.names.insert(Intermediary, "intField".into());
    set.fields.insert(0, overload);
    assert_eq!(
        set.method_name("A", "method", "([LA;)LA;", Named, Intermediary)
            .as_deref(),
        Some("method_B")
    );
    assert_eq!(
        set.method_name("A", "method", "(I)V", Named, Intermediary)
            .as_deref(),
        Some("intOverload")
    );
    assert_eq!(
        set.field_name("A", "field", "LA;", Named, Intermediary)
            .as_deref(),
        Some("field_B")
    );
    assert_eq!(
        set.field_name("A", "field", "I", Named, Intermediary)
            .as_deref(),
        Some("intField")
    );
    assert!(set
        .method_name("A", "absent", "([LA;)LA;", Named, Intermediary)
        .is_none());
    assert!(set.class_name("A", Official, Intermediary).is_none());
}
#[test]
fn inverse_many_to_one_names_keep_original_row_order() {
    let set=TinyV2Parser::parse("tiny\t2\t0\tnamed\tintermediary\nc\tFirst\tShared\n\tm\t()V\tfirst\tsame\n\tf\tI\tfirstField\tsameField\nc\tSecond\tShared\n\tm\t()V\tsecond\tsame\n\tf\tI\tsecondField\tsameField").unwrap();
    assert_eq!(
        set.class_name("Shared", Intermediary, Named).as_deref(),
        Some("First")
    );
    assert_eq!(
        set.method_name("Shared", "same", "()V", Intermediary, Named)
            .as_deref(),
        Some("first")
    );
    assert_eq!(
        set.field_name("Shared", "sameField", "I", Intermediary, Named)
            .as_deref(),
        Some("firstField")
    );
}
#[test]
fn tiny_and_srg_parsers_preserve_duplicate_row_order_without_deduplicating() {
    let tiny=TinyV2Parser::parse("tiny\t2\t0\tnamed\tintermediary\nc\tA\tB\n\tm\t()V\trun\tfirst\n\tf\tI\tfield\tfirstField\nc\tA\tC\n\tm\t()V\trun\tsecond\n\tf\tI\tfield\tsecondField").unwrap();
    assert_eq!(tiny.classes.len(), 2);
    assert_eq!(tiny.methods.len(), 2);
    assert_eq!(tiny.fields.len(), 2);
    assert_eq!(
        tiny.class_name("A", Named, Intermediary).as_deref(),
        Some("B")
    );
    assert_eq!(
        tiny.method_name("A", "run", "()V", Named, Intermediary)
            .as_deref(),
        Some("first")
    );
    assert_eq!(
        tiny.field_name("A", "field", "I", Named, Intermediary)
            .as_deref(),
        Some("firstField")
    );
    let srg=SrgParser::parse("CL: A B\nMD: A/run ()V B/first ()V\nFD: A/field B/firstField\nCL: A C\nMD: A/run ()V C/second ()V\nFD: A/field C/secondField").unwrap();
    assert_eq!(srg.classes.len(), 2);
    assert_eq!(srg.methods.len(), 2);
    assert_eq!(srg.fields.len(), 2);
    assert_eq!(srg.class_name("A", Official, Srg).as_deref(), Some("B"));
    assert_eq!(
        srg.method_name("A", "run", "()V", Official, Srg).as_deref(),
        Some("first")
    );
    assert_eq!(
        srg.field_name("A", "field", "", Official, Srg).as_deref(),
        Some("firstField")
    );
    assert_eq!(srg.methods[1].owner.names[&Srg], "C");
}
#[test]
fn composite_prefers_first_duplicate_set_but_still_falls_through_a_missing_first_destination() {
    for (targets, expected) in [([Some("B"), Some("C")], "B"), ([None, Some("C")], "D")] {
        let resolver = CompositeMappingResolver::new(vec![
            Arc::new(duplicates(&targets).as_resolver()),
            Arc::new(duplicates(&[Some("D")]).as_resolver()),
        ])
        .unwrap();
        assert_eq!(
            found(resolver.remap_class(&reference(), Intermediary)).internal_name,
            expected
        );
        let field = found(resolver.remap_field(&field(), Intermediary));
        assert_eq!(field.owner, expected);
        assert_eq!(field.name, format!("field_{expected}"));
        let method = found(resolver.remap_method(&method(), Intermediary));
        assert_eq!(method.owner, expected);
        assert_eq!(method.name, format!("method_{expected}"));
        assert_eq!(method.descriptor, format!("([L{expected};)L{expected};"));
        assert_eq!(
            found(resolver.remap_descriptor("([LA;)LA;", Named, Intermediary)),
            format!("([L{expected};)L{expected};")
        );
    }
}
#[test]
fn missing_and_invalid_descriptor_results_still_keep_the_original_contract() {
    let resolver = duplicates(&[Some("B"), Some("C")]).as_resolver();
    assert!(matches!(
        resolver.remap_descriptor("(LA;", Named, Intermediary),
        MappingLookupResult::InvalidDescriptor { .. }
    ));
    assert!(matches!(
        resolver.remap_class(
            &ClassRef {
                internal_name: "Absent".into(),
                namespace: Named
            },
            Intermediary
        ),
        MappingLookupResult::Missing {
            subject: MappingSubject::Class,
            ..
        }
    ));
    assert_eq!(
        resolver.remap_descriptor("(LUnmapped;)V", Named, Intermediary),
        MappingLookupResult::Missing {
            subject: MappingSubject::Class,
            owner: Some("Unmapped".into()),
            name: None,
            descriptor: None,
            from: Named,
            to: Intermediary
        }
    );
    let unchanged = MethodRef {
        owner: "Absent".into(),
        name: "ghost".into(),
        descriptor: "unparsed same-namespace descriptor".into(),
        namespace: Named,
    };
    assert_eq!(
        resolver.remap_method(&unchanged, Named),
        MappingLookupResult::Found(unchanged)
    );
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "mcdev-mapping-duplicate-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let root = self.0.canonicalize().unwrap();
        let parent = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            root.starts_with(&parent)
                && root.parent() == Some(parent.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("mcdev-mapping-duplicate-")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn discovered_project_resolver_keeps_file_order_and_duplicate_row_precedence() {
    use mcdev_core::project::{MappingDiscoveryService, ModPlatform};
    let temp = Temp::new();
    let first = temp.0.join("z-first.tiny");
    let second = temp.0.join("a-second.tiny");
    std::fs::write(&first, "tiny\t2\t0\tnamed\tintermediary\nc\tA\tB\nc\tA\tC").unwrap();
    std::fs::write(&second, "tiny\t2\t0\tnamed\tintermediary\nc\tA\tD").unwrap();
    for (files, wanted) in [
        (vec![first.clone(), second.clone()], "B"),
        (vec![second, first], "D"),
    ] {
        let context =
            MappingDiscoveryService::build_mapping_context(&files, ModPlatform::Fabric).unwrap();
        assert_eq!(context.source_namespace, Named);
        assert_eq!(context.runtime_namespace, Intermediary);
        assert_eq!(
            found(context.resolver.remap_class(&reference(), Intermediary)).internal_name,
            wanted
        );
    }
}
#[test]
fn completion_insertions_use_first_mapping_instead_of_named_fallback() {
    use mcdev_core::{
        at::AtMemberInsertFormatter,
        aw::{AccessWidenerKind, AwNamespaceHelper},
        bytecode::{AtTargetCandidate, AtTargetKind},
        mixin_completion::AtTargetInsertFormatter,
        FieldIndexEntry, MethodIndexEntry,
    };
    let resolver: Arc<dyn MappingResolver> =
        Arc::new(duplicates(&[Some("B"), Some("C")]).as_resolver());
    let method = AtTargetCandidate::new(AtTargetKind::Invoke, "A", "method", "([LA;)LA;");
    let field = AtTargetCandidate::new(AtTargetKind::Field, "A", "field", "LA;");
    assert_eq!(
        AtTargetInsertFormatter::format_insert(
            &method,
            Some(resolver.as_ref()),
            Named,
            Intermediary
        ),
        "LB;method_B([LB;)LB;"
    );
    assert_eq!(
        AtTargetInsertFormatter::format_insert(
            &field,
            Some(resolver.as_ref()),
            Named,
            Intermediary
        ),
        "LB;field_B:LB;"
    );
    assert_eq!(
        AwNamespaceHelper::insert_owner("A", Some(Intermediary), Some(resolver.as_ref())),
        "B"
    );
    assert_eq!(
        AwNamespaceHelper::insert_member_name(
            "A",
            "method",
            "([LA;)LA;",
            AccessWidenerKind::Method,
            Some(Intermediary),
            Some(resolver.as_ref())
        ),
        "method_B"
    );
    assert_eq!(
        AwNamespaceHelper::insert_descriptor(
            "([LA;)LA;",
            Some(Intermediary),
            Some(resolver.as_ref())
        ),
        "([LB;)LB;"
    );
    let context = ProjectMappingContext {
        source_namespace: Named,
        runtime_namespace: Intermediary,
        aw_namespace: Some(Intermediary),
        at_namespace: Some(Intermediary),
        available_namespaces: [Named, Intermediary].into_iter().collect(),
        resolver,
    };
    let inserted = AtMemberInsertFormatter::remap_method_for_entry(
        "A",
        &MethodIndexEntry {
            name: "method".into(),
            descriptor: "([LA;)LA;".into(),
            is_static: false,
            readable_signature: "method(A[]): A".into(),
        },
        &context,
    );
    assert!(inserted.mapping_found);
    assert_eq!(inserted.insert_text, "method_B([LB;)LB;");
    let inserted = AtMemberInsertFormatter::remap_field_for_entry(
        "A",
        &FieldIndexEntry {
            name: "field".into(),
            descriptor: "LA;".into(),
            is_static: false,
            readable_type: "A".into(),
        },
        &context,
    );
    assert!(inserted.mapping_found);
    assert_eq!(inserted.insert_text, "field_B");
}
