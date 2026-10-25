#[path = "support/semantic.rs"]
mod semantic;
mod support;
use semantic::*;
use serde_json::json;
use support::Process;

fn twice() -> Shadow {
    Shadow::method("@Shadow abstract int twice(int x);", "twice", "(I)I")
}
fn evaluate(case: &Case) -> Observation {
    let mut p = Process::new();
    p.initialize();
    let result = case.evaluate(&mut p, "semantic-case");
    p.eof();
    assert!(p.wait().success());
    result
}

#[test]
fn java_exact_methods_fields_dispatch_and_prefixes() {
    let case = Case::java(
        "@Mixin(Target.class)",
        vec![
            Shadow::field("@Shadow int count;", "count", "I"),
            Shadow::field("@Shadow static int total;", "total", "I").static_(),
            Shadow::method(
                "@Shadow abstract int shadow$twice(int x);",
                "shadow$twice",
                "(I)I",
            ),
            Shadow::method(
                "@Shadow(prefix=\"custom$\") abstract BlockState custom$lookup(BlockPos pos);",
                "custom$lookup",
                "(Lfixture/BlockPos;)Lfixture/BlockState;",
            )
            .prefix("custom$"),
            Shadow::method(
                "@Shadow static int global(int x) { return 0; }",
                "global",
                "(I)I",
            )
            .static_(),
        ],
    );
    let result = evaluate(&case);
    assert_eq!(result.bound(), 5);
    assert_eq!(result.contributions["sites"].as_array().unwrap().len(), 5);
    assert!(!result.incomplete());
    assert!(!result.missing());
    assert_eq!(result.requirements[2]["names"]["values"], json!(["twice"]));
    assert_eq!(
        result.requirements[2]["nameLink"],
        json!({"kind":"prefixed","prefix":"shadow$"})
    );
}
#[test]
fn exact_signature_selects_the_correct_overload_location() {
    let mut case = Case::java(
        "@Mixin(Target.class)",
        vec![Shadow::method(
            "@Shadow abstract int shadow$twice(int x);",
            "shadow$twice",
            "(I)I",
        )],
    );
    case.target_source="package fixture; public class Target { public int twice(long x) { return 0; } public int twice(int x) { return 0; } }".into();
    case.target_members = vec![
        json!({"id":"target-long","name":"twice","kind":"method","descriptor":"(J)I","dispatch":"instance"}),
        json!({"id":"target-int","name":"twice","kind":"method","descriptor":"(I)I","dispatch":"instance"}),
    ];
    let result = evaluate(&case);
    assert_eq!(result.bound(), 1);
    assert_eq!(result.bindings[0]["value"]["target"], "target-int");
    assert_eq!(
        result.requirements[0]["signature"]["value"]["descriptor"],
        "(I)I"
    );
}
#[test]
fn kotlin_nominal_types_arrays_and_backticks_use_shared_facts() {
    let case = Case::kotlin(vec![
        Shadow::field(
            "@field:Shadow private lateinit var anchor: BlockPos",
            "anchor",
            "Lfixture/BlockPos;",
        )
        .use_site("field"),
        Shadow::method(
            "@Shadow abstract fun `shadow$twice`(x: Int): Int",
            "`shadow$twice`",
            "(I)I",
        ),
        Shadow::method(
            "@Shadow abstract fun lookup(pos: BlockPos): BlockState",
            "lookup",
            "(Lfixture/BlockPos;)Lfixture/BlockState;",
        ),
        Shadow::method(
            "@Shadow abstract fun positions(state: BlockState): Array<BlockPos>",
            "positions",
            "(Lfixture/BlockState;)[Lfixture/BlockPos;",
        ),
    ]);
    let result = evaluate(&case);
    assert_eq!(result.bound(), 4);
    assert!(!result.incomplete());
    let site = &result.contributions["sites"][1];
    let start = site["editableName"]["start"].as_u64().unwrap() as usize;
    let end = site["editableName"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&case.source[start..end], "twice");
}
#[test]
fn single_class_and_fully_qualified_string_targets_bind() {
    let class_array = json!({"kind":"array","values":[arg(None,class_target())]});
    let string_array = json!({"kind":"array","values":[arg(None,string_target())]});
    for (annotation, arguments) in [
        (
            "@Mixin(value=Target.class)",
            vec![arg(Some("value"), class_target())],
        ),
        (
            "@Mixin({Target.class})",
            vec![arg(None, class_array.clone())],
        ),
        (
            "@Mixin(targets=\"fixture.Target\")",
            vec![arg(Some("targets"), string_target())],
        ),
        (
            "@Mixin(targets={\"fixture.Target\"})",
            vec![arg(Some("targets"), string_array.clone())],
        ),
    ] {
        let mut case = Case::java(annotation, vec![twice()]);
        case.target_arguments = arguments;
        assert_eq!(evaluate(&case).bound(), 1, "{annotation}");
    }
    for (annotation, arguments) in [
        ("@Mixin([Target::class])", vec![arg(None, class_array)]),
        (
            "@Mixin(targets = [\"fixture.Target\"])",
            vec![arg(Some("targets"), string_array)],
        ),
    ] {
        let shadow = Shadow::method("@Shadow abstract fun twice(x: Int): Int", "twice", "(I)I");
        let source=format!("package fixture\nimport org.spongepowered.asm.mixin.Mixin\nimport org.spongepowered.asm.mixin.Shadow\n{annotation}\nabstract class CaseMixin {{ {} }}",shadow.raw);
        let mut case = Case::source(source, annotation, vec![shadow]);
        case.target_arguments = arguments;
        assert_eq!(evaluate(&case).bound(), 1, "{annotation}");
    }
}
#[test]
fn unsupported_or_multiple_targets_never_choose_a_first_target() {
    for (annotation, arguments) in [
        (
            "@Mixin({Target.class, BlockPos.class})",
            vec![arg(
                None,
                json!({"kind":"array","values":[arg(None,class_target()),arg(None,json!({"kind":"class","typeRef":"extra-target-reference"}))]}),
            )],
        ),
        (
            "@Mixin(value=Target.class, targets=\"fixture.Target\")",
            vec![
                arg(Some("value"), class_target()),
                arg(Some("targets"), string_target()),
            ],
        ),
        (
            "@Mixin(targets=TARGET_NAME)",
            vec![json!({"name":"targets","value":unknown()})],
        ),
        (
            "@Mixin(value=\"fixture.Target\")",
            vec![arg(Some("value"), string_target())],
        ),
        (
            "@Mixin(targets=Target.class)",
            vec![arg(Some("targets"), class_target())],
        ),
    ] {
        let mut case = Case::java(annotation, vec![twice()]);
        case.target_arguments = arguments;
        let result = evaluate(&case);
        assert_eq!(result.bound(), 0);
        assert!(result.incomplete());
        assert!(result.requirements.is_empty());
        assert!(!result.missing());
    }
}
#[test]
fn exact_annotation_identity_is_required() {
    for (mixin, shadow) in [
        ("fake.Mixin", "org.spongepowered.asm.mixin.Shadow"),
        ("org.spongepowered.asm.mixin.Mixin", "fake.Shadow"),
    ] {
        let mut spec = twice();
        spec.raw = spec.raw.replace("@Shadow", &format!("@{shadow}"));
        spec.identity = shadow.into();
        let mut case = Case::java(&format!("@{mixin}(Target.class)"), vec![spec]);
        case.mixin_identity = mixin.into();
        let result = evaluate(&case);
        assert_eq!(result.bound(), 0);
        assert!(result.requirements.is_empty());
        assert!(!result.missing());
    }
}
#[test]
fn descriptor_dispatch_unknown_and_aliases_do_not_guess() {
    for spec in [
        Shadow::method("@Shadow abstract long twice(int x);", "twice", "(I)J"),
        Shadow::method(
            "@Shadow static int twice(int x) { return 0; }",
            "twice",
            "(I)I",
        )
        .static_(),
        Shadow::method("@Shadow abstract int global(int x);", "global", "(I)I"),
        Shadow::method("@Shadow abstract int twice(Unresolved x);", "twice", "(I)I")
            .unknown_signature(),
        Shadow::method(
            "@Shadow(prefix=PREFIX) abstract int twice(int x);",
            "twice",
            "(I)I",
        )
        .unknown_prefix(),
        Shadow::method(
            "@Shadow(aliases={\"other\"}) abstract int twice(int x);",
            "twice",
            "(I)I",
        )
        .aliases(),
        Shadow::method("@Shadow abstract <T> T twice(T x);", "twice", "(I)I").unknown_signature(),
    ] {
        let case = Case::java("@Mixin(Target.class)", vec![spec.clone()]);
        let result = evaluate(&case);
        assert_eq!(result.bound(), 0, "{}", spec.raw);
        if spec.descriptor.is_none() {
            assert!(result.incomplete());
            assert!(!result.missing());
        }
    }
    let mut case = Case::java("@Mixin(UnknownTarget.class)", vec![twice()]);
    case.target_fact = unknown();
    let result = evaluate(&case);
    assert_eq!(result.bound(), 0);
    assert!(result.incomplete());
    assert!(!result.missing());
    case.target_fact = absent();
    let result = evaluate(&case);
    assert_eq!(result.bound(), 0);
    assert!(result.missing());
}
#[test]
fn kotlin_accessors_and_unknown_abi_are_not_fields_or_plain_methods() {
    for spec in [
        Shadow::field("@get:Shadow val count: Int get() = 0", "count", "I").use_site("get"),
        Shadow::field("@set:Shadow var count: Int = 0", "count", "I").use_site("set"),
        Shadow::field("@Shadow val count: Int get() = 0", "count", "I").unknown_signature(),
        Shadow::field("@Shadow var count: Int = 0", "count", "I").unknown_signature(),
        Shadow::method(
            "@Shadow abstract fun lookup(pos: BlockPos?): BlockState",
            "lookup",
            "(Lfixture/BlockPos;)Lfixture/BlockState;",
        )
        .unknown_signature(),
        Shadow::method(
            "@Shadow abstract fun lookup(pos: BlockPos): BlockState?",
            "lookup",
            "(Lfixture/BlockPos;)Lfixture/BlockState;",
        )
        .unknown_signature(),
    ] {
        let result = evaluate(&Case::kotlin(vec![spec]));
        assert_eq!(result.bound(), 0);
        assert!(result.incomplete());
        assert!(!result.missing());
    }
}
#[test]
fn fields_do_not_strip_the_method_prefix() {
    let result = evaluate(&Case::java(
        "@Mixin(Target.class)",
        vec![Shadow::field(
            "@Shadow int shadow$count;",
            "shadow$count",
            "I",
        )],
    ));
    assert_eq!(result.bound(), 0);
    assert!(result.requirements.is_empty());
    assert!(result.contributions["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["message"]
            .as_str()
            .unwrap()
            .starts_with("[mcdev.mixin.shadow.field-prefix]")));
}
#[test]
fn empty_shadow_prefix_field_binds_without_field_prefix_error() {
    let result = evaluate(&Case::java(
        "@Mixin(Target.class)",
        vec![Shadow::field("@Shadow(prefix=\"\") int count;", "count", "I").prefix("")],
    ));
    assert_eq!(result.bound(), 1);
    assert_eq!(result.requirements.len(), 1);
    assert!(!result.incomplete());
    assert!(!result.missing());
    assert!(result.contributions["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["severity"] != "error"));
}
#[test]
fn empty_effective_name_does_not_discard_other_contributions() {
    let case = Case::java(
        "@Mixin(Target.class)",
        vec![
            Shadow::method("@Shadow(prefix=\"bad\") abstract int bad();", "bad", "()I")
                .prefix("bad"),
            twice(),
        ],
    );
    let result = evaluate(&case);
    assert_eq!(result.bound(), 1);
    assert_eq!(result.requirements.len(), 1);
    assert!(result.incomplete());
}
#[test]
fn ambiguous_owner_never_binds_to_another_declaration() {
    let mut case = Case::java("@Mixin(Target.class)", vec![twice()]);
    case.own_fact = unknown();
    let result = evaluate(&case);
    assert_eq!(result.bound(), 0);
    assert!(result.requirements.is_empty());
    assert!(result.incomplete());
    assert!(!result.missing());
}
#[test]
fn unsaved_edits_and_provider_configuration_retract_and_rebind() {
    let case = Case::java(
        "@Mixin(Target.class)",
        vec![Shadow::method(
            "@Shadow abstract int shadow$twice(int x);",
            "shadow$twice",
            "(I)I",
        )],
    );
    let mut p = Process::new();
    p.initialize();
    assert_eq!(case.evaluate(&mut p, "original").bound(), 1);
    let mut changed = case.clone();
    changed
        .target_members
        .iter_mut()
        .find(|m| m["name"] == "twice")
        .unwrap()["descriptor"] = json!("(I)J");
    assert_eq!(changed.evaluate(&mut p, "target-unsaved-edit").bound(), 0);
    assert_eq!(case.evaluate(&mut p, "target-restored").bound(), 1);
    let mut changed = case.clone();
    changed.source = changed
        .source
        .replace("int shadow$twice", "long shadow$twice");
    changed.shadows[0].raw = changed.shadows[0]
        .raw
        .replace("int shadow$twice", "long shadow$twice");
    changed.shadows[0].descriptor = Some("(I)J".into());
    assert_eq!(changed.evaluate(&mut p, "owner-unsaved-edit").bound(), 0);
    assert_eq!(case.evaluate(&mut p, "owner-restored").bound(), 1);
    p.eof();
    assert!(p.wait().success());
    // Provider disable/re-enable is host-owned: closing the old child and initializing
    // a fresh one establishes that no companion state is needed to rebind.
    let mut p = Process::new();
    p.initialize();
    assert_eq!(case.evaluate(&mut p, "provider-restarted").bound(), 1);
    p.eof();
    assert!(p.wait().success());
}
#[test]
fn rename_preserves_prefix_backticks_and_unrelated_source() {
    for mut case in [
        Case::java(
            "@Mixin(Target.class)",
            vec![Shadow::method(
                "@Shadow abstract int shadow$twice(int x);",
                "shadow$twice",
                "(I)I",
            )],
        ),
        Case::kotlin(vec![Shadow::method(
            "@Shadow abstract fun `shadow$twice`(x: Int): Int",
            "`shadow$twice`",
            "(I)I",
        )]),
    ] {
        let last = case.source.rfind('}').unwrap();
        case.source.insert_str(
            last,
            if case.source.contains("abstract fun") {
                " val unrelated = 1 "
            } else {
                " int unrelated; "
            },
        );
        case.source =
            case.source
                .replacen("package fixture", "/* 😀 unsaved */ package fixture", 1);
        let result = evaluate(&case);
        assert_eq!(result.bound(), 1);
        let site = &result.contributions["sites"][0];
        let start = site["editableName"]["start"].as_u64().unwrap() as usize;
        let end = site["editableName"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&case.source[start..end], "twice");
        let mut edited = case.source.clone();
        edited.replace_range(start..end, "doubled");
        assert_eq!(
            edited,
            case.source.replace("shadow$twice", "shadow$doubled")
        );
        assert_eq!(site["renameSubject"], "target");
    }
}

#[test]
fn more_than_one_fact_batch_preserves_every_exact_declaration_binding() {
    let shadows = (0..257)
        .map(|n| {
            Shadow::method(
                &format!("@Shadow abstract int member{n}(int x);"),
                &format!("member{n}"),
                "(I)I",
            )
        })
        .collect();
    let mut case = Case::java("@Mixin(Target.class)", shadows);
    case.target_members=(0..257).map(|n|json!({"id":format!("target-member-{n}"),"name":format!("member{n}"),"kind":"method","descriptor":"(I)I","dispatch":"instance"})).collect();
    let result = evaluate(&case);
    assert_eq!(result.bound(), 257);
    assert_eq!(result.contributions["sites"].as_array().unwrap().len(), 257);
    assert!(!result.incomplete());
    assert!(!result.missing());
    assert_eq!(
        result
            .queries
            .iter()
            .filter(|q| q["op"] == "declarationMember")
            .count(),
        257
    );
}

#[test]
fn cancellation_during_requirement_resolution_discards_sites_and_accumulated_diagnostics() {
    let case = Case::java(
        "@Mixin(Target.class)",
        vec![
            Shadow::method("@Shadow(prefix=\"bad\") abstract int bad();", "bad", "()I")
                .prefix("bad"),
            twice(),
        ],
    );
    let (evaluation, refs, members) = case.snapshot("cancel-after-facts");
    let mut p = Process::new();
    p.initialize();
    p.request("cancel-after-facts", "analysis/evaluate", evaluation);
    let pending = loop {
        let request = p.receive();
        assert!(
            request.get("method").is_some(),
            "unexpected early result {request}"
        );
        if request["method"] == "analysis/require" {
            break request;
        }
        assert_eq!(request["method"], "analysis/readFacts");
        let facts = request["params"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| match q["op"].as_str().unwrap() {
                "resolveTypeRef" => refs[q["reference"].as_str().unwrap()].clone(),
                "declarationMember" => members[q["declaration"].as_str().unwrap()].clone(),
                op => panic!("unexpected query {op}"),
            })
            .collect::<Vec<_>>();
        p.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"facts":facts}}));
    };
    assert_eq!(
        pending["params"]["requirements"].as_array().unwrap().len(),
        1
    );
    p.send(
        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"cancel-after-facts"}}),
    );
    assert_eq!(
        p.receive(),
        json!({"jsonrpc":"2.0","id":"cancel-after-facts","result":{"sites":[],"diagnostics":[]}})
    );
    p.send(json!({"jsonrpc":"2.0","id":pending["id"],"result":{"requirements":[{"id":"cancelled-requirement","binding":found(json!({"target":"old-target"})),"candidates":[]}]}}));
    p.request(
        "fresh-after-cancel",
        "analysis/evaluate",
        support::empty_evaluation("fresh-after-cancel"),
    );
    assert_eq!(p.receive()["result"], json!({"sites":[],"diagnostics":[]}));
    p.eof();
    assert!(p.wait().success());
}
