#![allow(non_snake_case)]
mod mixin_validation_support;
use mcdev_core::{
    mixin_diagnostics::MixinDiagnosticCodes as C, mixin_source::*, mixin_validation::*, model::*,
};
use mixin_validation_support::*;
fn range() -> McTextRange {
    McTextRange {
        start: McTextPosition {
            line: 1,
            character: 4,
        },
        end: McTextPosition {
            line: 1,
            character: 20,
        },
    }
}
fn targets() -> Vec<String> {
    vec![OWNER.into()]
}
fn shadow(
    name: &str,
    is_method: bool,
    descriptor: &str,
    is_static: bool,
) -> ShadowMemberDeclaration {
    ShadowMemberDeclaration {
        name: name.into(),
        is_method,
        descriptor: descriptor.into(),
        is_static,
        range: range(),
        parse_source: ParseSource::HandWritten,
        confidence: ParseConfidence::High,
        warnings: vec![],
    }
}
fn accessor(
    method_name: &str,
    return_type: &str,
    parameters: &[&str],
    explicit: Option<&str>,
) -> AccessorMethodDeclaration {
    AccessorMethodDeclaration {
        method_name: method_name.into(),
        return_type_descriptor: Some(return_type.into()),
        parameter_descriptors: parameters.iter().map(|s| s.to_string()).collect(),
        explicit_field_name: explicit.map(str::to_string),
        range: range(),
        parse_source: ParseSource::HandWritten,
        confidence: ParseConfidence::High,
        warnings: vec![],
    }
}
fn invoker(
    method_name: &str,
    parameters: &[&str],
    return_type: &str,
    explicit: Option<&str>,
) -> InvokerMethodDeclaration {
    InvokerMethodDeclaration {
        method_name: method_name.into(),
        parameter_descriptors: parameters.iter().map(|s| s.to_string()).collect(),
        return_type_descriptor: Some(return_type.into()),
        explicit_target_name: explicit.map(str::to_string),
        range: range(),
        parse_source: ParseSource::HandWritten,
        confidence: ParseConfidence::High,
        warnings: vec![],
    }
}
#[test]
fn shadowFieldValidationPassesForValidTarget() {
    assert!(ShadowValidationService::new(&LegacyIndex::default())
        .validate(
            &targets(),
            &shadow("currentScreen", false, FD, false),
            None,
            true
        )
        .is_empty());
}
#[test]
fn shadowFieldValidationFailsForMissingTarget() {
    assert_eq!(
        ShadowValidationService::new(&LegacyIndex::default()).validate(
            &targets(),
            &shadow("missingField", false, "I", false),
            None,
            true
        )[0]
        .code,
        C::SHADOW_TARGET_NOT_FOUND
    );
}
#[test]
fn shadowFieldStaticMismatchProducesDiagnostic() {
    assert_eq!(
        ShadowValidationService::new(&LegacyIndex::default()).validate(
            &targets(),
            &shadow("currentScreen", false, FD, true),
            None,
            true
        )[0]
        .code,
        C::SHADOW_STATIC_MISMATCH
    );
}
#[test]
fn shadowFieldValidationFailsForDescriptorMismatch() {
    assert_eq!(
        ShadowValidationService::new(&LegacyIndex::default()).validate(
            &targets(),
            &shadow("currentScreen", false, "I", false),
            None,
            true
        )[0]
        .code,
        C::SHADOW_DESCRIPTOR_MISMATCH
    );
}
#[test]
fn shadowMethodValidationPassesForValidTarget() {
    assert!(ShadowValidationService::new(&LegacyIndex::default())
        .validate(&targets(), &shadow("tick", true, "()V", false), None, true)
        .is_empty());
}
#[test]
fn shadowMethodValidationFailsForMissingMethod() {
    assert_eq!(
        ShadowValidationService::new(&LegacyIndex::default()).validate(
            &targets(),
            &shadow("missingMethod", true, "()V", false),
            None,
            true
        )[0]
        .code,
        C::SHADOW_TARGET_NOT_FOUND
    );
}
#[test]
fn shadowPrefixResolutionStripsPrefix() {
    assert!(ShadowValidationService::new(&LegacyIndex::default())
        .validate(
            &targets(),
            &shadow("shadow$currentScreen", false, FD, false),
            Some("shadow$"),
            true
        )
        .is_empty());
}
#[test]
fn accessorInfersGetterFieldName() {
    let d = accessor("getCurrentScreen", FD, &[], None);
    assert_eq!(
        AccessorService::infer_field_name(&d).as_deref(),
        Some("currentScreen")
    );
    assert_eq!(AccessorService::infer_kind(&d), AccessorKind::Getter);
}
#[test]
fn accessorInfersSetterFieldName() {
    let d = accessor("setCurrentScreen", "V", &[FD], None);
    assert_eq!(
        AccessorService::infer_field_name(&d).as_deref(),
        Some("currentScreen")
    );
    assert_eq!(AccessorService::infer_kind(&d), AccessorKind::Setter);
}
#[test]
fn accessorValidationPassesForValidGetter() {
    assert!(AccessorService::new(&LegacyIndex::default())
        .validate(
            &targets(),
            &accessor("getCurrentScreen", FD, &[], Some("currentScreen"))
        )
        .is_empty());
}
#[test]
fn accessorValidationFailsForMissingField() {
    assert_eq!(
        AccessorService::new(&LegacyIndex::default()).validate(
            &targets(),
            &accessor("getMissing", "I", &[], Some("missing"))
        )[0]
        .code,
        C::ACCESSOR_FIELD_NOT_FOUND
    );
}
#[test]
fn accessorValidationFailsForSignatureMismatch() {
    assert_eq!(
        AccessorService::new(&LegacyIndex::default()).validate(
            &targets(),
            &accessor("getCurrentScreen", "I", &[], Some("currentScreen"))
        )[0]
        .code,
        C::ACCESSOR_SIGNATURE_MISMATCH
    );
}
#[test]
fn accessorFieldCompletionReturnsCandidates() {
    assert_eq!(
        AccessorService::new(&LegacyIndex::default()).complete_fields(&targets(), "current")[0]
            .insert_text,
        "currentScreen"
    );
}
#[test]
fn accessorGeneratesGetterStub() {
    let field = LegacyIndex::default().get_fields(OWNER).remove(0);
    let stub = AccessorService::generate_method_stub(&field, AccessorKind::Getter, "    ");
    assert!(stub.contains("@Accessor(\"currentScreen\")"));
    assert!(stub.contains("getCurrentScreen"));
}
#[test]
fn invokerInfersTargetName() {
    let d = invoker("invokeSetScreen", &[FD], "V", None);
    assert_eq!(
        InvokerService::infer_target_name(&d).as_deref(),
        Some("setScreen")
    );
}
#[test]
fn invokerValidationPassesForValidTarget() {
    assert!(InvokerService::new(&LegacyIndex::default())
        .validate(
            &targets(),
            &invoker("invokeSetScreen", &[FD], "V", Some("setScreen"))
        )
        .is_empty());
}
#[test]
fn invokerValidationFailsForMissingMethod() {
    assert_eq!(
        InvokerService::new(&LegacyIndex::default()).validate(
            &targets(),
            &invoker("invokeMissing", &[], "V", Some("missing"))
        )[0]
        .code,
        C::INVOKER_METHOD_NOT_FOUND
    );
}
#[test]
fn invokerValidationFailsForDescriptorMismatch() {
    assert_eq!(
        InvokerService::new(&LegacyIndex::default()).validate(
            &targets(),
            &invoker("invokeSetScreen", &["I"], "V", Some("setScreen"))
        )[0]
        .code,
        C::INVOKER_DESCRIPTOR_MISMATCH
    );
}
#[test]
fn invokerMethodCompletionReturnsCandidates() {
    assert!(InvokerService::new(&LegacyIndex::default())
        .complete_methods(&targets(), "set")
        .iter()
        .any(|c| c.insert_text == "setScreen"));
}
#[test]
fn shadowMethodCompletionExcludesClassInitializers() {
    let mut index = LegacyIndex::default();
    let methods = index.methods.get_mut(OWNER).unwrap();
    methods.push(MethodIndexEntry {
        name: "<init>".into(),
        descriptor: "(I)V".into(),
        is_static: false,
        readable_signature: "<init>(int): void".into(),
    });
    methods.push(MethodIndexEntry {
        name: "<clinit>".into(),
        descriptor: "()V".into(),
        is_static: true,
        readable_signature: "<clinit>(): void".into(),
    });
    let items = ShadowValidationService::new(&index).complete_methods(&targets(), "", None);
    assert!(items.iter().any(|m| m.name == "tick"));
    assert!(!items.iter().any(|m| m.name == "<init>"));
    assert!(!items.iter().any(|m| m.name == "<clinit>"));
}
#[test]
fn invokerGeneratesMethodStub() {
    let method = LegacyIndex::default()
        .get_methods(OWNER)
        .into_iter()
        .find(|m| m.name == "setScreen")
        .unwrap();
    let stub = InvokerService::generate_method_stub(&method, "    ");
    assert!(stub.contains("@Invoker(\"setScreen\")"));
    assert!(stub.contains("invokeSetScreen"));
}
#[test]
fn mappedShadowPropertiesAndPrivateFunctionsMatchTargetDescriptorsWithoutLosingSourceNames() {
    struct TargetIndex;
    impl ClassIndex for TargetIndex {
        fn find_classes(&self, p: &str, limit: usize) -> Vec<ClassIndexEntry> {
            let c = ClassIndexEntry {
                simple_name: "Target".into(),
                package_name: "game".into(),
                internal_name: "game/Target".into(),
            };
            if (c.simple_name.to_lowercase().starts_with(&p.to_lowercase())
                || c.fqn().to_lowercase().starts_with(&p.to_lowercase()))
                && limit > 0
            {
                vec![c]
            } else {
                vec![]
            }
        }
        fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
            if n == "game/Target" {
                self.find_classes("", 1).pop()
            } else {
                None
            }
        }
        fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
            if n == "game.Target" {
                self.find_classes("", 1).pop()
            } else {
                None
            }
        }
        fn get_methods(&self, o: &str) -> Vec<MethodIndexEntry> {
            if o == "game/Target" {
                vec![MethodIndexEntry {
                    name: "secret".into(),
                    descriptor: "(I)I".into(),
                    is_static: false,
                    readable_signature: "secret(int)".into(),
                }]
            } else {
                vec![]
            }
        }
        fn get_fields(&self, o: &str) -> Vec<FieldIndexEntry> {
            if o == "game/Target" {
                vec![
                    FieldIndexEntry {
                        name: "count".into(),
                        descriptor: "I".into(),
                        is_static: false,
                        readable_type: "int".into(),
                    },
                    FieldIndexEntry {
                        name: "manualTotal".into(),
                        descriptor: "J".into(),
                        is_static: true,
                        readable_type: "long".into(),
                    },
                    FieldIndexEntry {
                        name: "total".into(),
                        descriptor: "J".into(),
                        is_static: true,
                        readable_type: "long".into(),
                    },
                ]
            } else {
                vec![]
            }
        }
    }
    let source = include_str!("fixtures/mixin-kotlin-shadow-legacy.kt").trim_end_matches('\n');
    let declarations =
        MixinMemberDeclarationParser::parse_shadow_declarations(source, Some(&TargetIndex));
    assert_eq!(
        declarations
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["count", "secret", "manualTotal", "total"]
    );
    let members = MixinSemanticModelParser::parse(source, "file:///Patch.kt").members;
    let hidden = members.iter().find(|m| m.java_name == "hidden").unwrap();
    assert_eq!(
        Some(hidden.name_range),
        MixinMemberDeclarationParser::find_member_name_text_range(
            source,
            declarations[1].range,
            "secret",
            "Shadow",
            true
        )
    );
    assert!(declarations
        .iter()
        .flat_map(|d| ShadowValidationService::new(&TargetIndex).validate(
            &["game/Target".into()],
            d,
            None,
            true
        ))
        .collect::<Vec<_>>()
        .is_empty());
    assert!(MixinMemberDeclarationParser::parse_declaration_diagnostics(
        source,
        Some(&TargetIndex)
    )
    .is_empty());
}
