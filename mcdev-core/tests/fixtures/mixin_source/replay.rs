//! Exact legacy input/whole-result replay. The JVM is used only by the one-time generator.
use mcdev_core::{mixin_source::*, *};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else {
            out.push(if upper { c.to_ascii_uppercase() } else { c });
            upper = false;
        }
    }
    out
}
fn snake(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
fn enum_key(k: &str) -> bool {
    matches!(
        k,
        "annotation"
            | "slot"
            | "parseSource"
            | "confidence"
            | "typeKind"
            | "annotationKind"
            | "parentInjectorAnnotation"
            | "expressionCompletionPosition"
            | "modifiers"
    )
}
pub fn canonical(value: Value, key: &str) -> Value {
    if matches!(key, "explicit" | "ambiguousExplicit" | "metadata") {
        return value;
    }
    match value {
        Value::Object(map) => {
            if map.len() == 1 {
                let (k, v) = map.iter().next().unwrap();
                if matches!(
                    k.as_str(),
                    "Resolved"
                        | "Unresolved"
                        | "Ambiguous"
                        | "Found"
                        | "MixinTarget"
                        | "InjectMethod"
                        | "AtValue"
                        | "AtTarget"
                        | "AccessorValue"
                        | "InvokerValue"
                        | "MemberName"
                ) {
                    return canonical(v.clone(), key);
                }
            }
            Value::Object(
                map.into_iter()
                    .map(|(k, v)| {
                        let k = camel(&k);
                        let v = canonical(v, &k);
                        (k, v)
                    })
                    .collect(),
            )
        }
        Value::Array(a) => {
            let mut a: Vec<_> = a.into_iter().map(|v| canonical(v, key)).collect();
            if matches!(
                key,
                "existingAttributes"
                    | "modifiers"
                    | "acceptedTypeDescriptors"
                    | "acceptedOperationGenericDescriptors"
                    | "acceptedReturnTypeDescriptors"
                    | "names"
                    | "expandZeroConditions"
            ) {
                a.sort_by_key(Value::to_string);
            }
            Value::Array(a)
        }
        Value::String(s) if enum_key(key) => Value::String(s.replace('_', "").to_ascii_lowercase()),
        other => other,
    }
}
fn pascal(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or(String::new(), |c| {
                c.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
            })
        })
        .collect()
}
fn rust_input(value: Value, key: &str) -> Value {
    if matches!(key, "explicit" | "ambiguousExplicit" | "metadata") {
        return value;
    }
    match value {
        Value::Object(m) => Value::Object(
            m.into_iter()
                .map(|(k, v)| {
                    let v = rust_input(v, &k);
                    (snake(&k), v)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(|v| rust_input(v, key)).collect()),
        Value::String(s) if enum_key(key) => Value::String(pascal(&s)),
        other => other,
    }
}
pub fn deserialize<T: serde::de::DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone())
        .or_else(|_| serde_json::from_value(rust_input(value.clone(), "")))
        .or_else(|_| {
            serde_json::from_value(
                value
                    .as_str()
                    .map(|s| json!(pascal(s)))
                    .unwrap_or(value.clone()),
            )
        })
        .unwrap()
}
pub fn byte(source: &str, units: i64) -> Option<usize> {
    if units < 0 {
        return None;
    }
    let mut count = 0;
    for (at, c) in source.char_indices() {
        if count == units as usize {
            return Some(at);
        }
        count += c.len_utf16();
    }
    if count == units as usize {
        Some(source.len())
    } else {
        None
    }
}
pub fn normalize_expected_offsets(value: &mut Value, source: &str, key: &str) {
    match value {
        Value::Object(m) => {
            for (k, v) in m {
                normalize_expected_offsets(v, source, k);
            }
        }
        Value::Array(a) => {
            for v in a {
                normalize_expected_offsets(v, source, key)
            }
        }
        Value::Number(n) if key.ends_with("Offset") => {
            if let Some(n) = n.as_i64().and_then(|n| byte(source, n)) {
                *value = json!(n);
            }
        }
        _ => {}
    }
}
struct Reads<'a>(&'a [Value]);
impl Reads<'_> {
    fn get(&self, api: &str, args: Value) -> Option<&Value> {
        self.0
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .map(|r| &r["output"])
    }
    fn classes(&self) -> Vec<ClassIndexEntry> {
        let mut out = BTreeMap::new();
        for r in self.0 {
            let v = &r["output"];
            let values: Vec<_> = if let Some(a) = v.as_array() {
                a.iter().collect()
            } else {
                vec![v]
            };
            for v in values {
                if v.get("simpleName").is_some() && v.get("packageName").is_some() {
                    let c: ClassIndexEntry = deserialize(v);
                    out.insert(c.internal_name.clone(), c);
                }
            }
        }
        out.into_values().collect()
    }
}
impl ClassIndex for Reads<'_> {
    fn find_classes(&self, p: &str, l: usize) -> Vec<ClassIndexEntry> {
        if let Some(v) = self.get("ClassIndex.findClasses", json!([p, l])) {
            return deserialize(v);
        }
        self.classes()
            .into_iter()
            .filter(|c| c.simple_name.starts_with(p))
            .take(l)
            .collect()
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        if let Some(v) = self.get("ClassIndex.findClass", json!([n])) {
            return if v.is_null() {
                None
            } else {
                Some(deserialize(v))
            };
        }
        self.classes().into_iter().find(|c| c.internal_name == n)
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        if let Some(v) = self.get("ClassIndex.findClassByFqn", json!([n])) {
            return if v.is_null() {
                None
            } else {
                Some(deserialize(v))
            };
        }
        self.classes()
            .into_iter()
            .find(|c| c.internal_name.replace('/', ".").replace('$', ".") == n)
    }
    fn get_methods(&self, n: &str) -> Vec<MethodIndexEntry> {
        self.get("ClassIndex.getMethods", json!([n]))
            .map_or(vec![], deserialize)
    }
    fn get_fields(&self, n: &str) -> Vec<FieldIndexEntry> {
        self.get("ClassIndex.getFields", json!([n]))
            .map_or(vec![], deserialize)
    }
}
fn lookup_value(v: Option<&Value>) -> TypeLookupResult {
    match v {
        Some(v) if v.get("internalName").is_some() => TypeLookupResult::Found {
            internal_name: v["internalName"].as_str().unwrap().into(),
        },
        Some(v) if v.get("internalNames").is_some() => TypeLookupResult::Ambiguous {
            internal_names: deserialize(&v["internalNames"]),
        },
        _ => TypeLookupResult::NotFound,
    }
}
impl JavaTypeLookup for Reads<'_> {
    fn find_internal_name_by_qualified_name(&self, n: &str) -> Option<String> {
        self.get("JavaTypeLookup.findInternalNameByQualifiedName", json!([n]))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| self.find_class_by_fqn(n).map(|c| c.internal_name))
    }
    fn find_in_package(&self, p: &str, n: &str) -> TypeLookupResult {
        lookup_value(self.get("JavaTypeLookup.findInPackage", json!([p, n])))
    }
    fn find_in_wildcard_imports(&self, p: &[String], n: &str) -> TypeLookupResult {
        lookup_value(self.get("JavaTypeLookup.findInWildcardImports", json!([p, n])))
    }
}
pub fn replay(row: &Value) -> Value {
    let args = &row["args"];
    let source = args["source"].as_str().unwrap_or("");
    let offset = |key: &str| byte(source, args[key].as_i64().unwrap_or(0));
    let num = |key: &str| args[key].as_u64().unwrap_or(0) as usize;
    let text = |key: &str| args[key].as_str().unwrap_or("");
    let api = row["api"].as_str().unwrap();
    let reads = Reads(row["reads"].as_array().unwrap());
    let index = args
        .get("classIndex")
        .filter(|v| !v.is_null())
        .map(|_| &reads as &dyn ClassIndex);
    match api {
        "AnnotationContextExtractor.extract" => json!(AnnotationContextExtractor::extract(
            source,
            num("line"),
            num("character")
        )),
        "AnnotationContextExtractor.extractAtOffset" => json!(offset("cursorOffset")
            .and_then(|o| AnnotationContextExtractor::extract_at_offset(source, o))),
        "AnnotationContextExtractor.findAnnotationOffsets" => {
            let a = MixinAnnotation::ALL
                .into_iter()
                .find(|a| {
                    a.simple_name().to_uppercase().replace('_', "")
                        == text("annotation").replace('_', "")
                })
                .unwrap();
            let imports = args
                .get("imports")
                .filter(|v| !v.is_null())
                .map(deserialize::<JavaSourceImports>);
            json!(if let Some(imports) = imports {
                annotations_with_imports(source, &imports)
                    .iter()
                    .filter(|s| s.annotation == a)
                    .map(|s| s.start)
                    .collect::<Vec<_>>()
            } else {
                AnnotationContextExtractor::find_annotation_offsets(source, a)
            })
        }
        "AnnotationContextExtractor.findInjectorAnnotationOffsets" => {
            json!(AnnotationContextExtractor::find_injector_annotation_offsets(source))
        }
        "AnnotationContextExtractor.extractAtAnnotationBodies" => json!(
            AnnotationContextExtractor::extract_at_annotation_bodies(source)
        ),
        "AnnotationContextExtractor.parseMixinTargetValues" => {
            json!(AnnotationContextExtractor::parse_mixin_target_values(
                source,
                byte(
                    source,
                    args.get("mixinAtOffset")
                        .or_else(|| args.get("mixinAt"))
                        .or_else(|| args.get("annotationStart"))
                        .or_else(|| args.get("annotationStartOffset"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                )
                .unwrap_or(0)
            ))
        }
        "AnnotationContextExtractor.resolveMixinTargets"
        | "AnnotationContextExtractor.resolveRawMixinTargets" => {
            json!(AnnotationContextExtractor::resolve_raw_mixin_targets(
                source,
                offset("cursorOffset").unwrap_or(0)
            ))
        }
        "AnnotationContextExtractor.resolveMixinClassScope" => json!(offset("cursorOffset")
            .and_then(|o| AnnotationContextExtractor::resolve_mixin_class_scope(source, o))),
        "AnnotationContextExtractor.fqnToInternal" => {
            json!(AnnotationContextExtractor::fqn_to_internal(text("fqn")))
        }
        "AnnotationContextExtractor.toOffset" => json!(position_to_offset(
            source,
            McTextPosition {
                line: num("line"),
                character: num("character")
            }
        )),
        "JavaTypeDescriptorResolver.importsFor" => json!(
            JavaTypeDescriptorResolver::imports_for_uri(source, text("sourceUri"))
        ),
        "JavaTypeDescriptorResolver.isKotlinSource" => json!(
            JavaTypeDescriptorResolver::is_kotlin_source(source, text("sourceUri"))
        ),
        "JavaTypeDescriptorResolver.splitTopLevel" => json!(split_top_level(
            text("value"),
            text("delimiter").chars().next().unwrap()
        )),
        "JavaTypeDescriptorResolver.rawParameterTypes" => json!(
            JavaTypeDescriptorResolver::raw_parameter_types(text("rawParams"))
        ),
        name if name.starts_with("JavaTypeDescriptorResolver.") => {
            let imp: JavaSourceImports =
                deserialize(args.get("imports").unwrap_or(&args["context"]["imports"]));
            let has_lookup = args["context"]["hasLookup"].as_bool().unwrap_or(false);
            let ctx = JavaTypeResolutionContext {
                imports: &imp,
                lookup: has_lookup.then_some(&reads as &dyn JavaTypeLookup),
            };
            match name {
                "JavaTypeDescriptorResolver.descriptorOrDiagnostic" => json!(
                    JavaTypeDescriptorResolver::descriptor_or_diagnostic(text("rawType"), &ctx)
                ),
                "JavaTypeDescriptorResolver.descriptor" => json!(
                    JavaTypeDescriptorResolver::descriptor(text("rawType"), &imp)
                ),
                "JavaTypeDescriptorResolver.descriptorOrNull" => json!(
                    JavaTypeDescriptorResolver::descriptor_or_null(text("rawType"), &ctx)
                ),
                "JavaTypeDescriptorResolver.methodDescriptor" => {
                    json!(JavaTypeDescriptorResolver::method_descriptor(
                        text("returnType"),
                        &deserialize::<Vec<String>>(&args["parameterTypes"]),
                        &imp
                    ))
                }
                "JavaTypeDescriptorResolver.methodDescriptorOrNull" => {
                    json!(JavaTypeDescriptorResolver::method_descriptor_or_null(
                        text("returnType"),
                        &deserialize::<Vec<String>>(&args["parameterTypes"]),
                        &ctx
                    ))
                }
                "JavaTypeDescriptorResolver.parameterTypes" => json!(
                    JavaTypeDescriptorResolver::raw_parameter_types(text("rawParams"))
                        .iter()
                        .filter_map(|p| JavaTypeDescriptorResolver::descriptor_or_null(p, &ctx))
                        .collect::<Vec<_>>()
                ),
                _ => panic!("unsupported {name}"),
            }
        }
        "MixinMemberDeclarationParser.parseShadowDeclarations" => json!(
            MixinMemberDeclarationParser::parse_shadow_declarations(source, index)
        ),
        "MixinMemberDeclarationParser.parseAccessorDeclarations" => json!(
            MixinMemberDeclarationParser::parse_accessor_declarations(source, index)
        ),
        "MixinMemberDeclarationParser.parseInvokerDeclarations" => json!(
            MixinMemberDeclarationParser::parse_invoker_declarations(source, index)
        ),
        "MixinMemberDeclarationParser.parseOverwriteDeclarations" => json!(
            MixinMemberDeclarationParser::parse_overwrite_declarations(source, index)
        ),
        "MixinMemberDeclarationParser.parseDeclarationDiagnostics" => json!(
            MixinMemberDeclarationParser::parse_declaration_diagnostics(source, index)
        ),
        "MixinMemberDeclarationParser.findShadowPrefix" => {
            json!(MixinMemberDeclarationParser::find_shadow_prefix(source))
        }
        "MixinMemberDeclarationParser.findShadowRemap" => {
            json!(MixinMemberDeclarationParser::find_shadow_remap(source))
        }
        "MixinMemberDeclarationParser.findMemberNameTextRange" => {
            json!(MixinMemberDeclarationParser::find_member_name_text_range(
                source,
                deserialize(&args["declarationRange"]),
                text("memberName"),
                text("annotationName"),
                args["isMethod"].as_bool().unwrap()
            ))
        }
        "MixinTargetResolver.resolveTarget" => {
            let imp = args
                .get("imports")
                .filter(|v| !v.is_null())
                .map(deserialize::<JavaSourceImports>);
            json!(MixinTargetResolver::resolve_target(
                text("raw"),
                &reads,
                imp.as_ref()
            ))
        }
        "MixinTargetResolver.resolveTargets" => {
            let imp = args
                .get("imports")
                .filter(|v| !v.is_null())
                .map(deserialize::<JavaSourceImports>);
            json!(MixinTargetResolver::resolve_targets(
                &deserialize::<Vec<String>>(&args["rawTargets"]),
                &reads,
                imp.as_ref()
            ))
        }
        "MixinTargetResolver.resolveTargetsFromSource" => json!(
            MixinTargetResolver::resolve_targets_from_source(source, &reads)
        ),
        "MixinSemanticModelParser.parse" => json!(MixinSemanticModelParser::parse_with_metadata(
            source,
            text("sourceUri"),
            deserialize(&args["parseSource"]),
            deserialize(&args["confidence"]),
            deserialize(&args["warnings"]),
            deserialize(&args["debugInfo"]),
            vec![]
        )),
        "SemanticCompletionContextExtractor.extract" => {
            let model = args
                .get("model")
                .filter(|v| !v.is_null())
                .map(deserialize::<MixinClassModel>);
            json!(SemanticCompletionContextExtractor::extract(
                source,
                num("line"),
                num("character"),
                model.as_ref()
            ))
        }
        "SemanticCompletionContextExtractor.toAnnotationContext" => {
            let model = args
                .get("model")
                .filter(|v| !v.is_null())
                .map(deserialize::<MixinClassModel>);
            let v = &args["context"];
            let kind = if v.get("valueStartOffset").is_some() {
                "MemberName"
            } else if v.get("member").is_some() {
                if v["member"]["annotationKind"] == "ACCESSOR" {
                    "AccessorValue"
                } else {
                    "InvokerValue"
                }
            } else if v.get("atSelector").is_some() {
                if v.get("owner").is_some() {
                    "AtTarget"
                } else {
                    "AtValue"
                }
            } else if v.get("injector").is_some() {
                "InjectMethod"
            } else {
                "MixinTarget"
            };
            let mut data = v.clone();
            normalize_expected_offsets(&mut data, source, "");
            let context: MixinCompletionContext =
                serde_json::from_value(json!({kind:rust_input(data,"")})).unwrap();
            json!(SemanticCompletionContextExtractor::to_annotation_context(
                source,
                num("line"),
                num("character"),
                model.as_ref(),
                &context
            ))
        }
        "JavaStringContentDecoder.decodeContent" => {
            json!(byte(source, args["contentStart"].as_i64().unwrap())
                .zip(byte(source, args["contentEnd"].as_i64().unwrap()))
                .and_then(|(a, b)| JavaStringContentDecoder::decode_content(source, a, b)))
        }
        "JavaStringContentDecoder.decodeEscape"
        | "JavaStringContentDecoder.decodeUnicodeEscape" => {
            let index = byte(source, args["index"].as_i64().unwrap()).unwrap();
            let limit = byte(source, args["limit"].as_i64().unwrap()).unwrap();
            json!(if api.ends_with("decodeUnicodeEscape") {
                JavaStringContentDecoder::decode_unicode_escape(source, index, limit)
            } else {
                JavaStringContentDecoder::decode_escape(source, index, limit)
            })
        }
        _ => panic!("unsupported source capture API {api}"),
    }
}
pub fn expected_output(row: &Value) -> Value {
    let mut expected = row["output"].clone();
    let source = row["args"]["source"].as_str().unwrap_or("");
    normalize_expected_offsets(&mut expected, source, "");
    let api = row["api"].as_str().unwrap();
    if api.starts_with("AnnotationContextExtractor.find") && expected.is_array() {
        for n in expected.as_array_mut().unwrap() {
            *n = json!(byte(source, n.as_i64().unwrap()).unwrap());
        }
    }
    if api == "AnnotationContextExtractor.toOffset" && !expected.is_null() {
        expected = json!(byte(source, expected.as_i64().unwrap()).unwrap());
    }
    if api.starts_with("JavaStringContentDecoder.") {
        if let Some(n) = expected.get("nextIndex").and_then(Value::as_i64) {
            expected["nextIndex"] = json!(byte(source, n).unwrap());
        }
    }
    canonical(expected, "")
}
pub fn output_kind(value: &Value) -> Option<&str> {
    value
        .as_object()
        .filter(|o| o.len() == 1)
        .and_then(|o| o.keys().next())
        .filter(|k| {
            matches!(
                k.as_str(),
                "Resolved"
                    | "Unresolved"
                    | "Ambiguous"
                    | "Found"
                    | "MixinTarget"
                    | "InjectMethod"
                    | "AtValue"
                    | "AtTarget"
                    | "AccessorValue"
                    | "InvokerValue"
                    | "MemberName"
            )
        })
        .map(String::as_str)
}
pub fn expected_kind(row: &Value) -> Option<&str> {
    row["outputType"]
        .as_str()
        .and_then(|n| n.rsplit('$').next())
        .filter(|k| {
            matches!(
                *k,
                "Resolved"
                    | "Unresolved"
                    | "Ambiguous"
                    | "Found"
                    | "MixinTarget"
                    | "InjectMethod"
                    | "AtValue"
                    | "AtTarget"
                    | "AccessorValue"
                    | "InvokerValue"
                    | "MemberName"
            )
        })
}
pub fn kinds_match(row: &Value, actual: &Value) -> bool {
    match expected_kind(row) {
        Some(kind) => output_kind(actual) == Some(kind),
        None => output_kind(actual).is_none(),
    }
}
