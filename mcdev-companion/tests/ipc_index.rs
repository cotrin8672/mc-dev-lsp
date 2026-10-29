use mcdev_companion::{
    commands::{
        host::CommandFailure,
        ipc::index::{EvaluationFactReader, EvaluationIndex},
    },
    host_facts::{
        bridge::FactReader,
        draft::TypeJvm,
        inputs::{InputOps, ScopedInputs},
        FactsError,
    },
};
use mcdev_core::{
    bytecode::{
        BytecodeAtTargetAdapter, BytecodeIndex, ClassMemberIndex, InMemoryClassBytesProvider,
    },
    extras_model::LocalCaptureResult,
    host_source::{HostSourceError, ProjectSourceQuery, SourceAuthority, SourceQueryResult},
    mixinextras_handler::HandlerReferenceTypeResolver,
    mixinextras_locals::LocalCaptureExtractor,
    model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry},
};
use mcdev_protocol::McdevErrorCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone)]
struct Class {
    owner: String,
    origin: String,
    entry: String,
    association: &'static str,
    authority: &'static str,
    abi: Value,
    bytes: Option<Vec<u8>>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
}
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}

fn class(owner: &str, origin: &str, superclass: Option<&str>, interfaces: &[&str]) -> Class {
    Class {
        owner: owner.into(),
        origin: origin.into(),
        entry: format!("entry:{owner}:{origin}"),
        association: "compileClasspath",
        authority: "compiled-input",
        abi: json!({"identity":{"originId":origin,"internalName":owner,"contentSha256":digest(owner.as_bytes())},
            "majorVersion":61,"minorVersion":0,"accessFlags":33,"superName":superclass,"interfaces":interfaces,
            "signature":null,"sourceFile":null,"attributeNames":[],"innerClasses":[],"fields":[],"methods":[]}),
        bytes: None,
    }
}

fn sdk(owner: &str, superclass: Option<&str>, interfaces: &[&str]) -> Class {
    let mut value = class(owner, "sdk:21", superclass, interfaces);
    value.association = "sdk";
    value.authority = "sdk-api-header";
    value
}

fn method(name: &str, descriptor: &str, flags: u16, attributes: &[&str]) -> Value {
    json!({"name":name,"descriptor":descriptor,"accessFlags":flags,"signature":null,"attributeNames":attributes})
}

fn field(name: &str, descriptor: &str, flags: u16) -> Value {
    json!({"name":name,"descriptor":descriptor,"accessFlags":flags,"signature":null,"attributeNames":[]})
}

fn runtime_class(bytes: &[u8]) -> Class {
    let mut result = class(
        "fixture/Annotated",
        "output:1",
        Some("java/lang/Object"),
        &[],
    );
    result.association = "output";
    result.abi["identity"]["contentSha256"] = json!(digest(bytes));
    result.abi["attributeNames"] = json!(["SourceFile", "Deprecated", "RuntimeVisibleAnnotations"]);
    result.abi["methods"] = json!([
        method("<init>", "()V", 1, &["Code"]),
        method(
            "sum",
            "(I)I",
            1,
            &["Code", "Deprecated", "RuntimeVisibleAnnotations"]
        )
    ]);
    result.bytes = Some(bytes.to_vec());
    result
}

#[derive(Default)]
struct HostState {
    classes: Vec<Class>,
    requests: Vec<Vec<Value>>,
    catalog_unknown: bool,
    abi_unknown: BTreeSet<String>,
    fail_op: Option<(String, FactsError)>,
    close_on_op: Option<String>,
}

#[derive(Clone)]
struct Host {
    state: Arc<Mutex<HostState>>,
    active: Arc<AtomicBool>,
}

impl Host {
    fn new(classes: Vec<Class>) -> Self {
        Self {
            state: Arc::new(Mutex::new(HostState {
                classes,
                ..HostState::default()
            })),
            active: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl FactReader for Host {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        if evaluation != "evaluation:42" {
            return Err(FactsError::Protocol("foreign evaluation".into()));
        }
        self.check_active()?;
        let mut state = self.state.lock().unwrap();
        state.requests.push(queries.to_vec());
        let mut values = Vec::new();
        for query in queries {
            let op = query["op"].as_str().unwrap();
            if let Some((failed, error)) = &state.fail_op {
                if failed == op {
                    return Err(error.clone());
                }
            }
            if state.close_on_op.as_deref() == Some(op) {
                self.active.store(false, Ordering::Release);
            }
            let value = match op {
                "jvmCatalog" => {
                    let prefix = query["prefix"].as_str().unwrap_or("");
                    let candidates: Vec<_> = state
                        .classes
                        .iter()
                        .filter(|class| class.owner.starts_with(prefix))
                        .collect();
                    let start = query["after"]
                        .as_str()
                        .map(|cursor| {
                            cursor
                                .strip_prefix("page:")
                                .unwrap()
                                .parse::<usize>()
                                .unwrap()
                        })
                        .unwrap_or(0);
                    let end = (start + 2).min(candidates.len());
                    let classes: Vec<_> = candidates[start..end].iter().map(|class| json!({
                        "entry":class.entry,"internalName":class.owner,"originId":class.origin,
                        "association":class.association,
                        "classpathOrder":state.classes.iter().position(|candidate|candidate.entry == class.entry).unwrap(),
                        "authority":class.authority
                    })).collect();
                    let next = (end < candidates.len()).then(|| format!("page:{end}"));
                    json!({"view":"selectedPhysicalJvmInputs","classes":classes,"next":next,
                        "coverage":if next.is_none() && !state.catalog_unknown {found(Value::Null)} else {unknown()},
                        "problems":[],"sourceCurrent":unknown()})
                }
                "jvmClass" => {
                    let key = query["entry"].as_str().unwrap();
                    let class = state
                        .classes
                        .iter()
                        .find(|class| class.entry == key)
                        .expect("foreign catalog entry");
                    if state.abi_unknown.contains(&class.owner) {
                        unknown()
                    } else {
                        found(
                            json!({"entry":class.entry,"authority":class.authority,"abi":class.abi,"sourceCurrent":unknown()}),
                        )
                    }
                }
                "runtimeClass" => {
                    let owner = query["internalName"].as_str().unwrap();
                    match state.classes.iter().find(|class| class.owner == owner) {
                        None => unknown(),
                        Some(class) if class.bytes.is_none() => found(json!({
                            "id":format!("runtime:{}",class.entry),"internalName":class.owner,"originId":class.origin,
                            "contentSha256":class.abi["identity"]["contentSha256"],"byteLength":0,
                            "authority":"sdk-api-header","codeAvailable":false,"runtimeOnly":false,
                            "classHeader":found(class.abi.clone()),"hierarchy":found(json!({
                                "superName":class.abi["superName"],"interfaces":class.abi["interfaces"],
                                "accessFlags":class.abi["accessFlags"],"signature":class.abi["signature"]})),
                            "runtimeAbiCoverage":unknown(),"sourceCurrent":unknown()
                        })),
                        Some(class) => found(json!({
                            "id":format!("runtime:{}",class.entry),"internalName":class.owner,"originId":class.origin,
                            "contentSha256":class.abi["identity"]["contentSha256"],"byteLength":class.bytes.as_ref().unwrap().len(),
                            "authority":"runtime-bytecode","codeAvailable":true,"runtimeOnly":true,
                            "classHeader":unknown(),"hierarchy":unknown(),"runtimeAbiCoverage":found(Value::Null),"sourceCurrent":unknown()
                        })),
                    }
                }
                "classBytes" => {
                    let id = query["classIdentity"].as_str().unwrap();
                    let key = id.strip_prefix("runtime:").unwrap();
                    let class = state
                        .classes
                        .iter()
                        .find(|class| class.entry == key)
                        .expect("foreign runtime identity");
                    match &class.bytes {
                        Some(bytes) => found(
                            json!({"classIdentity":id,"offset":0,"byteLength":bytes.len(),"bytesBase64":base64(bytes),"next":null}),
                        ),
                        None => {
                            json!({"state":"unknown","reasons":[{"code":"BYTECODE_UNAVAILABLE","detail":"sdk-api-header"}]})
                        }
                    }
                }
                _ => panic!("unexpected op: {op}"),
            };
            values.push(value);
        }
        Ok(json!({"facts":values}))
    }
}

impl EvaluationFactReader for Host {
    fn check_active(&self) -> Result<(), FactsError> {
        if self.active.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(FactsError::Cancelled("evaluation closed".into()))
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() >= 2 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() == 3 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn make_index(host: &Host) -> EvaluationIndex<Host> {
    let ops = InputOps::intersect(&json!({
        "capabilities":["jvm-catalog-v1","runtime-class-bytes-v1"],
        "supportedOps":{"jvm-catalog-v1":["jvmCatalog","jvmClass"],"runtime-class-bytes-v1":["runtimeClass","classBytes"]}
    })).unwrap();
    let inputs = ScopedInputs::new("evaluation:42", &ops, host.clone()).unwrap();
    EvaluationIndex::new(Arc::new(Mutex::new(inputs)), host.clone())
}

fn class_entry(owner: &str) -> ClassIndexEntry {
    let (package, simple) = owner.rsplit_once('/').unwrap_or(("", owner));
    ClassIndexEntry {
        simple_name: simple.into(),
        package_name: package.replace('/', "."),
        internal_name: owner.into(),
    }
}

struct Sources {
    authority: SourceAuthority,
    classes: BTreeMap<String, Vec<MethodIndexEntry>>,
}

impl Sources {
    fn complete() -> Self {
        Self {
            authority: SourceAuthority::Complete,
            classes: BTreeMap::new(),
        }
    }
    fn answer<T>(&self, value: T) -> Result<SourceQueryResult<T>, HostSourceError> {
        Ok(SourceQueryResult::new(value, self.authority))
    }
}

impl ProjectSourceQuery for Sources {
    fn find_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        let prefix = prefix.to_lowercase();
        self.answer(
            self.classes
                .keys()
                .map(|name| class_entry(name))
                .filter(|entry| {
                    entry.simple_name.to_lowercase().starts_with(&prefix)
                        || entry.fqn().to_lowercase().starts_with(&prefix)
                })
                .take(limit)
                .collect(),
        )
    }
    fn find_class(
        &self,
        internal: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.answer(
            self.classes
                .contains_key(internal)
                .then(|| class_entry(internal)),
        )
    }
    fn find_class_by_fqn(
        &self,
        fqn: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.answer(
            self.classes
                .keys()
                .map(|owner| class_entry(owner))
                .find(|entry| entry.fqn().eq_ignore_ascii_case(fqn)),
        )
    }
    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        self.answer(self.classes.get(owner).cloned().unwrap_or_default())
    }
    fn get_fields(
        &self,
        _owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        self.answer(Vec::new())
    }
}

fn host_type(class: &Class, names: &[&str], abi_available: bool) -> TypeJvm {
    let members: Vec<_> = names.iter().enumerate().map(|(index, name)| json!({
        "id":format!("member:{index}"),"name":name,"kind":"method",
        "signature":found(json!({"id":format!("signature:{index}"),"kind":"method","descriptor":"()V"})),
        "dispatch":found(json!("instance")),"visibility":found(json!("public")),
        "synthetic":found(json!(false)),"mismatches":[]
    })).collect();
    serde_json::from_value(json!({
        "binaryName":found(json!({"internalName":class.owner,"descriptor":format!("L{};",class.owner)})),
        "classFlags":unknown(),"declaredMembers":{"members":members,"coverage":found(Value::Null)},
        "abi":if abi_available { found(class.abi.clone()) } else { unknown() },"jvmAbiCoverage":unknown()
    })).unwrap()
}

fn failure(call: impl FnOnce()) -> CommandFailure {
    let value = catch_unwind(AssertUnwindSafe(call)).expect_err("expected typed failure");
    value
        .downcast::<CommandFailure>()
        .map(|value| *value)
        .expect("unexpected panic payload")
}

#[test]
fn source_overlay_beats_stale_binary_and_missing_authority_stays_unknown() {
    let mut binary = class("demo/Thing", "jar:old", Some("java/lang/Object"), &[]);
    binary.abi["methods"] = json!([method("oldMethod", "()V", 1, &[])]);
    let host = Host::new(vec![binary.clone()]);
    let index = make_index(&host);
    assert!(matches!(failure(|| { index.get_methods("demo/Thing"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext));
    assert!(index
        .physical_methods("demo/Thing")
        .unwrap()
        .iter()
        .any(|method| method.name == "oldMethod"));

    let mut source = Sources::complete();
    source.classes.insert(
        "demo/Thing".into(),
        vec![MethodIndexEntry {
            name: "newMethod".into(),
            descriptor: "()V".into(),
            is_static: false,
            readable_signature: "newMethod(): void".into(),
        }],
    );
    let host = Host::new(vec![binary]);
    let index = make_index(&host).with_source_query(Arc::new(source));
    let methods = index.get_methods("demo/Thing");
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "newMethod");
    assert!(index
        .raw_abi("demo/Thing")
        .unwrap()
        .methods
        .iter()
        .any(|method| method.name == "oldMethod"));
}

#[test]
fn editing_members_and_generated_raw_members_have_separate_authority() {
    let mut binary = class("demo/Generated", "jar:1", Some("java/lang/Object"), &[]);
    binary.abi["methods"] = json!([
        method("editable", "()V", 1, &[]),
        method("foo$default", "()V", 0x1048, &[])
    ]);
    let host = Host::new(vec![binary.clone()]);
    let index = make_index(&host);
    index
        .bind_host_type(&host_type(&binary, &["editable"], false))
        .unwrap();
    assert!(index.find_class("demo/Generated").is_some());
    assert_eq!(
        index.host_editing_methods("demo/Generated").unwrap()[0].name,
        "editable"
    );
    assert!(index
        .physical_methods("demo/Generated")
        .unwrap()
        .iter()
        .any(|entry| entry.name == "foo$default" && entry.is_static));
    assert!(
        matches!(failure(|| { index.get_methods("demo/Generated"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext)
    );
    let sourced = make_index(&host).with_source_query(Arc::new(Sources::complete()));
    assert!(sourced
        .get_methods("demo/Generated")
        .iter()
        .any(|entry| entry.name == "foo$default"));
}

#[test]
fn physical_catalog_needs_complete_coverage_and_explicit_duplicate_origin() {
    let mut old = class("demo/Shared", "jar:v1", Some("java/lang/Object"), &[]);
    old.abi["methods"] = json!([method("old", "()V", 1, &[])]);
    let mut new = class("demo/Shared", "jar:v2", Some("java/lang/Object"), &[]);
    new.abi["methods"] = json!([method("new", "()V", 1, &[])]);
    let host = Host::new(vec![old.clone(), new.clone()]);
    let index = make_index(&host);
    assert!(matches!(
        index.raw_abi("demo/Shared"),
        Err(FactsError::Unknown(_))
    ));
    index
        .bind_host_type(&host_type(&new, &["new"], true))
        .unwrap();
    assert_eq!(
        index.raw_abi("demo/Shared").unwrap().identity.origin_id,
        "jar:v2"
    );
    assert_eq!(
        index.physical_methods("demo/Shared").unwrap()[0].name,
        "new"
    );
    assert!(matches!(
        index.bind_host_type(&host_type(&old, &["old"], true)),
        Err(FactsError::Stale(_))
    ));

    let host = Host::new(vec![new]);
    host.state.lock().unwrap().catalog_unknown = true;
    let index = make_index(&host);
    assert!(matches!(
        index.raw_abi("demo/Shared"),
        Err(FactsError::Unknown(_))
    ));
    assert!(matches!(failure(|| { index.find_classes("Shared", 20); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext));
}

#[test]
fn source_complete_lookup_merges_physical_names_but_never_assumes_unknown_absence() {
    let host = Host::new(vec![class(
        "demo/ClassThing",
        "jar:1",
        Some("java/lang/Object"),
        &[],
    )]);
    let index = make_index(&host);
    assert!(matches!(failure(|| { index.find_class("demo/Missing"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext));
    let mut source = Sources::complete();
    source.classes.insert("demo/ClassSource".into(), Vec::new());
    let index = make_index(&host).with_source_query(Arc::new(source));
    let names: Vec<_> = index
        .find_classes("class", 20)
        .into_iter()
        .map(|entry| entry.internal_name)
        .collect();
    assert_eq!(names, ["demo/ClassSource", "demo/ClassThing"]);
    assert_eq!(
        index
            .find_class_by_fqn("demo.classthing")
            .unwrap()
            .internal_name,
        "demo/ClassThing"
    );
    assert!(index.find_class("demo/Missing").is_none());
}

#[test]
fn sdk_headers_support_closed_hierarchy_and_arrays_without_granting_code() {
    let classes = vec![
        sdk("java/lang/Object", None, &[]),
        sdk("java/lang/Cloneable", Some("java/lang/Object"), &[]),
        sdk("java/io/Serializable", Some("java/lang/Object"), &[]),
        sdk("java/lang/Number", Some("java/lang/Object"), &[]),
        sdk("java/lang/Integer", Some("java/lang/Number"), &[]),
        class("demo/Interface", "jar:1", Some("java/lang/Object"), &[]),
        class("demo/Parent", "jar:1", Some("java/lang/Object"), &[]),
        class(
            "demo/Child",
            "jar:1",
            Some("demo/Parent"),
            &["demo/Interface"],
        ),
        class("demo/Other", "jar:1", Some("java/lang/Object"), &[]),
    ];
    let host = Host::new(classes);
    let index = make_index(&host);
    assert!(index
        .physical_is_assignable("java/lang/Number", "java/lang/Integer")
        .unwrap());
    assert!(index
        .physical_is_assignable("demo/Parent", "demo/Child")
        .unwrap());
    assert!(index
        .physical_is_assignable("demo/Interface", "demo/Child")
        .unwrap());
    assert!(!index
        .physical_is_assignable("demo/Other", "demo/Child")
        .unwrap());
    assert!(index
        .physical_is_assignable("java/lang/Cloneable", "[I")
        .unwrap());
    assert!(index
        .physical_is_assignable("[Ljava/lang/Object;", "[Ldemo/Child;")
        .unwrap());
    assert!(!index.physical_is_assignable("[I", "[J").unwrap());
    assert_eq!(
        index
            .physical_common_super_class("Ldemo/Child;", "Ldemo/Parent;")
            .unwrap(),
        "Ldemo/Parent;"
    );
    assert!(
        matches!(failure(|| { index.is_assignable("demo/Parent", "demo/Child"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext)
    );
    assert!(
        matches!(failure(|| { index.get_class_bytes("java/lang/Number"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::BytecodeUnavailable)
    );
    let current = make_index(&host).with_source_query(Arc::new(Sources::complete()));
    assert_eq!(
        current.is_assignable("demo/Parent", "demo/Child"),
        Some(true)
    );
    assert_eq!(
        current.is_assignable("demo/Other", "demo/Child"),
        Some(false)
    );
    assert_eq!(current.is_known_reference("demo/Missing"), Some(false));
}

#[test]
fn unresolved_hierarchy_never_becomes_false_but_other_proven_branch_can_succeed() {
    let mut leaf = class("demo/Leaf", "jar:1", Some("demo/Missing"), &["demo/Marker"]);
    leaf.abi["methods"] = json!([method("m", "()V", 1, &[])]);
    let host = Host::new(vec![
        sdk("java/lang/Object", None, &[]),
        class("demo/Other", "jar:1", Some("java/lang/Object"), &[]),
        class("demo/Marker", "jar:1", Some("java/lang/Object"), &[]),
        leaf,
    ]);
    let index = make_index(&host);
    assert!(index
        .physical_is_assignable("java/lang/Object", "demo/Leaf")
        .unwrap());
    assert!(index
        .physical_is_assignable("demo/Marker", "demo/Leaf")
        .unwrap());
    assert!(matches!(
        index.physical_is_assignable("demo/Other", "demo/Leaf"),
        Err(FactsError::Unknown(_))
    ));
}

#[test]
fn explicit_physical_view_preserves_class_and_member_results_without_claiming_source_absence() {
    let mut sdk_type = sdk("java/util/List", Some("java/lang/Object"), &[]);
    sdk_type.abi["methods"] = json!([
        method("size", "()I", 1, &[]),
        method("bridge", "()V", 0x1048, &[])
    ]);
    sdk_type.abi["fields"] = json!([field("FLAG", "I", 0x0008)]);
    let host = Host::new(vec![sdk("java/lang/Object", None, &[]), sdk_type]);
    let index = Arc::new(make_index(&host));
    let physical = index.physical_view();
    let names: Vec<_> = physical
        .find_classes("li", 10)
        .into_iter()
        .map(|entry| entry.internal_name)
        .collect();
    assert_eq!(names, ["java/util/List"]);
    assert_eq!(
        physical
            .find_class_by_fqn("java.util.list")
            .unwrap()
            .internal_name,
        "java/util/List"
    );
    assert!(physical.find_class("java/util/Missing").is_none());
    let methods = physical.get_methods("java/util/List");
    assert!(methods
        .iter()
        .any(|method| method.name == "bridge" && method.is_static));
    assert!(physical
        .get_fields("java/util/List")
        .iter()
        .any(|field| field.name == "FLAG" && field.is_static));
    assert!(index
        .raw_abi("java/util/List")
        .unwrap()
        .methods
        .iter()
        .any(|method| method.name == "bridge" && method.access_flags == 0x1048));
    assert!(matches!(failure(|| { index.find_classes("li", 10); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext));
    assert!(
        matches!(failure(|| { index.get_methods("java/util/List"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext)
    );
    assert_eq!(
        physical.is_assignable("java/lang/Object", "java/util/List"),
        Some(true)
    );
}

#[test]
fn runtime_bytes_and_real_core_preserve_code_locals_and_annotations() {
    let host = Host::new(vec![runtime_class(ANNOTATED)]);
    let index = make_index(&host);
    let returned = index.get_class_bytes("fixture/Annotated").unwrap();
    assert_eq!(returned, ANNOTATED);
    for marker in [
        b"Code".as_slice(),
        b"LocalVariableTable",
        b"RuntimeVisibleAnnotations",
    ] {
        assert!(returned
            .windows(marker.len())
            .any(|window| window == marker));
    }
    let abi = index.raw_abi("fixture/Annotated").unwrap();
    assert!(abi
        .attribute_names
        .iter()
        .any(|name| name == "RuntimeVisibleAnnotations"));
    assert!(abi
        .methods
        .iter()
        .find(|entry| entry.name == "sum")
        .unwrap()
        .attribute_names
        .iter()
        .any(|name| name == "RuntimeVisibleAnnotations"));
    let selected = BTreeSet::from([4]);
    let actual = LocalCaptureExtractor::extract(Some(&returned), "sum", "(I)I", &selected).unwrap();
    let expected =
        LocalCaptureExtractor::extract(Some(ANNOTATED), "sum", "(I)I", &selected).unwrap();
    assert_eq!(actual, expected);
    assert!(
        matches!(&actual, LocalCaptureResult::Success { snapshots } if snapshots.iter().flat_map(|snapshot|&snapshot.candidates).any(|candidate|candidate.name.as_deref()==Some("local")))
    );
    let provider = InMemoryClassBytesProvider::new(BTreeMap::from([(
        "fixture/Annotated".into(),
        ANNOTATED.to_vec(),
    )]))
    .unwrap();
    let baseline = BytecodeAtTargetAdapter::new(Arc::new(provider));
    assert_eq!(
        index.get_at_target_candidates("fixture/Annotated", "sum", Some("(I)I"), "RETURN"),
        baseline.get_at_target_candidates("fixture/Annotated", "sum", Some("(I)I"), "RETURN")
    );
    assert_eq!(
        index.get_return_ordinal_count("fixture/Annotated", "sum", Some("(I)I")),
        1
    );
    let expected_members = ClassMemberIndex::index_class(ANNOTATED, None, None).unwrap();
    assert_eq!(
        index.physical_methods("fixture/Annotated").unwrap(),
        expected_members.methods_by_owner["fixture/Annotated"]
    );
}

#[test]
fn explicit_runtime_view_uses_host_runtime_selection_and_respects_bound_type_origin() {
    let first = runtime_class(ANNOTATED);
    let mut second = runtime_class(ANNOTATED);
    second.origin = "output:2".into();
    second.entry = "entry:fixture/Annotated:output:2".into();
    second.abi["identity"]["originId"] = json!("output:2");
    let host = Host::new(vec![first, second.clone()]);
    let index = Arc::new(make_index(&host));
    let physical = index.physical_view();
    assert_eq!(
        physical.get_class_bytes("fixture/Annotated").unwrap(),
        ANNOTATED
    );
    assert!(
        matches!(failure(|| { index.get_class_bytes("fixture/Annotated"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext)
    );
    index
        .bind_host_type(&host_type(&second, &["sum"], true))
        .unwrap();
    assert!(
        matches!(failure(|| { physical.get_class_bytes("fixture/Annotated"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext)
    );
}

#[test]
fn evaluation_close_and_stale_fail_even_when_cache_is_populated() {
    let host = Host::new(vec![class(
        "demo/Cached",
        "jar:1",
        Some("java/lang/Object"),
        &[],
    )]);
    let index = make_index(&host);
    assert!(index.raw_abi("demo/Cached").is_ok());
    let calls = host.state.lock().unwrap().requests.len();
    host.active.store(false, Ordering::Release);
    assert!(matches!(
        index.raw_abi("demo/Cached"),
        Err(FactsError::Cancelled(_))
    ));
    assert_eq!(host.state.lock().unwrap().requests.len(), calls);
    assert!(matches!(
        failure(|| {
            index.find_class("demo/Cached");
        }),
        CommandFailure::Cancelled(_)
    ));

    let host = Host::new(vec![class(
        "demo/Stale",
        "jar:1",
        Some("java/lang/Object"),
        &[],
    )]);
    host.state.lock().unwrap().fail_op =
        Some(("jvmCatalog".into(), FactsError::Stale("changed".into())));
    let index = make_index(&host);
    assert!(matches!(
        index.raw_abi("demo/Stale"),
        Err(FactsError::Stale(_))
    ));

    let host = Host::new(vec![class(
        "demo/ClosedDuringRead",
        "jar:1",
        Some("java/lang/Object"),
        &[],
    )]);
    host.state.lock().unwrap().close_on_op = Some("jvmCatalog".into());
    let index = make_index(&host);
    assert!(matches!(
        index.raw_abi("demo/ClosedDuringRead"),
        Err(FactsError::Cancelled(_))
    ));
}

#[test]
fn partial_source_or_incomplete_host_editing_never_returns_empty_success() {
    let host = Host::new(vec![class(
        "demo/Known",
        "jar:1",
        Some("java/lang/Object"),
        &[],
    )]);
    let source = Sources {
        authority: SourceAuthority::Partial,
        classes: BTreeMap::new(),
    };
    let index = make_index(&host).with_source_query(Arc::new(source));
    assert!(matches!(failure(|| { index.get_methods("demo/Known"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext));
    assert!(matches!(failure(|| { index.find_class("demo/Missing"); }),
        CommandFailure::Unavailable(error) if error.code == McdevErrorCode::IncompleteProjectContext));
}

// JDK 21 javac -g; SHA-256 93425c7a3166e7a56a6ffd2c1b91d2a844938a7ee0c2eb762d268f2f76959f22.
const ANNOTATED: &[u8] = &[
    202, 254, 186, 190, 0, 0, 0, 65, 0, 24, 10, 0, 2, 0, 3, 7, 0, 4, 12, 0, 5, 0, 6, 1, 0, 16, 106,
    97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 1, 0, 6, 60, 105, 110, 105,
    116, 62, 1, 0, 3, 40, 41, 86, 7, 0, 8, 1, 0, 17, 102, 105, 120, 116, 117, 114, 101, 47, 65,
    110, 110, 111, 116, 97, 116, 101, 100, 1, 0, 4, 67, 111, 100, 101, 1, 0, 15, 76, 105, 110, 101,
    78, 117, 109, 98, 101, 114, 84, 97, 98, 108, 101, 1, 0, 18, 76, 111, 99, 97, 108, 86, 97, 114,
    105, 97, 98, 108, 101, 84, 97, 98, 108, 101, 1, 0, 4, 116, 104, 105, 115, 1, 0, 19, 76, 102,
    105, 120, 116, 117, 114, 101, 47, 65, 110, 110, 111, 116, 97, 116, 101, 100, 59, 1, 0, 3, 115,
    117, 109, 1, 0, 4, 40, 73, 41, 73, 1, 0, 8, 97, 114, 103, 117, 109, 101, 110, 116, 1, 0, 1, 73,
    1, 0, 5, 108, 111, 99, 97, 108, 1, 0, 10, 68, 101, 112, 114, 101, 99, 97, 116, 101, 100, 1, 0,
    25, 82, 117, 110, 116, 105, 109, 101, 86, 105, 115, 105, 98, 108, 101, 65, 110, 110, 111, 116,
    97, 116, 105, 111, 110, 115, 1, 0, 22, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 68,
    101, 112, 114, 101, 99, 97, 116, 101, 100, 59, 1, 0, 10, 83, 111, 117, 114, 99, 101, 70, 105,
    108, 101, 1, 0, 14, 65, 110, 110, 111, 116, 97, 116, 101, 100, 46, 106, 97, 118, 97, 0, 33, 0,
    7, 0, 2, 0, 0, 0, 0, 0, 2, 0, 1, 0, 5, 0, 6, 0, 1, 0, 9, 0, 0, 0, 47, 0, 1, 0, 1, 0, 0, 0, 5,
    42, 183, 0, 1, 177, 0, 0, 0, 2, 0, 10, 0, 0, 0, 6, 0, 1, 0, 0, 0, 4, 0, 11, 0, 0, 0, 12, 0, 1,
    0, 0, 0, 5, 0, 12, 0, 13, 0, 0, 0, 1, 0, 14, 0, 15, 0, 3, 0, 9, 0, 0, 0, 72, 0, 2, 0, 3, 0, 0,
    0, 6, 27, 4, 96, 61, 28, 172, 0, 0, 0, 2, 0, 10, 0, 0, 0, 10, 0, 2, 0, 0, 0, 7, 0, 4, 0, 8, 0,
    11, 0, 0, 0, 32, 0, 3, 0, 0, 0, 6, 0, 12, 0, 13, 0, 0, 0, 0, 0, 6, 0, 16, 0, 17, 0, 1, 0, 4, 0,
    2, 0, 18, 0, 17, 0, 2, 0, 19, 0, 0, 0, 0, 0, 20, 0, 0, 0, 6, 0, 1, 0, 21, 0, 0, 0, 3, 0, 22, 0,
    0, 0, 2, 0, 23, 0, 19, 0, 0, 0, 0, 0, 20, 0, 0, 0, 6, 0, 1, 0, 21, 0, 0,
];
