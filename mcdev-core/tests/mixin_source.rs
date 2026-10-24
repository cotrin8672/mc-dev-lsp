use mcdev_core::mixin_source::*;
use mcdev_core::*;

#[derive(Default)]
struct Index(Vec<&'static str>);
impl ClassIndex for Index {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.0
            .iter()
            .filter(|n| n.rsplit('/').next().unwrap().starts_with(prefix))
            .take(limit)
            .map(|n| entry(n))
            .collect()
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        self.0.iter().find(|v| **v == n).map(|n| entry(n))
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        self.0
            .iter()
            .find(|v| v.replace('/', ".").replace('$', ".") == n)
            .map(|n| entry(n))
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        vec![]
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
fn entry(n: &str) -> ClassIndexEntry {
    let (p, c) = n.rsplit_once('/').unwrap_or(("", n));
    ClassIndexEntry {
        simple_name: c.rsplit('$').next().unwrap().into(),
        package_name: p.replace('/', "."),
        internal_name: n.into(),
    }
}
fn kotlin(body: &str) -> String {
    format!("import io.github.diskria.lapis.annotations.KMixin\nimport org.spongepowered.asm.mixin.injection.*\n@KMixin(Target::class)\nabstract class Patch {{\n{body}\n}}")
}
fn context(source: &str, needle: &str, chars: usize) -> AnnotationContext {
    AnnotationContextExtractor::extract_at_offset(source, source.find(needle).unwrap() + chars)
        .unwrap()
}

#[test]
fn imports_aliases_ambiguity_crlf_and_comments() {
    let src="// import bad.Fake\r\npackage patches\r\nimport game.Target as Game\r\nimport first.Duplicate\r\nimport second.Duplicate\r\nimport game.*\r\nclass Patch";
    let i = JavaTypeDescriptorResolver::imports_for(src);
    assert!(i.is_kotlin);
    assert_eq!(i.package_name.as_deref(), Some("patches"));
    assert_eq!(
        i.explicit.get("Game").map(String::as_str),
        Some("game.Target")
    );
    assert_eq!(i.wildcard_packages, vec!["game"]);
    assert_eq!(
        i.ambiguous_explicit["Duplicate"],
        vec!["first.Duplicate", "second.Duplicate"]
    );
    assert!(!i.explicit.contains_key("Fake"));
}
#[test]
fn primitive_nullable_unit_and_alias_descriptors() {
    let i = JavaTypeDescriptorResolver::imports_for("import kotlin.Int as Count\n");
    let c = JavaTypeResolutionContext {
        imports: &i,
        lookup: None,
    };
    for (raw, expected) in [
        ("Int", "I"),
        ("Int?", "Ljava/lang/Integer;"),
        ("Count", "I"),
        ("Boolean?", "Ljava/lang/Boolean;"),
        ("Unit", "Lkotlin/Unit;"),
        ("String?", "Ljava/lang/String;"),
        ("Array<Int>", "[Ljava/lang/Integer;"),
        ("Array<out Long>", "[Ljava/lang/Long;"),
        ("Array<*>", "[Ljava/lang/Object;"),
        ("IntArray", "[I"),
        ("vararg Int", "[I"),
        ("Array<IntArray>", "[[I"),
        ("MutableMap<String,Int>", "Ljava/util/Map;"),
        ("Map.Entry<String,Int>", "Ljava/util/Map$Entry;"),
    ] {
        assert_eq!(
            JavaTypeDescriptorResolver::descriptor_or_null(raw, &c).as_deref(),
            Some(expected),
            "{raw}"
        );
    }
    assert_eq!(
        JavaTypeDescriptorResolver::method_descriptor_or_null("Unit", &["Int".into()], &c)
            .as_deref(),
        Some("(I)V")
    );
    assert_eq!(
        JavaTypeDescriptorResolver::method_descriptor_or_null("Unit?", &[], &c).as_deref(),
        Some("()Lkotlin/Unit;")
    );
}
#[test]
fn no_guessed_object_and_host_lookup_precedence() {
    let index = Index(vec![
        "patches/Own",
        "a/Thing",
        "b/Thing",
        "custom/Int",
        "pkg/Outer$Inner",
    ]);
    let lookup = ClassIndexJavaTypeLookup {
        class_index: &index,
    };
    let i = JavaTypeDescriptorResolver::imports_for(
        "package patches\nimport a.*\nimport b.*\nimport pkg.Outer as OuterAlias",
    );
    let c = JavaTypeResolutionContext {
        imports: &i,
        lookup: Some(&lookup),
    };
    assert_eq!(
        JavaTypeDescriptorResolver::descriptor_or_null("Own", &c).as_deref(),
        Some("Lpatches/Own;")
    );
    assert!(
        matches!(JavaTypeDescriptorResolver::descriptor_or_diagnostic("Thing",&c),TypeDescriptorResult::Ambiguous{candidates,..}if candidates.len()==2)
    );
    assert!(JavaTypeDescriptorResolver::descriptor_or_null("Missing", &c).is_none());
    assert_eq!(
        JavaTypeDescriptorResolver::descriptor_or_null("OuterAlias.Inner", &c).as_deref(),
        Some("Lpkg/Outer$Inner;")
    );
    let i = JavaTypeDescriptorResolver::imports_for("package custom\n");
    assert_eq!(
        JavaTypeDescriptorResolver::descriptor_or_null(
            "Int",
            &JavaTypeResolutionContext {
                imports: &i,
                lookup: Some(&lookup)
            }
        )
        .as_deref(),
        Some("Lcustom/Int;")
    );
}
#[test]
fn java_descriptors_arrays_generics_inner_and_vararg() {
    let i = JavaTypeDescriptorResolver::imports_for(
        "package x;\nimport java.util.List;\nimport pkg.Outer.Inner;",
    );
    assert!(!i.is_kotlin);
    let c = JavaTypeResolutionContext {
        imports: &i,
        lookup: None,
    };
    for (raw, expected) in [
        ("int", "I"),
        ("java.lang.String[][]", "[[Ljava/lang/String;"),
        ("List<String>", "Ljava/util/List;"),
        ("Inner...", "[Lpkg/Outer$Inner;"),
        ("@Nullable final String[]", "[Ljava/lang/String;"),
    ] {
        assert_eq!(
            JavaTypeDescriptorResolver::descriptor_or_null(raw, &c).as_deref(),
            Some(expected)
        );
    }
    assert!(JavaTypeDescriptorResolver::descriptor_or_null("Unknown", &c).is_none());
}
#[test]
fn lapis_identity_named_target_and_alias() {
    for header in [
        "import io.github.diskria.lapis.annotations.KMixin\n@KMixin",
        "import io.github.diskria.lapis.annotations.KMixin as Patch\n@Patch",
        "@io.github.diskria.lapis.annotations.KMixin",
    ] {
        let s = format!("{header}(target = net.game.Target::class)");
        let c = context(&s, "net.game.Target", 12);
        assert_eq!(c.annotation, MixinAnnotation::Mixin);
        assert_eq!(c.slot, AnnotationSlot::Class);
        assert_eq!(c.partial_value, "Tar");
        assert_eq!(c.value_start_offset, s.find("net.game.Target").unwrap());
        assert_eq!(
            c.resolved_annotation_fqn.as_deref(),
            Some("io.github.diskria.lapis.annotations.KMixin")
        );
    }
}
#[test]
fn unrelated_and_ambiguous_annotations_rejected() {
    for s in ["import example.KMixin\n@KMixin(Target::class)","import example.KMixin\nimport io.github.diskria.lapis.annotations.KMixin\n@KMixin(Target::class)","@example.KMixin(Target::class)","@KMixin(Target::class)","import example.Inject;\n@Inject(method=\"tick\")"]{assert!(AnnotationContextExtractor::extract_at_offset(s,s.find("Target").or_else(||s.find("tick")).unwrap()+2).is_none(),"{s}");}
}
#[test]
fn kotlin_active_array_string_and_nested_at_parent() {
    let s=kotlin("@Inject(method = [\"first\", /* between */ \"second\"], at = [At(\"HEAD\"), At(value=\"INVOKE\",target=\"Lgame/Target;call()V\")]) fun handler() {}");
    let c = context(&s, "second", 3);
    assert_eq!(c.slot, AnnotationSlot::Method);
    assert_eq!(c.partial_value, "sec");
    assert_eq!(c.value_end_offset, s.find("second").unwrap() + 6);
    assert_eq!(c.at_value.as_deref(), Some("HEAD"));
    assert_eq!(c.mixin_target_internal_names, vec!["Target"]);
    let c = context(&s, "Lgame/Target", 5);
    assert_eq!(c.annotation, MixinAnnotation::At);
    assert_eq!(c.slot, AnnotationSlot::Target);
    assert_eq!(c.at_value.as_deref(), Some("INVOKE"));
    assert_eq!(c.inject_method_name.as_deref(), Some("first"));
    assert_eq!(c.parent_injector_annotation, Some(MixinAnnotation::Inject));
}
#[test]
fn at_slice_alias_and_unknown_nested_identity() {
    let s = kotlin("@Inject(method=[\"tick\"],at=[At(\"HEAD\")],slice=[Slice(from=At(\"TAIL\"))])");
    let c = context(&s, "TAIL", 2);
    assert!(c.at_inside_slice);
    assert_eq!(c.inject_method_name.as_deref(), Some("tick"));
    assert!(!context(&s, "HEAD", 2).at_inside_slice);
    let s = s.replace(
        "import org.spongepowered.asm.mixin.injection.*",
        "import org.spongepowered.asm.mixin.injection.*\nimport example.At",
    );
    assert!(
        AnnotationContextExtractor::extract_at_offset(&s, s.find("TAIL").unwrap() + 2).is_none()
    );
}
#[test]
fn kotlin_raw_strings_comments_and_non_code_gating() {
    let s=kotlin("val fake = \"\"\" @Inject(method=[\"fake\"], at=[At(\"BAD\")]) class Wrong {} \"\"\"\n/* outer /* nested */ @Inject(method=[\"comment\"]) */\n// @Inject(method=[\"line\"])\n@Inject(method=[\"real\"],at=[At(\"HEAD\")])");
    assert_eq!(
        AnnotationContextExtractor::find_injector_annotation_offsets(&s),
        vec![s.find("@Inject(method=[\"real").unwrap()]
    );
    assert!(
        AnnotationContextExtractor::extract_at_offset(&s, s.find("fake\"").unwrap() + 2).is_none()
    );
    assert_eq!(
        AnnotationContextExtractor::find_annotation_offsets(&s, MixinAnnotation::At),
        vec![s.find("At(\"HEAD").unwrap()]
    );
}
#[test]
fn java_comments_end_at_first_close() {
    let s="/* opener /* */ @Mixin /* opener /* */ (Target.class) class Patch { @Inject /* opener /* */ (method=\"tick\",at=@At(\"HEAD\")) void handler() {} }";
    assert_eq!(context(s, "tick", 4).partial_value, "tick");
    assert_eq!(
        AnnotationContextExtractor::find_injector_annotation_offsets(s),
        vec![s.find("@Inject").unwrap()]
    );
}
#[test]
fn class_scope_companion_nested_mixin_and_other_class() {
    let s="package patches\nimport io.github.diskria.lapis.annotations.KMixin\n@KMixin(First::class) class FirstPatch { companion object { fun staticTick() {} } class Helper { fun helper() {} } @KMixin(Inner::class) class InnerPatch {fun inner() {}} } @KMixin(Second::class) class SecondPatch {fun second() {}}";
    for (needle, name, target) in [
        ("staticTick", "patches.FirstPatch", "First"),
        ("fun inner", "patches.FirstPatch$InnerPatch", "Inner"),
        ("fun second", "patches.SecondPatch", "Second"),
    ] {
        assert_eq!(
            AnnotationContextExtractor::resolve_mixin_class_scope(s, s.find(needle).unwrap()),
            Some(MixinClassScope {
                class_name: name.into(),
                raw_targets: vec![target.into()]
            })
        );
    }
    assert!(AnnotationContextExtractor::resolve_mixin_class_scope(
        s,
        s.find("fun helper").unwrap()
    )
    .is_none());
}
#[test]
fn raw_target_ranges_crlf_utf16_and_edits() {
    let s=kotlin("// 🐈\r\n@Inject(method=[\"tick\"],at=[At(\"INVOKE\",target=\"\"\"Lgame/Target;call()V\"\"\")])");
    let at = s.find("Lgame/Target").unwrap();
    let c = AnnotationContextExtractor::extract_at_offset(&s, at + 14).unwrap();
    assert_eq!(c.value_start_offset, at);
    assert_eq!(c.value_end_offset, at + 20);
    let p = offset_to_position(&s, at + 14).unwrap();
    assert_eq!(
        Some(c.clone()),
        AnnotationContextExtractor::extract(&s, p.line, p.character)
    );
    assert_eq!(
        apply_text_edits(
            &s,
            &[McTextEdit {
                start_offset: c.value_start_offset,
                end_offset: c.value_end_offset,
                new_text: "NEW".into()
            }]
        )
        .unwrap(),
        s.replace("Lgame/Target;call()V", "NEW")
    );
}
#[test]
fn quoted_values_preserve_quotes_and_replace_whole_value() {
    for (a, slot) in [
        ("Inject(method", AnnotationSlot::Method),
        ("At(target", AnnotationSlot::Target),
        ("Accessor(value", AnnotationSlot::AccessorValue),
        ("Invoker(value", AnnotationSlot::InvokerValue),
        ("Shadow(prefix", AnnotationSlot::Prefix),
    ] {
        let s = format!("@{a}=\"complete\")");
        let c = context(&s, "complete", 3);
        assert_eq!(c.slot, slot);
        assert_eq!(&s[c.value_start_offset..c.value_end_offset], "complete");
    }
}
#[test]
fn decoded_escapes_and_offsets() {
    let s = "a\\141\\t\\u0042🐈";
    let d = JavaStringContentDecoder::decode_with_offsets(s, 0, s.len(), false, false).unwrap();
    assert_eq!(d.value, "aa\tB🐈");
    assert_eq!(d.source_offsets, vec![0, 1, 5, 7, 13, 13, 13, 13, 17]);
    assert!(JavaStringContentDecoder::decode_content("\\x", 0, 2).is_none());
    assert!(JavaStringContentDecoder::decode_with_offsets("$value", 0, 6, true, true).is_none());
    assert_eq!(
        JavaStringContentDecoder::decode_with_offsets("\\$value", 0, 7, false, true)
            .unwrap()
            .value,
        "$value"
    );
}
#[test]
fn targets_imports_and_ambiguity_never_arbitrary() {
    let index = Index(vec!["a/Target", "b/Target", "a/Outer$Inner", "patches/Own"]);
    assert!(MixinTargetResolver::resolve_target("Target", &index, None).is_none());
    assert_eq!(
        MixinTargetResolver::resolve_target("a.Target", &index, None).as_deref(),
        Some("a/Target")
    );
    for src in ["import a.Target\n", "import a.*\n"] {
        let i = JavaTypeDescriptorResolver::imports_for(src);
        assert_eq!(
            MixinTargetResolver::resolve_target("Target", &index, Some(&i)).as_deref(),
            Some("a/Target")
        );
    }
    let i = JavaTypeDescriptorResolver::imports_for("import a.*\nimport b.*\n");
    assert!(MixinTargetResolver::resolve_target("Target", &index, Some(&i)).is_none());
    let i = JavaTypeDescriptorResolver::imports_for("package patches\n");
    assert_eq!(
        MixinTargetResolver::resolve_target("Own", &index, Some(&i)).as_deref(),
        Some("patches/Own")
    );
}
#[test]
fn java_member_parser_multiline_annotated_generic_and_varargs() {
    let s="import org.spongepowered.asm.mixin.gen.Invoker;\nimport java.util.List;\nclass Patch { @Invoker(\"tick\") public abstract\nvoid invokeTick(@Nullable final String text, int... values); @Accessor abstract List<String> getValues(); @Shadow private java.lang.String value; @Overwrite public static int tick(int value) {return value;} }";
    let m = MixinSemanticModelParser::parse(s, "file:///Patch.java");
    assert_eq!(
        m.members
            .iter()
            .map(|m| m.java_name.as_str())
            .collect::<Vec<_>>(),
        vec!["value", "getValues", "invokeTick", "tick"]
    );
    assert_eq!(
        m.members[2].method_descriptor.as_deref(),
        Some("(Ljava/lang/String;[I)V")
    );
    assert_eq!(
        m.members[1].method_descriptor.as_deref(),
        Some("()Ljava/util/List;")
    );
    assert_eq!(
        m.members[0].return_descriptor.as_deref(),
        Some("Ljava/lang/String;")
    );
    assert!(m.members[3].modifiers.contains(&JavaModifier::Static));
}
#[test]
fn kotlin_mapping_shadow_property_and_modifiers() {
    let s="import io.github.diskria.lapis.annotations.*\nimport javax.lang.model.element.Modifier.PRIVATE as Hidden\nimport javax.lang.model.element.Modifier.STATIC as ClassMember\n@KMixin(game.Target::class) abstract class Patch { @KShadow(Hidden) @MappingName(name=\"count\") abstract var observed: Int\n@MappingName(\"secret\") @KShadow(Hidden) abstract fun hidden(argument: Int): Int\n@KShadow(ClassMember) @MappingName(\"manualTotal\") abstract val manualView: Long\ncompanion object { @KShadow @MappingName(\"total\") val totalView: Long = 0 } }";
    let m = MixinSemanticModelParser::parse(s, "file:///Patch.kt");
    assert_eq!(m.members.len(), 4);
    for (member, name, descriptor) in [
        (&m.members[0], "count", "I"),
        (&m.members[1], "secret", "I"),
        (&m.members[2], "manualTotal", "J"),
        (&m.members[3], "total", "J"),
    ] {
        assert_eq!(member.explicit_target_name.as_deref(), Some(name));
        assert_eq!(member.return_descriptor.as_deref(), Some(descriptor));
    }
    assert!(m.members[1].modifiers.contains(&JavaModifier::Private));
    assert!(m.members[2].modifiers.contains(&JavaModifier::Static));
    assert!(m.members[3].modifiers.contains(&JavaModifier::Static));
}
#[test]
fn kotlin_standard_unit_receiver_and_unresolved_return() {
    let s=kotlin("@org.spongepowered.asm.mixin.gen.Accessor(\"count\") abstract fun setCount(value: Int): Unit\n@org.spongepowered.asm.mixin.gen.Invoker(\"tick\") abstract fun invokeTick()\n@org.spongepowered.asm.mixin.Overwrite fun Target.twice(value: Int): Int {return value*2}\n@org.spongepowered.asm.mixin.Shadow val unknown = expression()\n@org.spongepowered.asm.mixin.Shadow fun unresolved() = expression()\n");
    let m = MixinSemanticModelParser::parse(&s, "file:///Patch.kt");
    assert_eq!(m.members[0].method_descriptor.as_deref(), Some("(I)V"));
    assert_eq!(m.members[1].method_descriptor.as_deref(), Some("()V"));
    assert_eq!(m.members[4].method_descriptor.as_deref(), Some("(I)I"));
    assert!(m.members[2].return_descriptor.is_none());
    assert!(m.members[3].method_descriptor.is_none());
    assert!(!m.members[3].warnings.is_empty());
}
#[test]
fn semantic_model_selector_values_ranges_and_metadata() {
    let s=kotlin("@Inject(method=[\"tick\",\"tock(I)V\"],at=[At(\"HEAD\"),At(target=\"Lgame/Target;twice(I)I\",value=\"INVOKE\",ordinal=2)]) fun handler() {}");
    let m = MixinSemanticModelParser::parse(&s, "file:///Patch.kt");
    assert_eq!(m.parse_source, ParseSource::HandWrittenFallback);
    assert_eq!(m.confidence, ParseConfidence::Medium);
    assert_eq!(m.injectors.len(), 1);
    let i = &m.injectors[0];
    assert_eq!(
        i.method_selectors
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["tick", "tock"]
    );
    assert_eq!(i.method_selectors[1].descriptor.as_deref(), Some("(I)V"));
    assert_eq!(i.at_selectors[1].ordinal, Some(2));
    let start = s.find("Lgame/Target").unwrap();
    assert_eq!(
        i.at_selectors[1].target_range,
        Some(source_range(&s, start, start + 22))
    );
}
#[test]
fn all_injectors_and_official_annotation_namespaces() {
    for a in MixinAnnotation::ALL {
        for fqn in a.official_fqns() {
            assert_eq!(MixinAnnotation::from_official_fqn(&fqn), Some(a));
            if a.is_injector() {
                let s = format!("@{fqn}(method=\"tick\",at=@At(\"HEAD\"))");
                let c = context(&s, "tick", 2);
                assert_eq!(c.slot, AnnotationSlot::Method);
                assert_eq!(c.annotation, a);
            }
        }
    }
}
#[test]
fn attribute_prefix_schemas_and_existing_attributes() {
    for (annotation, prefix) in [
        ("Mixin", "rem"),
        ("Inject", "meth"),
        ("Local", "args"),
        ("Share", "name"),
        ("Definition", "fie"),
        ("Expression", "va"),
        ("Definitions", "v"),
        ("Expressions", "v"),
        ("Constant", "intV"),
        ("Slice", "fr"),
    ] {
        let s = format!("@{annotation}({prefix})");
        let c = AnnotationContextExtractor::extract_at_offset(&s, s.find(')').unwrap()).unwrap();
        assert_eq!(c.slot, AnnotationSlot::Attribute, "{annotation}");
        assert_eq!(c.partial_value, prefix);
    }
    for a in ["Local", "Share", "Definition", "Expression"] {
        let s = format!("@{a}(garbage)");
        assert!(AnnotationContextExtractor::extract_at_offset(&s, s.find(')').unwrap()).is_none());
    }
    let s = kotlin("@Inject(method=[\"tick\",\"tock\"],at=[At(\"HEAD\")],rem)");
    let c = context(&s, "rem)", 3);
    assert_eq!(
        c.existing_attributes.into_iter().collect::<Vec<_>>(),
        vec!["at", "method"]
    );
    let s = "@Local(ordinal=)";
    let c = AnnotationContextExtractor::extract_at_offset(s, s.find(')').unwrap()).unwrap();
    assert_eq!(c.slot, AnnotationSlot::Value);
    assert_eq!(c.partial_value, "");
}
#[test]
fn definition_values_keep_attribute_and_shorthand_identity() {
    for name in ["method", "field", "type", "local"] {
        let s = format!("@Definition(id=\"id\",{name}=\"foo\")");
        let c = context(&s, "foo", 2);
        assert_eq!(c.slot, AnnotationSlot::Value);
        assert_eq!(c.attribute_name.as_deref(), Some(name));
    }
    let c = context("@Definition(\"foo\")", "foo", 2);
    assert!(c.attribute_name.is_none());
}
#[test]
fn expression_token_replacement_punctuation_and_decoded_prefix() {
    for (body, needle, cursor, prefix, position) in [
        (
            "a + methodCall()",
            "methodCall",
            3,
            "a + met",
            ExpressionCompletionPosition::ValueStart,
        ),
        (
            "receiver.",
            "receiver.",
            9,
            "receiver.",
            ExpressionCompletionPosition::AfterDot,
        ),
        (
            "Thing::",
            "Thing::",
            7,
            "Thing::",
            ExpressionCompletionPosition::AfterMethodReference,
        ),
        (
            "{ meth",
            "meth",
            4,
            "{ meth",
            ExpressionCompletionPosition::StatementStart,
        ),
    ] {
        let s = format!("@Expression(\"{body}\")");
        let c = context(&s, needle, cursor);
        assert_eq!(
            c.decoded_expression_prefix
                .as_ref()
                .map(|d| d.prefix.as_str()),
            Some(prefix)
        );
        assert_eq!(c.expression_completion_position, Some(position));
        if position == ExpressionCompletionPosition::AfterDot
            || position == ExpressionCompletionPosition::AfterMethodReference
        {
            assert_eq!(c.value_start_offset, c.value_end_offset);
        } else {
            assert_eq!(&s[c.value_start_offset..c.value_end_offset], needle);
        }
    }
    let s = "@Expression(id=\"wholeValue\")";
    let c = context(s, "wholeValue", 3);
    assert_eq!(&s[c.value_start_offset..c.value_end_offset], "wholeValue");
    assert!(c.decoded_expression_prefix.is_none());
}
#[test]
fn expression_unicode_octal_escapes_preserve_raw_token_range() {
    for raw in ["\\155ethod", "\\u006dethod", "me\\164hod"] {
        let s = format!("@Expression(value=\"obj.{raw}()\")");
        let at = s.find(raw).unwrap();
        let c = AnnotationContextExtractor::extract_at_offset(&s, at + raw.len()).unwrap();
        assert_eq!(&s[c.value_start_offset..c.value_end_offset], raw);
        assert_eq!(c.decoded_partial_value.as_deref(), Some("method"));
        assert_eq!(
            c.decoded_expression_prefix.as_ref().unwrap().prefix,
            "obj.method"
        );
    }
    for extra in [0, 1, 2] {
        let s = "@Expression(\"\\155ethod\")";
        let at = s.find("\\155").unwrap();
        let c = AnnotationContextExtractor::extract_at_offset(s, at + extra).unwrap();
        assert_eq!(c.decoded_partial_value.as_deref(), Some("method"));
        assert_eq!(c.value_start_offset, at);
    }
}
#[test]
fn at_target_concatenation_replaces_entire_expression() {
    for newline in ["\n", "\r\n"] {
        let s=format!("@Inject(method=\"tick\",at=@At(value=\"INVOKE\",target=\"Lgame/Target;\" + //split{newline} \"call(I)I\"))");
        let at = s.find("call(I)").unwrap();
        let c = AnnotationContextExtractor::extract_at_offset(&s, at + 2).unwrap();
        assert_eq!(c.partial_value, "ca");
        assert_eq!(
            &s[c.value_start_offset..c.value_end_offset],
            format!("Lgame/Target;\" + //split{newline} \"call(I)I")
        );
    }
}
#[test]
fn expression_unicode_phase_one_precedes_octal_and_preserves_raw_ranges() {
    let source = r#"@Expression("\u005c155ethod")"#;
    let at = source.find("\\u005c").unwrap();
    let context = AnnotationContextExtractor::extract_at_offset(source, at + 2).unwrap();
    assert_eq!(context.decoded_partial_value.as_deref(), Some("method"));
    assert_eq!(context.decoded_expression_prefix.unwrap().prefix, "m");
    assert_eq!(
        &source[context.value_start_offset..context.value_end_offset],
        "\\u005c155ethod"
    );
    let source = r#"@Expression("abc\qrest")"#;
    let context =
        AnnotationContextExtractor::extract_at_offset(source, source.find("rest").unwrap())
            .unwrap();
    assert_eq!(context.decoded_partial_value.as_deref(), Some("abc"));
    assert_eq!(
        &source[context.value_start_offset..context.value_end_offset],
        "abc"
    );
    assert!(JavaStringContentDecoder::decode_content("\\u0061", 0, 6).is_none());
    let mut value = String::new();
    assert_eq!(
        JavaStringContentDecoder::append_decoded_char("\\141", 0, 4, &mut value),
        Some(4)
    );
    assert_eq!(value, "a");
}
#[test]
fn java_expression_units_keep_isolated_surrogates_and_raw_byte_ranges_losslessly() {
    let decoded = JavaStringContentDecoder::decode_unicode_escape_utf16("\\uD800", 0, 6).unwrap();
    assert_eq!(decoded.units, vec![0xd800]);
    assert_eq!(decoded.next_index, 6);
    assert!(JavaStringContentDecoder::decode_unicode_escape("\\uD800", 0, 6).is_none());
    for (source, units, offsets) in [
        ("\\uD800", vec![0xd800], vec![0, 6]),
        ("\\uD83D\\uDE00", vec![0xd83d, 0xde00], vec![0, 6, 12]),
        ("😀", vec![0xd83d, 0xde00], vec![0, 0, 4]),
    ] {
        let mapped = JavaStringContentDecoder::decode_expression_content_utf16(
            source,
            0,
            source.len(),
            false,
            false,
        )
        .unwrap();
        assert_eq!(mapped.value, units);
        assert_eq!(mapped.source_offsets, offsets);
    }
}
#[test]
fn first_mixin_in_same_header_remains_scope_authority() {
    let source="@Mixin(com.example.target.SimpleTarget.class)\n@Mixin(SimpleTarget.class)\nabstract class ExampleMixin { @Inject(method=\"run\",at=@At(\"HEAD\")) void handler(){} }";
    let scope = AnnotationContextExtractor::resolve_mixin_class_scope(
        source,
        source.find("handler").unwrap(),
    )
    .unwrap();
    assert_eq!(scope.raw_targets, vec!["com/example/target/SimpleTarget"]);
}
#[test]
fn unfinished_string_replacement_stops_at_caret_before_following_line() {
    let source = "@Mixin(Foo.class)\nclass Patch {\n    @Inject(method = \"\n}";
    let caret = source.find("method = \"").unwrap() + "method = \"".len();
    let context = AnnotationContextExtractor::extract_at_offset(source, caret).unwrap();
    assert_eq!(context.value_start_offset, caret);
    assert_eq!(context.value_end_offset, caret);
    assert_eq!(context.partial_value, "");
}
#[test]
fn incomplete_strings_and_attribute_at_eof() {
    let s = "@Expression(\"obj.meth";
    let c = AnnotationContextExtractor::extract_at_offset(s, s.len()).unwrap();
    assert_eq!(c.partial_value, "meth");
    assert_eq!(c.decoded_expression_prefix.unwrap().prefix, "obj.meth");
    let s = "@Inject(method=\"tick\", rem";
    let c = AnnotationContextExtractor::extract_at_offset(s, s.len()).unwrap();
    assert_eq!(c.slot, AnnotationSlot::Attribute);
    assert_eq!(c.partial_value, "rem");
}
#[test]
fn shadow_name_completion_kind_and_configured_ranges() {
    for s in [
        "@Shadow private int coun;",
        "@Shadow(prefix=\"shadow$\",remap=false) abstract void shadow$call();",
    ] {
        let needle = if s.contains("coun") {
            "coun"
        } else {
            "shadow$call"
        };
        let c = context(s, needle, needle.len() - 1);
        assert_eq!(c.slot, AnnotationSlot::ShadowMember);
        assert_eq!(&s[c.value_start_offset..c.value_end_offset], needle);
        if s.contains("false") {
            assert_eq!(c.shadow_prefix, None);
            assert!(c.shadow_remap);
            assert_eq!(c.shadow_member_is_method, None);
        }
    }
    let s = "@Overwrite public void completed() {}";
    let c = context(s, "completed", 3);
    assert_eq!(c.slot, AnnotationSlot::OverwriteMethod);
    assert_eq!(&s[c.value_start_offset..c.value_end_offset], "completed");
    assert!(AnnotationContextExtractor::extract_at_offset(s, s.find("{}").unwrap() + 1).is_none());
}
#[test]
fn language_detection_ignores_fake_syntax_and_honors_uri() {
    let source =
        "class Patch { String x = \"::class\"; // import x.Y\n void method() { var x = 1; } }";
    assert!(!JavaTypeDescriptorResolver::imports_for(source).is_kotlin);
    assert!(
        !JavaTypeDescriptorResolver::imports_for_uri("import game.Target\n", "file:///Patch.java")
            .is_kotlin
    );
    assert!(
        JavaTypeDescriptorResolver::imports_for_uri("class Patch {}", "file:///Patch.kt").is_kotlin
    );
    assert!(JavaTypeDescriptorResolver::imports_for("fun top() {} ").is_kotlin);
}
