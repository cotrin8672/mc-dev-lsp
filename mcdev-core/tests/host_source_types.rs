use mcdev_core::host_source::*;
use std::collections::BTreeMap;
fn class(name: &str) -> HostTypeFact {
    HostTypeFact::Class {
        internal_name: name.replace('.', "/"),
    }
}
fn primitive(p: HostPrimitive) -> HostTypeFact {
    HostTypeFact::Primitive(p)
}
fn array(ty: HostTypeFact, n: usize) -> HostTypeFact {
    HostTypeFact::Array {
        element: Box::new(ty),
        dimensions: n,
    }
}
fn expression(kind: HostExpressionKind) -> HostExpressionFact {
    HostExpressionFact {
        validity: Default::default(),
        kind,
        source_text: String::new(),
        source_range: None,
    }
}
fn constant(value: HostConstant) -> HostExpressionFact {
    expression(HostExpressionKind::Constant(value))
}
fn string(value: &str) -> HostExpressionFact {
    constant(HostConstant::String(value.encode_utf16().collect()))
}
fn elements(elements: Vec<Option<HostExpressionFact>>) -> HostExpressionFact {
    expression(HostExpressionKind::Array(elements))
}
fn annotation(label: &str) -> HostExpressionFact {
    expression(HostExpressionKind::Annotation(Box::new(
        HostAnnotationFact {
            source_name: label.into(),
            qualified_name: None,
            validity: Default::default(),
            exists: true,
            members: vec![],
            members_available: true,
            source_range: None,
            body_range: None,
        },
    )))
}
fn literal(ty: HostTypeFact) -> HostExpressionFact {
    expression(HostExpressionKind::TypeLiteral(ty))
}

#[test]
fn recovered_types_and_their_arrays_have_no_descriptor() {
    for ty in [
        HostTypeFact::Recovered,
        array(HostTypeFact::Recovered, 2),
        array(HostTypeFact::Recovered, 1),
    ] {
        assert_eq!(ty.descriptor(), None);
    }
}
#[test]
fn descriptor_from_binding_uses_element_type_and_dimensions_for_multidimensional_primitive_arrays()
{
    assert_eq!(
        array(primitive(HostPrimitive::Int), 2)
            .descriptor()
            .as_deref(),
        Some("[[I")
    );
    assert_eq!(
        array(primitive(HostPrimitive::Int), 3)
            .descriptor()
            .as_deref(),
        Some("[[[I")
    );
}
#[test]
fn descriptor_from_binding_uses_element_type_and_dimensions_for_multidimensional_object_arrays() {
    assert_eq!(
        array(class("java.lang.String"), 2).descriptor().as_deref(),
        Some("[[Ljava/lang/String;")
    );
    assert_eq!(
        array(class("java.lang.String"), 3).descriptor().as_deref(),
        Some("[[[Ljava/lang/String;")
    );
}
#[test]
fn descriptor_from_binding_adds_one_dimension_per_component_when_element_type_is_absent() {
    assert_eq!(
        array(array(primitive(HostPrimitive::Int), 1), 1)
            .descriptor()
            .as_deref(),
        Some("[[I")
    );
    assert_eq!(
        array(array(array(class("java.lang.String"), 1), 1), 1)
            .descriptor()
            .as_deref(),
        Some("[[[Ljava/lang/String;")
    );
}
#[test]
fn constant_string_accepts_resolved_scalar_value() {
    assert_eq!(string("hello").constant_string().as_deref(), Some("hello"));
}
#[test]
fn constant_string_accepts_escaped_and_concatenated_resolver_values() {
    assert_eq!(string("a\tb").constant_string().as_deref(), Some("a\tb"));
    assert_eq!(
        string("prefixsuffix").constant_string().as_deref(),
        Some("prefixsuffix")
    );
}
#[test]
fn constant_string_rejects_recovered_and_malformed_nodes_via_get_flags() {
    // Host adapter normalizes the authoritative node flags, never probes JDT.
    for validity in [
        HostNodeValidity {
            recovered: true,
            malformed: false,
        },
        HostNodeValidity {
            recovered: false,
            malformed: true,
        },
    ] {
        let mut n = string("ok");
        n.validity = validity;
        assert_eq!(n.constant_string(), None);
    }
    assert_eq!(string("ok").constant_string().as_deref(), Some("ok"));
}
#[test]
fn constant_string_rejects_null_constant_wrong_type_and_recovered_node() {
    assert_eq!(constant(HostConstant::Null).constant_string(), None);
    assert_eq!(constant(HostConstant::Int(42)).constant_string(), None);
    let mut n = string("ok");
    n.validity.recovered = true;
    assert_eq!(n.constant_string(), None);
    n.validity.recovered = false;
    n.validity.malformed = true;
    assert_eq!(n.constant_string(), None);
}
#[test]
fn constant_boolean_and_int_accept_only_matching_runtime_types() {
    assert_eq!(
        constant(HostConstant::Boolean(true)).constant_boolean(),
        Some(true)
    );
    assert_eq!(
        constant(HostConstant::Boolean(false)).constant_boolean(),
        Some(false)
    );
    assert_eq!(constant(HostConstant::Int(16)).constant_int(), Some(16));
    assert_eq!(constant(HostConstant::Int(1)).constant_boolean(), None);
    assert_eq!(constant(HostConstant::Null).constant_int(), None);
    assert_eq!(constant(HostConstant::Long(16)).constant_int(), None);
}
#[test]
fn string_array_accepts_scalar_or_array_initializer_preserving_order() {
    assert_eq!(string("only").string_array(), Some(vec!["only".into()]));
    assert_eq!(
        elements(vec![Some(string("first")), Some(string("second"))]).string_array(),
        Some(vec!["first".into(), "second".into()])
    );
    assert_eq!(elements(vec![]).string_array(), Some(vec![]));
}
#[test]
fn string_array_rejects_unresolved_null_or_wrong_type_elements() {
    for second in [
        Some(constant(HostConstant::Null)),
        Some(constant(HostConstant::Int(1))),
        None,
    ] {
        assert_eq!(
            elements(vec![Some(string("good")), second]).string_array(),
            None
        );
    }
}
#[test]
fn type_literal_descriptor_uses_binding_conversion_for_primitive_object_and_array_types() {
    assert_eq!(
        literal(primitive(HostPrimitive::Int))
            .type_literal_descriptor()
            .as_deref(),
        Some("I")
    );
    assert_eq!(
        literal(class("java.lang.String"))
            .type_literal_descriptor()
            .as_deref(),
        Some("Ljava/lang/String;")
    );
    assert_eq!(
        literal(array(class("java.lang.String"), 1))
            .type_literal_descriptor()
            .as_deref(),
        Some("[Ljava/lang/String;")
    );
}
#[test]
fn type_literal_source_name_converts_every_primitive_and_preserves_object_descriptors() {
    let types = [
        HostPrimitive::Void,
        HostPrimitive::Boolean,
        HostPrimitive::Byte,
        HostPrimitive::Char,
        HostPrimitive::Short,
        HostPrimitive::Int,
        HostPrimitive::Long,
        HostPrimitive::Float,
        HostPrimitive::Double,
    ];
    let names = types
        .into_iter()
        .map(|p| literal(primitive(p)).type_literal_source_name().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["void", "boolean", "byte", "char", "short", "int", "long", "float", "double"]
    );
    assert_eq!(
        literal(class("java.lang.String"))
            .type_literal_source_name()
            .as_deref(),
        Some("Ljava/lang/String;")
    );
    assert_eq!(
        literal(array(class("java.lang.String"), 1))
            .type_literal_source_name()
            .as_deref(),
        Some("[Ljava/lang/String;")
    );
}
#[test]
fn type_literal_descriptor_uses_referenced_type_instead_of_class_expression_binding() {
    assert_eq!(
        literal(class("com.example.SimpleTarget"))
            .type_literal_descriptor()
            .as_deref(),
        Some("Lcom/example/SimpleTarget;")
    );
}
#[test]
fn type_literal_descriptor_rejects_null_binding_and_recovered_node() {
    assert_eq!(
        literal(HostTypeFact::Unresolved).type_literal_descriptor(),
        None
    );
    let mut n = literal(primitive(HostPrimitive::Int));
    n.validity.recovered = true;
    assert_eq!(n.type_literal_descriptor(), None);
}
#[test]
fn type_array_accepts_scalar_or_array_initializer_preserving_order() {
    assert_eq!(
        literal(class("java.lang.String")).type_array(),
        Some(vec!["Ljava/lang/String;".into()])
    );
    assert_eq!(
        elements(vec![
            Some(literal(class("java.lang.String"))),
            Some(literal(primitive(HostPrimitive::Int)))
        ])
        .type_array(),
        Some(vec!["Ljava/lang/String;".into(), "I".into()])
    );
    // Legacy typeArray returns descriptors, typeArraySourceNames supplies primitives as names.
}
#[test]
fn type_array_rejects_unresolved_element_for_whole_array() {
    assert_eq!(
        elements(vec![
            Some(literal(class("java.lang.String"))),
            Some(literal(HostTypeFact::Unresolved))
        ])
        .type_array(),
        None
    );
}
#[test]
fn nested_annotation_array_accepts_single_annotation_or_array_initializer_preserving_order() {
    let first = annotation("first");
    let second = annotation("second");
    assert_eq!(
        first
            .nested_annotations()
            .unwrap()
            .iter()
            .map(|a| a.source_name.as_str())
            .collect::<Vec<_>>(),
        ["first"]
    );
    assert_eq!(
        elements(vec![Some(first), Some(second)])
            .nested_annotations()
            .unwrap()
            .iter()
            .map(|a| a.source_name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
}
#[test]
fn nested_annotation_array_rejects_wrong_type_recovered_or_null_elements() {
    assert_eq!(string("not-annotation").nested_annotations(), None);
    let mut recovered = annotation("recovered");
    recovered.validity.recovered = true;
    for invalid in [Some(string("bad")), None, Some(recovered)] {
        assert_eq!(
            elements(vec![Some(annotation("valid")), invalid]).nested_annotations(),
            None
        );
    }
}

#[test]
fn all_fourteen_legacy_private_signatures_have_explicit_normalized_type_facts() {
    // Signature parsing is performed by the host. This test preserves each old
    // private input, the explicit host normalization and its MC projection result.
    let rows: [(&str, &str, HostTypeFact, Option<&str>); 14] = [
        (
            "convertsPrimitiveInt",
            "I",
            primitive(HostPrimitive::Int),
            Some("I"),
        ),
        (
            "convertsIntArrayOfArrays",
            "[[I",
            array(primitive(HostPrimitive::Int), 2),
            Some("[[I"),
        ),
        (
            "convertsUnresolvedStringViaResolver",
            "QString;",
            class("java.lang.String"),
            Some("Ljava/lang/String;"),
        ),
        (
            "erasesParameterizedUnresolvedList",
            "Qjava.util.List<Ljava.lang.String;>;",
            HostTypeFact::Erasure(Box::new(class("java.util.List"))),
            Some("Ljava/util/List;"),
        ),
        (
            "convertsResolvedJavaLangString",
            "Ljava.lang.String;",
            class("java.lang.String"),
            Some("Ljava/lang/String;"),
        ),
        (
            "convertsUnresolvedInnerClassViaResolver",
            "QOuter.Inner;",
            class("com/example/Outer$Inner"),
            Some("Lcom/example/Outer$Inner;"),
        ),
        (
            "erasesTypeVariableToObject",
            "TT;",
            HostTypeFact::Erasure(Box::new(class("java.lang.Object"))),
            Some("Ljava/lang/Object;"),
        ),
        (
            "returnsNullForUnknownUnresolvedClass",
            "QUnknown.Type;",
            HostTypeFact::Unresolved,
            None,
        ),
        (
            "returnsNullForUnknownUnresolvedClassWithoutResolver",
            "QString;",
            HostTypeFact::Unresolved,
            None,
        ),
        (
            "returnsNullForArrayOfUnknownUnresolvedClass",
            "[QMissing.Type;",
            array(HostTypeFact::Unresolved, 1),
            None,
        ),
        (
            "returnsNullForEmptySignature",
            "",
            HostTypeFact::Unresolved,
            None,
        ),
        (
            "returnsNullForUnsupportedLeadingCharacter",
            "@I",
            HostTypeFact::Unresolved,
            None,
        ),
        (
            "returnsNullForMissingClassTerminator",
            "Ljava.lang.String",
            HostTypeFact::Unresolved,
            None,
        ),
        (
            "returnsNullForUnbalancedTypeArguments",
            "Ljava.util.List<Ljava.lang.String;",
            HostTypeFact::Unresolved,
            None,
        ),
    ];
    let mut ids = BTreeMap::new();
    for (id, old_input, normalized, expected) in rows {
        assert_eq!(
            normalized.descriptor().as_deref(),
            expected,
            "{id}: {old_input}"
        );
        assert!(ids.insert(id, true).is_none());
    }
    assert_eq!(ids.len(), 14);
}
#[test]
fn unknown_or_ambiguous_types_never_become_object_and_utf16_is_lossless() {
    assert_eq!(
        HostTypeFact::Candidates(vec![class("a.One"), class("b.One")]).descriptor(),
        None
    );
    assert_eq!(
        HostTypeFact::Candidates(vec![class("a.One"), class("a.One")])
            .descriptor()
            .as_deref(),
        Some("La/One;")
    );
    let raw = constant(HostConstant::String(vec![0xd800, 0x61]));
    assert_eq!(raw.constant_string(), None);
    assert_eq!(raw.constant_utf16(), Some([0xd800, 0x61].as_slice()));
    assert!(array(primitive(HostPrimitive::Int), 255)
        .descriptor()
        .is_some());
    assert_eq!(
        array(array(primitive(HostPrimitive::Int), 255), 1).descriptor(),
        None
    );
}
