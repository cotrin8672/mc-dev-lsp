//! One-to-one legacy parser and mapping tests. Selector tests are in foundation_legacy_selectors.rs.
use mcdev_core::descriptor::*;
use mcdev_core::mapping::*;
use mcdev_core::MappingNamespace::{Intermediary, Named, Official, Srg};

macro_rules! legacy {
 ($path:literal,$case:ident,$name:ident,$body:block)=>{
  #[test]
  #[doc=concat!("mcdev-core/src/test/kotlin/io/github/mcdev/core/",$path,"::",stringify!($case))]
  fn $name() $body
 };
}
legacy!(
    "descriptor/DescriptorParserTest.kt",
    parsesPrimitiveFieldDescriptor,
    parses_primitive_field_descriptor,
    {
        let value = parse_field_descriptor("I").unwrap();
        assert_eq!(JvmType::IntType, value);
        assert_eq!("int", render(&value));
    }
);
legacy!(
    "descriptor/DescriptorParserTest.kt",
    parsesObjectAndArrayDescriptors,
    parses_object_and_array_descriptors,
    {
        let value = parse_field_descriptor("[Ljava/lang/String;").unwrap();
        assert_eq!("String[]", render(&value));
        assert_eq!("[Ljava/lang/String;", to_descriptor(&value));
    }
);
legacy!(
    "descriptor/DescriptorParserTest.kt",
    parsesMethodDescriptor,
    parses_method_descriptor,
    {
        let value = parse_method_descriptor("(Ljava/lang/String;FFI)I").unwrap();
        assert_eq!("(String, float, float, int): int", render_method(&value));
        assert_eq!("(Ljava/lang/String;FFI)I", method_to_descriptor(&value));
    }
);
legacy!(
    "descriptor/DescriptorParserTest.kt",
    rejectsInvalidObjectDescriptor,
    rejects_invalid_object_descriptor,
    {
        assert!(parse_field_descriptor("Ljava/lang/String").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserTest.kt",
    parsesMethodTarget,
    parses_method_target,
    {
        let MemberTarget::Method {
            owner,
            name,
            descriptor,
        } = MemberTargetParser::parse(
            "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I",
        )
        .unwrap()
        else {
            panic!("expected method")
        };
        assert_eq!("net/minecraft/client/font/TextRenderer", owner);
        assert_eq!("draw", name);
        assert_eq!(
            "(String, float, float, int): int",
            render_method(&descriptor)
        );
    }
);
legacy!(
    "descriptor/DescriptorParserTest.kt",
    parsesFieldTarget,
    parses_field_target,
    {
        let MemberTarget::Field{name,descriptor,..}=MemberTargetParser::parse("Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;").unwrap()else{panic!("expected field")};
        assert_eq!("currentScreen", name);
        assert_eq!("Screen", render(&descriptor));
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rejectsMethodDescriptorMissingReturnType,
    rejects_method_descriptor_missing_return_type,
    {
        assert!(parse_method_descriptor("(I)").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rejectsMethodDescriptorWithVoidParameter,
    rejects_method_descriptor_with_void_parameter,
    {
        assert!(parse_method_descriptor("(V)V").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rejectsFieldDescriptorUsingDotNotation,
    rejects_field_descriptor_using_dot_notation,
    {
        assert!(parse_field_descriptor("Ljava.lang.String;").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rejectsObjectDescriptorWithBracketInInternalName,
    rejects_object_descriptor_with_bracket_in_internal_name,
    {
        assert!(parse_field_descriptor("Lfoo[Bar;").is_err());
        assert!(parse_method_descriptor("(Lfoo[Bar;)V").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    acceptsArrayDescriptorWithLeadingBracketPrefix,
    accepts_array_descriptor_with_leading_bracket_prefix,
    {
        let value = parse_field_descriptor("[Ljava/lang/String;").unwrap();
        assert_eq!("String[]", render(&value));
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rendersNestedArrayDescriptor,
    renders_nested_array_descriptor,
    {
        let value = parse_field_descriptor("[[I").unwrap();
        assert_eq!("int[][]", render(&value));
        assert_eq!("[[I", to_descriptor(&value));
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rendersConstructorTarget,
    renders_constructor_target,
    {
        let MemberTarget::Method {
            name, descriptor, ..
        } = MemberTargetParser::parse("Lcom/example/Foo;<init>()V").unwrap()
        else {
            panic!()
        };
        assert_eq!("<init>", name);
        assert_eq!("()", render_method(&descriptor).split(':').next().unwrap());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rejectsMemberTargetMissingOwnerSemicolon,
    rejects_member_target_missing_owner_semicolon,
    {
        assert!(MemberTargetParser::parse("Lcom/example/Foo").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    rejectsMemberTargetWithInvalidFieldDescriptor,
    rejects_member_target_with_invalid_field_descriptor,
    {
        assert!(MemberTargetParser::parse("Lcom/example/Foo;bar:Ljava.lang.String").is_err());
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    exactTargetRenderingMatchesInputForMethod,
    exact_target_rendering_matches_input_for_method,
    {
        let input = "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I";
        let MemberTarget::Method {
            owner,
            name,
            descriptor,
        } = MemberTargetParser::parse(input).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            input,
            format!("L{owner};{name}{}", method_to_descriptor(&descriptor))
        );
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    exactTargetRenderingMatchesInputForField,
    exact_target_rendering_matches_input_for_field,
    {
        let input="Lnet/minecraft/client/MinecraftClient;currentScreen:Lnet/minecraft/client/gui/screen/Screen;";
        let MemberTarget::Field {
            owner,
            name,
            descriptor,
        } = MemberTargetParser::parse(input).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            input,
            format!("L{owner};{name}:{}", to_descriptor(&descriptor))
        );
    }
);
legacy!(
    "descriptor/DescriptorParserEdgeCasesTest.kt",
    parseFailureIncludesOffset,
    parse_failure_includes_offset,
    {
        let failure = parse_field_descriptor("Ljava/lang/String").unwrap_err();
        let offset: isize = failure.offset.try_into().unwrap();
        assert!(offset >= 0);
    }
);

fn fixture() -> MappingSetResolver {
    TinyV2Parser::parse(include_str!("fixtures/fabric-basic-mappings.tiny"))
        .unwrap()
        .as_resolver()
}
fn found<T: std::fmt::Debug>(result: MappingLookupResult<T>) -> T {
    match result {
        MappingLookupResult::Found(value) => value,
        other => panic!("expected Found, got {other:?}"),
    }
}
legacy!(
    "mapping/MappingFixtureRegressionTest.kt",
    remapsSimpleTargetClassNamedToIntermediary,
    remaps_simple_target_class_named_to_intermediary,
    {
        let result = found(fixture().remap_class(
            &ClassRef {
                internal_name: "com/example/target/SimpleTarget".into(),
                namespace: Named,
            },
            Intermediary,
        ));
        assert_eq!("com/example/target/class_1", result.internal_name);
    }
);
legacy!(
    "mapping/MappingFixtureRegressionTest.kt",
    remapsSimpleTargetMethodNamedToIntermediary,
    remaps_simple_target_method_named_to_intermediary,
    {
        let result = found(fixture().remap_method(
            &MethodRef {
                owner: "com/example/target/SimpleTarget".into(),
                name: "draw".into(),
                descriptor: "(Ljava/lang/String;FF)V".into(),
                namespace: Named,
            },
            Intermediary,
        ));
        assert_eq!("com/example/target/class_1", result.owner);
        assert_eq!("method_1", result.name);
        assert_eq!("(Ljava/lang/String;FF)V", result.descriptor);
    }
);
legacy!(
    "mapping/MappingFixtureRegressionTest.kt",
    remapsSimpleTargetFieldNamedToIntermediary,
    remaps_simple_target_field_named_to_intermediary,
    {
        let result = found(fixture().remap_field(
            &FieldRef {
                owner: "com/example/target/SimpleTarget".into(),
                name: "counter".into(),
                descriptor: "I".into(),
                namespace: Named,
            },
            Intermediary,
        ));
        assert_eq!("field_1", result.name);
    }
);
legacy!(
    "mapping/MappingFixtureRegressionTest.kt",
    remapsMethodDescriptorTypesNamedToIntermediary,
    remaps_method_descriptor_types_named_to_intermediary,
    {
        let result = found(fixture().remap_descriptor(
            "(Lcom/example/target/SimpleTarget;)V",
            Named,
            Intermediary,
        ));
        assert_eq!("(Lcom/example/target/class_1;)V", result);
    }
);
legacy!(
    "mapping/MappingFixtureRegressionTest.kt",
    reportsMissingIntermediaryMappingForUnknownClass,
    reports_missing_intermediary_mapping_for_unknown_class,
    {
        let missing = fixture().remap_class(
            &ClassRef {
                internal_name: "com/example/missing/Unknown".into(),
                namespace: Named,
            },
            Intermediary,
        );
        let MappingLookupResult::Missing {
            subject, from, to, ..
        } = missing
        else {
            panic!("expected missing")
        };
        assert_eq!(MappingSubject::Class, subject);
        assert_eq!(Named, from);
        assert_eq!(Intermediary, to);
    }
);
legacy!(
    "mapping/MappingParserTest.kt",
    parsesTinyV2ClassesAndMembers,
    parses_tiny_v2_classes_and_members,
    {
        let mappings=TinyV2Parser::parse("tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/client/MinecraftClient\tnet/minecraft/class_310\n\tm\t()V\ttick\tmethod_1574\n\tf\tLnet/minecraft/client/gui/screen/Screen;\tcurrentScreen\tfield_1755").unwrap();
        assert_eq!(
            Some("net/minecraft/class_310".into()),
            mappings.class_name("net/minecraft/client/MinecraftClient", Named, Intermediary)
        );
        assert_eq!(
            Some("method_1574".into()),
            mappings.method_name(
                "net/minecraft/client/MinecraftClient",
                "tick",
                "()V",
                Named,
                Intermediary
            )
        );
    }
);
legacy!(
    "mapping/MappingParserTest.kt",
    parsesSrgMappings,
    parses_srg_mappings,
    {
        let mappings=SrgParser::parse("CL: net/minecraft/client/Minecraft net/minecraft/client/Minecraft\nMD: net/minecraft/client/Minecraft/func_91152_a (Ljava/lang/String;)V net/minecraft/client/Minecraft/m_91152_ (Ljava/lang/String;)V\nFD: net/minecraft/client/Minecraft/field_91074_ net/minecraft/client/Minecraft/f_91074_").unwrap();
        assert_eq!(
            Some("m_91152_".into()),
            mappings.method_name(
                "net/minecraft/client/Minecraft",
                "func_91152_a",
                "(Ljava/lang/String;)V",
                Official,
                Srg
            )
        );
    }
);
legacy!(
    "mapping/MappingParserTest.kt",
    remapsClassesMembersAndDescriptors,
    remaps_classes_members_and_descriptors,
    {
        let mappings=TinyV2Parser::parse("tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/client/MinecraftClient\tnet/minecraft/class_310\n\tm\t(Lnet/minecraft/client/gui/screen/Screen;)V\tsetScreen\tmethod_1507\n\tf\tLnet/minecraft/client/gui/screen/Screen;\tcurrentScreen\tfield_1755\nc\tnet/minecraft/client/gui/screen/Screen\tnet/minecraft/class_437").unwrap().as_resolver();
        let method = found(mappings.remap_method(
            &MethodRef {
                owner: "net/minecraft/client/MinecraftClient".into(),
                name: "setScreen".into(),
                descriptor: "(Lnet/minecraft/client/gui/screen/Screen;)V".into(),
                namespace: Named,
            },
            Intermediary,
        ));
        assert_eq!("net/minecraft/class_310", method.owner);
        assert_eq!("method_1507", method.name);
        assert_eq!("(Lnet/minecraft/class_437;)V", method.descriptor);
        let field = found(mappings.remap_field(
            &FieldRef {
                owner: "net/minecraft/client/MinecraftClient".into(),
                name: "currentScreen".into(),
                descriptor: "Lnet/minecraft/client/gui/screen/Screen;".into(),
                namespace: Named,
            },
            Intermediary,
        ));
        assert_eq!("net/minecraft/class_310", field.owner);
        assert_eq!("field_1755", field.name);
        assert_eq!("Lnet/minecraft/class_437;", field.descriptor);
    }
);
legacy!(
    "mapping/MappingParserTest.kt",
    reportsMissingMemberMappingSeparatelyFromMissingClassMapping,
    reports_missing_member_mapping_separately_from_missing_class_mapping,
    {
        let mappings=TinyV2Parser::parse("tiny\t2\t0\tnamed\tintermediary\nc\tnet/minecraft/client/MinecraftClient\tnet/minecraft/class_310").unwrap().as_resolver();
        assert!(matches!(
            mappings.remap_method(
                &MethodRef {
                    owner: "net/minecraft/client/MinecraftClient".into(),
                    name: "missing".into(),
                    descriptor: "()V".into(),
                    namespace: Named
                },
                Intermediary
            ),
            MappingLookupResult::Missing {
                subject: MappingSubject::Method,
                ..
            }
        ));
        assert!(matches!(
            mappings.remap_class(
                &ClassRef {
                    internal_name: "net/minecraft/client/Unknown".into(),
                    namespace: Named
                },
                Intermediary
            ),
            MappingLookupResult::Missing {
                subject: MappingSubject::Class,
                ..
            }
        ));
    }
);
