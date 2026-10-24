#[allow(dead_code)]
mod mixin_validation_support;
#[path = "fixtures/mixin_facade/replay.rs"]
mod replay;
use mcdev_core::{
    mixin_actions::MixinCodeActionService,
    mixin_diagnostics::MixinDiagnosticCodes,
    mixin_facade::{MixinFacadeRequest, MixinServiceFacade},
    mixin_validation::{AccessorService, InvokerService, ShadowValidationService},
    McDiagnostic, McFix, McSeverity, McTextRange,
};
use mixin_validation_support::{LegacyBytecode, LegacyIndex};
use std::collections::BTreeMap;

const ACTION_SOURCE: &str = "@Mixin(MinecraftClient.class) class M { }";
fn missing_member_diagnostic(code: &str, key: &str, value: &str) -> McDiagnostic {
    McDiagnostic {
        code: code.into(),
        severity: McSeverity::Error,
        message: "missing".into(),
        range: McTextRange::default(),
        metadata: BTreeMap::from([(key.into(), value.into())]),
    }
}

/// Original MixinCodeActionE2ETest.generateAccessorFixContainsFieldName:
/// the accessor-only service and all FakeClassIndex facts are preserved.
#[test]
fn generate_accessor_fix_contains_field_name() {
    let accessor_index = LegacyIndex::default();
    let target_index = LegacyIndex::default();
    let service = MixinCodeActionService {
        accessor_service: Some(AccessorService::new(&accessor_index)),
        invoker_service: None,
    };
    let fixes = service.fixes_for_diagnostics(
        &[missing_member_diagnostic(
            MixinDiagnosticCodes::ACCESSOR_FIELD_NOT_FOUND,
            "field",
            "currentScreen",
        )],
        "file:///Mixin.java",
        ACTION_SOURCE,
        None,
        None,
        None,
        Some(&target_index),
    );
    let method_source = fixes
        .iter()
        .find_map(|fix| match fix {
            McFix::GenerateAccessorMethodFix { method_source, .. } => Some(method_source),
            _ => None,
        })
        .expect("the original filterIsInstance<GenerateAccessorMethodFix>().first()");
    assert!(method_source.contains("currentScreen"));
}

/// Original MixinCodeActionE2ETest.producesGenerateInvokerFix final API.
#[test]
fn produces_generate_invoker_fix() {
    let accessor_index = LegacyIndex::default();
    let invoker_index = LegacyIndex::default();
    let target_index = LegacyIndex::default();
    let service = MixinCodeActionService {
        accessor_service: Some(AccessorService::new(&accessor_index)),
        invoker_service: Some(InvokerService::new(&invoker_index)),
    };
    let fixes = service.fixes_for_diagnostics(
        &[missing_member_diagnostic(
            MixinDiagnosticCodes::INVOKER_METHOD_NOT_FOUND,
            "name",
            "setScreen",
        )],
        "file:///Mixin.java",
        ACTION_SOURCE,
        None,
        None,
        None,
        Some(&target_index),
    );
    assert!(fixes
        .iter()
        .any(|fix| matches!(fix, McFix::GenerateInvokerMethodFix { .. })));
}

/// Original producesGenerateAccessorFix first diagnoses through the facade,
/// then independently generates a fix from its selected/fallback diagnostic.
#[test]
fn produces_generate_accessor_fix_after_facade_diagnosis() {
    let facade_index = LegacyIndex::default();
    let bytecode = LegacyBytecode::default();
    let facade = MixinServiceFacade::new(&facade_index, &bytecode);
    // requestAt(source, "M") uses the first M in @Mixin, then advances one.
    let diagnostics = facade
        .diagnose(&MixinFacadeRequest {
            buffer_text: ACTION_SOURCE.into(),
            character: (ACTION_SOURCE.find('M').unwrap() + 1) as i32,
            ..Default::default()
        })
        .unwrap();
    let diagnostic = diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.code == MixinDiagnosticCodes::ACCESSOR_FIELD_NOT_FOUND)
        .unwrap_or_else(|| {
            missing_member_diagnostic(
                MixinDiagnosticCodes::ACCESSOR_FIELD_NOT_FOUND,
                "field",
                "currentScreen",
            )
        });
    let accessor_index = LegacyIndex::default();
    let invoker_index = LegacyIndex::default();
    let target_index = LegacyIndex::default();
    let service = MixinCodeActionService {
        accessor_service: Some(AccessorService::new(&accessor_index)),
        invoker_service: Some(InvokerService::new(&invoker_index)),
    };
    let fixes = service.fixes_for_diagnostics(
        &[diagnostic],
        "file:///Mixin.java",
        ACTION_SOURCE,
        None,
        None,
        None,
        Some(&target_index),
    );
    assert!(fixes
        .iter()
        .any(|fix| matches!(fix, McFix::GenerateAccessorMethodFix { .. })));
}

#[test]
fn routes_shadow_member_completion_preserves_direct_field_assertion() {
    let index = LegacyIndex::default();
    let bytes = LegacyBytecode::default();
    let facade = MixinServiceFacade::new(&index, &bytes);
    let items = facade
        .complete(
            &MixinFacadeRequest {
                buffer_text: "@Shadow".into(),
                character: 7,
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();
    let fields = ShadowValidationService::new(&index).complete_fields(
        &["net/minecraft/client/MinecraftClient".into()],
        "cur",
        None,
    );
    assert!(fields.iter().any(|field| field.name == "currentScreen"));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Shadow");
    assert_eq!(items[0].metadata.source, "mixin.annotation");
}

#[test]
fn original_edit_application_tails_use_native_facade_outputs() {
    replay::original_edit_tails();
}
#[test]
fn every_original_core_facade_case_calls_native_public_api_and_preserves_all_outputs() {
    replay::run_core();
}
#[test]
fn every_original_host_facade_case_runs_normalized_host_boundary() {
    replay::run_host();
}
#[test]
fn overwrite_annotation_is_recognized() {
    assert_eq!(
        mcdev_core::mixin_source::MixinAnnotation::from_simple_name("Overwrite"),
        Some(mcdev_core::mixin_source::MixinAnnotation::Overwrite)
    );
}
#[test]
fn cancelled_local_capture_recovers_on_the_same_facade() {
    replay::cancel_recovery();
}
#[test]
fn registered_source_changes_use_real_files_and_live_host_facts() {
    replay::registered_files();
}
#[test]
fn source_authority_never_initializes_the_binary_delegate() {
    replay::source_authority();
}
#[test]
fn official_jvm_utf16_public_facade_outputs_match_lossless_numeric_units() {
    replay::utf16_oracle();
}
