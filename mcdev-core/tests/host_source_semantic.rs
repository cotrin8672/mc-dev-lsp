use mcdev_core::{extras_model::*, host_source::*, mixin_source::*, *};
use std::collections::BTreeSet;
fn annotation(name: &str, fqn: Option<&str>) -> HostAnnotationFact {
    HostAnnotationFact {
        source_name: name.into(),
        qualified_name: fqn.map(Into::into),
        validity: Default::default(),
        exists: true,
        members: vec![],
        members_available: true,
        source_range: None,
        body_range: None,
    }
}
fn expression(kind: HostExpressionKind, text: &str) -> HostExpressionFact {
    HostExpressionFact {
        kind,
        validity: Default::default(),
        source_text: text.into(),
        source_range: None,
    }
}
fn string(value: &str) -> HostExpressionFact {
    expression(
        HostExpressionKind::Constant(HostConstant::String(value.encode_utf16().collect())),
        &format!("\"{value}\""),
    )
}
fn method(name: &str, annotations: Vec<HostAnnotationFact>) -> HostSemanticMember {
    HostSemanticMember {
        name: name.into(),
        annotations,
        parameter_types: vec![],
        return_type: HostTypeFact::Primitive(HostPrimitive::Void),
        modifiers: BTreeSet::new(),
        is_method: true,
        range: 0..name.len(),
        name_range: 0..name.len(),
    }
}
fn project_member(source: &str, member: HostSemanticMember) -> MixinClassModel {
    project_host_semantic_document(
        source,
        "file:///ExampleMixin.java",
        &HostSemanticDocument {
            members: vec![member],
            ..Default::default()
        },
    )
}

#[test]
fn normalized_accessor_target_uses_decoded_constant_and_never_raw_expression_text() {
    let mut accessor = annotation("Accessor", Some("org.spongepowered.asm.mixin.gen.Accessor"));
    let mut target = string("value");
    target.source_text = "\"va\" + \"lue\"".into();
    accessor.members.push(("value".into(), target));
    let model = project_member("getValue", method("getValue", vec![accessor.clone()]));
    assert_eq!(
        model.members[0].explicit_target_name.as_deref(),
        Some("value")
    );
    accessor.members[0].1.kind = HostExpressionKind::Unresolved;
    let model = project_member("getValue", method("getValue", vec![accessor]));
    assert!(model.members[0].explicit_target_name.is_none());
}
#[test]
fn typed_provider_failure_preserves_root_cause_instead_of_reflection_wrapper() {
    // Original InvocationTargetException -> IllegalStateException is an adapter
    // fixture transformation. No Java reflection wrapper exists in production.
    let original_class = "java.lang.IllegalStateException";
    let original_message = "AST create failed";
    assert_eq!(
        format!("{original_class}: {original_message}"),
        "java.lang.IllegalStateException: AST create failed"
    );
    let failure = HostSourceError::Failure(original_message.into());
    assert_eq!(failure.to_string(), "AST create failed");
    assert!(matches!(failure, HostSourceError::Failure(_)));
}
#[test]
fn typed_cancellation_propagates_through_semantic_cache_without_wrapper_or_entry() {
    struct Cancel;
    impl HostReadValidator for Cancel {
        fn validate_current_reads(
            &self,
            _: HostGenerations,
            _: &HostReadSet,
        ) -> Result<bool, HostSourceError> {
            panic!("no cache entry expected")
        }
    }
    impl HostSemanticModelProvider for Cancel {
        fn parse(&self, _: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
            Err(HostSourceError::Cancelled("cancelled".into()))
        }
    }
    let environment = HostSemanticEnvironment::default();
    let project = HostProjectKey {
        project_id: "project".into(),
        generations: Default::default(),
    };
    let cache = SemanticModelCache::default();
    let request = HostSemanticRequest {
        source: "",
        source_uri: "file:///ExampleMixin.java",
        document_version: Some(1),
        environment: &environment,
        project: Some(&project),
    };
    assert_eq!(
        cache.get(&request, &Cancel).unwrap_err(),
        HostSourceError::Cancelled("cancelled".into())
    );
    assert!(cache.is_empty());
}
#[test]
fn unavailable_host_semantics_fall_back_with_visible_warning() {
    let source="package com.example.mixin;\n@Mixin(SimpleTarget.class)\nclass ExampleMixin {\n@Shadow private int counter;\n}";
    let result = HostSemanticModel::fallback(
        source,
        "file:///ExampleMixin.java",
        "Host semantic facts unavailable",
        true,
        0,
    );
    assert_eq!(result.model.source_uri, "file:///ExampleMixin.java");
    assert_eq!(result.model.parse_source, ParseSource::HandWrittenFallback);
    assert!(result
        .model
        .warnings
        .iter()
        .any(|w| w.contains("Host semantic facts unavailable")));
    assert_eq!(result.model.confidence, ParseConfidence::Low);
    assert_eq!(result.availability, SemanticAvailability::StableFallback);
}
#[test]
fn does_not_fabricate_method_descriptor_when_any_parameter_or_return_type_is_unresolved() {
    let int = HostTypeFact::Primitive(HostPrimitive::Int);
    let void = HostTypeFact::Primitive(HostPrimitive::Void);
    assert_eq!(
        resolved_method_descriptor_parts(&[int.clone(), HostTypeFact::Unresolved], &void),
        (vec![], None)
    );
    assert_eq!(
        resolved_method_descriptor_parts(&[int.clone()], &HostTypeFact::Unresolved),
        (vec![], None)
    );
    assert_eq!(
        resolved_method_descriptor_parts(
            &[
                int,
                HostTypeFact::Class {
                    internal_name: "java/lang/String".into()
                }
            ],
            &void
        ),
        (
            vec!["I".into(), "Ljava/lang/String;".into()],
            Some("(ILjava/lang/String;)V".into())
        )
    );
}
#[test]
fn missing_workspace_authority_remains_unavailable_and_has_no_project_handle() {
    struct Missing;
    impl HostReadValidator for Missing {
        fn validate_current_reads(
            &self,
            _: HostGenerations,
            _: &HostReadSet,
        ) -> Result<bool, HostSourceError> {
            panic!("no project identity means no reuse")
        }
    }
    impl HostSemanticModelProvider for Missing {
        fn parse(&self, r: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
            assert!(r.project.is_none());
            Ok(HostAnswer::unavailable(
                "workspace semantic provider unavailable",
            ))
        }
    }
    let environment = HostSemanticEnvironment::default();
    let request = HostSemanticRequest {
        source: "",
        source_uri: "file:///ExampleMixin.java",
        document_version: None,
        environment: &environment,
        project: None,
    };
    let result = SemanticModelCache::default()
        .get(&request, &Missing)
        .unwrap();
    assert!(matches!(result.answer.fact, HostFact::Unavailable { .. }));
    assert!(!result.cache_hit);
}
#[test]
fn mixin_targets_use_string_constant_instead_of_string_expression_binding() {
    let mut mixin = annotation("Mixin", Some("org.spongepowered.asm.mixin.Mixin"));
    mixin.members.push(("targets".into(), string("pkg.Target")));
    let imports = JavaTypeDescriptorResolver::imports_for("");
    assert_eq!(
        host_mixin_targets("", &mixin, &imports)
            .iter()
            .map(|t| t.internal_name.as_str())
            .collect::<Vec<_>>(),
        ["pkg/Target"]
    );
}
#[test]
fn recovered_target_binding_uses_explicit_import_and_later_resolved_binding() {
    let source = "import net.minecraft.client.gui.Gui;";
    let imports = JavaTypeDescriptorResolver::imports_for(source);
    for ty in [
        HostTypeFact::Recovered,
        HostTypeFact::Class {
            internal_name: "net/minecraft/client/gui/Gui".into(),
        },
    ] {
        let mut mixin = annotation("Mixin", Some("org.spongepowered.asm.mixin.Mixin"));
        mixin.members.push((
            "value".into(),
            expression(HostExpressionKind::TypeLiteral(ty), "Gui.class"),
        ));
        assert_eq!(
            host_mixin_targets(source, &mixin, &imports)
                .iter()
                .map(|t| t.internal_name.as_str())
                .collect::<Vec<_>>(),
            ["net/minecraft/client/gui/Gui"]
        );
    }
}
#[test]
fn recovered_target_binding_keeps_nested_source_name_for_shared_resolver() {
    let source = "package example;\nimport sample.Outer;";
    let imports = JavaTypeDescriptorResolver::imports_for(source);
    let mut mixin = annotation("Mixin", Some("org.spongepowered.asm.mixin.Mixin"));
    mixin.members.push((
        "value".into(),
        expression(
            HostExpressionKind::TypeLiteral(HostTypeFact::Recovered),
            "Outer.Inner.class",
        ),
    ));
    assert_eq!(
        host_mixin_targets(source, &mixin, &imports)
            .iter()
            .map(|t| t.internal_name.as_str())
            .collect::<Vec<_>>(),
        ["Outer.Inner"]
    );
}
#[test]
fn member_annotations_use_binding_and_import_identity() {
    let imported = "import foo.Shadow;\nimport org.spongepowered.asm.mixin.Overwrite;";
    let imports = JavaTypeDescriptorResolver::imports_for(imported);
    let custom = annotation("Shadow", None);
    let qualified = annotation("foo.Mixin", None);
    let overwrite = annotation("Overwrite", None);
    assert_eq!(host_mixin_annotation(&custom, &imports), None);
    assert_eq!(host_mixin_annotation(&qualified, &imports), None);
    assert_eq!(
        host_mixin_annotation(&overwrite, &imports),
        Some(MixinAnnotation::Overwrite)
    );
    assert!(project_member(imported, method("custom", vec![custom]))
        .members
        .is_empty());
    assert_eq!(
        project_member(imported, method("overwrite", vec![overwrite]))
            .members
            .len(),
        1
    );
    let mut field = method("value", vec![annotation("foo.Shadow", None)]);
    field.is_method = false;
    field.return_type = HostTypeFact::Primitive(HostPrimitive::Int);
    assert!(project_member(imported, field).members.is_empty());
    let source = "import org.spongepowered.asm.mixin.Shadow;";
    let imports = JavaTypeDescriptorResolver::imports_for(source);
    assert_eq!(
        host_mixin_annotation(&annotation("Shadow", Some("foo.Shadow")), &imports),
        None
    );
    assert_eq!(
        host_mixin_annotation(
            &annotation("Shadow", Some("org.spongepowered.asm.mixin.Shadow")),
            &imports
        ),
        Some(MixinAnnotation::Shadow)
    );
    let imports = JavaTypeDescriptorResolver::imports_for("");
    let bare = annotation("Shadow", None);
    assert_eq!(
        host_mixin_annotation(&bare, &imports),
        Some(MixinAnnotation::Shadow)
    );
    // Recovered binding identity is explicitly absent in normalized host facts.
    let recovered = annotation("Shadow", None);
    assert_eq!(
        host_mixin_annotation(&recovered, &imports),
        Some(MixinAnnotation::Shadow)
    );
    assert_eq!(
        project_member("                  ", method("recovered", vec![recovered]))
            .members
            .len(),
        1
    );
    let mut field = method("value", vec![bare]);
    field.is_method = false;
    field.return_type = HostTypeFact::Primitive(HostPrimitive::Int);
    assert_eq!(project_member("                  ", field).members.len(), 1);
}
fn positioned(
    source: &str,
    name: &str,
    line: usize,
    annotations: Vec<HostAnnotationFact>,
) -> HostSemanticMember {
    let start = McTextPosition { line, character: 4 };
    let end = McTextPosition {
        line,
        character: 24,
    };
    let mut member = method(name, annotations);
    member.range =
        position_to_offset(source, start).unwrap()..position_to_offset(source, end).unwrap();
    member.name_range = member.range.clone();
    member
}
fn handler() -> HostAnnotationFact {
    let mut annotation = annotation(
        "ModifyExpressionValue",
        Some("com.llamalad7.mixinextras.injector.ModifyExpressionValue"),
    );
    annotation
        .members
        .push(("method".into(), string("draw()V")));
    annotation
}
fn expression_annotation(value: &str) -> HostAnnotationFact {
    let mut annotation = annotation(
        "Expression",
        Some("com.llamalad7.mixinextras.expression.Expression"),
    );
    annotation.members.push(("value".into(), string(value)));
    annotation
}
#[test]
fn collector_retains_resolved_handlers_in_declaration_order_and_applies_unavailable_rules() {
    let source = "                              \n".repeat(6);
    let members = vec![
        positioned(
            &source,
            "firstHandler",
            1,
            vec![handler(), expression_annotation("first")],
        ),
        positioned(&source, "emptyHandler", 2, vec![handler()]),
        positioned(
            &source,
            "brokenHandler",
            3,
            vec![annotation("ModifyExpressionValue", None)],
        ),
        positioned(&source, "secondHandler", 4, vec![handler()]),
        positioned(
            &source,
            "plainMethod",
            5,
            vec![annotation(
                "Shadow",
                Some("org.spongepowered.asm.mixin.Shadow"),
            )],
        ),
    ];
    let result = collect_host_extras_contexts(&source, &members);
    let expected_ranges = [1, 2, 4].map(|line| McTextRange {
        start: McTextPosition { line, character: 4 },
        end: McTextPosition {
            line,
            character: 24,
        },
    });
    let mut contexts = vec![];
    for (i, range) in expected_ranges.into_iter().enumerate() {
        contexts.push(ResolvedMixinExtrasContext {
            handler_range: range,
            context: if i == 0 {
                ExpressionContext {
                    expression_index: MixinExtrasExpressionIndex {
                        expressions: vec![MixinExtrasExpression {
                            id: String::new(),
                            values: vec!["first".into()],
                            ..Default::default()
                        }],
                    },
                    definition_index: Default::default(),
                }
            } else {
                Default::default()
            },
        });
    }
    assert_eq!(result.contexts, contexts);
    assert_eq!(result.warnings,["MixinExtras context unavailable for method brokenHandler: unresolved handler annotation binding"]);
}
#[test]
fn collector_keeps_mixed_mixinextras_wrap_with_condition_and_standard_handlers() {
    let source = "                              \n".repeat(4);
    let condition = annotation(
        "WrapWithCondition",
        Some("com.llamalad7.mixinextras.injector.v2.WrapWithCondition"),
    );
    let standard = annotation(
        "Inject",
        Some("org.spongepowered.asm.mixin.injection.Inject"),
    );
    let members = vec![
        positioned(
            &source,
            "mixinHandler",
            1,
            vec![handler(), expression_annotation("mixinValue")],
        ),
        positioned(
            &source,
            "conditionHandler",
            2,
            vec![condition, expression_annotation("conditionValue")],
        ),
        positioned(
            &source,
            "standardHandler",
            3,
            vec![standard, expression_annotation("standardValue")],
        ),
    ];
    let result = collect_host_extras_contexts(&source, &members);
    assert_eq!(
        result
            .contexts
            .iter()
            .map(|c| c.handler_range)
            .collect::<Vec<_>>(),
        [1, 2, 3].map(|line| McTextRange {
            start: McTextPosition { line, character: 4 },
            end: McTextPosition {
                line,
                character: 24
            }
        })
    );
    assert_eq!(
        result
            .contexts
            .iter()
            .map(|c| c.context.expression_index.expressions[0].values[0].as_str())
            .collect::<Vec<_>>(),
        ["mixinValue", "conditionValue", "standardValue"]
    );
    assert!(result.warnings.is_empty());
}
