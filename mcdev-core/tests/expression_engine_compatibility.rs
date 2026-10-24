use mcdev_core::expression_engine::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;
fn sample() -> Vec<u8> {
    let deltas: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/expression_engine/jsr_fatal_deltas.json"
    ))
    .unwrap();
    let delta = &deltas[0];
    let classes: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/expression_engine/class_bytes.json")).unwrap();
    let bytes = classes[delta["classSha256"].as_str().unwrap()]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as u8)
        .collect::<Vec<_>>();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        delta["classSha256"].as_str().unwrap()
    );
    assert_eq!(
        u16::from_be_bytes([bytes[6], bytes[7]]) as u64,
        delta["classfileVersion"].as_u64().unwrap()
    );
    bytes
}
fn context(pool: &IdentifierPool) -> MatchContext<'_> {
    static RESOLVE: fn(&str, &str) -> Option<String> = |_, _| None;
    MatchContext {
        context_type: MatchContextType::WrapOperation,
        identifier_pool: pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &RESOLVE,
        cancellation: &NO_CANCELLATION,
    }
}
#[test]
fn only_the_official_undefined_jsr_fatal_is_a_typed_unavailable_and_exact_locals_still_support_ret()
{
    let bytes = sample();
    let pool = IdentifierPool::default();
    let context = context(&pool);
    let decoded = decode_method(&bytes, "run", "()I", &context)
        .unwrap()
        .unwrap();
    assert_eq!(decoded.instructions[0].opcode, 0xa8);
    assert_eq!(decoded.instructions[0].byte_offset, 0);
    assert_eq!(match_expressions(&bytes,"run","()I",&["?".into()],&context).unwrap(),MatchResult::Unavailable{reason:"Failed to analyze method bytecode flow".into(),cause:Some("Failed to analyze method bytecode flow: MixinExtras 0.0.6 does not define a JSR expression value".into())});
    let locals = analyze_method_with_policy(
        &bytes,
        "run",
        "()I",
        &context,
        AnalysisPolicy::ExactLocalCapture,
    )
    .unwrap()
    .unwrap();
    let returned = locals
        .instructions
        .iter()
        .position(|i| i.opcode == 0xac)
        .unwrap();
    assert!(locals.frames[returned].is_some());
    let ret = locals
        .instructions
        .iter()
        .position(|i| i.opcode == 0xa9)
        .unwrap();
    assert!(locals.frames[ret].is_some());
}
#[derive(Debug)]
struct Marker;
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("original cancellation")
    }
}
impl std::error::Error for Marker {}
struct DuringAnalysis(Arc<dyn std::error::Error + Send + Sync>);
impl CancellationChecker for DuringAnalysis {
    fn check_cancelled(&self) -> Result<(), Cancellation> {
        Ok(())
    }
    fn check_phase(&self, phase: CancellationPhase) -> Result<(), Cancellation> {
        if phase == CancellationPhase::LocalAnalysis {
            Err(Cancellation(self.0.clone()))
        } else {
            Ok(())
        }
    }
}
#[test]
fn jsr_adaptation_and_invalid_receiver_never_convert_cancellation_to_unavailable() {
    let bytes = sample();
    let pool = IdentifierPool::default();
    let marker: Arc<dyn std::error::Error + Send + Sync> = Arc::new(Marker);
    let checker = DuringAnalysis(marker.clone());
    let mut context = context(&pool);
    context.cancellation = &checker;
    let error = match_expressions(&bytes, "run", "()I", &["?".into()], &context).unwrap_err();
    assert!(Arc::ptr_eq(&error.0, &marker));
    let cancelled = || Err(Cancellation(marker.clone()));
    context.cancellation = &cancelled;
    let error = complete_receiver_members(&bytes, "run", "()I", "new", "", &context).unwrap_err();
    assert!(Arc::ptr_eq(&error.0, &marker));
    let error =
        complete_receiver_members_utf16(&bytes, "run", "()I", &[0xd800], "", &context).unwrap_err();
    assert!(Arc::ptr_eq(&error.0, &marker));
}
#[test]
fn cancellation_scope_restores_the_same_default_after_unwind_and_nested_scopes() {
    let original = current_cancellation();
    assert!(Arc::ptr_eq(&original, &current_cancellation()));
    let outer: Arc<dyn CancellationChecker> = Arc::new(NoCancellation);
    let inner: Arc<dyn CancellationChecker> = Arc::new(NoCancellation);
    with_cancellation(outer.clone(), || {
        assert!(Arc::ptr_eq(&outer, &current_cancellation()));
        with_cancellation(inner.clone(), || {
            assert!(Arc::ptr_eq(&inner, &current_cancellation()))
        });
        assert!(Arc::ptr_eq(&outer, &current_cancellation()));
    });
    assert!(Arc::ptr_eq(&original, &current_cancellation()));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_cancellation(outer, || panic!("scope unwind"))
    }));
    assert!(Arc::ptr_eq(&original, &current_cancellation()));
    current_cancellation().check_cancelled().unwrap();
}
