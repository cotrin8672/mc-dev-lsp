use mcdev_core::{
    mixin_completion::*,
    mixin_source::{AnnotationContext, AnnotationSlot, MixinAnnotation},
    *,
};
#[path = "fixtures/mixin_completion/index.rs"]
mod support;
use support::*;
fn source(annotation: &str, header: &str) -> String {
    format!("@Mixin(MinecraftClient.class)\n{header} {{\n    {annotation}\n}}")
}
fn complete(index: &Index, src: &str, annotation: MixinAnnotation) -> Vec<McCompletionItem> {
    MixinDeclarationSnippetService::new(index).complete_at(
        src,
        annotation,
        src.find(&format!("@{}", annotation.simple_name())).unwrap(),
        &[OWNER.into()],
        "",
    )
}
#[test]
fn accessor_getter_setter_placeholders_imports_and_edit_application() {
    let index = Index::default();
    let src = source("@Accessor(\"currentScreen\")", "class Patch");
    let items = complete(&index, &src, MixinAnnotation::Accessor);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].insert_text, "public abstract Screen ${1}();$0");
    assert_eq!(
        items[1].insert_text,
        "public abstract void ${1}(Screen value);$0"
    );
    for item in items {
        assert_eq!(
            item.insert_text_format,
            McCompletionInsertTextFormat::Snippet
        );
        let applied = apply_text_edits(&src, &item.additional_edits).unwrap();
        assert!(applied.contains("abstract class Patch"));
        assert!(applied.contains("import net.minecraft.client.gui.screen.Screen;"));
    }
}
#[test]
fn static_accessors_use_throwing_body_without_abstract() {
    let src = source("@Accessor(\"LOGGER\")", "class Patch");
    let items = complete(&Index::default(), &src, MixinAnnotation::Accessor);
    assert_eq!(items.len(), 2);
    assert!(items[0].insert_text.starts_with("static String ${1}()"));
    assert!(items[1]
        .insert_text
        .starts_with("static void ${1}(String value)"));
    for item in items {
        assert!(
            item.insert_text.find("$0").unwrap()
                < item
                    .insert_text
                    .find("throw new AssertionError();")
                    .unwrap()
        );
        assert!(!item.insert_text.contains("abstract"));
        assert!(item.additional_edits.is_empty());
    }
}
#[test]
fn instance_interface_abstract_and_unsafe_enclosing_types() {
    for header in ["interface Patch", "abstract class Patch"] {
        let src = source("@Accessor(\"currentScreen\")", header);
        let items = complete(&Index::default(), &src, MixinAnnotation::Accessor);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| !i
            .additional_edits
            .iter()
            .any(|e| e.new_text == "abstract class")));
    }
    for header in ["final class Patch", "record Patch()", "enum Patch"] {
        let src = source("@Accessor(\"currentScreen\")", header);
        assert!(complete(&Index::default(), &src, MixinAnnotation::Accessor).is_empty());
    }
}
#[test]
fn invoker_overloads_descriptor_selectors_and_static_methods() {
    let mut index = Index::default();
    let src = source("@Invoker(\"render\")", "interface Patch");
    let items = complete(&index, &src, MixinAnnotation::Invoker);
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .all(|i| i.insert_text.starts_with("public abstract void ${1}(")));
    assert_ne!(items[0].metadata.descriptor, items[1].metadata.descriptor);
    let src = source("@Invoker(value = \"render(I)V\")", "abstract class Patch");
    let item = complete(&index, &src, MixinAnnotation::Invoker).remove(0);
    assert_eq!(item.metadata.descriptor.as_deref(), Some("(I)V"));
    assert_eq!(item.insert_text, "public abstract void ${1}(int arg0);$0");
    index.methods.get_mut(OWNER).unwrap().push(method(
        "factory",
        "(I)Lnet/minecraft/client/MinecraftClient;",
        true,
    ));
    let src = source("@Invoker(\"factory\")", "class Patch");
    let item = complete(&index, &src, MixinAnnotation::Invoker).remove(0);
    assert!(item
        .insert_text
        .starts_with("static MinecraftClient ${1}(int arg0)"));
    assert!(item.insert_text.contains("throw new AssertionError();"));
}
#[test]
fn constructor_invoker_explicit_and_simple_owner_alias() {
    let mut index = Index::default();
    index
        .methods
        .get_mut(OWNER)
        .unwrap()
        .push(method("<init>", "(Ljava/lang/String;)V", false));
    for target in ["<init>", "MinecraftClient"] {
        let src = source(&format!("@Invoker(\"{target}\")"), "class Patch");
        let items = complete(&index, &src, MixinAnnotation::Invoker);
        assert_eq!(items.len(), 1);
        assert!(items[0].label.contains("constructor"));
        assert!(items[0]
            .insert_text
            .starts_with("static MinecraftClient ${1}(String arg0)"));
    }
}
#[test]
fn overwrite_signature_choices_throwing_nonvoid_and_prefix() {
    let mut index = Index::default();
    let src = source("@Overwrite", "class Patch");
    let items = complete(&index, &src, MixinAnnotation::Overwrite);
    assert_eq!(items.len(), 4);
    assert!(items
        .iter()
        .any(|i| i.insert_text == "public void ${1}() {\n    $0\n}"));
    assert!(items.iter().any(|i| i.insert_text.contains("long arg1")));
    index
        .methods
        .get_mut(OWNER)
        .unwrap()
        .push(method("compute", "(I)I", false));
    index
        .methods
        .get_mut(OWNER)
        .unwrap()
        .push(method("staticCompute", "()I", true));
    let service = MixinDeclarationSnippetService::new(&index);
    let item = service
        .complete_at(
            &src,
            MixinAnnotation::Overwrite,
            src.find("@Overwrite").unwrap(),
            &[OWNER.into()],
            "compute",
        )
        .remove(0);
    assert!(item.insert_text.starts_with("public int ${1}(int arg0)"));
    assert!(item.insert_text.find("$0").unwrap() < item.insert_text.find("throw").unwrap());
    assert!(complete(&index, &src, MixinAnnotation::Overwrite)
        .iter()
        .any(|i| i.insert_text.starts_with("public static int")));
}
#[test]
fn multi_target_fields_require_same_signature_and_invokers_intersection() {
    let mut index = Index::default();
    let other = "net/minecraft/world/World";
    index
        .fields
        .insert(other.into(), vec![field("currentScreen", "I", false)]);
    index
        .methods
        .insert(other.into(), vec![method("render", "(I)V", false)]);
    let service = MixinDeclarationSnippetService::new(&index);
    let src = source("@Accessor(\"currentScreen\")", "class Patch");
    assert!(service
        .complete_at(
            &src,
            MixinAnnotation::Accessor,
            src.find("@Accessor").unwrap(),
            &[OWNER.into(), other.into()],
            ""
        )
        .is_empty());
    let src = source("@Invoker(\"render\")", "abstract class Patch");
    let items = service.complete_at(
        &src,
        MixinAnnotation::Invoker,
        src.find("@Invoker").unwrap(),
        &[OWNER.into(), other.into()],
        "",
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].metadata.descriptor.as_deref(), Some("(I)V"));
    let src = source("@Overwrite", "class Patch");
    assert!(service
        .complete_at(
            &src,
            MixinAnnotation::Overwrite,
            src.find("@Overwrite").unwrap(),
            &[OWNER.into(), other.into()],
            ""
        )
        .is_empty());
}
#[test]
fn existing_members_after_annotations_and_stacked_markers_suppress_duplicates() {
    for (annotation, kind) in [
        (
            "@Accessor(\"currentScreen\")\npublic abstract Screen getCurrentScreen();",
            MixinAnnotation::Accessor,
        ),
        (
            "@Invoker(\"render\")\npublic abstract void render();",
            MixinAnnotation::Invoker,
        ),
        (
            "@Overwrite\npublic void tick() {}",
            MixinAnnotation::Overwrite,
        ),
        (
            "@Accessor(\"counter\")\n// note\n@Deprecated\npublic int getCounter();",
            MixinAnnotation::Accessor,
        ),
    ] {
        let src = source(annotation, "abstract class Patch");
        assert!(complete(&Index::default(), &src, kind).is_empty());
    }
}
#[test]
fn comment_type_does_not_get_class_edit_and_unresolved_never_fabricates() {
    let src = "/* class Fake {\n@Accessor(\"currentScreen\")\n}\n*/";
    let items = complete(&Index::default(), src, MixinAnnotation::Accessor);
    assert!(!items.is_empty());
    assert!(items.iter().all(|i| !i
        .additional_edits
        .iter()
        .any(|e| e.new_text == "abstract class")));
    for annotation in ["@Accessor(\"missing\")", "@Accessor", "@Accessor(\"\")"] {
        let src = source(annotation, "class Patch");
        assert!(complete(&Index::default(), &src, MixinAnnotation::Accessor).is_empty());
    }
    let src = source("@Invoker(\"render(bad)\")", "class Patch");
    assert!(complete(&Index::default(), &src, MixinAnnotation::Invoker).is_empty());
}
#[test]
fn context_handler_contract_resolves_targets_and_explicit_offsets() {
    let index = Index::default();
    let src = source("@Accessor(\"counter\")", "abstract class Patch");
    let mut context =
        AnnotationContext::new(MixinAnnotation::Accessor, AnnotationSlot::Handler, "");
    context.annotation_start_offset = src.find("@Accessor").unwrap();
    context.mixin_target_internal_names = vec![OWNER.into()];
    let service = MixinDeclarationSnippetService::new(&index);
    assert_eq!(service.complete(&src, &context).len(), 2);
    context.slot = AnnotationSlot::Attribute;
    assert!(service.complete(&src, &context).is_empty());
    context.slot = AnnotationSlot::Handler;
    context.annotation_start_offset = 1;
    context.value_start_offset = src.len();
    assert_eq!(service.complete(&src, &context).len(), 2);
}
#[test]
fn descriptor_types_nested_conflicts_arrays_and_merged_import_edits() {
    let mut index = Index::default();
    index.methods.get_mut(OWNER).unwrap().push(method(
        "complex",
        "([La/Thing;Lb/Thing;Lx/Outer$Inner;)Ljava/util/List;",
        false,
    ));
    let src = source("@Invoker(\"complex\")", "abstract class Patch");
    let item = complete(&index, &src, MixinAnnotation::Invoker).remove(0);
    assert!(item.insert_text.contains("a.Thing[] arg0"));
    assert!(item.insert_text.contains("b.Thing arg1"));
    assert!(item.insert_text.contains("x.Outer.Inner arg2"));
    assert_eq!(item.additional_edits.len(), 1);
    let applied = apply_text_edits(&src, &item.additional_edits).unwrap();
    assert!(applied.contains("import java.util.List;"));
}

#[test]
fn unfinished_accessor_and_invoker_arguments_never_generate_a_declaration() {
    let index = Index::default();
    for (text, annotation) in [
        ("@Accessor(\"counter\"", MixinAnnotation::Accessor),
        ("@Invoker(\"render\"", MixinAnnotation::Invoker),
    ] {
        let source = source(text, "abstract class Patch");
        assert!(complete(&index, &source, annotation).is_empty());
    }
}
