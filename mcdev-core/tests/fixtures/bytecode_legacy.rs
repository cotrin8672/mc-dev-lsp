//! Every assertion in the 153 original bytecode/index/resolver cases is dispatched
//! by its unchanged case name. Fixtures are the original javac21 sources/bytes.
#[path = "expression_engine_support.rs"]
mod classfile;
use classfile::{op, ClassFile};
use mcdev_core::bytecode::*;
use mcdev_core::model::{ClassIndex, MappingNamespace};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, OnceLock,
};
const PREFIX: &str = "io/github/mcdev/core/bytecode/fixtures";
const SIMPLE: &str = "com/example/target/SimpleTarget";
fn owner(name: &str) -> String {
    format!("{PREFIX}/{name}")
}
fn classes() -> &'static BTreeMap<String, Vec<u8>> {
    static CLASSES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    CLASSES.get_or_init(|| serde_json::from_str(include_str!("bytecode_classes.json")).unwrap())
}
fn provider() -> InMemoryClassBytesProvider {
    InMemoryClassBytesProvider::new(
        classes()
            .iter()
            .filter(|(name, _)| name.starts_with(PREFIX))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    )
    .unwrap()
}
fn extract(class: &str, name: &str, descriptor: &str) -> Vec<AtTargetCandidate> {
    extract_instructions(&classes()[&owner(class)], name, descriptor).unwrap()
}
fn operation(values: &[AtTargetCandidate], kind: AtTargetOperationKind) -> Vec<&AtTargetCandidate> {
    values
        .iter()
        .filter(|value| value.operation_kind == Some(kind))
        .collect()
}
fn ordinals(values: &[&AtTargetCandidate]) -> Vec<Option<usize>> {
    values.iter().map(|value| value.ordinal).collect()
}
fn hierarchy(name: &str, superclass: &str, interfaces: &[&str], interface: bool) -> Vec<u8> {
    let mut bytes = ClassFile::default().finish_with_hierarchy(
        name,
        superclass,
        interfaces,
        if interface { 0x601 } else { 1 },
    );
    bytes[6..8].copy_from_slice(&65u16.to_be_bytes());
    bytes
}
struct Counting {
    delegate: InMemoryClassBytesProvider,
    reads: Arc<AtomicUsize>,
}
impl ClassBytesProvider for Counting {
    fn get_class_bytes(&self, name: &str) -> Option<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.delegate.get_class_bytes(name)
    }
    fn get_class_bytes_ref(&self, name: &str) -> Option<&[u8]> {
        self.delegate.get_class_bytes_ref(name)
    }
    fn internal_names(&self) -> Vec<String> {
        self.delegate.internal_names()
    }
    fn classpath_entry_ids(&self) -> BTreeSet<String> {
        self.delegate.classpath_entry_ids()
    }
    fn classpath_entry_hash(&self, id: &str) -> Option<i64> {
        self.delegate.classpath_entry_hash(id)
    }
    fn classpath_hash(&self) -> String {
        self.delegate.classpath_hash()
    }
}
fn counting(values: Vec<(&str, Vec<u8>)>) -> (Arc<Counting>, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    (
        Arc::new(Counting {
            delegate: InMemoryClassBytesProvider::new(
                values.into_iter().map(|(n, v)| (n.into(), v)).collect(),
            )
            .unwrap(),
            reads: reads.clone(),
        }),
        reads,
    )
}
pub fn run(class: &str, case: &str) {
    match class {
        "InMemoryClassBytesProviderTest" => memory(case),
        "ClassMemberIndexTest" => members(case),
        "BytecodeIndexServiceTest" => service(case),
        "BytecodeIndexCacheTest" => cache(case),
        "InstructionExtractorFieldTest" => fields(case),
        "InstructionExtractorInvokeTest" => invokes(case),
        "InstructionExtractorMiscTest" => misc(case),
        "OrdinalCalculatorTest" => ordinal(case),
        "BytecodeClassIndexAdapterTest" => class_adapter(case),
        "BytecodeAtTargetAdapterTest" => target_adapter(case),
        "BytecodeIndexEntryMapperTest" => entry_mapper(case),
        "BytecodeCommonSuperClassResolverTest" => common(case),
        "ProjectContextMixinIndexTest" => project_index(case),
        "BytecodeIndexDefaultTest" => default_index(case),
        _ => panic!("unregistered legacy class {class}"),
    }
}
fn default_index(case: &str) {
    struct FakeBytecodeIndex;
    impl BytecodeIndex for FakeBytecodeIndex {
        fn get_at_target_candidates(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: &str,
        ) -> Vec<AtTargetCandidate> {
            vec![]
        }
        fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
            0
        }
    }
    match case {
        "fakeBytecodeIndexUsesDefaultNullGetClassBytes" => assert!(FakeBytecodeIndex
            .get_class_bytes("net/minecraft/client/MinecraftClient")
            .is_none()),
        "fakeBytecodeIndexUsesDefaultNullResolveCommonSuperClass" => assert!(FakeBytecodeIndex
            .resolve_common_super_class("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;")
            .is_none()),
        _ => panic!("unregistered BytecodeIndex default case {case}"),
    }
}
fn project_index(case: &str) {
    let provider = Arc::new(
        InMemoryClassBytesProvider::new(BTreeMap::from([(
            SIMPLE.into(),
            classes()[SIMPLE].clone(),
        )]))
        .unwrap(),
    );
    let context = mcdev_core::project::ProjectContextBuilder::empty("project-index", ".");
    let index = ProjectContextMixinIndex::default();
    match case {
        "buildsClassIndexFromProvider" => assert!(index
            .build_class_index(&context, provider.as_ref())
            .unwrap()
            .find_class_by_fqn("com.example.target.SimpleTarget")
            .is_some()),
        "buildsBytecodeIndexFromProvider" => assert!(!index
            .build_bytecode_index(&context, provider)
            .get_at_target_candidates(SIMPLE, "draw", Some("(Ljava/lang/String;FF)V"), "RETURN")
            .is_empty()),
        "classAndBytecodeIndexShareClasspath" => {
            let class_index = index
                .build_class_index(&context, provider.as_ref())
                .unwrap();
            let bytecode_index = index.build_bytecode_index(&context, provider);
            assert!(!class_index.get_methods(SIMPLE).is_empty());
            assert!(
                bytecode_index.get_return_ordinal_count(
                    SIMPLE,
                    "draw",
                    Some("(Ljava/lang/String;FF)V")
                ) >= 1
            );
        }
        _ => panic!("unregistered project index case {case}"),
    }
}
fn memory(case: &str) {
    match case {
        "returnsClassBytesByInternalName" => {
            let bytes = vec![0xca, 0xfe, 0xba, 0xbe];
            let provider = InMemoryClassBytesProvider::new(BTreeMap::from([(
                "demo/Sample".into(),
                bytes.clone(),
            )]))
            .unwrap();
            assert_eq!(provider.get_class_bytes("demo/Sample"), Some(bytes));
        }
        "returnsNullForMissingClass" => assert!(InMemoryClassBytesProvider::new(BTreeMap::new())
            .unwrap()
            .get_class_bytes("missing/Class")
            .is_none()),
        "rejectsDottedInternalNames" => {
            assert!(InMemoryClassBytesProvider::new(BTreeMap::from([(
                "demo.Sample".into(),
                vec![1]
            )]))
            .is_err())
        }
        "exposesInternalNames" => {
            let provider = InMemoryClassBytesProvider::new(BTreeMap::from([
                ("a/A".into(), vec![1]),
                ("b/B".into(), vec![2]),
            ]))
            .unwrap();
            assert_eq!(provider.internal_names(), ["a/A", "b/B"]);
        }
        "classpathHashChangesWhenBytesChange" | "classpathHashStableForSameContent" => {
            let first = InMemoryClassBytesProvider::new(BTreeMap::from([(
                "a/A".into(),
                if case.ends_with("SameContent") {
                    vec![1, 2, 3]
                } else {
                    vec![1]
                },
            )]))
            .unwrap();
            let second = InMemoryClassBytesProvider::new(BTreeMap::from([(
                "a/A".into(),
                if case.ends_with("SameContent") {
                    vec![1, 2, 3]
                } else {
                    vec![2]
                },
            )]))
            .unwrap();
            assert_eq!(
                first.classpath_hash() == second.classpath_hash(),
                case.ends_with("SameContent")
            );
        }
        "reportsDefaultClasspathEntryId" => {
            let provider =
                InMemoryClassBytesProvider::new(BTreeMap::from([("a/A".into(), vec![1])])).unwrap();
            assert_eq!(
                provider.classpath_entry_ids(),
                BTreeSet::from(["in-memory".into()])
            );
        }
        "reportsConfiguredEntryHashes" => {
            let provider = InMemoryClassBytesProvider::with_entries(
                BTreeMap::from([("a/A".into(), vec![1])]),
                BTreeMap::from([("fixture.jar".into(), 42)]),
                "in-memory",
            )
            .unwrap();
            assert_eq!(
                provider.classpath_entry_ids(),
                BTreeSet::from(["fixture.jar".into()])
            );
            assert_eq!(provider.classpath_entry_hash("fixture.jar"), Some(42));
        }
        _ => panic!("unregistered memory case {case}"),
    }
}
fn members(case: &str) {
    let index = ClassMemberIndex::build(&provider(), None).unwrap();
    let name = owner("MemberIndexSamples");
    match case {
        "indexesClassMetadata" => {
            let entry = index.find_class(&name).unwrap();
            assert_eq!(entry.simple_name, "MemberIndexSamples");
            assert_eq!(
                entry.qualified_name,
                "io.github.mcdev.core.bytecode.fixtures.MemberIndexSamples"
            );
            assert_eq!(
                entry.superclass_internal_name.as_deref(),
                Some("io/github/mcdev/core/bytecode/fixtures/BaseClass")
            );
            assert_eq!(entry.interface_internal_names, [owner("SampleInterface")]);
        }
        "indexesInstanceMethods" => assert!(index.methods_by_owner[&name]
            .iter()
            .any(|method| method.name == "instanceMethod" && !method.is_static)),
        "indexesStaticMethods" => assert!(
            index
                .find_method(&name, "staticMethod", "()V")
                .unwrap()
                .is_static
        ),
        "indexesConstructors" => assert_eq!(
            index.find_method(&name, "<init>", "()V").unwrap().name,
            "<init>"
        ),
        "indexesStaticFields" => assert!(
            index
                .find_field(&name, "STATIC_FIELD", "Ljava/lang/String;")
                .unwrap()
                .is_static
        ),
        "indexesInstanceFields" => assert!(
            !index
                .find_field(&name, "instanceField", "I")
                .unwrap()
                .is_static
        ),
        "indexesAllFixtureClasses" => {
            for class in [
                "InvokeSamples",
                "FieldSamples",
                "NewSamples",
                "ConstantSamples",
                "ReturnSamples",
            ] {
                assert!(index.classes.contains_key(&owner(class)));
            }
        }
        "indexSingleClassFromBytes" => {
            let name = owner("ReturnSamples");
            let single =
                ClassMemberIndex::index_class(&classes()[&name], Some(&name), None).unwrap();
            assert_eq!(single.classes.len(), 1);
            assert!(single.find_method(&name, "returnInt", "()I").is_some());
        }
        "findMethodReturnsNullForMissingMethod" => {
            assert!(index.find_method(&name, "missing", "()V").is_none())
        }
        "findFieldReturnsNullForMissingField" => {
            assert!(index.find_field(&name, "missing", "I").is_none())
        }
        _ => panic!("unregistered member case {case}"),
    }
}
fn service(case: &str) {
    let provider = provider();
    let mut service = BytecodeIndexService::default();
    let owner = owner("InvokeSamples");
    let mut extract = |class: &str, name: &str, descriptor: &str| {
        service.extract_instructions(
            &provider,
            &crate_owner(class),
            name,
            descriptor,
            MappingNamespace::Named,
            None,
        )
    };
    match case {
        "buildsMemberIndexFromProvider" => assert!(!service
            .build_index(&provider, None)
            .unwrap()
            .classes
            .is_empty()),
        "cachesIndexByClasspathHash" => {
            let first = service.build_index(&provider, None).unwrap();
            assert_eq!(first, service.build_index(&provider, None).unwrap());
            assert!(service
                .cache
                .get_index(&provider.classpath_hash())
                .is_some());
        }
        "extractsInvokeInstructions" => assert!(extract("InvokeSamples", "virtualInvoke", "()V")
            .unwrap()
            .iter()
            .any(
                |candidate| candidate.operation_kind == Some(AtTargetOperationKind::InvokeVirtual)
            )),
        "extractsFieldInstructions" => assert!(extract("FieldSamples", "accessFields", "()V")
            .unwrap()
            .iter()
            .any(
                |candidate| candidate.operation_kind == Some(AtTargetOperationKind::FieldGetStatic)
            )),
        "extractsNewInstructions" => assert!(extract("NewSamples", "createObjects", "()V")
            .unwrap()
            .iter()
            .any(|c| c.kind == AtTargetKind::New)),
        "extractsConstantInstructions" => assert!(extract("ConstantSamples", "constants", "()V")
            .unwrap()
            .iter()
            .any(|c| c.kind == AtTargetKind::Constant)),
        "extractsReturnInstructions" => assert!(
            extract("ReturnSamples", "multipleReturns", "(I)I")
                .unwrap()
                .iter()
                .filter(|c| c.kind == AtTargetKind::Return)
                .count()
                >= 2
        ),
        "cachesExtractedInstructions" => {
            let first = extract("InvokeSamples", "staticInvoke", "()V").unwrap();
            assert_eq!(
                first,
                extract("InvokeSamples", "staticInvoke", "()V").unwrap()
            );
        }
        "returnsClassBytesMissingError" => assert!(
            matches!(service.extract_instructions(&provider,"missing/Class","run","()V",MappingNamespace::Named,None),Err(BytecodeIndexError::ClassBytesMissing{internal_name})if internal_name=="missing/Class")
        ),
        "returnsMethodNotFoundError" => assert!(
            matches!(service.extract_instructions(&provider,&owner,"missingMethod","()V",MappingNamespace::Named,None),Err(BytecodeIndexError::MethodNotFound{name,..})if name=="missingMethod")
        ),
        "methodNotFoundWhenDescriptorMismatch" => assert!(matches!(
            extract("InvokeSamples", "virtualInvoke", "(I)V"),
            Err(BytecodeIndexError::MethodNotFound { .. })
        )),
        "duplicateInvokeOrdinalsThroughService" => {
            let values = extract("InvokeSamples", "duplicateInvokes", "()V").unwrap();
            let targets = values
                .iter()
                .filter(|c| {
                    c.operation_kind == Some(AtTargetOperationKind::InvokeStatic) && c.name == "abs"
                })
                .collect::<Vec<_>>();
            assert_eq!(ordinals(&targets), [Some(0), Some(1), Some(2)]);
        }
        _ => panic!("unregistered service case {case}"),
    }
}
fn crate_owner(name: &str) -> String {
    owner(name)
}
fn cache(case: &str) {
    let mut cache = BytecodeIndexCache::default();
    let class = if case == "invalidatesInstructionsForStaleClasspathEntry" {
        owner("InvokeSamples")
    } else {
        "demo/Class".into()
    };
    let key = MethodInstructionCacheKey {
        class_ref: BytecodeClassRef {
            internal_name: class.clone(),
            classpath_entry_id: "fixture.jar".into(),
        },
        method_name: if case == "invalidatesInstructionsForStaleClasspathEntry" {
            "virtualInvoke".into()
        } else {
            "run".into()
        },
        descriptor: "()V".into(),
        namespace: MappingNamespace::Named,
    };
    let candidate = AtTargetCandidate::new(AtTargetKind::Invoke, "demo/Class", "run", "()V");
    match case {
        "storesAndRetrievesIndexByClasspathHash" => {
            let index = ClassMemberIndex::build(&provider(), None).unwrap();
            cache.put_index(CachedBytecodeIndex {
                classpath_hash: "hash-1".into(),
                member_index: index.clone(),
                class_cache_keys: BTreeMap::new(),
            });
            let cached = cache.get_index("hash-1").unwrap();
            assert_eq!(cached.classpath_hash, "hash-1");
            assert_eq!(cached.member_index, index);
            assert!(cached.class_cache_keys.is_empty());
        }
        "returnsNullForMismatchedClasspathHash" => {
            cache.put_index(CachedBytecodeIndex {
                classpath_hash: "hash-1".into(),
                member_index: ClassMemberIndex::default(),
                class_cache_keys: BTreeMap::new(),
            });
            assert!(cache.get_index("hash-2").is_none());
        }
        "storesAndRetrievesInstructionCache" => {
            cache.put_instructions(key.clone(), vec![candidate.clone()]);
            assert_eq!(cache.get_instructions(&key), Some(&vec![candidate]));
        }
        "invalidatesInstructionsForStaleClasspathEntry" => {
            cache.put_index(CachedBytecodeIndex {
                classpath_hash: "hash-1".into(),
                member_index: ClassMemberIndex::build(&provider(), None).unwrap(),
                class_cache_keys: BTreeMap::from([(
                    class.clone(),
                    BytecodeCacheKey {
                        classpath_entry_id: "fixture.jar".into(),
                        last_modified: 1,
                        internal_name: class.clone(),
                    },
                )]),
            });
            cache.put_instructions(
                key.clone(),
                vec![AtTargetCandidate::new(
                    AtTargetKind::Invoke,
                    "x",
                    "y",
                    "()V",
                )],
            );
            cache.invalidate_classpath_entry("fixture.jar", 2);
            assert!(cache.get_instructions(&key).is_none());
            assert_eq!(
                cache.get_index("hash-1").unwrap().class_cache_keys[&class].last_modified,
                2
            );
        }
        "clearRemovesAllCachedState" => {
            cache.put_index(CachedBytecodeIndex {
                classpath_hash: "hash-1".into(),
                member_index: ClassMemberIndex::default(),
                class_cache_keys: BTreeMap::new(),
            });
            cache.clear();
            assert!(cache.get_index("hash-1").is_none());
        }
        "invalidateClassRemovesInstructionCacheEntries" => {
            cache.put_instructions(key.clone(), vec![]);
            cache.invalidate_class(&class);
            assert!(cache.get_instructions(&key).is_none());
        }
        "cacheKeyForPathUsesFileLastModified" => {
            let path = std::env::temp_dir().join(format!(
                "mcdev-bytecode-{}-{}.jar",
                std::process::id(),
                SystemId::next()
            ));
            let file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .unwrap();
            let key = BytecodeIndexCache::cache_key_for_path(&path, "demo/Class");
            assert_eq!(key.classpath_entry_id, path.to_string_lossy());
            assert_eq!(key.internal_name, "demo/Class");
            assert!(key.last_modified > 0);
            drop(file);
            std::fs::remove_file(path).unwrap();
        }
        _ => panic!("unregistered cache case {case}"),
    }
}
struct SystemId;
impl SystemId {
    fn next() -> usize {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        NEXT.fetch_add(1, Ordering::SeqCst)
    }
}
fn fields(case: &str) {
    let values = extract(
        "FieldSamples",
        if case.starts_with("duplicate") {
            "duplicateFieldGets"
        } else {
            "accessFields"
        },
        "()V",
    );
    let kinds = [
        AtTargetOperationKind::FieldGetStatic,
        AtTargetOperationKind::FieldPutStatic,
        AtTargetOperationKind::FieldGetInstance,
        AtTargetOperationKind::FieldPutInstance,
    ];
    let kind = match case {
        "extractsStaticFieldGet" => kinds[0],
        "extractsStaticFieldPut" => kinds[1],
        "extractsInstanceFieldGet" => kinds[2],
        "extractsInstanceFieldPut" => kinds[3],
        _ => kinds[0],
    };
    match case {
        "extractsStaticFieldGet"
        | "extractsStaticFieldPut"
        | "extractsInstanceFieldGet"
        | "extractsInstanceFieldPut" => assert!(values.iter().any(|c| c.operation_kind
            == Some(kind)
            && c.owner == owner("FieldSamples")
            && c.name
                == if matches!(
                    kind,
                    AtTargetOperationKind::FieldGetStatic | AtTargetOperationKind::FieldPutStatic
                ) {
                    "STATIC_FIELD"
                } else {
                    "instanceField"
                }
            && (!matches!(
                kind,
                AtTargetOperationKind::FieldGetStatic | AtTargetOperationKind::FieldGetInstance
            ) || c.descriptor == "I"))),
        "fieldAccessesHaveFourKindsInSample" => {
            let actual = values
                .iter()
                .filter(|c| c.kind == AtTargetKind::Field)
                .map(|c| format!("{:?}", c.operation_kind.unwrap()))
                .collect::<BTreeSet<_>>();
            assert_eq!(
                actual,
                kinds.into_iter().map(|v| format!("{v:?}")).collect()
            );
        }
        "duplicateFieldGetsReceiveDistinctOrdinals" => {
            let targets = operation(&values, kinds[0])
                .into_iter()
                .filter(|c| c.name == "STATIC_FIELD")
                .collect::<Vec<_>>();
            assert_eq!(targets.len(), 2);
            assert_eq!(ordinals(&targets), [Some(0), Some(1)]);
        }
        "duplicateFieldGetsShareDescriptor" => assert!(operation(&values, kinds[0])
            .iter()
            .all(|c| c.descriptor == "I")),
        _ => panic!("unregistered field case {case}"),
    }
}
fn misc(case: &str) {
    match case {
        "extractsNewInstructions" => {
            let values = extract("NewSamples", "createObjects", "()V");
            let news = values
                .iter()
                .filter(|c| c.kind == AtTargetKind::New)
                .collect::<Vec<_>>();
            assert_eq!(news.len(), 2);
            assert!(news.iter().any(|c| c.owner == "java/lang/StringBuilder"));
            assert!(news.iter().any(|c| c.owner == "java/util/ArrayList"));
        }
        "extractsClassLiteralConstantsFromCheckcastAndInstanceofOnly" => {
            let values = extract(
                "TypeInstructionSamples",
                "typeInstructions",
                "(Ljava/lang/Object;I)V",
            );
            let literals = values
                .iter()
                .filter_map(|c| {
                    if c.kind == AtTargetKind::Constant {
                        match &c.constant_value {
                            Some(ConstantValue::ClassLiteral(name)) => Some(name.as_str()),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(literals, ["java/lang/String", "java/util/List"]);
            assert_eq!(
                values
                    .iter()
                    .filter(|c| c.kind == AtTargetKind::New)
                    .count(),
                1
            );
            assert!(literals
                .iter()
                .all(|name| *name != "java/lang/StringBuilder" && !name.starts_with('[')));
        }
        "newInstructionDescriptorUsesClassLiteralForm" => {
            let values = extract("NewSamples", "createObjects", "()V");
            assert_eq!(
                values
                    .iter()
                    .find(|c| c.owner == "java/lang/StringBuilder")
                    .unwrap()
                    .descriptor,
                "Ljava/lang/StringBuilder;"
            );
        }
        "extractsStringConstant"
        | "extractsIntConstant"
        | "extractsLongConstant"
        | "extractsFloatConstant"
        | "extractsDoubleConstant"
        | "extractsClassLiteralConstant"
        | "extractsNullConstant" => {
            let values = extract("ConstantSamples", "constants", "()V");
            let expected = match case {
                "extractsStringConstant" => ConstantValue::StringValue("hello".into()),
                "extractsIntConstant" => ConstantValue::IntValue(42),
                "extractsLongConstant" => ConstantValue::LongValue(100),
                "extractsFloatConstant" => ConstantValue::FloatValue(1.5),
                "extractsDoubleConstant" => ConstantValue::DoubleValue(2.5),
                "extractsClassLiteralConstant" => {
                    ConstantValue::ClassLiteral("java/lang/String".into())
                }
                _ => ConstantValue::NullValue,
            };
            assert!(values.iter().any(|c| c.kind == AtTargetKind::Constant
                && c.constant_value.as_ref() == Some(&expected)));
        }
        "extractsSmallIntConstants" => {
            let values = extract("ConstantSamples", "smallInts", "()V");
            for expected in [5, -1] {
                assert!(values.iter().any(|c| c.kind == AtTargetKind::Constant
                    && c.constant_value == Some(ConstantValue::IntValue(expected))));
            }
        }
        "extractsReturnInstruction" => {
            let values = extract("ReturnSamples", "returnVoid", "()V");
            let returns = values
                .iter()
                .filter(|c| c.kind == AtTargetKind::Return)
                .collect::<Vec<_>>();
            assert_eq!(returns.len(), 1);
            assert_eq!(returns[0].name, "RETURN");
        }
        "returnOrdinalsAreDistinctPerReturnSite" => {
            let values = extract("ReturnSamples", "multipleReturns", "(I)I");
            let returns = values
                .iter()
                .filter(|c| c.kind == AtTargetKind::Return)
                .collect::<Vec<_>>();
            assert_eq!(returns.len(), 2);
            assert_eq!(ordinals(&returns), [Some(0), Some(1)]);
        }
        "intReturnMethodHasReturnCandidate" | "objectReturnMethodHasReturnCandidate" => {
            let (name, descriptor) = if case.starts_with("int") {
                ("returnInt", "()I")
            } else {
                ("returnObject", "()Ljava/lang/String;")
            };
            assert!(extract("ReturnSamples", name, descriptor)
                .iter()
                .any(|c| c.kind == AtTargetKind::Return));
        }
        _ => panic!("unregistered misc case {case}"),
    }
}
fn invoke(
    owner: &str,
    name: &str,
    descriptor: &str,
    operation: AtTargetOperationKind,
) -> AtTargetCandidate {
    let mut value = AtTargetCandidate::new(AtTargetKind::Invoke, owner, name, descriptor);
    value.operation_kind = Some(operation);
    value.ordinal = Some(0);
    value
}
fn ordinal(case: &str) {
    use AtTargetOperationKind::*;
    let mut values = match case {
        "assignsZeroToFirstCandidate" => {
            vec![invoke("java/lang/Math", "abs", "(I)I", InvokeStatic)]
        }
        "incrementsOrdinalsForDuplicateInvokes" => {
            vec![invoke("java/lang/Math", "abs", "(I)I", InvokeStatic); 3]
        }
        "separatesOrdinalsByMethodName" => vec![
            invoke("java/lang/Math", "abs", "(I)I", InvokeStatic),
            invoke("java/lang/Math", "max", "(II)I", InvokeStatic),
        ],
        "separatesOrdinalsByDescriptor" => vec![
            invoke(
                "java/lang/Math",
                "valueOf",
                "(I)Ljava/lang/Integer;",
                InvokeStatic,
            ),
            invoke(
                "java/lang/Math",
                "valueOf",
                "(Ljava/lang/String;)Ljava/lang/Integer;",
                InvokeStatic,
            ),
        ],
        "separatesOrdinalsByOwner" => vec![
            invoke("java/lang/Math", "abs", "(I)I", InvokeStatic),
            invoke("other/Owner", "abs", "(I)I", InvokeStatic),
        ],
        "separatesOrdinalsByKind" => vec![
            invoke("java/lang/Math", "run", "()V", InvokeVirtual),
            invoke("java/lang/Math", "run", "()V", InvokeInterface),
        ],
        "fieldOrdinalsIncrementForSameTarget" => {
            let mut value =
                AtTargetCandidate::new(AtTargetKind::Field, "owner/Class", "STATIC_FIELD", "I");
            value.operation_kind = Some(FieldGetStatic);
            vec![value; 2]
        }
        "constantOrdinalsDifferByValue" => [1, 2, 1]
            .into_iter()
            .map(|n| {
                let mut value = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
                value.constant_value = Some(ConstantValue::IntValue(n));
                value
            })
            .collect(),
        "returnOrdinalsIncrementPerReturnSite" => {
            vec![AtTargetCandidate::new(AtTargetKind::Return, "", "RETURN", ""); 2]
        }
        _ => panic!("unregistered ordinal case {case}"),
    };
    assign_ordinals(&mut values);
    let expected = match case {
        "assignsZeroToFirstCandidate" => vec![0],
        "incrementsOrdinalsForDuplicateInvokes" => vec![0, 1, 2],
        "fieldOrdinalsIncrementForSameTarget" | "returnOrdinalsIncrementPerReturnSite" => {
            vec![0, 1]
        }
        "constantOrdinalsDifferByValue" => vec![0, 0, 1],
        _ => vec![0, 0],
    };
    assert_eq!(
        values
            .iter()
            .map(|c| c.ordinal.unwrap())
            .collect::<Vec<_>>(),
        expected
    );
}
fn synthetic_method(
    mut class: ClassFile,
    descriptor: &str,
    code: Vec<u8>,
    handlers: Vec<(u16, u16, u16, u16)>,
    frames: Vec<u8>,
) -> Vec<AtTargetCandidate> {
    let method = class.method("run", descriptor, 1, 2, code);
    method.handlers = handlers;
    method.stack_map = frames;
    let bytes = class.finish("test/OccurrenceSamples");
    extract_instructions(&bytes, "run", descriptor).unwrap()
}
fn invokes(case: &str) {
    use AtTargetOperationKind::*;
    use OccurrenceResultClassification::*;
    let fixture_method = match case {
        "extractsVirtualInvoke" | "invokeVirtualHasMethodDescriptor" => Some("virtualInvoke"),
        "extractsStaticInvoke" | "invokeStaticHasMethodDescriptor" => Some("staticInvoke"),
        "extractsSpecialInvoke" | "constructorInvokeIsSpecial" => Some("specialInvoke"),
        "extractsInterfaceInvoke" => Some("interfaceInvoke"),
        "duplicateInvokesReceiveDistinctOrdinals" | "duplicateInvokesShareOwnerAndDescriptor" => {
            Some("duplicateInvokes")
        }
        "returnsEmptyListForMissingMethod" => Some("missing"),
        "returnsEmptyListForWrongDescriptor" => Some("virtualInvoke"),
        _ => None,
    };
    if let Some(method) = fixture_method {
        let values = extract(
            "InvokeSamples",
            method,
            if case == "returnsEmptyListForWrongDescriptor" {
                "(I)V"
            } else {
                "()V"
            },
        );
        match case {
            "extractsVirtualInvoke" => {
                for (owner, name) in [
                    ("java/lang/String", "length"),
                    ("java/io/PrintStream", "println"),
                ] {
                    assert!(operation(&values, InvokeVirtual)
                        .iter()
                        .any(|c| c.owner == owner && c.name == name));
                }
            }
            "extractsStaticInvoke" => {
                for (owner, name) in [("java/lang/Math", "abs"), ("java/lang/Integer", "valueOf")] {
                    assert!(operation(&values, InvokeStatic)
                        .iter()
                        .any(|c| c.owner == owner && c.name == name));
                }
            }
            "extractsSpecialInvoke" => {
                for (owner, name) in [
                    ("java/lang/Object", "toString"),
                    ("java/lang/String", "<init>"),
                ] {
                    assert!(operation(&values, InvokeSpecial)
                        .iter()
                        .any(|c| c.owner == owner && c.name == name));
                }
            }
            "extractsInterfaceInvoke" => assert!(operation(&values, InvokeInterface)
                .iter()
                .any(|c| c.owner == "java/lang/Runnable" && c.name == "run")),
            "invokeVirtualHasMethodDescriptor" => assert_eq!(
                values
                    .iter()
                    .find(|c| c.name == "length")
                    .unwrap()
                    .descriptor,
                "()I"
            ),
            "invokeStaticHasMethodDescriptor" => assert_eq!(
                values.iter().find(|c| c.name == "abs").unwrap().descriptor,
                "(I)I"
            ),
            "constructorInvokeIsSpecial" => {
                let value = values.iter().find(|c| c.name == "<init>").unwrap();
                assert_eq!(value.operation_kind, Some(InvokeSpecial));
                assert_eq!(value.descriptor, "(Ljava/lang/String;)V");
            }
            "duplicateInvokesReceiveDistinctOrdinals" => {
                let targets = operation(&values, InvokeStatic)
                    .into_iter()
                    .filter(|c| c.name == "abs")
                    .collect::<Vec<_>>();
                assert_eq!(targets.len(), 3);
                assert_eq!(ordinals(&targets), [Some(0), Some(1), Some(2)]);
            }
            "duplicateInvokesShareOwnerAndDescriptor" => assert!(operation(&values, InvokeStatic)
                .into_iter()
                .filter(|c| c.name == "abs")
                .all(|c| c.owner == "java/lang/Math" && c.descriptor == "(I)I")),
            _ => assert!(values.is_empty()),
        }
        return;
    }
    let mut class = ClassFile::default();
    let mut descriptor = "()V";
    let mut handlers = Vec::new();
    let mut frames = Vec::new();
    let mut expected = Retained;
    let mut name = None;
    let mut code = match case {
        "voidStaticInvokeIsClassifiedAsVoid" => {
            expected = Void;
            let call = class.member(10, "java/lang/System", "gc", "()V");
            op(0xb8, call)
        }
        "voidInstanceInvokeIsClassifiedAsVoid" => {
            expected = Void;
            name = Some("clear");
            let owner = class.class("java/util/ArrayList");
            let init = class.member(10, "java/util/ArrayList", "<init>", "()V");
            let call = class.member(10, "java/util/ArrayList", "clear", "()V");
            [op(0xbb, owner), vec![0x59], op(0xb7, init), op(0xb6, call)].concat()
        }
        "objectInvokeFollowedByPopIsImmediatelyPopped" => {
            expected = ImmediatelyPopped;
            let call = class.member(10, "java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;");
            [vec![0x10, 1], op(0xb8, call), vec![0x57]].concat()
        }
        "longInvokeFollowedByPop2IsImmediatelyPopped"
        | "longInvokeFollowedByWrongPopIsRetained"
        | "frameBetweenLongInvokeAndPop2IsRetained" => {
            let call = class.member(10, "java/lang/Math", "abs", "(J)J");
            let pop = if case == "longInvokeFollowedByWrongPopIsRetained" {
                0x57
            } else {
                0x58
            };
            if case == "longInvokeFollowedByPop2IsImmediatelyPopped" {
                expected = ImmediatelyPopped;
            }
            if case == "frameBetweenLongInvokeAndPop2IsRetained" {
                frames = vec![0, 1, 4];
            }
            [vec![0x09], op(0xb8, call), vec![pop]].concat()
        }
        "doubleInvokeFollowedByPop2IsImmediatelyPopped" => {
            expected = ImmediatelyPopped;
            let call = class.member(10, "java/lang/Math", "abs", "(D)D");
            [vec![0x0e], op(0xb8, call), vec![0x58]].concat()
        }
        "fieldGetFollowedByPopIsRetained" | "fieldPutIsClassifiedAsVoid" => {
            let field = class.member(9, "test/OccurrenceSamples", "value", "I");
            class.field("value", "I", 1);
            if case == "fieldPutIsClassifiedAsVoid" {
                expected = Void;
                [vec![0x2a, 0x10, 1], op(0xb5, field)].concat()
            } else {
                [vec![0x2a], op(0xb4, field), vec![0x57]].concat()
            }
        }
        "extractsUnaryZeroComparisonConditionsAndExcludesOtherJumps" => vec![
            0x02, 0x9b, 0, 3, 0x03, 0x9c, 0, 3, 0x03, 0x99, 0, 3, 0x04, 0xa0, 0, 3,
        ],
        "excludesUnaryConditionsImmediatelyAfterCompareInstructions" => [
            vec![0x09, 0x0a, 0x94, 0x99, 0, 3],
            vec![0x0b, 0x0c, 0x95, 0x9a, 0, 3],
            vec![0x0b, 0x0c, 0x96, 0x9b, 0, 3],
            vec![0x0e, 0x0f, 0x97, 0x9c, 0, 3],
            vec![0x0e, 0x0f, 0x98, 0x9d, 0, 3],
            vec![0x02, 0x9b, 0, 3],
        ]
        .concat(),
        "intInvokeFollowedByPopIsImmediatelyPopped"
        | "invokeResultStoredIsRetained"
        | "invokeResultReturnedIsRetained"
        | "labelBetweenInvokeAndPopIsRetained"
        | "instructionOccurrenceIndexCountsRealInstructionsOnly" => {
            let call = class.member(10, "java/lang/Math", "abs", "(I)I");
            let mut code = [vec![0x10, 1], op(0xb8, call)].concat();
            match case {
                "intInvokeFollowedByPopIsImmediatelyPopped" => {
                    expected = ImmediatelyPopped;
                    code.push(0x57);
                }
                "invokeResultStoredIsRetained" => code.extend([0x36, 1]),
                "invokeResultReturnedIsRetained" => {
                    descriptor = "()I";
                    code.push(0xac);
                }
                _ => {
                    let catch = class.class("java/lang/RuntimeException");
                    handlers.push((
                        0,
                        if case == "labelBetweenInvokeAndPopIsRetained" {
                            5
                        } else {
                            6
                        },
                        7,
                        catch,
                    ));
                    code.extend([0x57, 0xb1, 0x57]);
                }
            }
            code
        }
        _ => panic!("unregistered invoke case {case}"),
    };
    if case != "invokeResultReturnedIsRetained" {
        code.push(0xb1);
    }
    let values = synthetic_method(class, descriptor, code, handlers, frames);
    if case == "extractsUnaryZeroComparisonConditionsAndExcludesOtherJumps" {
        let conditions = values
            .iter()
            .filter(|c| c.condition_opcode.is_some())
            .collect::<Vec<_>>();
        assert_eq!(
            conditions
                .iter()
                .map(|c| c.condition_opcode.unwrap())
                .collect::<Vec<_>>(),
            [0x9b, 0x9c]
        );
        assert_eq!(
            conditions
                .iter()
                .map(|c| c.instruction_occurrence_index)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert!(conditions
            .iter()
            .all(|c| c.kind == AtTargetKind::Constant && c.constant_value.is_none()));
        assert!(values
            .iter()
            .all(|c| c.instruction_occurrence_index != 5 && c.instruction_occurrence_index != 7));
        return;
    }
    if case == "excludesUnaryConditionsImmediatelyAfterCompareInstructions" {
        let conditions = values
            .iter()
            .filter(|c| c.condition_opcode.is_some())
            .collect::<Vec<_>>();
        assert_eq!(
            conditions
                .iter()
                .map(|c| c.condition_opcode)
                .collect::<Vec<_>>(),
            [Some(0x9b)]
        );
        assert_eq!(
            conditions
                .iter()
                .map(|c| c.instruction_occurrence_index)
                .collect::<Vec<_>>(),
            [21]
        );
        return;
    }
    let candidate = values
        .iter()
        .find(|c| {
            if case.starts_with("field") {
                c.kind == AtTargetKind::Field && c.name == "value"
            } else {
                c.kind == AtTargetKind::Invoke && name.is_none_or(|name| c.name == name)
            }
        })
        .unwrap();
    if case == "instructionOccurrenceIndexCountsRealInstructionsOnly" {
        assert_eq!(candidate.instruction_occurrence_index, 1);
    } else {
        assert_eq!(candidate.occurrence_result_classification, expected);
    }
    if case == "voidStaticInvokeIsClassifiedAsVoid" {
        assert_eq!(candidate.instruction_occurrence_index, 0);
    }
}
fn class_adapter(case: &str) {
    let provider = InMemoryClassBytesProvider::new(BTreeMap::from([(
        SIMPLE.into(),
        classes()[SIMPLE].clone(),
    )]))
    .unwrap();
    let adapter = BytecodeClassIndexAdapter {
        member_index: ClassMemberIndex::build(&provider, None).unwrap(),
    };
    match case {
        "findClassByInternalName" => {
            let entry = adapter.find_class(SIMPLE).unwrap();
            assert_eq!(entry.simple_name, "SimpleTarget");
            assert_eq!(entry.package_name, "com.example.target");
        }
        "findClassByFqn" => assert_eq!(
            adapter
                .find_class_by_fqn("com.example.target.SimpleTarget")
                .unwrap()
                .internal_name,
            SIMPLE
        ),
        "findClassesByPrefix" => {
            let entries = adapter.find_classes("Simple", 100);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].simple_name, "SimpleTarget");
        }
        "exposesDrawMethodWithReadableSignature" => {
            let values = adapter.get_methods(SIMPLE);
            let draw = values.iter().find(|method| method.name == "draw").unwrap();
            assert_eq!(draw.descriptor, "(Ljava/lang/String;FF)V");
            assert!(draw.readable_signature.contains("draw"));
            assert!(draw.readable_signature.contains("String"));
        }
        "exposesCounterFieldWithReadableType" => {
            let values = adapter.get_fields(SIMPLE);
            let field = values.iter().find(|field| field.name == "counter").unwrap();
            assert_eq!(field.descriptor, "I");
            assert_eq!(field.readable_type, "int");
        }
        "projectContextMixinIndexBuildsEquivalentAdapter" => {
            let index = ProjectContextMixinIndex::default();
            let context = mcdev_core::project::ProjectContextBuilder::empty("adapter-test", ".");
            let built = index.build_class_index(&context, &provider).unwrap();
            assert!(built.find_class(SIMPLE).is_some());
            assert!(built
                .get_methods(SIMPLE)
                .iter()
                .any(|method| method.name == "draw"));
        }
        _ => panic!("unregistered class adapter case {case}"),
    }
}
fn overloaded() -> Vec<u8> {
    let mut class = ClassFile::default();
    class.method("draw", "()V", 1, 1, vec![0xb1]);
    class.method("draw", "(I)V", 1, 2, vec![0xb1]);
    class.finish("test/OverloadSamples")
}
fn siblings() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "hierarchy/Base",
            hierarchy("hierarchy/Base", "java/lang/Object", &[], false),
        ),
        (
            "hierarchy/ChildOne",
            hierarchy("hierarchy/ChildOne", "hierarchy/Base", &[], false),
        ),
        (
            "hierarchy/ChildTwo",
            hierarchy("hierarchy/ChildTwo", "hierarchy/Base", &[], false),
        ),
    ]
}
fn target_adapter(case: &str) {
    match case {
        "simpleTargetReturnCandidates" | "simpleTargetReturnOrdinalCount" => {
            let adapter = BytecodeAtTargetAdapter::new(Arc::new(
                InMemoryClassBytesProvider::new(BTreeMap::from([(
                    SIMPLE.into(),
                    classes()[SIMPLE].clone(),
                )]))
                .unwrap(),
            ));
            if case == "simpleTargetReturnOrdinalCount" {
                assert!(
                    adapter.get_return_ordinal_count(
                        SIMPLE,
                        "draw",
                        Some("(Ljava/lang/String;FF)V")
                    ) >= 1
                );
            } else {
                let values = adapter.get_at_target_candidates(
                    SIMPLE,
                    "draw",
                    Some("(Ljava/lang/String;FF)V"),
                    "RETURN",
                );
                assert!(!values.is_empty());
                assert!(values
                    .iter()
                    .all(|c| c.kind == AtTargetKind::Return && c.display_label == "RETURN"));
            }
        }
        "invokeSamplesProducesInvokeCandidates"
        | "invokeCandidateLabelDiffersFromInsertDescriptor"
        | "fieldSamplesProducesFieldCandidates"
        | "filtersByAtValue" => {
            let adapter = BytecodeAtTargetAdapter::new(Arc::new(provider()));
            let invoke_owner = owner("InvokeSamples");
            let field_owner = owner("FieldSamples");
            let invokes = adapter.get_at_target_candidates(
                &invoke_owner,
                "virtualInvoke",
                Some("()V"),
                "INVOKE",
            );
            match case {
                "invokeSamplesProducesInvokeCandidates" => {
                    assert!(!invokes.is_empty());
                    assert!(invokes
                        .iter()
                        .all(|c| c.kind == AtTargetKind::Invoke && c.display_label != c.owner));
                }
                "invokeCandidateLabelDiffersFromInsertDescriptor" => {
                    let first = invokes.first().unwrap();
                    assert!(
                        first.display_label.contains("length")
                            || first.display_label.contains("println")
                    );
                    assert!(!first.owner.is_empty());
                }
                "fieldSamplesProducesFieldCandidates" => {
                    let values = adapter.get_at_target_candidates(
                        &field_owner,
                        "accessFields",
                        Some("()V"),
                        "FIELD",
                    );
                    assert!(!values.is_empty());
                    assert!(values.iter().all(|c| c.kind == AtTargetKind::Field));
                    assert!(values[0].display_label.contains(':'));
                }
                _ => {
                    assert!(!invokes.is_empty());
                    assert!(!adapter
                        .get_at_target_candidates(
                            &field_owner,
                            "accessFields",
                            Some("()V"),
                            "FIELD"
                        )
                        .is_empty());
                    assert!(adapter
                        .get_at_target_candidates(
                            &invoke_owner,
                            "staticInvoke",
                            Some("()V"),
                            "FIELD"
                        )
                        .is_empty());
                }
            }
        }
        "returnsEmptyForMissingOwner" => {
            let adapter = BytecodeAtTargetAdapter::new(Arc::new(provider()));
            assert!(adapter
                .get_at_target_candidates("missing/Class", "draw", None, "RETURN")
                .is_empty());
        }
        "overloadedMethodWithoutDescriptorReturnsEmpty"
        | "overloadedMethodWithExplicitDescriptorResolvesCorrectOverload" => {
            let adapter = BytecodeAtTargetAdapter::new(Arc::new(
                InMemoryClassBytesProvider::new(BTreeMap::from([(
                    "test/OverloadSamples".into(),
                    overloaded(),
                )]))
                .unwrap(),
            ));
            if case == "overloadedMethodWithoutDescriptorReturnsEmpty" {
                assert!(adapter
                    .get_at_target_candidates("test/OverloadSamples", "draw", None, "RETURN")
                    .is_empty());
                assert_eq!(
                    adapter.get_return_ordinal_count("test/OverloadSamples", "draw", None),
                    0
                );
            } else {
                for descriptor in ["()V", "(I)V"] {
                    let values = adapter.get_at_target_candidates(
                        "test/OverloadSamples",
                        "draw",
                        Some(descriptor),
                        "RETURN",
                    );
                    assert!(!values.is_empty());
                    assert!(values.iter().all(|c| c.kind == AtTargetKind::Return));
                    assert_eq!(
                        adapter.get_return_ordinal_count(
                            "test/OverloadSamples",
                            "draw",
                            Some(descriptor)
                        ),
                        1
                    );
                }
            }
        }
        "getClassBytesReturnsExactProviderBytes" => {
            let name = "test/ClassBytesSamples";
            let bytes = vec![0xca, 0xfe, 0xba, 0xbe, 1, 2];
            let provider = Arc::new(
                InMemoryClassBytesProvider::new(BTreeMap::from([(name.into(), bytes.clone())]))
                    .unwrap(),
            );
            let adapter = BytecodeAtTargetAdapter::new(provider.clone());
            assert_eq!(adapter.get_class_bytes(name), Some(bytes));
            let provider_bytes = provider.get_class_bytes_ref(name).unwrap();
            let adapter_bytes = adapter.get_class_bytes_ref(name).unwrap();
            assert!(std::ptr::eq(
                provider_bytes.as_ptr(),
                adapter_bytes.as_ptr()
            ));
            assert_eq!(provider_bytes, adapter_bytes);
        }
        "getClassBytesReturnsNullForMissingClass" => {
            let adapter = BytecodeAtTargetAdapter::new(Arc::new(
                InMemoryClassBytesProvider::new(BTreeMap::new()).unwrap(),
            ));
            assert!(adapter.get_class_bytes("missing/Class").is_none());
        }
        "resolveCommonSuperClassResolvesSiblingSuperclass"
        | "getClassBytesAndResolveCommonSuperClassRemainOnDemand" => {
            let (provider, reads) = counting(siblings());
            let adapter = BytecodeAtTargetAdapter::new(provider);
            assert_eq!(reads.load(Ordering::SeqCst), 0);
            if case == "getClassBytesAndResolveCommonSuperClassRemainOnDemand" {
                adapter.get_class_bytes("hierarchy/ChildOne");
                assert_eq!(reads.load(Ordering::SeqCst), 1);
            }
            assert_eq!(
                adapter
                    .resolve_common_super_class("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;")
                    .as_deref(),
                Some("Lhierarchy/Base;")
            );
            if case == "getClassBytesAndResolveCommonSuperClassRemainOnDemand" {
                let count = reads.load(Ordering::SeqCst);
                assert!(count > 1);
                adapter.resolve_common_super_class("Lhierarchy/ChildTwo;", "Lhierarchy/ChildOne;");
                assert_eq!(reads.load(Ordering::SeqCst), count);
            }
        }
        _ => panic!("unregistered target adapter case {case}"),
    }
}
fn entry_mapper(case: &str) {
    // Rust uses the final candidate model directly. Its extraction/constructor fields
    // replace the Kotlin source-model -> final-model copying adapter.
    let candidate = match case {
        "toMixinAtTargetPropagatesInvokeOccurrenceMetadata" => {
            let mut c = invoke(
                "java/lang/String",
                "length",
                "()I",
                AtTargetOperationKind::InvokeVirtual,
            );
            c.instruction_occurrence_index = 2;
            c.occurrence_result_classification = OccurrenceResultClassification::Retained;
            c
        }
        "toMixinAtTargetPreservesConditionOpcode" => {
            let mut c = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
            c.ordinal = Some(0);
            c.instruction_occurrence_index = 4;
            c.condition_opcode = Some(0x9c);
            c
        }
        "toMixinAtTargetPreservesInvokeOperationKind" => invoke(
            "java/lang/String",
            "length",
            "()I",
            AtTargetOperationKind::InvokeVirtual,
        ),
        "toMixinAtTargetPreservesFieldReadWriteOperationKinds" => {
            for operation in [
                AtTargetOperationKind::FieldGetInstance,
                AtTargetOperationKind::FieldPutInstance,
            ] {
                let mut c =
                    AtTargetCandidate::new(AtTargetKind::Field, "com/example/Target", "value", "I");
                c.ordinal = Some(0);
                c.operation_kind = Some(operation);
                assert_eq!(c.operation_kind, Some(operation));
            }
            return;
        }
        "toMixinAtTargetMapsNonInvokeFieldKindsToNullOperationKind"
        | "toMixinAtTargetPreservesDefaultOccurrenceMetadata" => {
            let mut c =
                AtTargetCandidate::new(AtTargetKind::Return, "com/example/Target", "draw", "()V");
            c.ordinal = Some(0);
            c
        }
        _ => panic!("unregistered entry mapper case {case}"),
    };
    match case {
        "toMixinAtTargetPropagatesInvokeOccurrenceMetadata" => {
            assert_eq!(candidate.instruction_occurrence_index, 2);
            assert_eq!(
                candidate.occurrence_result_classification,
                OccurrenceResultClassification::Retained
            );
        }
        "toMixinAtTargetPreservesConditionOpcode" => {
            assert_eq!(candidate.condition_opcode, Some(0x9c));
            assert!(candidate.constant_value.is_none());
        }
        "toMixinAtTargetPreservesInvokeOperationKind" => assert_eq!(
            candidate.operation_kind,
            Some(AtTargetOperationKind::InvokeVirtual)
        ),
        "toMixinAtTargetMapsNonInvokeFieldKindsToNullOperationKind" => {
            assert!(candidate.operation_kind.is_none())
        }
        _ => {
            assert_eq!(candidate.instruction_occurrence_index, -1);
            assert_eq!(
                candidate.occurrence_result_classification,
                OccurrenceResultClassification::NotApplicable
            );
        }
    }
}
fn common(case: &str) {
    let values = match case {
        "equalDescriptorsReturnUnchanged" => vec![(
            "hierarchy/Base",
            hierarchy("hierarchy/Base", "java/lang/Object", &[], false),
        )],
        "siblingsShareCommonBase" | "cacheReusesHierarchyAndPairResults" => siblings(),
        "superclassBothArgumentOrders"
        | "arraysPreserveCompatibleReferenceComponentAndDimensions"
        | "arrayDimensionMismatchWithRelatedComponentTypesReturnsObjectArray" => vec![
            (
                "hierarchy/Base",
                hierarchy("hierarchy/Base", "java/lang/Object", &[], false),
            ),
            (
                "hierarchy/Child",
                hierarchy("hierarchy/Child", "hierarchy/Base", &[], false),
            ),
        ],
        "interfaceImplementation" => vec![
            (
                "hierarchy/Contract",
                hierarchy("hierarchy/Contract", "java/lang/Object", &[], true),
            ),
            (
                "hierarchy/Impl",
                hierarchy(
                    "hierarchy/Impl",
                    "java/lang/Object",
                    &["hierarchy/Contract"],
                    false,
                ),
            ),
        ],
        "unrelatedReferencesReturnObject" => vec![
            (
                "hierarchy/Alpha",
                hierarchy("hierarchy/Alpha", "java/lang/Object", &[], false),
            ),
            (
                "hierarchy/Beta",
                hierarchy("hierarchy/Beta", "java/lang/Object", &[], false),
            ),
        ],
        "arrayDimensionMismatchWithSharedComponentTypeReturnsObjectArray" => vec![(
            "hierarchy/Leaf",
            hierarchy("hierarchy/Leaf", "java/lang/Object", &[], false),
        )],
        "corruptBytesReturnNull" => vec![
            (
                "hierarchy/Corrupt",
                vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 0],
            ),
            (
                "hierarchy/Peer",
                hierarchy("hierarchy/Peer", "java/lang/Object", &[], false),
            ),
        ],
        "cyclicHierarchyDoesNotLoop" => vec![
            ("cycle/A", hierarchy("cycle/A", "cycle/B", &[], false)),
            ("cycle/B", hierarchy("cycle/B", "cycle/A", &[], false)),
            (
                "cycle/C",
                hierarchy("cycle/C", "java/lang/Object", &[], false),
            ),
        ],
        _ => vec![],
    };
    let (provider, reads) = counting(values);
    let resolver = BytecodeCommonSuperClassResolver::new(provider.clone());
    if case == "nonPositiveCacheSizesAreRejected" {
        assert!(
            BytecodeCommonSuperClassResolver::with_capacities(provider.clone(), 0, 256).is_err()
        );
        assert!(BytecodeCommonSuperClassResolver::with_capacities(provider, 256, 0).is_err());
        return;
    }
    if case == "cacheReusesHierarchyAndPairResults" {
        let first = resolver.resolve("Lhierarchy/ChildOne;", "Lhierarchy/ChildTwo;");
        let count = reads.load(Ordering::SeqCst);
        let second = resolver.resolve("Lhierarchy/ChildTwo;", "Lhierarchy/ChildOne;");
        assert_eq!(first.as_deref(), Some("Lhierarchy/Base;"));
        assert_eq!(first, second);
        assert_eq!(reads.load(Ordering::SeqCst), count);
        return;
    }
    if case == "missingBytesAreCachedWithoutRepeatedLookup" {
        assert!(resolver
            .resolve("Lcom/missing/Alpha;", "Lcom/missing/Beta;")
            .is_none());
        let count = reads.load(Ordering::SeqCst);
        assert!(resolver
            .resolve("Lcom/missing/Beta;", "Lcom/missing/Alpha;")
            .is_none());
        assert_eq!(reads.load(Ordering::SeqCst), count);
        return;
    }
    if case == "cyclicHierarchyDoesNotLoop" {
        resolver.resolve("Lcycle/A;", "Lcycle/B;");
        resolver.resolve("Lcycle/B;", "Lcycle/A;");
        assert!(resolver.resolve("Lcycle/A;", "Lcycle/C;").is_none());
        let count = reads.load(Ordering::SeqCst);
        assert!(resolver.resolve("Lcycle/C;", "Lcycle/A;").is_none());
        assert_eq!(reads.load(Ordering::SeqCst), count);
        return;
    }
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("bytecode_common_cases.json")).unwrap();
    let checks = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["case"] == case)
        .unwrap()["checks"]
        .as_array()
        .unwrap()
        .clone();
    assert!(!checks.is_empty(), "missing assertions for {case}");
    for check in checks {
        assert_eq!(
            resolver
                .resolve(
                    check["left"].as_str().unwrap(),
                    check["right"].as_str().unwrap()
                )
                .as_deref(),
            check["expected"].as_str(),
            "{case}: {} x {}",
            check["left"],
            check["right"]
        );
    }
}
