#[allow(dead_code)]
#[path = "fixtures/mixinextras_locals/replay.rs"]
mod replay;
#[test]
fn debug_frames_preserve_loop_exception_merge_wide_constructor_and_unicode_facts() {
    replay::replay_additional_case(
        "debugFramesPreserveLoopExceptionMergeWideConstructorAndUnicodeFacts",
    );
}
#[test]
fn absent_debug_frames_preserve_loop_exception_merge_wide_and_constructor_facts() {
    replay::replay_additional_case(
        "absentDebugFramesPreserveLoopExceptionMergeWideAndConstructorFacts",
    );
}
