use mcdev_core::{host_source::*, *};
use std::sync::{Arc, Mutex};
const OWNER: &str = "com/example/Target";
fn entry(name: &str) -> ClassIndexEntry {
    ClassIndexEntry {
        simple_name: name.into(),
        package_name: "com.example".into(),
        internal_name: format!("com/example/{name}"),
    }
}
fn method(name: &str, label: Option<&str>) -> MethodIndexEntry {
    MethodIndexEntry {
        name: name.into(),
        descriptor: "()V".into(),
        is_static: false,
        readable_signature: label
            .map(Into::into)
            .unwrap_or_else(|| format!("{name}(): void")),
    }
}
fn field(name: &str, descriptor: &str, label: &str) -> FieldIndexEntry {
    FieldIndexEntry {
        name: name.into(),
        descriptor: descriptor.into(),
        is_static: false,
        readable_type: label.into(),
    }
}
#[derive(Default)]
struct Fixed {
    classes: Vec<ClassIndexEntry>,
    methods: Vec<MethodIndexEntry>,
    fields: Vec<FieldIndexEntry>,
    complete: bool,
    error: Option<HostSourceError>,
    invocations: Mutex<Vec<String>>,
    limits: Mutex<Vec<usize>>,
}
impl Fixed {
    fn answer<T>(&self, value: T) -> Result<SourceQueryResult<T>, HostSourceError> {
        if let Some(e) = &self.error {
            Err(e.clone())
        } else {
            Ok(SourceQueryResult::new(
                value,
                if self.complete {
                    SourceAuthority::Complete
                } else {
                    SourceAuthority::Partial
                },
            ))
        }
    }
    fn class(&self, name: &str) -> Option<ClassIndexEntry> {
        self.classes
            .iter()
            .find(|c| c.internal_name == name)
            .cloned()
    }
    fn fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.classes.iter().find(|c| c.fqn() == name).cloned()
    }
}
impl ProjectSourceQuery for Fixed {
    fn find_classes(
        &self,
        _: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        self.limits.lock().unwrap().push(limit);
        self.answer(self.classes.clone())
    }
    fn find_class(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.answer(self.class(name))
    }
    fn find_class_by_fqn(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.answer(self.fqn(name))
    }
    fn get_methods(
        &self,
        _: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        self.answer(self.methods.clone())
    }
    fn get_fields(
        &self,
        _: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        self.answer(self.fields.clone())
    }
    fn covers_project_dependencies(&self) -> bool {
        self.complete
    }
}
impl ClassIndex for Fixed {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.invocations
            .lock()
            .unwrap()
            .push(format!("findClasses:{prefix}:{limit}"));
        self.classes.clone()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        self.invocations
            .lock()
            .unwrap()
            .push(format!("findClass:{name}"));
        self.class(name)
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.invocations
            .lock()
            .unwrap()
            .push(format!("findClassByFqn:{name}"));
        self.fqn(name)
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.invocations
            .lock()
            .unwrap()
            .push(format!("getMethods:{owner}"));
        self.methods.clone()
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.invocations
            .lock()
            .unwrap()
            .push(format!("getFields:{owner}"));
        self.fields.clone()
    }
}
fn class_fact(name: &str, method_name: &str) -> HostClassFact {
    HostClassFact {
        internal_name: format!("com/example/{name}"),
        simple_name: name.into(),
        package_name: "com.example".into(),
        methods: vec![HostMethodFact {
            name: method_name.into(),
            is_constructor: Some(false),
            access_flags: Some(1),
            parameter_types: vec![],
            return_type: HostTypeFact::Primitive(HostPrimitive::Void),
        }],
        fields: vec![],
        methods_complete: true,
        fields_complete: true,
    }
}
#[test]
fn project_source_query_defaults_covers_project_dependencies_to_false() {
    assert!(!SnapshotSourceQuery::from_buffer(vec![], vec![]).covers_project_dependencies());
    assert!(!Fixed::default().covers_project_dependencies());
}
#[test]
fn composite_covers_project_dependencies_when_any_delegate_does() {
    let query = CompositeProjectSourceQuery::new(vec![
        Arc::new(Fixed::default()),
        Arc::new(Fixed {
            complete: true,
            ..Default::default()
        }),
    ]);
    assert!(query.covers_project_dependencies());
}
#[test]
fn composite_does_not_cover_project_dependencies_when_no_delegate_does() {
    let query = CompositeProjectSourceQuery::new(vec![
        Arc::new(Fixed::default()),
        Arc::new(SnapshotSourceQuery::from_buffer(vec![], vec![])),
    ]);
    assert!(!query.covers_project_dependencies());
}
#[test]
fn composite_prefers_buffer_over_host_delegate() {
    let mut host_entry = entry("Current");
    host_entry.simple_name = "JdtCurrent".into();
    let query = CompositeProjectSourceQuery::new(vec![
        Arc::new(SnapshotSourceQuery::from_buffer(
            vec![class_fact("Current", "bufferMethod")],
            vec![],
        )),
        Arc::new(Fixed {
            classes: vec![host_entry],
            methods: vec![method("jdtMethod", None)],
            ..Default::default()
        }),
    ]);
    assert_eq!(
        query
            .find_class("com/example/Current")
            .unwrap()
            .value
            .unwrap()
            .simple_name,
        "Current"
    );
    assert_eq!(
        query
            .find_class_by_fqn("com.example.Current")
            .unwrap()
            .value
            .unwrap()
            .simple_name,
        "Current"
    );
    let methods = query.get_methods("com/example/Current").unwrap().value;
    assert!(methods.iter().any(|m| m.name == "bufferMethod"));
    assert!(methods.iter().any(|m| m.name == "jdtMethod"));
}
#[test]
fn composite_merges_classes_in_delegate_order_with_limit() {
    let query = CompositeProjectSourceQuery::new(vec![
        Arc::new(Fixed {
            classes: vec![entry("Shared"), entry("FirstOnly")],
            ..Default::default()
        }),
        Arc::new(Fixed {
            classes: vec![entry("Shared"), entry("SecondOnly")],
            ..Default::default()
        }),
    ]);
    assert_eq!(
        query
            .find_classes("com", 10)
            .unwrap()
            .value
            .iter()
            .map(|c| c.internal_name.as_str())
            .collect::<Vec<_>>(),
        [
            "com/example/Shared",
            "com/example/FirstOnly",
            "com/example/SecondOnly"
        ]
    );
    assert_eq!(
        query
            .find_classes("com", 2)
            .unwrap()
            .value
            .iter()
            .map(|c| c.internal_name.as_str())
            .collect::<Vec<_>>(),
        ["com/example/Shared", "com/example/FirstOnly"]
    );
    for legacy_limit in [0i32, -1] {
        assert!(query
            .find_classes("com", usize::try_from(legacy_limit).unwrap_or(0))
            .unwrap()
            .value
            .is_empty());
    }
}
#[test]
fn composite_merges_methods_in_delegate_order_and_dedups_by_name_and_descriptor() {
    let query = CompositeProjectSourceQuery::new(vec![
        Arc::new(Fixed {
            methods: vec![method("run", Some("run(): void [buffer]"))],
            ..Default::default()
        }),
        Arc::new(Fixed {
            methods: vec![
                method("run", Some("run(): void [jdt]")),
                method("other", None),
            ],
            ..Default::default()
        }),
    ]);
    let methods = query.get_methods(OWNER).unwrap().value;
    assert_eq!(
        methods
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["run()V", "other()V"]
    );
    assert_eq!(methods[0].readable_signature, "run(): void [buffer]");
}
#[test]
fn authoritative_source_query_skips_delegate_for_all_operations() {
    let delegate = Arc::new(Fixed {
        classes: vec![entry("DelegateOnly")],
        methods: vec![method("delegateRun", None)],
        fields: vec![field("delegateValue", "Z", "boolean")],
        ..Default::default()
    });
    let source = Arc::new(Fixed {
        classes: vec![entry("SourceOnly")],
        methods: vec![method("sourceRun", None)],
        fields: vec![field("sourceValue", "I", "int")],
        complete: true,
        ..Default::default()
    });
    let index = SourceBackedClassIndex::new(delegate.clone(), source);
    assert_eq!(index.find_classes("com", 10), vec![entry("SourceOnly")]);
    assert_eq!(
        index.find_class("com/example/SourceOnly"),
        Some(entry("SourceOnly"))
    );
    assert_eq!(
        index.find_class_by_fqn("com.example.SourceOnly"),
        Some(entry("SourceOnly"))
    );
    assert_eq!(index.get_methods(OWNER), vec![method("sourceRun", None)]);
    assert_eq!(
        index.get_fields(OWNER),
        vec![field("sourceValue", "I", "int")]
    );
    assert!(delegate.invocations.lock().unwrap().is_empty());
}
#[test]
fn host_find_type_failure_falls_back_but_authoritative_empty_stays_empty() {
    let delegate = Arc::new(Fixed {
        classes: vec![entry("Target")],
        methods: vec![method("run", None)],
        fields: vec![field("value", "I", "int")],
        ..Default::default()
    });
    let index = SourceBackedClassIndex::new(
        delegate.clone(),
        Arc::new(Fixed {
            error: Some(HostSourceError::Failure("findType failed".into())),
            ..Default::default()
        }),
    );
    assert_eq!(index.find_class(OWNER), Some(entry("Target")));
    assert_eq!(
        index.find_class_by_fqn("com.example.Target"),
        Some(entry("Target"))
    );
    assert_eq!(index.get_methods(OWNER), vec![method("run", None)]);
    assert_eq!(index.get_fields(OWNER), vec![field("value", "I", "int")]);
    delegate.invocations.lock().unwrap().clear();
    let empty = SourceBackedClassIndex::new(
        delegate.clone(),
        Arc::new(Fixed {
            complete: true,
            ..Default::default()
        }),
    );
    assert_eq!(empty.find_class(OWNER), None);
    assert_eq!(empty.find_class_by_fqn("com.example.Target"), None);
    assert!(empty.get_methods(OWNER).is_empty());
    assert!(empty.get_fields(OWNER).is_empty());
    assert!(delegate.invocations.lock().unwrap().is_empty());
}
#[test]
fn composite_keeps_buffered_members_when_host_find_type_fails() {
    let buffered = method("run", Some("run(): void [buffer]"));
    let value = field("value", "I", "int [buffer]");
    let delegate = Arc::new(Fixed {
        methods: vec![buffered.clone(), method("stale", None)],
        fields: vec![value.clone(), field("stale", "Z", "boolean")],
        ..Default::default()
    });
    let query = CompositeProjectSourceQuery::new(vec![
        Arc::new(Fixed {
            methods: vec![buffered],
            fields: vec![value],
            ..Default::default()
        }),
        Arc::new(Fixed {
            error: Some(HostSourceError::Failure("findType failed".into())),
            ..Default::default()
        }),
    ]);
    let index = SourceBackedClassIndex::new(delegate, Arc::new(query));
    let methods = index.get_methods(OWNER);
    assert_eq!(
        methods
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["run()V", "stale()V"]
    );
    assert_eq!(methods[0].readable_signature, "run(): void [buffer]");
    let fields = index.get_fields(OWNER);
    assert_eq!(
        fields
            .iter()
            .map(|f| format!("{}{}", f.name, f.descriptor))
            .collect::<Vec<_>>(),
        ["valueI", "staleZ"]
    );
    assert_eq!(fields[0].readable_type, "int [buffer]");
}
#[test]
fn non_authoritative_source_query_falls_back_to_delegate_for_find_class() {
    let delegate = Arc::new(Fixed {
        classes: vec![entry("DelegateOnly")],
        ..Default::default()
    });
    let index = SourceBackedClassIndex::new(delegate.clone(), Arc::new(Fixed::default()));
    assert_eq!(
        index.find_class("com/example/DelegateOnly"),
        Some(entry("DelegateOnly"))
    );
    assert_eq!(
        *delegate.invocations.lock().unwrap(),
        ["findClass:com/example/DelegateOnly"]
    );
}
#[test]
fn source_query_takes_precedence_over_delegate_for_class_lookup() {
    let source = entry("SourceFoo");
    let mut other = source.clone();
    other.simple_name = "DelegateFoo".into();
    let index = SourceBackedClassIndex::new(
        Arc::new(Fixed {
            classes: vec![other],
            ..Default::default()
        }),
        Arc::new(Fixed {
            classes: vec![source.clone()],
            ..Default::default()
        }),
    );
    assert_eq!(
        index.find_class(&source.internal_name),
        Some(source.clone())
    );
    assert_eq!(
        index.find_class_by_fqn("com.example.SourceFoo"),
        Some(source)
    );
}
#[test]
fn find_classes_passes_limit_through_to_source_query() {
    let source = Arc::new(Fixed::default());
    SourceBackedClassIndex::new(Arc::new(Fixed::default()), source.clone()).find_classes("com", 7);
    assert_eq!(*source.limits.lock().unwrap(), [7]);
}
#[test]
fn find_classes_falls_back_when_source_search_becomes_non_authoritative() {
    let delegate = Arc::new(Fixed {
        classes: vec![entry("DelegateOnly")],
        ..Default::default()
    });
    let index = SourceBackedClassIndex::new(
        delegate.clone(),
        Arc::new(Fixed {
            classes: vec![entry("SourceOnly")],
            ..Default::default()
        }),
    );
    assert_eq!(
        index.find_classes("com", 10),
        [entry("SourceOnly"), entry("DelegateOnly")]
    );
    assert_eq!(
        *delegate.invocations.lock().unwrap(),
        ["findClasses:com:10"]
    );
}
#[test]
fn authoritative_empty_find_classes_skips_delegate() {
    let delegate = Arc::new(Fixed {
        classes: vec![entry("DelegateOnly")],
        ..Default::default()
    });
    let index = SourceBackedClassIndex::new(
        delegate.clone(),
        Arc::new(Fixed {
            complete: true,
            ..Default::default()
        }),
    );
    assert!(index.find_classes("com", 10).is_empty());
    assert!(delegate.invocations.lock().unwrap().is_empty());
}
#[test]
fn find_classes_merges_source_before_delegate_and_distinct_by_internal_name() {
    let index = SourceBackedClassIndex::new(
        Arc::new(Fixed {
            classes: vec![entry("Shared"), entry("DelegateOnly")],
            ..Default::default()
        }),
        Arc::new(Fixed {
            classes: vec![entry("Shared"), entry("SourceOnly")],
            ..Default::default()
        }),
    );
    assert_eq!(
        index
            .find_classes("com", 10)
            .iter()
            .map(|c| c.internal_name.as_str())
            .collect::<Vec<_>>(),
        [
            "com/example/Shared",
            "com/example/SourceOnly",
            "com/example/DelegateOnly"
        ]
    );
}
#[test]
fn get_methods_merges_source_before_delegate_and_distinct_by_name_and_descriptor() {
    let index = SourceBackedClassIndex::new(
        Arc::new(Fixed {
            methods: vec![
                method("run", Some("run(): void [source]")),
                method("other", None),
            ],
            ..Default::default()
        }),
        Arc::new(Fixed {
            methods: vec![method("run", None)],
            ..Default::default()
        }),
    );
    let methods = index.get_methods(OWNER);
    assert_eq!(
        methods
            .iter()
            .map(|m| format!("{}{}", m.name, m.descriptor))
            .collect::<Vec<_>>(),
        ["run()V", "other()V"]
    );
    assert_eq!(methods[0].readable_signature, "run(): void");
}
#[test]
fn get_fields_merges_source_before_delegate_and_distinct_by_name_and_descriptor() {
    let index = SourceBackedClassIndex::new(
        Arc::new(Fixed {
            fields: vec![
                field("value", "I", "int [delegate]"),
                field("other", "Z", "boolean"),
            ],
            ..Default::default()
        }),
        Arc::new(Fixed {
            fields: vec![field("value", "I", "int")],
            ..Default::default()
        }),
    );
    let fields = index.get_fields(OWNER);
    assert_eq!(
        fields
            .iter()
            .map(|f| format!("{}{}", f.name, f.descriptor))
            .collect::<Vec<_>>(),
        ["valueI", "otherZ"]
    );
    assert_eq!(fields[0].readable_type, "int");
}
#[test]
fn get_fields_falls_back_to_delegate_when_source_query_returns_empty() {
    let expected = field("value", "I", "int");
    let index = SourceBackedClassIndex::new(
        Arc::new(Fixed {
            fields: vec![expected.clone()],
            ..Default::default()
        }),
        Arc::new(Fixed::default()),
    );
    assert_eq!(index.get_fields(OWNER), [expected]);
}
#[test]
fn buffer_source_query_exposes_unsaved_class_and_latest_method() {
    let query =
        SnapshotSourceQuery::from_buffer(vec![class_fact("Unsaved", "latestMethod")], vec![]);
    assert_eq!(
        query
            .find_class_by_fqn("com.example.Unsaved")
            .unwrap()
            .value
            .unwrap()
            .internal_name,
        "com/example/Unsaved"
    );
    assert_eq!(
        query
            .find_class("com/example/Unsaved")
            .unwrap()
            .value
            .unwrap()
            .internal_name,
        "com/example/Unsaved"
    );
    assert!(query
        .get_methods("com/example/Unsaved")
        .unwrap()
        .value
        .iter()
        .any(|m| m.name == "latestMethod"));
}
#[test]
fn buffer_source_query_reflects_each_version_without_disk_access() {
    for version in 0..30 {
        let query = SnapshotSourceQuery::from_buffer(
            vec![class_fact("Current", &format!("keystroke{version}"))],
            vec![],
        );
        let index = SourceBackedClassIndex::new(Arc::new(Fixed::default()), Arc::new(query));
        assert_eq!(
            index
                .find_class_by_fqn("com.example.Current")
                .unwrap()
                .internal_name,
            "com/example/Current"
        );
        assert!(index
            .get_methods("com/example/Current")
            .iter()
            .any(|m| m.name == format!("keystroke{version}")));
    }
    assert!(HostSourceSnapshotCache::default().is_empty());
}
