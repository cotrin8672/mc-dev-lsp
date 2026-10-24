#[path = "fixtures/expression_engine_support.rs"]
mod support;
use mcdev_core::{
    bytecode::*, mixin_completion::*, mixin_facade::*, mixin_source::*, mixinextras_expression::*,
    mixinextras_handler::*, *,
};
use support::{op, ClassFile};
struct Index {
    bytes: Vec<u8>,
}
impl ClassIndex for Index {
    fn find_classes(&self, prefix: &str, _limit: usize) -> Vec<ClassIndexEntry> {
        ["Utf16Expressions", "java/lang/String"]
            .into_iter()
            .filter_map(|n| self.find_class(n))
            .filter(|c| c.simple_name.starts_with(prefix))
            .collect()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        ["Utf16Expressions", "java/lang/String"]
            .contains(&name)
            .then(|| ClassIndexEntry {
                simple_name: name.rsplit('/').next().unwrap().into(),
                package_name: name.rsplit_once('/').map_or("", |p| p.0).replace('/', "."),
                internal_name: name.into(),
            })
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.find_class(&name.replace('.', "/"))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        if owner != "Utf16Expressions" {
            return vec![];
        }
        vec![MethodIndexEntry {
            name: "run".into(),
            descriptor: "()Ljava/lang/String;".into(),
            is_static: true,
            readable_signature: "run(): String".into(),
        }]
    }
    fn get_fields(&self, _owner: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
impl BytecodeIndex for Index {
    fn get_at_target_candidates(
        &self,
        _owner: &str,
        _name: &str,
        _desc: Option<&str>,
        _value: &str,
    ) -> Vec<AtTargetCandidate> {
        vec![]
    }
    fn get_return_ordinal_count(&self, _owner: &str, _name: &str, _desc: Option<&str>) -> usize {
        1
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        (owner == "Utf16Expressions").then(|| self.bytes.clone())
    }
}
fn index(units: &[u16], member: bool) -> Index {
    let mut class = ClassFile::default();
    let utf = class.utf16(units);
    let constant = class.item([vec![8], utf.to_be_bytes().to_vec()].concat(), false);
    let mut code = op(0x13, constant);
    if member {
        let call = class.member(
            10,
            "Helper",
            "someMethod",
            "(Ljava/lang/String;)Ljava/lang/String;",
        );
        let trim = class.member(10, "java/lang/String", "trim", "()Ljava/lang/String;");
        code.extend(op(0xb8, call));
        code.extend(op(0xb6, trim));
    }
    code.push(0xb0);
    class.method("run", "()Ljava/lang/String;", 9, 0, code);
    Index {
        bytes: class.finish("Utf16Expressions"),
    }
}
fn source(expression: &str, kotlin: bool) -> String {
    if kotlin {
        format!("import Utf16Expressions\r\nimport org.spongepowered.asm.mixin.Mixin\r\nimport com.llamalad7.mixinextras.expression.Expression\r\nimport com.llamalad7.mixinextras.injector.ModifyExpressionValue\r\nimport org.spongepowered.asm.mixin.injection.At\r\n@Mixin(Utf16Expressions::class)\r\nabstract class Patch {{\r\n @Expression(\"{expression}\")\r\n @ModifyExpressionValue(method = [\"run\"], at = [At(\"MIXINEXTRAS:EXPRESSION\")])\r\n private fun handler(original: String): String = original\r\n}}")
    } else {
        format!("@Mixin(Utf16Expressions.class)\r\nabstract class Patch {{\r\n @Expression(\"{expression}\")\r\n @ModifyExpressionValue(method = \"run\", at = @At(\"MIXINEXTRAS:EXPRESSION\"))\r\n private static String handler(String original) {{return original;}}\r\n}}")
    }
}
#[test]
fn source_annotation_context_to_handler_inference_matches_exact_jvm_utf16_values() {
    for (raw, units) in [
        (r"\uD800", vec![0xd800]),
        (r"\uDC00", vec![0xdc00]),
        (r"\uD83D\uDE42", vec![0xd83d, 0xde42]),
        (r"\u0000", vec![0]),
        ("�", vec![0xfffd]),
    ] {
        for kotlin in [false, true] {
            let source = source(&format!("'{raw}'"), kotlin);
            let index = index(&units, false);
            let site = HandlerSignatureService::find_annotation_sites(&source)
                .into_iter()
                .next()
                .unwrap();
            let context = ExpressionContextResolver::resolve_expression_context(&source, &site);
            let values = context.values_for_at_id_utf16(None);
            let mut expected = vec![39];
            expected.extend_from_slice(&units);
            expected.push(39);
            assert_eq!(values, vec![expected], "{raw} kotlin={kotlin}");
            assert_eq!(
                ExpressionContextResolver::infer_expression_value_type(
                    &source,
                    &site,
                    &index.get_methods("Utf16Expressions")[0],
                    &["Utf16Expressions".into()],
                    &index,
                    &index,
                    Some(&context)
                ),
                Some("Ljava/lang/String;".into())
            );
            if String::from_utf16(&units).is_err() {
                assert!(context.expression_index.expressions[0]
                    .checked_values()
                    .is_err());
                assert!(!context.expression_index.expressions[0]
                    .values_utf16()
                    .concat()
                    .contains(&0xfffd));
            }
        }
    }
}
#[test]
fn public_facade_completes_lone_surrogate_receiver_and_applies_lossless_source_edits() {
    for (raw, units) in [
        (r"\uD800", vec![0xd800]),
        (r"\uDC00", vec![0xdc00]),
        (r"\u0000", vec![0]),
        (r"\uD83D\uDE42", vec![0xd83d, 0xde42]),
        ("�", vec![0xfffd]),
    ] {
        for kotlin in [false, true] {
            let source = source(&format!("someMethod('{raw}').tr"), kotlin);
            let at = source.find(".tr").unwrap() + 3;
            let pos = offset_to_position(&source, at).unwrap();
            let index = index(&units, true);
            let context = AnnotationContextExtractor::extract_at_offset(&source, at).unwrap();
            assert_eq!(
                &source[context.value_start_offset..context.value_end_offset],
                "tr"
            );
            assert_eq!(
                context.expression_completion_position,
                Some(ExpressionCompletionPosition::AfterDot)
            );
            if String::from_utf16(&units).is_err() {
                let prefix = &context
                    .decoded_expression_prefix_utf16
                    .as_ref()
                    .expect("lossless prefix")
                    .prefix;
                assert!(prefix.contains(&units[0]));
                assert!(!prefix.contains(&0xfffd));
            }
            let request = MixinFacadeRequest {
                buffer_text: source.clone(),
                line: pos.line as i32,
                character: pos.character as i32,
                document_uri: if kotlin {
                    "file:///Patch.kt"
                } else {
                    "file:///Patch.java"
                }
                .into(),
                ..Default::default()
            };
            let result = MixinServiceFacade::new(&index, &index)
                .complete_with_debug(
                    &request,
                    &MixinCompletionOptions::default(),
                    "mcdev.completion",
                    if kotlin { "kotlin" } else { "java" },
                )
                .unwrap();
            let item = result
                .items
                .iter()
                .find(|i| i.metadata.name.as_deref() == Some("trim"))
                .unwrap_or_else(|| panic!("trim absent {raw} kotlin={kotlin}: {result:?}"));
            let mut edits = item.additional_edits.clone();
            edits.push(McTextEdit {
                start_offset: context.value_start_offset,
                end_offset: context.value_end_offset,
                new_text: item.insert_text.clone(),
            });
            let applied = apply_text_edits(&source, &edits).unwrap();
            assert!(applied.contains(raw), "original raw literal preserved");
            assert!(!applied.contains(".tr\""));
            assert!(applied.contains("trim"));
            // A different JVM value must not reuse a surrogate's receiver cache entry.
            if String::from_utf16(&units).is_err() {
                let wrong = request.buffer_text.replace(raw, "�");
                let wrong_at = wrong.find(".tr").unwrap() + 3;
                let p = offset_to_position(&wrong, wrong_at).unwrap();
                let wrong = MixinFacadeRequest {
                    buffer_text: wrong,
                    line: p.line as i32,
                    character: p.character as i32,
                    ..request
                };
                let items = MixinServiceFacade::new(&index, &index)
                    .complete(&wrong, &MixinCompletionOptions::default())
                    .unwrap();
                assert!(items
                    .iter()
                    .all(|i| i.metadata.name.as_deref() != Some("trim")));
            }
        }
    }
}
#[test]
fn kotlin_raw_strings_keep_backslashes_literal_and_never_match_java_surrogate_escapes() {
    let expression = r"someMethod('\uD800').tr";
    let mut source = source(expression, true);
    source = source.replace(
        &format!("@Expression(\"{expression}\")"),
        &format!("@Expression(\"\"\"{expression}\"\"\")"),
    );
    let cursor = source.find(".tr").unwrap() + 3;
    let context = AnnotationContextExtractor::extract_at_offset(&source, cursor).unwrap();
    assert!(context.decoded_expression_prefix_utf16.is_none());
    assert!(context
        .decoded_expression_prefix
        .as_ref()
        .unwrap()
        .prefix
        .contains(r"\uD800"));
    assert_eq!(
        &source[context.value_start_offset..context.value_end_offset],
        "tr"
    );
    let site = HandlerSignatureService::find_annotation_sites(&source)
        .into_iter()
        .next()
        .unwrap();
    let values = ExpressionContextResolver::resolve_expression_context(&source, &site)
        .values_for_at_id_utf16(None);
    assert_eq!(values, [expression.encode_utf16().collect::<Vec<_>>()]);
    let position = offset_to_position(&source, cursor).unwrap();
    let request = MixinFacadeRequest {
        buffer_text: source,
        line: position.line as i32,
        character: position.character as i32,
        document_uri: "file:///Patch.kt".into(),
        ..Default::default()
    };
    let index = index(&[0xd800], true);
    assert!(MixinServiceFacade::new(&index, &index)
        .complete(&request, &MixinCompletionOptions::default())
        .unwrap()
        .is_empty());
}
#[test]
fn option_returning_expression_inference_propagates_typed_cancellation_through_the_facade() {
    use mcdev_core::expression_engine as engine;
    let index = index(&[0xd800], false);
    let request = MixinFacadeRequest {
        buffer_text: source(r"'\uD800'", false),
        ..Default::default()
    };
    let facade = MixinServiceFacade::new(&index, &index);
    let cancellation = engine::Cancellation(std::sync::Arc::new(std::io::Error::other(
        "cancelled expression inference",
    )));
    let expected = cancellation.clone();
    engine::with_cancellation(
        std::sync::Arc::new(move || Err(cancellation.clone())),
        || {
            let error = facade.diagnose(&request).unwrap_err();
            assert!(std::sync::Arc::ptr_eq(&error.0, &expected.0));
        },
    );
    let recovered = facade.diagnose(&request).unwrap();
    assert!(recovered
        .iter()
        .all(|d| d.code != "MIXINEXTRAS_MISSING_EXPRESSION_ANNOTATION"));
}
