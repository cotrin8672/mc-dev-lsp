use mcdev_core::{
    bytecode::{AtTargetCandidate, AtTargetKind, ConstantValue},
    mapping::{MappingSetResolver, TinyV2Parser},
    mixin_completion::*,
    mixin_source::{AnnotationContext, AnnotationSlot, MixinAnnotation},
    *,
};
#[path = "fixtures/mixin_completion/index.rs"]
mod support;
use std::{cell::Cell, collections::BTreeSet};
use support::*;
fn context(annotation: MixinAnnotation, slot: AnnotationSlot, partial: &str) -> AnnotationContext {
    AnnotationContext::new(annotation, slot, partial)
}
fn at_value(partial: &str) -> AnnotationContext {
    context(MixinAnnotation::At, AnnotationSlot::Value, partial)
}
fn labels(items: Vec<McCompletionItem>) -> Vec<String> {
    items.into_iter().map(|i| i.label).collect()
}
fn candidate(kind: AtTargetKind, name: &str, desc: &str) -> AtTargetCandidate {
    let mut c = AtTargetCandidate::new(kind, OWNER, name, desc);
    c.display_label = format!("{name}{desc}");
    c.detail = "readable detail".into();
    c
}
#[test]
fn constant_hint_uses_lossless_shared_java_number_formatting() {
    for (value, wanted) in [
        (
            ConstantValue::FloatValue(f32::from_bits(1)),
            "@Constant(floatValue = 1.4E-45f)",
        ),
        (
            ConstantValue::DoubleValue(f64::from_bits(1)),
            "@Constant(doubleValue = 4.9E-324)",
        ),
    ] {
        let mut c = candidate(AtTargetKind::Constant, "", "");
        c.constant_value = Some(value);
        assert_eq!(
            ConstantAtHintFormatter::hint_for(&c).as_deref(),
            Some(wanted)
        );
    }
}
#[test]
fn at_value_all_and_prefixes() {
    let service = AtValueCompletionService::default();
    assert_eq!(labels(service.complete(&at_value("HE"))), vec!["HEAD"]);
    assert_eq!(service.complete(&at_value("")).len(), 13);
    assert!(service.complete(&at_value("unknown")).is_empty());
    assert_eq!(
        labels(service.complete(&at_value("\"head\""))),
        vec!["HEAD"]
    );
    let mut c = at_value("");
    c.annotation = MixinAnnotation::Inject;
    assert!(service.complete(&c).is_empty());
    c.annotation = MixinAnnotation::At;
    c.slot = AnnotationSlot::Target;
    assert!(service.complete(&c).is_empty());
}
#[test]
fn at_value_parent_constraints_and_slice_override() {
    let service = AtValueCompletionService::default();
    for (parent, expected) in [
        (
            MixinAnnotation::ModifyReturnValue,
            vec!["RETURN", "TAIL", "MIXINEXTRAS:EXPRESSION"],
        ),
        (
            MixinAnnotation::ModifyExpressionValue,
            vec![
                "INVOKE",
                "FIELD",
                "NEW",
                "CONSTANT",
                "MIXINEXTRAS:EXPRESSION",
            ],
        ),
        (
            MixinAnnotation::ModifyReceiver,
            vec!["INVOKE", "FIELD", "MIXINEXTRAS:EXPRESSION"],
        ),
        (
            MixinAnnotation::WrapWithCondition,
            vec!["INVOKE", "FIELD", "MIXINEXTRAS:EXPRESSION"],
        ),
        (
            MixinAnnotation::WrapOperation,
            vec!["INVOKE", "FIELD", "NEW", "MIXINEXTRAS:EXPRESSION"],
        ),
    ] {
        let mut c = at_value("");
        c.parent_injector_annotation = Some(parent);
        assert_eq!(labels(service.complete(&c)), expected);
        c.at_inside_slice = true;
        assert_eq!(service.complete(&c).len(), 13);
    }
}
#[test]
fn at_specifiers_prefix_namespaced_and_lazy_probe() {
    let calls = Cell::new(0);
    let service = AtValueCompletionService::new(
        || {
            calls.set(calls.get() + 1);
            true
        },
        Vec::new,
    );
    for partial in ["HE", "UNKNOWN:"] {
        service.complete(&at_value(partial));
        assert_eq!(calls.get(), 0);
    }
    let mut c = at_value("INVOKE:");
    c.at_inside_slice = true;
    assert_eq!(
        labels(service.complete(&c)),
        vec![
            "INVOKE:FIRST",
            "INVOKE:LAST",
            "INVOKE:ONE",
            "INVOKE:ALL",
            "INVOKE:DEFAULT"
        ]
    );
    assert_eq!(calls.get(), 0);
    c.at_inside_slice = false;
    c.partial_value = "INVOKE:F".into();
    assert_eq!(labels(service.complete(&c)), vec!["INVOKE:FIRST"]);
    assert_eq!(calls.get(), 1);
    c.partial_value = "MIXINEXTRAS:EXPRESSION:L".into();
    assert_eq!(
        labels(service.complete(&c)),
        vec!["MIXINEXTRAS:EXPRESSION:LAST"]
    );
    assert_eq!(calls.get(), 2);
    c.parent_injector_annotation = Some(MixinAnnotation::ModifyReturnValue);
    c.partial_value = "INVOKE:".into();
    assert!(service.complete(&c).is_empty());
    assert_eq!(calls.get(), 2);
}
#[test]
fn at_specifier_disabled_and_additional_dedup_restrictions() {
    let service = AtValueCompletionService::new(
        || false,
        || {
            vec![
                "head".into(),
                "custom:POINT".into(),
                "custom:point".into(),
                "com.example.Point".into(),
                "INVOKE:FIRST".into(),
            ]
        },
    );
    assert!(service.complete(&at_value("RETURN:")).is_empty());
    assert_eq!(
        labels(service.complete(&at_value("custom"))),
        vec!["custom:POINT"]
    );
    assert_eq!(
        labels(service.complete(&at_value("com"))),
        vec!["com.example.Point"]
    );
    assert_eq!(
        service
            .complete(&at_value(""))
            .iter()
            .filter(|i| i.label.eq_ignore_ascii_case("HEAD"))
            .count(),
        1
    );
    let mut c = at_value("");
    c.parent_injector_annotation = Some(MixinAnnotation::ModifyReturnValue);
    assert!(!labels(service.complete(&c))
        .iter()
        .any(|s| s.starts_with("INVOKE")));
    let custom = AtValueCompletionService::new(|| true, || vec!["custom:POINT".into()]);
    assert_eq!(
        labels(custom.complete(&at_value("custom:POINT:A"))),
        vec!["custom:POINT:ALL"]
    );
}
#[test]
fn at_target_formats_every_kind_and_preserves_metadata() {
    let c = context(MixinAnnotation::At, AnnotationSlot::Target, "");
    for (kind, name, desc, expected) in [
        (
            AtTargetKind::Invoke,
            "tick",
            "()V",
            format!("L{OWNER};tick()V"),
        ),
        (
            AtTargetKind::Field,
            "counter",
            "I",
            format!("L{OWNER};counter:I"),
        ),
        (AtTargetKind::New, "new", "", format!("L{OWNER};")),
        (
            AtTargetKind::New,
            "<init>",
            "(I)V",
            format!("L{OWNER};<init>(I)V"),
        ),
        (AtTargetKind::Return, "return", "V", "RETURN".into()),
    ] {
        let candidate = candidate(kind, name, desc);
        let items = AtTargetCompletionService::complete(&c, &[candidate]);
        assert_eq!(items[0].insert_text, expected);
        assert_eq!(items[0].metadata.owner.as_deref(), Some(OWNER));
        assert_eq!(items[0].metadata.namespace.as_deref(), Some("NAMED"));
        assert_eq!(items[0].detail.as_deref(), Some("readable detail"));
        assert_eq!(items[0].documentation.as_deref(), Some(expected.as_str()));
    }
}
#[test]
fn at_target_dedups_selectors_keeps_overloads_constants_and_first_metadata() {
    let mut first = candidate(AtTargetKind::Invoke, "render", "(I)V");
    first.detail = "first".into();
    let mut second = first.clone();
    second.ordinal = Some(1);
    second.detail = "second".into();
    let overload = candidate(AtTargetKind::Invoke, "render", "(IJ)V");
    let field = candidate(AtTargetKind::Field, "counter", "I");
    let mut dup_field = field.clone();
    dup_field.ordinal = Some(1);
    let mut a = candidate(AtTargetKind::Constant, "constant", "");
    a.display_label = "1".into();
    let mut b = a.clone();
    b.display_label = "2".into();
    let all = vec![first, second, overload, field, dup_field, a, b];
    let result = AtTargetCompletionService::complete(
        &context(MixinAnnotation::At, AnnotationSlot::Target, ""),
        &all,
    );
    assert_eq!(result.len(), 5);
    assert_eq!(result[0].detail.as_deref(), Some("first"));
    assert_eq!(
        AtTargetCompletionService::complete(
            &context(MixinAnnotation::At, AnnotationSlot::Target, "RENDER"),
            &all
        )
        .len(),
        2
    );
    assert!(AtTargetCompletionService::complete(
        &context(MixinAnnotation::Inject, AnnotationSlot::Target, ""),
        &all
    )
    .is_empty());
}
#[test]
fn constant_hints_cover_all_types_and_escape_values() {
    for (value, want) in [
        (
            ConstantValue::StringValue("a\"\\\n\r\t".into()),
            "@Constant(stringValue = \"a\\\"\\\\\\n\\r\\t\")".into(),
        ),
        (ConstantValue::IntValue(2), "@Constant(intValue = 2)".into()),
        (
            ConstantValue::LongValue(3),
            "@Constant(longValue = 3L)".into(),
        ),
        (
            ConstantValue::FloatValue(0.1),
            "@Constant(floatValue = 0.1f)".into(),
        ),
        (
            ConstantValue::FloatValue(1.0),
            "@Constant(floatValue = 1.0f)".into(),
        ),
        (
            ConstantValue::DoubleValue(2.0),
            "@Constant(doubleValue = 2.0)".into(),
        ),
        (
            ConstantValue::ClassLiteral("a/B".into()),
            "@Constant(classValue = \"a.B.class\")".into(),
        ),
        (
            ConstantValue::NullValue,
            "@Constant(nullValue = true)".into(),
        ),
    ] {
        let mut c = candidate(AtTargetKind::Constant, "constant", "");
        c.constant_value = Some(value);
        assert_eq!(ConstantAtHintFormatter::hint_for(&c), Some(want));
    }
    let c = candidate(AtTargetKind::Invoke, "tick", "()V");
    assert_eq!(ConstantAtHintFormatter::hint_for(&c), None);
}
#[test]
fn at_target_remaps_methods_fields_constructors_and_preserves_unmapped() {
    let mappings=TinyV2Parser::parse(&format!("tiny\t2\t0\tnamed\tintermediary\nc\t{OWNER}\tx/Y\n\tm\t()V\ttick\tm_1\n\tf\tI\tcounter\tf_1\n\tm\t(I)V\t<init>\t<init>\n")).unwrap();
    let resolver = MappingSetResolver::new(mappings);
    for (kind, name, desc, expected) in [
        (AtTargetKind::Invoke, "tick", "()V", "Lx/Y;m_1()V"),
        (AtTargetKind::Field, "counter", "I", "Lx/Y;f_1:I"),
        (AtTargetKind::New, "<init>", "(I)V", "Lx/Y;<init>(I)V"),
    ] {
        let c = candidate(kind, name, desc);
        assert_eq!(
            AtTargetInsertFormatter::format_insert(
                &c,
                Some(&resolver),
                MappingNamespace::Named,
                MappingNamespace::Intermediary
            ),
            expected
        );
        assert_eq!(
            AtTargetInsertFormatter::format_insert(
                &c,
                None,
                MappingNamespace::Named,
                MappingNamespace::Intermediary
            ),
            AtTargetCompletionService::format_target(&c)
        );
    }
    for kind in [
        AtTargetKind::New,
        AtTargetKind::Constant,
        AtTargetKind::Return,
        AtTargetKind::Invoke,
    ] {
        let c = candidate(kind, "missing", "");
        assert_eq!(
            AtTargetInsertFormatter::format_insert(
                &c,
                Some(&resolver),
                MappingNamespace::Named,
                MappingNamespace::Intermediary
            ),
            AtTargetCompletionService::format_target(&c)
        );
    }
}
#[test]
fn inject_method_descriptor_modes_filters_labels_and_metadata() {
    let index = Index::default();
    let service = InjectMethodCompletionService::new(&index);
    let mut c = context(MixinAnnotation::Inject, AnnotationSlot::Method, "render");
    c.mixin_target_internal_names = vec![OWNER.into()];
    let auto = service.complete(&c, &MixinCompletionOptions::default());
    assert_eq!(auto.len(), 2);
    assert!(auto.iter().all(|i| i.insert_text.starts_with("render(")));
    assert_eq!(auto[0].metadata.descriptor.as_deref(), Some("(I)V"));
    assert_eq!(
        auto[0].detail.as_deref(),
        Some("net.minecraft.client.MinecraftClient")
    );
    let never = service.complete(
        &c,
        &MixinCompletionOptions {
            inject_method_descriptor_mode: InjectMethodDescriptorMode::Never,
            ..Default::default()
        },
    );
    assert_eq!(never.len(), 1);
    assert_eq!(never[0].insert_text, "render");
    c.partial_value = "tick".into();
    assert_eq!(
        service.complete(&c, &Default::default())[0].insert_text,
        "tick"
    );
    assert_eq!(
        service.complete(
            &c,
            &MixinCompletionOptions {
                inject_method_descriptor_mode: InjectMethodDescriptorMode::Always,
                ..Default::default()
            }
        )[0]
        .insert_text,
        "tick()V"
    );
    c.mixin_target_internal_names.clear();
    assert!(service.complete(&c, &Default::default()).is_empty());
    c.slot = AnnotationSlot::Target;
    assert!(service.complete(&c, &Default::default()).is_empty());
}
#[test]
fn inject_multiple_targets_and_partial_descriptor() {
    let mut index = Index::default();
    index.methods.insert(
        "net/minecraft/world/World".into(),
        vec![method("tick", "()V", false)],
    );
    let service = InjectMethodCompletionService::new(&index);
    let mut c = context(MixinAnnotation::Inject, AnnotationSlot::Method, "tick(");
    c.mixin_target_internal_names = vec![OWNER.into(), "net/minecraft/world/World".into()];
    assert_eq!(service.complete(&c, &Default::default()).len(), 2);
}
#[test]
fn class_targets_import_fqn_strings_prefix_limit_and_metadata() {
    let mut index = Index::default();
    let service = MixinTargetCompletionService::new(&index);
    let c = context(MixinAnnotation::Mixin, AnnotationSlot::Class, "Mine");
    let value = service.complete(&c, &Default::default(), "").remove(0);
    assert_eq!(value.insert_text, "MinecraftClient.class");
    assert_eq!(value.label, "MinecraftClient");
    assert_eq!(value.detail.as_deref(), Some("net.minecraft.client"));
    assert_eq!(value.sort_key, "0100_MinecraftClient");
    assert_eq!(value.metadata.owner.as_deref(), Some(OWNER));
    assert_eq!(
        service.complete(
            &c,
            &MixinCompletionOptions {
                class_insert_mode: MixinClassInsertMode::Fqn,
                ..Default::default()
            },
            ""
        )[0]
        .insert_text,
        "net.minecraft.client.MinecraftClient.class"
    );
    let strings = context(
        MixinAnnotation::Mixin,
        AnnotationSlot::Targets,
        "net.minecraft.client.Mine",
    );
    assert_eq!(
        service.complete(&strings, &Default::default(), "")[0].insert_text,
        "net.minecraft.client.MinecraftClient"
    );
    assert!(service
        .complete(
            &context(MixinAnnotation::Inject, AnnotationSlot::Class, ""),
            &Default::default(),
            ""
        )
        .is_empty());
    assert!(service
        .complete(
            &context(MixinAnnotation::Mixin, AnnotationSlot::Value, ""),
            &Default::default(),
            ""
        )
        .is_empty());
    for i in 0..60 {
        index.classes.push(ClassIndexEntry {
            simple_name: format!("A{i:02}"),
            package_name: "many".into(),
            internal_name: format!("many/A{i:02}"),
        });
    }
    let service = MixinTargetCompletionService::new(&index);
    assert_eq!(
        service
            .complete(
                &context(MixinAnnotation::Mixin, AnnotationSlot::Class, "A"),
                &Default::default(),
                ""
            )
            .len(),
        CLASS_COMPLETION_LIMIT
    );
    assert_eq!(
        service.complete(
            &context(MixinAnnotation::Mixin, AnnotationSlot::Class, "A59"),
            &Default::default(),
            ""
        )[0]
        .label,
        "A59"
    );
}
#[test]
fn kotlin_target_alias_prefix_class_reference_fqn_and_java_unchanged() {
    let index = Index::default();
    let service = MixinTargetCompletionService::new(&index);
    let source = "import net.minecraft.client.MinecraftClient as PatchTarget\nclass Patch";
    let mut c = context(MixinAnnotation::Mixin, AnnotationSlot::Class, "PatchT");
    c.is_kotlin = true;
    let items = service.complete(&c, &Default::default(), source);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].insert_text, "PatchTarget::class");
    assert!(items[0].filter_text.contains("PatchTarget"));
    c.partial_value = "Mine".into();
    assert_eq!(
        service.complete(&c, &Default::default(), source)[0].insert_text,
        "PatchTarget::class"
    );
    assert_eq!(
        service.complete(
            &c,
            &MixinCompletionOptions {
                class_insert_mode: MixinClassInsertMode::Fqn,
                ..Default::default()
            },
            source
        )[0]
        .insert_text,
        "net.minecraft.client.MinecraftClient::class"
    );
    c.partial_value = "".into();
    assert_eq!(
        service
            .complete(&c, &Default::default(), source)
            .iter()
            .filter(|i| i.metadata.owner.as_deref() == Some(OWNER))
            .count(),
        1
    );
    c.is_kotlin = false;
    c.partial_value = "PatchT".into();
    assert!(service.complete(&c, &Default::default(), source).is_empty());
    c.partial_value = "Mine".into();
    assert_eq!(
        service.complete(&c, &Default::default(), source)[0].insert_text,
        "MinecraftClient.class"
    );
}
fn attrs(annotation: MixinAnnotation, kotlin: bool, source: &str) -> Vec<McCompletionItem> {
    let mut c = context(annotation, AnnotationSlot::Attribute, "");
    c.is_kotlin = kotlin;
    MixinAttributeCompletionService::complete(&c, source)
}
fn attr(annotation: MixinAnnotation, name: &str, kotlin: bool, source: &str) -> String {
    attrs(annotation, kotlin, source)
        .into_iter()
        .find(|i| i.metadata.name.as_deref() == Some(name))
        .unwrap()
        .insert_text
}
#[test]
fn attribute_tables_cover_every_annotation_exactly() {
    use MixinAnnotation::*;
    for(a,names)in[(Mixin,"value targets priority remap"),(Inject,"method at slice require expect allow remap constraints cancellable locals id"),(Redirect,"method at slice require expect allow remap constraints"),(ModifyArg,"method at slice require expect allow remap constraints index"),(ModifyArgs,"method at slice require expect allow remap constraints"),(ModifyVariable,"method at slice require expect allow remap constraints ordinal index argsOnly print"),(ModifyConstant,"method constant slice require expect allow remap constraints"),(ModifyExpressionValue,"method at slice require expect allow remap order"),(ModifyReturnValue,"method at slice require expect allow remap order"),(ModifyReceiver,"method at slice require expect allow remap order"),(WrapWithCondition,"method at slice require expect allow remap order"),(WrapOperation,"method at slice require expect allow remap order constant"),(WrapMethod,"method require expect allow remap order"),(At,"value target slice ordinal opcode shift by args remap id"),(Constant,"nullValue intValue floatValue longValue doubleValue stringValue classValue ordinal slice expandZeroConditions log"),(Slice,"id from to"),(Accessor,"value remap"),(Invoker,"value remap"),(Shadow,"prefix aliases remap"),(Overwrite,"constraints remap"),(Local,"print ordinal index name argsOnly type"),(Share,"value namespace"),(Definition,"id method field type local remap"),(Expression,"value id"),(Definitions,"value"),(Expressions,"value"),(Final,""),(Mutable,""),(Pseudo,""),(SoftOverride,""),(Surrogate,"")]{let items=attrs(a,false,"");assert_eq!(items.iter().map(|i|i.metadata.name.as_ref().unwrap().as_str()).collect::<Vec<_>>(),names.split_whitespace().collect::<Vec<_>>(),"{a:?}");assert!(items.iter().all(|i|i.insert_text_format==McCompletionInsertTextFormat::Snippet));}
}
#[test]
fn attributes_prefix_existing_same_annotation_and_wrap_exclusivity() {
    use MixinAnnotation::*;
    for annotation in [
        Local,
        Share,
        Definition,
        Expression,
        Definitions,
        Expressions,
        Constant,
        Slice,
        Inject,
    ] {
        let mut c = context(annotation, AnnotationSlot::Attribute, "");
        let all = MixinAttributeCompletionService::complete(&c, "");
        if let Some(first) = all.first() {
            let name = first.metadata.name.clone().unwrap();
            c.existing_attributes.insert(name.clone());
            assert!(!MixinAttributeCompletionService::complete(&c, "")
                .iter()
                .any(|i| i.metadata.name.as_ref() == Some(&name)));
        }
        c.existing_attributes.clear();
        c.partial_value = "i".into();
        assert!(MixinAttributeCompletionService::complete(&c, "")
            .iter()
            .all(|i| i.metadata.name.as_ref().unwrap().starts_with('i')));
    }
    for (existing, absent) in [("at", "constant"), ("constant", "at")] {
        let mut c = context(WrapOperation, AnnotationSlot::Attribute, "");
        c.existing_attributes.insert(existing.into());
        assert!(!MixinAttributeCompletionService::complete(&c, "")
            .iter()
            .any(|i| i.metadata.name.as_deref() == Some(absent)));
    }
    let mut c = context(WrapWithCondition, AnnotationSlot::Attribute, "");
    c.resolved_annotation_fqn = Some("com.llamalad7.mixinextras.injector.WrapWithCondition".into());
    assert!(!MixinAttributeCompletionService::complete(&c, "")
        .iter()
        .any(|i| i.metadata.name.as_deref() == Some("order")));
    c.resolved_annotation_fqn =
        Some("com.llamalad7.mixinextras.injector.v2.WrapWithCondition".into());
    assert!(MixinAttributeCompletionService::complete(&c, "")
        .iter()
        .any(|i| i.metadata.name.as_deref() == Some("order")));
}
#[test]
fn java_attribute_snippets_exact() {
    use MixinAnnotation::*;
    for (a, n, want) in [
        (Inject, "method", "method = \"${1}\"$0"),
        (Inject, "at", "at = @At(\"${1}\")$0"),
        (Local, "print", "print = ${1|true,false|}$0"),
        (Local, "name", "name = { \"${1}\" }$0"),
        (Local, "type", "type = ${1}.class$0"),
        (Definition, "method", "method = { \"${1}\" }$0"),
        (Definition, "field", "field = { \"${1}\" }$0"),
        (Definition, "type", "type = { ${1}.class }$0"),
        (Definition, "local", "local = { @Local(${1}) }$0"),
        (
            Definitions,
            "value",
            "value = { @Definition(id = \"${1}\") }$0",
        ),
        (Expressions, "value", "value = { @Expression(\"${1}\") }$0"),
    ] {
        assert_eq!(attr(a, n, false, ""), want);
    }
}
#[test]
fn kotlin_attributes_arrays_constructors_fqns_aliases_and_lapis() {
    use MixinAnnotation::*;
    let source="import org.spongepowered.asm.mixin.injection.At as Point\nimport org.spongepowered.asm.mixin.injection.Slice as Region\nimport org.spongepowered.asm.mixin.injection.At.Shift as ShiftKind\nimport org.spongepowered.asm.mixin.injection.Constant as Literal\nimport org.spongepowered.asm.mixin.injection.callback.LocalCapture as CaptureMode\nimport com.llamalad7.mixinextras.sugar.Local as Variable\nimport com.llamalad7.mixinextras.expression.Definition as Binding\nimport com.llamalad7.mixinextras.expression.Expression as Expr\nclass Patch";
    for (a, n, want) in [
        (Inject, "method", "method = [ \"${1}\" ]$0"),
        (Inject, "at", "at = [ Point(\"${1}\") ]$0"),
        (Inject, "slice", "slice = [ Region(${1}) ]$0"),
        (Inject, "locals", "locals = CaptureMode.${1}$0"),
        (WrapOperation, "constant", "constant = [ Literal(${1}) ]$0"),
        (Slice, "from", "from = Point(${1})$0"),
        (Definition, "local", "local = [ Variable(${1}) ]$0"),
        (Definitions, "value", "value = [ Binding(id = \"${1}\") ]$0"),
        (Expressions, "value", "value = [ Expr(\"${1}\") ]$0"),
        (At, "shift", "shift = ShiftKind.${1}$0"),
    ] {
        assert_eq!(attr(a, n, true, source), want);
    }
    assert_eq!(
        attr(
            Inject,
            "at",
            true,
            "package example\nimport other.At\nclass Patch"
        ),
        "at = [ org.spongepowered.asm.mixin.injection.At(\"${1}\") ]$0"
    );
    let mut c = context(Mixin, AnnotationSlot::Attribute, "");
    c.is_kotlin = true;
    c.resolved_annotation_fqn = Some("io.github.diskria.lapis.annotations.KMixin".into());
    let lapis = MixinAttributeCompletionService::complete(&c, "");
    assert_eq!(
        lapis
            .iter()
            .map(|i| i.metadata.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["target", "env", "initStrategy"]
    );
    assert_eq!(lapis[0].insert_text, "target = ${1}::class$0");
    c.annotation = Shadow;
    c.resolved_annotation_fqn = Some("io.github.diskria.lapis.annotations.KShadow".into());
    assert_eq!(
        MixinAttributeCompletionService::complete(&c, "")[0].insert_text,
        "modifiers = [ ${1} ]$0"
    );
}
#[test]
fn default_options_and_attribute_metadata() {
    let options = MixinCompletionOptions::default();
    assert_eq!(options.preferred_at_target, "smart");
    let item = &attrs(MixinAnnotation::Inject, false, "")[0];
    assert_eq!(item.metadata.source, "mixin.attribute");
    assert_eq!(item.detail.as_deref(), Some("@Inject attribute"));
    assert_eq!(item.sort_key, "0000_00_method");
    assert_eq!(item.filter_text, "method");
    assert!(item.documentation.as_deref().unwrap().contains("method"));
    let _set: BTreeSet<String> = BTreeSet::new();
}

#[test]
fn attribute_filtering_returns_each_exact_requested_name_and_keeps_unset_names() {
    for annotation in MixinAnnotation::ALL {
        let all = attrs(annotation, false, "");
        for item in &all {
            let name = item.metadata.name.as_deref().unwrap();
            let context = context(annotation, AnnotationSlot::Attribute, name);
            let selected = MixinAttributeCompletionService::complete(&context, "");
            assert!(
                selected
                    .iter()
                    .any(|i| i.metadata.name.as_deref() == Some(name)),
                "{annotation:?}.{name}"
            );
            let mut excluded = context.clone();
            excluded.partial_value.clear();
            excluded.existing_attributes.insert(name.into());
            let remaining = MixinAttributeCompletionService::complete(&excluded, "");
            let removed = if annotation == MixinAnnotation::WrapOperation
                && matches!(name, "at" | "constant")
            {
                2
            } else {
                1
            };
            assert_eq!(
                remaining.len(),
                all.len() - removed,
                "{annotation:?}.{name}"
            );
        }
    }
}

#[test]
fn unimported_kotlin_nested_snippets_use_fqns_without_required_edits() {
    use MixinAnnotation::*;
    let source = "package example\nimport other.At\nclass Patch";
    for (annotation, name, expected) in [
        (
            Inject,
            "at",
            "at = [ org.spongepowered.asm.mixin.injection.At(\"${1}\") ]$0",
        ),
        (
            Inject,
            "slice",
            "slice = [ org.spongepowered.asm.mixin.injection.Slice(${1}) ]$0",
        ),
        (
            Inject,
            "locals",
            "locals = org.spongepowered.asm.mixin.injection.callback.LocalCapture.${1}$0",
        ),
        (
            WrapOperation,
            "constant",
            "constant = [ org.spongepowered.asm.mixin.injection.Constant(${1}) ]$0",
        ),
        (
            Slice,
            "from",
            "from = org.spongepowered.asm.mixin.injection.At(${1})$0",
        ),
        (
            Definition,
            "local",
            "local = [ com.llamalad7.mixinextras.sugar.Local(${1}) ]$0",
        ),
        (
            Definitions,
            "value",
            "value = [ com.llamalad7.mixinextras.expression.Definition(id = \"${1}\") ]$0",
        ),
        (
            Expressions,
            "value",
            "value = [ com.llamalad7.mixinextras.expression.Expression(\"${1}\") ]$0",
        ),
        (
            At,
            "shift",
            "shift = org.spongepowered.asm.mixin.injection.At.Shift.${1}$0",
        ),
    ] {
        assert_eq!(attr(annotation, name, true, source), expected);
    }
}

#[test]
fn extracted_kotlin_alias_completion_replaces_the_whole_class_literal() {
    use mcdev_core::mixin_source::AnnotationContextExtractor;
    let marked="import net.minecraft.client.MinecraftClient as PatchTarget\nimport io.github.diskria.lapis.annotations.KMixin\n@KMixin(PatchT<caret>::class) class Patch";
    let offset = marked.find("<caret>").unwrap();
    let source = marked.replace("<caret>", "");
    let context = AnnotationContextExtractor::extract_at_offset(&source, offset).unwrap();
    let index = Index::default();
    let items =
        MixinTargetCompletionService::new(&index).complete(&context, &Default::default(), &source);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].insert_text, "PatchTarget::class");
    let edited = apply_text_edits(
        &source,
        &[McTextEdit {
            start_offset: context.value_start_offset,
            end_offset: context.value_end_offset,
            new_text: items[0].insert_text.clone(),
        }],
    )
    .unwrap();
    assert_eq!(
        edited,
        source.replace("PatchT::class", "PatchTarget::class")
    );
}
