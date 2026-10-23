use std::sync::Arc;

use ktlsp::workspace::Workspace;
use mcdev_ktlsp::{McdevMixinProvider, PROVIDER_ID};

const TARGET: &str = "package fixture; public class Target { public int count; public int shadow$count; public static int total; public int twice(int x) { return x; } public static int global(int x) { return x; } public BlockPos anchor; public BlockState lookup(BlockPos pos) { return null; } public BlockPos[] positions(BlockState state) { return null; } }";

fn workspace(file: &str, source: &str) -> Workspace {
    let mut workspace = Workspace::with_analysis_providers(vec![Arc::new(McdevMixinProvider)]);
    for (file, source) in [
        ("Mixin.java", "package org.spongepowered.asm.mixin; public @interface Mixin { Class<?>[] value() default {}; String[] targets() default {}; }"),
        ("Shadow.java", "package org.spongepowered.asm.mixin; public @interface Shadow { String prefix() default \"shadow$\"; String[] aliases() default {}; }"),
        ("BlockPos.java", "package fixture; public class BlockPos {}"),
        ("BlockState.java", "package fixture; public class BlockState {}"),
        ("Target.java", TARGET),
        (file, source),
    ] {
        workspace.open(file, source.into());
    }
    workspace
}

fn java(annotation: &str, members: &str) -> String {
    format!("package fixture; import org.spongepowered.asm.mixin.Mixin; import org.spongepowered.asm.mixin.Shadow; {annotation} abstract class CaseMixin {{ {members} }}")
}

fn bound(workspace: &mut Workspace, file: &str, source: &str, name: &str) -> bool {
    workspace
        .goto_definition(file, source.find(name).unwrap())
        .iter()
        .any(|target| target.file == "Target.java")
}

fn incomplete(workspace: &mut Workspace, file: &str) -> bool {
    workspace.diagnostics(file).iter().any(|diagnostic| {
        diagnostic.message.starts_with("[mcdev.mixin.incomplete]")
            && diagnostic.severity == ktcore::diagnostics::Severity::Hint
    })
}

#[test]
fn java_exact_methods_fields_dispatch_and_prefixes() {
    let source = java(
        "@Mixin(Target.class)",
        "@Shadow int count; @Shadow static int total;
         @Shadow abstract int shadow$twice(int x);
         @Shadow(prefix=\"custom$\") abstract BlockState custom$lookup(BlockPos pos);
         @Shadow static int global(int x) { return 0; }",
    );
    let mut workspace = workspace("CaseMixin.java", &source);
    for name in [
        "count;",
        "total;",
        "shadow$twice",
        "custom$lookup",
        "global(",
    ] {
        assert!(
            bound(&mut workspace, "CaseMixin.java", &source, name),
            "{name}"
        );
    }
    assert!(!incomplete(&mut workspace, "CaseMixin.java"));
}

#[test]
fn exact_signature_selects_the_correct_overload_location() {
    let source = java(
        "@Mixin(Target.class)",
        "@Shadow abstract int shadow$twice(int x);",
    );
    let mut workspace = workspace("CaseMixin.java", &source);
    let target = "package fixture; public class Target { public int twice(long x) { return 0; } public int twice(int x) { return 0; } }";
    workspace.change("Target.java", target.into());
    let definitions =
        workspace.goto_definition("CaseMixin.java", source.find("shadow$twice").unwrap());
    assert_eq!(definitions.len(), 1, "{definitions:?}");
    assert_eq!(definitions[0].file, "Target.java");
    assert_eq!(definitions[0].start_byte, target.find("twice(int").unwrap());
}

#[test]
fn kotlin_nominal_types_arrays_and_backticks_use_shared_facts() {
    let source = "package fixture
import org.spongepowered.asm.mixin.Mixin
import org.spongepowered.asm.mixin.Shadow
@Mixin(Target::class)
abstract class CaseMixin {
    @field:Shadow private lateinit var anchor: BlockPos
    @Shadow abstract fun `shadow$twice`(x: Int): Int
    @Shadow abstract fun lookup(pos: BlockPos): BlockState
    @Shadow abstract fun positions(state: BlockState): Array<BlockPos>
}";
    let mut workspace = workspace("CaseMixin.kt", source);
    for name in ["anchor:", "shadow$twice", "lookup(", "positions("] {
        assert!(
            bound(&mut workspace, "CaseMixin.kt", source, name),
            "{name}"
        );
    }
    assert!(!incomplete(&mut workspace, "CaseMixin.kt"));
}

#[test]
fn single_class_and_fully_qualified_string_targets_bind() {
    for annotation in [
        "@Mixin(value=Target.class)",
        "@Mixin({Target.class})",
        "@Mixin(targets=\"fixture.Target\")",
        "@Mixin(targets={\"fixture.Target\"})",
    ] {
        let source = java(annotation, "@Shadow abstract int twice(int x);");
        let mut workspace = workspace("CaseMixin.java", &source);
        assert!(
            bound(&mut workspace, "CaseMixin.java", &source, "twice("),
            "{annotation}"
        );
    }
    for annotation in [
        "@Mixin([Target::class])",
        "@Mixin(targets = [\"fixture.Target\"])",
    ] {
        let source = format!("package fixture\nimport org.spongepowered.asm.mixin.Mixin\nimport org.spongepowered.asm.mixin.Shadow\n{annotation}\nabstract class CaseMixin {{ @Shadow abstract fun twice(x: Int): Int }}");
        let mut workspace = workspace("CaseMixin.kt", &source);
        assert!(
            bound(&mut workspace, "CaseMixin.kt", &source, "twice("),
            "{annotation}"
        );
    }
}

#[test]
fn unsupported_or_multiple_targets_never_choose_a_first_target() {
    for annotation in [
        "@Mixin({Target.class, BlockPos.class})",
        "@Mixin(value=Target.class, targets=\"fixture.Target\")",
        "@Mixin(targets=TARGET_NAME)",
        "@Mixin(value=\"fixture.Target\")",
        "@Mixin(targets=Target.class)",
    ] {
        let source = java(annotation, "@Shadow abstract int twice(int x);");
        let mut workspace = workspace("CaseMixin.java", &source);
        assert!(
            !bound(&mut workspace, "CaseMixin.java", &source, "twice("),
            "{annotation}"
        );
        assert!(incomplete(&mut workspace, "CaseMixin.java"), "{annotation}");
    }
}

#[test]
fn exact_annotation_identity_is_required() {
    for (mixin, shadow) in [
        ("fake.Mixin", "org.spongepowered.asm.mixin.Shadow"),
        ("org.spongepowered.asm.mixin.Mixin", "fake.Shadow"),
    ] {
        let source = format!("package fixture; @{mixin}(Target.class) abstract class CaseMixin {{ @{shadow} abstract int twice(int x); }}");
        let mut workspace = workspace("CaseMixin.java", &source);
        workspace.open(
            "FakeMixin.java",
            "package fake; public @interface Mixin { Class<?> value(); }".into(),
        );
        workspace.open(
            "FakeShadow.java",
            "package fake; public @interface Shadow {}".into(),
        );
        assert!(!bound(&mut workspace, "CaseMixin.java", &source, "twice("));
    }
}

#[test]
fn descriptor_dispatch_unknown_and_aliases_do_not_guess() {
    for member in [
        "@Shadow abstract long twice(int x);",
        "@Shadow static int twice(int x) { return 0; }",
        "@Shadow abstract int global(int x);",
        "@Shadow abstract int twice(Unresolved x);",
        "@Shadow(prefix=PREFIX) abstract int twice(int x);",
        "@Shadow(aliases={\"other\"}) abstract int twice(int x);",
        "@Shadow abstract <T> T twice(T x);",
    ] {
        let source = java("@Mixin(Target.class)", member);
        let mut workspace = workspace("CaseMixin.java", &source);
        let name = if member.contains("global") {
            "global("
        } else {
            "twice("
        };
        assert!(
            !bound(&mut workspace, "CaseMixin.java", &source, name),
            "{member}"
        );
    }
    let source = java(
        "@Mixin(UnknownTarget.class)",
        "@Shadow abstract int twice(int x);",
    );
    let mut workspace = workspace("CaseMixin.java", &source);
    assert!(!bound(&mut workspace, "CaseMixin.java", &source, "twice("));
    assert!(incomplete(&mut workspace, "CaseMixin.java"));
    assert!(!workspace
        .diagnostics("CaseMixin.java")
        .iter()
        .any(|item| item.message.starts_with("[mcdev.mixin.shadow.missing]")));
}

#[test]
fn kotlin_accessors_and_unknown_abi_are_not_fields_or_plain_methods() {
    for member in [
        "@get:Shadow val count: Int get() = 0",
        "@set:Shadow var count: Int = 0",
        "@Shadow val count: Int get() = 0",
        "@Shadow var count: Int = 0",
        "@Shadow abstract fun lookup(pos: BlockPos?): BlockState",
        "@Shadow abstract fun lookup(pos: BlockPos): BlockState?",
    ] {
        let source = format!("package fixture\nimport org.spongepowered.asm.mixin.Mixin\nimport org.spongepowered.asm.mixin.Shadow\n@Mixin(Target::class)\nabstract class CaseMixin {{ {member} }}");
        let mut workspace = workspace("CaseMixin.kt", &source);
        let name = if member.contains("count") {
            "count:"
        } else {
            "lookup("
        };
        assert!(
            !bound(&mut workspace, "CaseMixin.kt", &source, name),
            "{member}"
        );
    }
}

#[test]
fn fields_do_not_strip_the_method_prefix() {
    for member in [
        "@Shadow int shadow$count;",
        "@Shadow(prefix=\"\") int count;",
    ] {
        let source = java("@Mixin(Target.class)", member);
        let mut workspace = workspace("CaseMixin.java", &source);
        let name = if member.contains("shadow$count") {
            "shadow$count"
        } else {
            "count;"
        };
        assert!(!bound(&mut workspace, "CaseMixin.java", &source, name));
        assert!(workspace
            .diagnostics("CaseMixin.java")
            .iter()
            .any(|item| item
                .message
                .starts_with("[mcdev.mixin.shadow.field-prefix]")));
    }
}

#[test]
fn empty_effective_name_does_not_discard_other_contributions() {
    let source = java(
        "@Mixin(Target.class)",
        "@Shadow(prefix=\"bad\") abstract int bad(); @Shadow abstract int twice(int x);",
    );
    let mut workspace = workspace("CaseMixin.java", &source);
    assert!(!bound(&mut workspace, "CaseMixin.java", &source, "bad()"));
    assert!(bound(&mut workspace, "CaseMixin.java", &source, "twice("));
    assert!(incomplete(&mut workspace, "CaseMixin.java"));
}

#[test]
fn ambiguous_owner_never_binds_to_another_declaration() {
    let source = java("@Mixin(Target.class)", "@Shadow abstract int twice(int x);");
    let mut workspace = workspace("CaseMixin.java", &source);
    workspace.open(
        "Duplicate.java",
        "package fixture; class CaseMixin { long twice(long x) { return x; } }".into(),
    );
    assert!(!bound(&mut workspace, "CaseMixin.java", &source, "twice("));
    assert!(incomplete(&mut workspace, "CaseMixin.java"));
}

#[test]
fn unsaved_edits_and_provider_configuration_retract_and_rebind() {
    let source = java(
        "@Mixin(Target.class)",
        "@Shadow abstract int shadow$twice(int x);",
    );
    let mut workspace = workspace("CaseMixin.java", &source);
    assert!(bound(
        &mut workspace,
        "CaseMixin.java",
        &source,
        "shadow$twice"
    ));
    workspace.change("Target.java", TARGET.replace("int twice", "long twice"));
    assert!(!bound(
        &mut workspace,
        "CaseMixin.java",
        &source,
        "shadow$twice"
    ));
    workspace.change("Target.java", TARGET.into());
    assert!(bound(
        &mut workspace,
        "CaseMixin.java",
        &source,
        "shadow$twice"
    ));
    let changed = source.replace("int shadow$twice", "long shadow$twice");
    workspace.change("CaseMixin.java", changed.clone());
    assert!(!bound(
        &mut workspace,
        "CaseMixin.java",
        &changed,
        "shadow$twice"
    ));
    workspace.change("CaseMixin.java", source.clone());
    assert!(workspace.configure_analysis_provider(PROVIDER_ID, false, Arc::from("{}")));
    assert!(!bound(
        &mut workspace,
        "CaseMixin.java",
        &source,
        "shadow$twice"
    ));
    assert!(!incomplete(&mut workspace, "CaseMixin.java"));
    assert!(workspace.configure_analysis_provider(PROVIDER_ID, true, Arc::from("{}")));
    assert!(bound(
        &mut workspace,
        "CaseMixin.java",
        &source,
        "shadow$twice"
    ));
}

#[test]
fn rename_preserves_prefix_backticks_and_unrelated_source() {
    for (file, source, old) in [
        ("CaseMixin.java", java("@Mixin(Target.class)", "@Shadow abstract int shadow$twice(int x); int unrelated;"), "shadow$twice"),
        ("CaseMixin.kt", "package fixture\nimport org.spongepowered.asm.mixin.Mixin\nimport org.spongepowered.asm.mixin.Shadow\n@Mixin(Target::class)\nabstract class CaseMixin { @Shadow abstract fun `shadow$twice`(x: Int): Int; val unrelated = 1 }".into(), "shadow$twice"),
    ] {
        let mut workspace = workspace(file, &source);
        let edits = workspace.rename(file, source.find(old).unwrap() + "shadow$".len(), "doubled").expect("rename");
        ktlsp::edit::validate_non_overlapping(&edits).unwrap();
        let mut edited = source.clone();
        let mut local: Vec<_> = edits.iter().filter(|edit| edit.file == file).collect();
        local.sort_by_key(|edit| std::cmp::Reverse(edit.start_byte));
        for edit in local { edited.replace_range(edit.start_byte..edit.end_byte, &edit.new_text); }
        assert_eq!(edited, source.replace(old, "shadow$doubled"));
        assert!(edits.iter().any(|edit| edit.file == "Target.java" && edit.new_text == "doubled"));
    }
}
