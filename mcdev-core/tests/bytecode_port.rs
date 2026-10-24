use mcdev_core::bytecode::*;
use mcdev_core::model::{ClassIndex, MappingNamespace};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
struct Fixture {
    pool: Vec<Vec<u8>>,
    slots: u16,
    methods: Vec<(u16, u16, u16, Vec<u8>, Vec<u8>)>,
    fields: Vec<(u16, u16, u16)>,
}
impl Fixture {
    fn item(&mut self, bytes: Vec<u8>, wide: bool) -> u16 {
        let index = self.slots + 1;
        self.slots += if wide { 2 } else { 1 };
        self.pool.push(bytes);
        index
    }
    fn utf(&mut self, value: &str) -> u16 {
        let mut bytes = vec![1];
        bytes.extend((value.len() as u16).to_be_bytes());
        bytes.extend(value.as_bytes());
        self.item(bytes, false)
    }
    fn class(&mut self, name: &str) -> u16 {
        let utf = self.utf(name);
        let mut value = vec![7];
        value.extend(utf.to_be_bytes());
        self.item(value, false)
    }
    fn name_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let name = self.utf(name);
        let desc = self.utf(descriptor);
        let mut bytes = vec![12];
        bytes.extend(name.to_be_bytes());
        bytes.extend(desc.to_be_bytes());
        self.item(bytes, false)
    }
    fn member(&mut self, tag: u8, owner: &str, name: &str, descriptor: &str) -> u16 {
        let class = self.class(owner);
        let nt = self.name_type(name, descriptor);
        let mut bytes = vec![tag];
        bytes.extend(class.to_be_bytes());
        bytes.extend(nt.to_be_bytes());
        self.item(bytes, false)
    }
    fn method(&mut self, name: &str, descriptor: &str, flags: u16, code: Vec<u8>) {
        let name = self.utf(name);
        let descriptor = self.utf(descriptor);
        self.methods
            .push((name, descriptor, flags, code, Vec::new()));
    }
    fn field(&mut self, name: &str, descriptor: &str, flags: u16) {
        let name = self.utf(name);
        let descriptor = self.utf(descriptor);
        self.fields.push((name, descriptor, flags));
    }
    fn finish(mut self, name: &str, super_name: &str, interfaces: &[&str]) -> Vec<u8> {
        let owner = self.class(name);
        let superclass = self.class(super_name);
        let interfaces = interfaces
            .iter()
            .map(|name| self.class(name))
            .collect::<Vec<_>>();
        let code_name = self.utf("Code");
        let frame_name = self.utf("StackMapTable");
        let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52];
        bytes.extend((self.slots + 1).to_be_bytes());
        for value in self.pool {
            bytes.extend(value);
        }
        bytes.extend(0x21u16.to_be_bytes());
        bytes.extend(owner.to_be_bytes());
        bytes.extend(superclass.to_be_bytes());
        bytes.extend((interfaces.len() as u16).to_be_bytes());
        for interface in interfaces {
            bytes.extend(interface.to_be_bytes());
        }
        bytes.extend((self.fields.len() as u16).to_be_bytes());
        for (name, desc, flags) in self.fields {
            bytes.extend(flags.to_be_bytes());
            bytes.extend(name.to_be_bytes());
            bytes.extend(desc.to_be_bytes());
            bytes.extend(0u16.to_be_bytes());
        }
        bytes.extend((self.methods.len() as u16).to_be_bytes());
        for (name, desc, flags, code, frames) in self.methods {
            bytes.extend(flags.to_be_bytes());
            bytes.extend(name.to_be_bytes());
            bytes.extend(desc.to_be_bytes());
            bytes.extend(1u16.to_be_bytes());
            bytes.extend(code_name.to_be_bytes());
            let mut content = vec![0, 16, 1, 44];
            content.extend((code.len() as u32).to_be_bytes());
            content.extend(code);
            content.extend(0u16.to_be_bytes());
            content.extend(u16::from(!frames.is_empty()).to_be_bytes());
            if !frames.is_empty() {
                content.extend(frame_name.to_be_bytes());
                content.extend((frames.len() as u32).to_be_bytes());
                content.extend(frames);
            }
            bytes.extend((content.len() as u32).to_be_bytes());
            bytes.extend(content);
        }
        bytes.extend(0u16.to_be_bytes());
        bytes
    }
}
fn op(opcode: u8, index: u16) -> Vec<u8> {
    vec![opcode, (index >> 8) as u8, index as u8]
}
fn simple(name: &str, super_name: &str) -> Vec<u8> {
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![0xb1]);
    fixture.finish(name, super_name, &[])
}
fn provider(classes: Vec<(&str, Vec<u8>)>) -> InMemoryClassBytesProvider {
    InMemoryClassBytesProvider::new(
        classes
            .into_iter()
            .map(|(name, bytes)| (name.into(), bytes))
            .collect(),
    )
    .unwrap()
}
fn extract(code: Vec<u8>) -> Vec<AtTargetCandidate> {
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, code);
    extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap()
}

#[test]
fn indexes_class_hierarchy_members_and_readable_adapters() {
    let mut fixture = Fixture::default();
    fixture.field("count", "I", 9);
    fixture.field("names", "[Ljava/lang/String;", 1);
    fixture.method("<init>", "()V", 1, vec![0xb1]);
    fixture.method("length", "(Ljava/lang/String;I)J", 9, vec![9, 0xad]);
    let bytes = fixture.finish("demo/Sample", "demo/Base", &["java/lang/Runnable"]);
    let index = ClassMemberIndex::index_class(&bytes, None, Some("jar")).unwrap();
    let class = index.find_class("demo/Sample").unwrap();
    assert_eq!(class.superclass_internal_name.as_deref(), Some("demo/Base"));
    assert_eq!(class.interface_internal_names, ["java/lang/Runnable"]);
    assert_eq!(class.classpath_entry_id.as_deref(), Some("jar"));
    assert!(index.find_method("demo/Sample", "<init>", "()V").is_some());
    assert!(
        index
            .find_field("demo/Sample", "count", "I")
            .unwrap()
            .is_static
    );
    let adapter = BytecodeClassIndexAdapter {
        member_index: index,
    };
    assert_eq!(adapter.find_classes("sample", 50).len(), 1);
    assert_eq!(adapter.find_classes("DEMO.", 1).len(), 1);
    assert_eq!(adapter.find_classes("", 0).len(), 0);
    assert_eq!(
        adapter.find_class_by_fqn("demo.Sample").unwrap().fqn(),
        "demo.Sample"
    );
    assert_eq!(
        adapter.get_fields("demo/Sample")[1].readable_type,
        "String[]"
    );
    assert!(adapter.get_methods("demo/Sample")[1]
        .readable_signature
        .contains("String"));
    assert!(adapter.get_methods("missing").is_empty());
}
#[test]
fn forced_internal_name_and_object_superclass() {
    let bytes = simple("demo/A", "java/lang/Object");
    let index = ClassMemberIndex::index_class(&bytes, Some("x/Override"), None).unwrap();
    let class = index.find_class("x/Override").unwrap();
    assert_eq!(class.qualified_name, "x.Override");
    assert_eq!(class.superclass_internal_name, None);
}
#[test]
fn extracts_invoke_operations_and_pop_widths() {
    let mut fixture = Fixture::default();
    let virtual_ref = fixture.member(10, "java/lang/String", "length", "()I");
    let static_ref = fixture.member(10, "demo/Factory", "wide", "()J");
    let special_ref = fixture.member(10, "java/lang/Object", "<init>", "()V");
    let interface_ref = fixture.member(11, "java/util/List", "size", "()I");
    let mut code = op(0xb6, virtual_ref);
    code.push(0x57);
    code.extend(op(0xb8, static_ref));
    code.push(0x58);
    code.extend(op(0xb7, special_ref));
    code.extend(op(0xb9, interface_ref));
    code.extend([1, 0, 0x57]);
    code.extend(op(0xb6, virtual_ref));
    code.push(0x58);
    code.push(0xb1);
    fixture.method("run", "()V", 1, code);
    let candidates = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    let calls = candidates
        .iter()
        .filter(|c| c.kind == AtTargetKind::Invoke)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0].operation_kind,
        Some(AtTargetOperationKind::InvokeVirtual)
    );
    assert_eq!(
        calls[1].operation_kind,
        Some(AtTargetOperationKind::InvokeStatic)
    );
    assert_eq!(
        calls[2].operation_kind,
        Some(AtTargetOperationKind::InvokeSpecial)
    );
    assert_eq!(
        calls[3].operation_kind,
        Some(AtTargetOperationKind::InvokeInterface)
    );
    assert_eq!(
        calls
            .iter()
            .map(|c| c.occurrence_result_classification)
            .collect::<Vec<_>>(),
        [
            OccurrenceResultClassification::ImmediatelyPopped,
            OccurrenceResultClassification::ImmediatelyPopped,
            OccurrenceResultClassification::Void,
            OccurrenceResultClassification::ImmediatelyPopped,
            OccurrenceResultClassification::Retained
        ]
    );
    assert_eq!(calls[4].ordinal, Some(1));
    assert_eq!(calls[0].instruction_occurrence_index, 0);
    assert_eq!(calls[1].instruction_occurrence_index, 2);
}
#[test]
fn extracts_all_field_operations_and_assignments_are_void() {
    let mut fixture = Fixture::default();
    let field = fixture.member(9, "demo/Sample", "value", "I");
    let mut code = Vec::new();
    for opcode in [0xb2, 0xb3, 0xb4, 0xb5] {
        code.extend(op(opcode, field));
    }
    code.push(0xb1);
    fixture.method("run", "()V", 1, code);
    let candidates = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    assert_eq!(
        candidates[..4]
            .iter()
            .map(|c| c.operation_kind.unwrap())
            .collect::<Vec<_>>(),
        [
            AtTargetOperationKind::FieldGetStatic,
            AtTargetOperationKind::FieldPutStatic,
            AtTargetOperationKind::FieldGetInstance,
            AtTargetOperationKind::FieldPutInstance
        ]
    );
    assert_eq!(candidates[0].display_label, "value: int");
    assert_eq!(
        candidates[1].occurrence_result_classification,
        OccurrenceResultClassification::Void
    );
    assert_eq!(
        candidates[2].occurrence_result_classification,
        OccurrenceResultClassification::Retained
    );
    assert!(candidates[..4].iter().all(|c| c.ordinal == Some(0)));
}
#[test]
fn new_cast_instanceof_exclude_array_allocations() {
    let mut fixture = Fixture::default();
    let class = fixture.class("java/lang/String");
    let array = fixture.class("[Ljava/lang/String;");
    let mut code = op(0xbb, class);
    code.extend(op(0xc0, class));
    code.extend(op(0xc1, class));
    code.extend(op(0xbd, class));
    code.extend([0xbc, 10]);
    code.extend(op(0xc5, array));
    code.extend([1, 0xb1]);
    fixture.method("run", "()V", 1, code);
    let candidates = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    assert_eq!(candidates.len(), 4);
    assert_eq!(candidates[0].descriptor, "Ljava/lang/String;");
    assert_eq!(candidates[0].kind, AtTargetKind::New);
    assert_eq!(
        candidates[0].occurrence_result_classification,
        OccurrenceResultClassification::NotApplicable
    );
    assert_eq!(
        candidates[1].constant_value,
        Some(ConstantValue::ClassLiteral("java/lang/String".into()))
    );
    assert_eq!(candidates[2].ordinal, Some(1));
}
#[test]
fn extracts_every_constant_form_and_per_value_ordinals() {
    let mut fixture = Fixture::default();
    let string_utf = fixture.utf("hello");
    let mut string = vec![8];
    string.extend(string_utf.to_be_bytes());
    let string = fixture.item(string, false);
    let int = fixture.item([vec![3], 42i32.to_be_bytes().to_vec()].concat(), false);
    let long = fixture.item([vec![5], 100i64.to_be_bytes().to_vec()].concat(), true);
    let float = fixture.item([vec![4], 1.5f32.to_be_bytes().to_vec()].concat(), false);
    let double = fixture.item([vec![6], 2.5f64.to_be_bytes().to_vec()].concat(), true);
    let class = fixture.class("java/lang/String");
    let mut code = (1..=15).collect::<Vec<_>>();
    code.extend([0x10, 0x80, 0x11, 0x80, 0]);
    for cp in [string, int, float, class] {
        code.extend(op(0x13, cp));
    }
    for cp in [long, double] {
        code.extend(op(0x14, cp));
    }
    code.extend([0x12, int as u8, 0xb1]);
    fixture.method("run", "()V", 1, code);
    let candidates = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    let values = candidates
        .iter()
        .filter_map(|c| c.constant_value.clone())
        .collect::<Vec<_>>();
    for value in [
        ConstantValue::NullValue,
        ConstantValue::IntValue(-1),
        ConstantValue::IntValue(5),
        ConstantValue::IntValue(-128),
        ConstantValue::IntValue(-32768),
        ConstantValue::IntValue(42),
        ConstantValue::LongValue(100),
        ConstantValue::FloatValue(1.5),
        ConstantValue::DoubleValue(2.5),
        ConstantValue::StringValue("hello".into()),
        ConstantValue::ClassLiteral("java/lang/String".into()),
    ] {
        assert!(values.contains(&value), "missing {value:?}");
    }
    assert_eq!(
        candidates
            .iter()
            .filter(|c| c.constant_value == Some(ConstantValue::IntValue(42)))
            .map(|c| c.ordinal)
            .collect::<Vec<_>>(),
        [Some(0), Some(1)]
    );
}
#[test]
fn extracts_return_opcodes_and_does_not_reset_ordinals() {
    let candidates = extract(vec![0xac, 0xad, 0xae, 0xaf, 0xb0, 0xb1]);
    assert_eq!(
        candidates
            .iter()
            .map(|c| c.ordinal.unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert!(candidates
        .iter()
        .all(|c| c.kind == AtTargetKind::Return && c.display_label == "RETURN"));
}
#[test]
fn condition_metadata_excludes_comparison_preceded_jumps() {
    let candidates = extract(vec![
        0x03, 0x9b, 0, 3, 0x94, 0x9c, 0, 3, 0x95, 0x9d, 0, 3, 0x96, 0x9e, 0, 3, 0x97, 0x9b, 0, 3,
        0x98, 0x9b, 0, 3, 0, 0x9c, 0, 3, 0xb1,
    ]);
    assert_eq!(
        candidates
            .iter()
            .filter_map(|c| c.condition_opcode)
            .collect::<Vec<_>>(),
        [0x9b, 0x9c]
    );
}
#[test]
fn invokedynamic_counts_as_instruction_but_is_not_an_invoke_candidate() {
    let mut fixture = Fixture::default();
    let nt = fixture.name_type("lambda", "()Ljava/lang/Runnable;");
    let mut dynamic = vec![18, 0, 0];
    dynamic.extend(nt.to_be_bytes());
    let dynamic = fixture.item(dynamic, false);
    let mut code = op(0xba, dynamic);
    code.extend([0, 0, 0x57, 0xb1]);
    fixture.method("run", "()V", 1, code);
    let candidates = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].instruction_occurrence_index, 2);
}
#[test]
fn wide_instructions_count_once() {
    let candidates = extract(vec![0xc4, 0x15, 1, 0, 0xc4, 0x84, 1, 0, 0, 1, 0x03, 0xb1]);
    assert_eq!(candidates[0].instruction_occurrence_index, 2);
    assert_eq!(candidates[1].instruction_occurrence_index, 3);
}
#[test]
fn switch_padding_and_tables_do_not_change_instruction_indices() {
    for lookup in [false, true] {
        let mut code = vec![if lookup { 0xab } else { 0xaa }, 0, 0, 0];
        code.extend(20i32.to_be_bytes());
        if lookup {
            code.extend(1i32.to_be_bytes());
            code.extend(7i32.to_be_bytes());
            code.extend(20i32.to_be_bytes());
        } else {
            code.extend(0i32.to_be_bytes());
            code.extend(0i32.to_be_bytes());
            code.extend(20i32.to_be_bytes());
        }
        code.extend([3, 0xb1]);
        let candidates = extract(code);
        assert_eq!(candidates[0].instruction_occurrence_index, 1);
        assert_eq!(candidates[1].instruction_occurrence_index, 2);
    }
}
#[test]
fn jump_label_or_stackmap_between_call_and_pop_prevents_immediate_pop() {
    for with_frames in [false, true] {
        let mut fixture = Fixture::default();
        let call = fixture.member(10, "demo/Factory", "create", "()I");
        let mut code = if with_frames {
            Vec::new()
        } else {
            vec![0xa7, 0, 6]
        };
        code.extend(op(0xb8, call));
        code.extend([0x57, 0xb1]);
        fixture.method("run", "()V", 1, code);
        if with_frames {
            fixture.methods[0].4 = vec![0, 1, 3];
        }
        let candidates = extract_instructions(
            &fixture.finish("demo/Sample", "java/lang/Object", &[]),
            "run",
            "()V",
        )
        .unwrap();
        assert_eq!(
            candidates[0].occurrence_result_classification,
            OccurrenceResultClassification::Retained
        );
    }
}
#[test]
fn malformed_and_truncated_class_files_return_errors_without_panics() {
    let bytes = simple("demo/Sample", "java/lang/Object");
    for length in 0..bytes.len() {
        assert!(
            ClassMemberIndex::index_class(&bytes[..length], None, None).is_err(),
            "length {length}"
        );
    }
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![0xb6, 0]);
    assert!(extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V"
    )
    .is_err());
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![0xff]);
    assert!(extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V"
    )
    .is_err());
}
#[test]
fn missing_method_extracts_empty_but_service_reports_typed_error() {
    let bytes = simple("demo/Sample", "java/lang/Object");
    assert!(extract_instructions(&bytes, "missing", "()V")
        .unwrap()
        .is_empty());
    let provider = provider(vec![("demo/Sample", bytes)]);
    let mut service = BytecodeIndexService::default();
    assert!(matches!(
        service.extract_instructions(
            &provider,
            "missing",
            "run",
            "()V",
            MappingNamespace::Named,
            None
        ),
        Err(BytecodeIndexError::ClassBytesMissing { .. })
    ));
    assert!(matches!(
        service.extract_instructions(
            &provider,
            "demo/Sample",
            "missing",
            "()V",
            MappingNamespace::Named,
            None
        ),
        Err(BytecodeIndexError::MethodNotFound { .. })
    ));
}
#[test]
fn provider_hash_changes_for_bytes_and_entries_and_rejects_dots() {
    let bytes = simple("demo/A", "java/lang/Object");
    let a = provider(vec![("demo/A", bytes.clone())]);
    let b = provider(vec![("demo/A", simple("demo/A", "demo/Base"))]);
    assert_ne!(a.classpath_hash(), b.classpath_hash());
    assert_eq!(a.classpath_hash().len(), 64);
    let c = InMemoryClassBytesProvider::with_entries(
        BTreeMap::from([("demo/A".into(), bytes)]),
        BTreeMap::from([("jar".into(), 1)]),
        "default",
    )
    .unwrap();
    assert_ne!(a.classpath_hash(), c.classpath_hash());
    assert_eq!(c.classpath_entry_hash("jar"), Some(1));
    assert_eq!(c.classpath_entry_hash("missing"), None);
    assert_eq!(
        c.classpath_entry_ids().into_iter().collect::<Vec<_>>(),
        ["jar"]
    );
    assert!(
        InMemoryClassBytesProvider::new(BTreeMap::from([("demo.Bad".into(), vec![])])).is_err()
    );
    assert!(a.get_class_bytes("missing").is_none());
}
#[test]
fn service_cache_refreshes_for_changed_bytes_and_removed_classes() {
    let a = provider(vec![("demo/A", simple("demo/A", "java/lang/Object"))]);
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![3, 0xb1]);
    let b = provider(vec![(
        "demo/A",
        fixture.finish("demo/A", "java/lang/Object", &[]),
    )]);
    let mut service = BytecodeIndexService::default();
    assert_eq!(
        service
            .extract_instructions(&a, "demo/A", "run", "()V", MappingNamespace::Named, None)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        service
            .extract_instructions(&b, "demo/A", "run", "()V", MappingNamespace::Named, None)
            .unwrap()
            .len(),
        2
    );
    let empty = provider(vec![]);
    assert!(service
        .build_index(&empty, None)
        .unwrap()
        .classes
        .is_empty());
}
#[test]
fn cache_invalidation_preserves_unaffected_entries_and_namespace_keys() {
    let mut cache = BytecodeIndexCache::default();
    let key = MethodInstructionCacheKey {
        class_ref: BytecodeClassRef {
            internal_name: "demo/A".into(),
            classpath_entry_id: "jar".into(),
        },
        method_name: "run".into(),
        descriptor: "()V".into(),
        namespace: MappingNamespace::Named,
    };
    cache.put_index(CachedBytecodeIndex {
        classpath_hash: "hash".into(),
        member_index: ClassMemberIndex::default(),
        class_cache_keys: BTreeMap::from([(
            "demo/A".into(),
            BytecodeCacheKey {
                internal_name: "demo/A".into(),
                classpath_entry_id: "jar".into(),
                last_modified: 1,
            },
        )]),
    });
    cache.put_instructions(key.clone(), vec![]);
    let mut other = key.clone();
    other.class_ref.internal_name = "demo/B".into();
    cache.put_instructions(other.clone(), vec![]);
    cache.invalidate_classpath_entry("jar", 1);
    assert!(cache.get_instructions(&key).is_some());
    cache.invalidate_classpath_entry("jar", 2);
    assert!(cache.get_instructions(&key).is_none());
    assert!(cache.get_instructions(&other).is_some());
    assert_eq!(
        cache.get_index("hash").unwrap().class_cache_keys["demo/A"].last_modified,
        2
    );
    cache.put_instructions(key.clone(), vec![]);
    cache.invalidate_class("demo/A");
    assert!(cache.get_instructions(&key).is_none());
    assert!(cache.get_index("hash").unwrap().class_cache_keys.is_empty());
    cache.clear();
    assert!(cache.get_index("hash").is_none());
    assert!(cache.get_instructions(&other).is_none());
}
#[test]
fn adapter_requires_descriptor_when_overloaded_and_filters_at_values() {
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![0xb1]);
    fixture.method("run", "(I)V", 1, vec![3, 0xb1]);
    let adapter = BytecodeAtTargetAdapter::new(Arc::new(provider(vec![(
        "demo/Sample",
        fixture.finish("demo/Sample", "java/lang/Object", &[]),
    )])));
    assert!(adapter
        .get_at_target_candidates("demo/Sample", "run", None, "RETURN")
        .is_empty());
    assert_eq!(
        adapter.get_return_ordinal_count("demo/Sample", "run", None),
        0
    );
    assert_eq!(
        adapter.get_return_ordinal_count("demo/Sample", "run", Some("(I)V")),
        1
    );
    assert_eq!(
        adapter
            .get_at_target_candidates("demo/Sample", "run", Some("(I)V"), "constant")
            .len(),
        1
    );
    assert!(adapter
        .get_at_target_candidates("demo/Sample", "run", Some("(I)V"), "INVOKE")
        .is_empty());
    assert!(adapter
        .get_at_target_candidates("missing", "run", None, "RETURN")
        .is_empty());
    assert!(adapter
        .get_at_target_candidates("demo/Sample", "", None, "RETURN")
        .is_empty());
    assert!(adapter.get_class_bytes("demo/Sample").is_some());
    assert!(adapter.get_class_bytes("missing").is_none());
    assert!(matches_at_value(AtTargetKind::Invoke, "invoke_assign"));
    assert!(!matches_at_value(AtTargetKind::Invoke, "unknown"));
}
#[test]
fn common_superclass_handles_siblings_interfaces_arrays_and_unknowns() {
    let provider = Arc::new(provider(vec![
        ("demo/Base", simple("demo/Base", "java/lang/Object")),
        ("demo/Left", simple("demo/Left", "demo/Base")),
        ("demo/Right", simple("demo/Right", "demo/Base")),
    ]));
    let resolver = BytecodeCommonSuperClassResolver::new(provider.clone());
    assert_eq!(
        resolver.resolve("Ldemo/Left;", "Ldemo/Right;").as_deref(),
        Some("Ldemo/Base;")
    );
    assert_eq!(
        resolver.resolve("Ldemo/Right;", "Ldemo/Left;").as_deref(),
        Some("Ldemo/Base;")
    );
    assert_eq!(
        resolver.resolve("[Ldemo/Left;", "[Ldemo/Right;").as_deref(),
        Some("[Ldemo/Base;")
    );
    assert_eq!(
        resolver.resolve("[I", "[J").as_deref(),
        Some("Ljava/lang/Object;")
    );
    assert_eq!(
        resolver.resolve("[I", "Ljava/lang/Cloneable;").as_deref(),
        Some("Ljava/lang/Cloneable;")
    );
    assert_eq!(
        resolver.resolve("[[I", "[[J").as_deref(),
        Some("[Ljava/lang/Object;")
    );
    assert_eq!(resolver.resolve("I", "I").as_deref(), Some("I"));
    assert_eq!(resolver.resolve("I", "J"), None);
    assert_eq!(resolver.resolve("V", "V"), None);
    assert_eq!(resolver.resolve("bad", "bad"), None);
    assert_eq!(resolver.resolve("Lmissing/A;", "Lmissing/B;"), None);
    assert!(BytecodeCommonSuperClassResolver::with_capacities(provider, 0, 1).is_err());
}
#[test]
fn interface_assignability_and_hierarchy_cycles_are_bounded() {
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![0xb1]);
    let implementor = fixture.finish("demo/Impl", "java/lang/Object", &["demo/Api"]);
    let resolver = BytecodeCommonSuperClassResolver::new(Arc::new(provider(vec![
        ("demo/Impl", implementor),
        ("demo/CycleA", simple("demo/CycleA", "demo/CycleB")),
        ("demo/CycleB", simple("demo/CycleB", "demo/CycleA")),
    ])));
    assert_eq!(
        resolver.resolve("Ldemo/Impl;", "Ldemo/Api;").as_deref(),
        Some("Ldemo/Api;")
    );
    assert_eq!(resolver.resolve("Ldemo/CycleA;", "Lmissing/X;"), None);
}

#[test]
fn classification_matrix_matches_legacy_pop_and_retention_rules() {
    for descriptor in [
        "()I",
        "()Ljava/lang/Object;",
        "()[I",
        "()F",
        "()J",
        "()D",
        "()V",
    ] {
        for opcode in [
            None,
            Some(0x57),
            Some(0x58),
            Some(0xac),
            Some(0x36),
            Some(0x00),
        ] {
            let expected = if descriptor == "()V" {
                OccurrenceResultClassification::Void
            } else if opcode
                == Some(if descriptor == "()J" || descriptor == "()D" {
                    0x58
                } else {
                    0x57
                })
            {
                OccurrenceResultClassification::ImmediatelyPopped
            } else {
                OccurrenceResultClassification::Retained
            };
            assert_eq!(
                classify_method_invoke_result(descriptor, opcode),
                expected,
                "{descriptor} {opcode:?}"
            );
        }
    }
}
#[test]
fn ordinal_groups_separate_owner_name_descriptor_operation_and_constant_value() {
    let initial = AtTargetCandidate::new(AtTargetKind::Invoke, "demo/A", "call", "()V");
    let mut values = vec![initial.clone(), initial.clone()];
    for (owner, name, descriptor) in [
        ("demo/B", "call", "()V"),
        ("demo/A", "other", "()V"),
        ("demo/A", "call", "(I)V"),
    ] {
        values.push(AtTargetCandidate::new(
            AtTargetKind::Invoke,
            owner,
            name,
            descriptor,
        ));
    }
    let mut special = initial.clone();
    special.operation_kind = Some(AtTargetOperationKind::InvokeSpecial);
    values.push(special.clone());
    values.push(special);
    let mut constant = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
    constant.constant_value = Some(ConstantValue::StringValue("same".into()));
    values.extend([constant.clone(), constant]);
    let mut constant = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
    constant.constant_value = Some(ConstantValue::ClassLiteral("same".into()));
    values.push(constant);
    assign_ordinals(&mut values);
    assert_eq!(
        values
            .iter()
            .map(|value| value.ordinal.unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 0, 0, 0, 0, 1, 0, 1, 0]
    );
}
#[test]
fn modified_utf8_null_and_supplementary_character_constants_decode_correctly() {
    let mut fixture = Fixture::default();
    let mut utf = vec![
        1, 0, 9, b'a', 0xc0, 0x80, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80,
    ];
    let utf = fixture.item(std::mem::take(&mut utf), false);
    let mut string = vec![8];
    string.extend(utf.to_be_bytes());
    let string = fixture.item(string, false);
    fixture.method(
        "run",
        "()V",
        1,
        [vec![0x12, string as u8], vec![0xb1]].concat(),
    );
    let values = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    assert_eq!(
        values[0].constant_value,
        Some(ConstantValue::StringValue("a\0😀".into()))
    );
}
#[test]
fn arrays_and_reference_hierarchy_match_legacy_cases_in_both_orders() {
    let resolver = BytecodeCommonSuperClassResolver::new(Arc::new(provider(vec![
        (
            "java/lang/String",
            simple("java/lang/String", "java/lang/Object"),
        ),
        ("demo/Base", simple("demo/Base", "java/lang/Object")),
        ("demo/Sub", simple("demo/Sub", "demo/Base")),
        ("demo/Other", simple("demo/Other", "java/lang/Object")),
    ])));
    for (left, right, expected) in [
        ("Ldemo/Sub;", "Ldemo/Base;", "Ldemo/Base;"),
        ("Ldemo/Sub;", "Ldemo/Other;", "Ljava/lang/Object;"),
        (
            "[Ljava/lang/String;",
            "[[Ljava/lang/String;",
            "[Ljava/lang/Object;",
        ),
        (
            "[[Ljava/lang/String;",
            "[Ljava/lang/Object;",
            "[Ljava/lang/Object;",
        ),
        ("[I", "Ljava/io/Serializable;", "Ljava/io/Serializable;"),
        ("[I", "Ljava/lang/Object;", "Ljava/lang/Object;"),
        ("[I", "[I", "[I"),
        ("[[[I", "[[[J", "[[Ljava/lang/Object;"),
        ("[I", "[[J", "Ljava/lang/Object;"),
        ("[I", "[Ljava/lang/String;", "Ljava/lang/Object;"),
        ("[[Ldemo/Sub;", "[Ldemo/Base;", "[Ljava/lang/Object;"),
        ("Lmissing/Type;", "Ljava/lang/Object;", "Ljava/lang/Object;"),
    ] {
        assert_eq!(
            resolver.resolve(left, right).as_deref(),
            Some(expected),
            "{left} vs {right}"
        );
        assert_eq!(
            resolver.resolve(right, left).as_deref(),
            Some(expected),
            "{right} vs {left}"
        );
    }
}
struct CountingProvider {
    inner: InMemoryClassBytesProvider,
    reads: std::sync::atomic::AtomicUsize,
}
impl ClassBytesProvider for CountingProvider {
    fn get_class_bytes(&self, name: &str) -> Option<Vec<u8>> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.get_class_bytes(name)
    }
    fn internal_names(&self) -> Vec<String> {
        self.inner.internal_names()
    }
    fn classpath_entry_ids(&self) -> std::collections::BTreeSet<String> {
        self.inner.classpath_entry_ids()
    }
    fn classpath_entry_hash(&self, name: &str) -> Option<i64> {
        self.inner.classpath_entry_hash(name)
    }
    fn classpath_hash(&self) -> String {
        self.inner.classpath_hash()
    }
}
#[test]
fn superclass_lookup_caches_pairs_hierarchies_missing_and_corrupt_bytes() {
    let counted = Arc::new(CountingProvider {
        inner: provider(vec![
            ("demo/A", simple("demo/A", "java/lang/Object")),
            ("demo/B", simple("demo/B", "java/lang/Object")),
            ("demo/Corrupt", vec![1, 2, 3]),
        ]),
        reads: 0.into(),
    });
    let resolver = BytecodeCommonSuperClassResolver::new(counted.clone());
    assert_eq!(counted.reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
        resolver.resolve("Ldemo/A;", "Ldemo/B;").as_deref(),
        Some("Ljava/lang/Object;")
    );
    let reads = counted.reads.load(std::sync::atomic::Ordering::SeqCst);
    assert!(reads >= 2);
    assert!(resolver.resolve("Ldemo/B;", "Ldemo/A;").is_some());
    assert_eq!(
        counted.reads.load(std::sync::atomic::Ordering::SeqCst),
        reads
    );
    assert_eq!(resolver.resolve("Lmissing/A;", "Ldemo/A;"), None);
    let reads = counted.reads.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(resolver.resolve("Lmissing/A;", "Ldemo/B;"), None);
    assert_eq!(
        counted.reads.load(std::sync::atomic::Ordering::SeqCst),
        reads
    );
    assert_eq!(resolver.resolve("Ldemo/Corrupt;", "Ldemo/A;"), None);
}
#[test]
fn bounded_hierarchy_and_pair_caches_evict_least_recently_used_values() {
    let counted = Arc::new(CountingProvider {
        inner: provider(vec![
            ("demo/A", simple("demo/A", "java/lang/Object")),
            ("demo/B", simple("demo/B", "java/lang/Object")),
            ("demo/C", simple("demo/C", "java/lang/Object")),
        ]),
        reads: 0.into(),
    });
    let resolver =
        BytecodeCommonSuperClassResolver::with_capacities(counted.clone(), 1, 1).unwrap();
    resolver.resolve("Ldemo/A;", "Ldemo/B;");
    resolver.resolve("Ldemo/A;", "Ldemo/C;");
    let reads = counted.reads.load(std::sync::atomic::Ordering::SeqCst);
    resolver.resolve("Ldemo/A;", "Ldemo/B;");
    assert!(counted.reads.load(std::sync::atomic::Ordering::SeqCst) > reads);
}
#[test]
fn cache_path_key_uses_file_timestamp_and_resources_load_classes() {
    let directory = std::env::temp_dir().join(format!("mcdev-bytecode-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("demo")).unwrap();
    let path = directory.join("demo/A.class");
    let bytes = simple("demo/A", "java/lang/Object");
    std::fs::write(&path, &bytes).unwrap();
    let result = InMemoryClassBytesProvider::from_resources(&directory).unwrap();
    assert_eq!(result.get_class_bytes("demo/A"), Some(bytes));
    let key = BytecodeIndexCache::cache_key_for_path(&path, "demo/A");
    assert_eq!(key.classpath_entry_id, path.to_string_lossy());
    assert_eq!(key.internal_name, "demo/A");
    assert!(key.last_modified > 0);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory.join("demo")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn constant_display_labels_preserve_jvm_numeric_spelling() {
    for (value, expected) in [
        (ConstantValue::FloatValue(1.2), "1.2f"),
        (ConstantValue::FloatValue(-0.0), "-0.0f"),
        (ConstantValue::FloatValue(0.0001), "1.0E-4f"),
        (ConstantValue::DoubleValue(10_000_000.0), "1.0E7"),
        (ConstantValue::DoubleValue(f64::INFINITY), "Infinity"),
        (ConstantValue::DoubleValue(f64::NEG_INFINITY), "-Infinity"),
        (ConstantValue::DoubleValue(f64::NAN), "NaN"),
        (ConstantValue::StringValue("hello".into()), "\"hello\""),
        (
            ConstantValue::ClassLiteral("java/lang/String".into()),
            "String",
        ),
        (ConstantValue::NullValue, "null"),
    ] {
        assert_eq!(constant_label(Some(&value)), expected);
    }
    assert_eq!(constant_label(None), "CONSTANT");
}

#[test]
fn duplicate_identical_signatures_use_first_code_but_are_ambiguous_without_descriptor() {
    let mut fixture = Fixture::default();
    fixture.method("run", "()V", 1, vec![0xb1]);
    fixture.method("run", "()V", 1, vec![3, 0xb1]);
    let bytes = fixture.finish("demo/Sample", "java/lang/Object", &[]);
    assert_eq!(extract_instructions(&bytes, "run", "()V").unwrap().len(), 1);
    let adapter = BytecodeAtTargetAdapter::new(Arc::new(provider(vec![("demo/Sample", bytes)])));
    assert!(adapter
        .get_at_target_candidates("demo/Sample", "run", None, "RETURN")
        .is_empty());
    assert_eq!(
        adapter
            .get_at_target_candidates("demo/Sample", "run", Some("()V"), "RETURN")
            .len(),
        1
    );
}

#[test]
fn floating_constants_use_kotlin_equality_and_ordinal_groups() {
    assert_ne!(
        ConstantValue::FloatValue(0.0),
        ConstantValue::FloatValue(-0.0)
    );
    assert_ne!(
        ConstantValue::DoubleValue(0.0),
        ConstantValue::DoubleValue(-0.0)
    );
    assert_eq!(
        ConstantValue::FloatValue(f32::NAN),
        ConstantValue::FloatValue(f32::from_bits(0x7fc0_0001))
    );
    assert_eq!(
        ConstantValue::DoubleValue(f64::NAN),
        ConstantValue::DoubleValue(f64::from_bits(0x7ff8_0000_0000_0001))
    );
    let mut candidates = [0.0, -0.0, f32::NAN, f32::from_bits(0x7fc0_0001), -0.0].map(|value| {
        let mut candidate = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
        candidate.constant_value = Some(ConstantValue::FloatValue(value));
        candidate
    });
    assign_ordinals(&mut candidates);
    assert_eq!(
        candidates.map(|candidate| candidate.ordinal),
        [Some(0), Some(0), Some(0), Some(1), Some(1)]
    );
}

#[test]
fn consecutive_stack_map_deltas_use_bytecode_offsets_not_attribute_positions() {
    let mut fixture = Fixture::default();
    let call = fixture.member(10, "demo/Factory", "create", "()I");
    let mut code = Vec::new();
    for _ in 0..3 {
        code.extend(op(0xb8, call));
        code.push(0x57);
    }
    code.push(0xb1);
    fixture.method("run", "()V", 1, code);
    fixture.methods[0].4 = vec![0, 2, 3, 3];
    let values = extract_instructions(
        &fixture.finish("demo/Sample", "java/lang/Object", &[]),
        "run",
        "()V",
    )
    .unwrap();
    assert_eq!(
        values[..3]
            .iter()
            .map(|value| value.occurrence_result_classification)
            .collect::<Vec<_>>(),
        [
            OccurrenceResultClassification::Retained,
            OccurrenceResultClassification::Retained,
            OccurrenceResultClassification::ImmediatelyPopped
        ]
    );
}
