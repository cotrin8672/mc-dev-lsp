//! Differential migration corpus: see fixtures/expression_engine/generate_capture.py.
#[path = "fixtures/expression_engine_replay.rs"]
mod replay;
use mcdev_core::expression_engine::*;
use replay::*;
use serde_json::Value;
#[test]
fn original_parser_requests() {
    check_rows("OfficialExpressionParser.parse", |row| {
        let actual = parse_source(&row["args"]["source"]);
        if row["outputType"].as_str().unwrap().ends_with("$Success") {
            actual
                .map_err(|e| format!("expected success, {e}"))
                .and_then(|value| {
                    equal(
                        ast_source(&value, &row["args"]["source"]),
                        row["output"]["expression"].clone(),
                    )
                })
        } else {
            match actual {
                Err(error) => {
                    if error.message.is_empty() || error.message != error.cause.message {
                        Err("syntax failure must retain equal nonempty cause/message".into())
                    } else {
                        Ok(())
                    }
                }
                Ok(value) => Err(format!("expected syntax failure, got {value:?}")),
            }
        }
    });
}
#[test]
fn original_receiver_probe_requests() {
    check_rows("OfficialExpressionMemberCompletionProbes.build", |row| {
        let actual = receiver_probes(
            row["args"]["receiverExpression"].as_str().unwrap(),
            &pool(&row["args"]["identifierPool"]),
        );
        let expected = if row["output"].is_null() {
            None
        } else {
            Some(
                row["output"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        (
                            if v["kind"] == "FIELD" {
                                MemberKind::Field
                            } else {
                                MemberKind::Method
                            },
                            s(&v["source"]),
                        )
                    })
                    .collect(),
            )
        };
        equal(actual, expected)
    });
}
#[test]
fn original_identifier_build_requests() {
    check_rows(
        "OfficialExpressionIdentifierPoolBuilder.build",
        replay_build_request,
    );
}

fn instruction(value: &Value) -> Instruction {
    let opcode = value["opcode"].as_u64().unwrap() as u8;
    let fields = value.get("fields").unwrap_or(value);
    let operand = if fields.get("owner").is_some() {
        InstructionOperand::Member {
            owner: s(&fields["owner"]),
            name: s(&fields["name"]),
            descriptor: s(&fields["desc"]),
            is_interface: fields["itf"].as_bool().unwrap_or(false),
        }
    } else if fields.get("var").is_some() {
        InstructionOperand::Local(fields["var"].as_u64().unwrap() as usize)
    } else {
        InstructionOperand::None
    };
    Instruction {
        opcode,
        original_opcode: opcode,
        original_index: 0,
        byte_offset: 0,
        operand,
    }
}
#[test]
fn original_identifier_predicates() {
    for (api, count) in [
        ("IdentifierPool.matchesMember", 29),
        ("IdentifierPool.matchesType", 17),
        ("IdentifierPool.memberExists", 11),
        ("IdentifierPool.typeExists", 4),
    ] {
        assert_eq!(rows(api).len(), count);
        check_rows(api, |row| {
            let args = &row["args"];
            let pool = pool(&args["pool"]);
            let id = args["id"].as_str().unwrap();
            let actual = match api {
                "IdentifierPool.memberExists" => pool.member_exists(id),
                "IdentifierPool.typeExists" => pool.type_exists(id),
                "IdentifierPool.matchesType" => pool
                    .types
                    .get(id)
                    .unwrap()
                    .iter()
                    .any(|v| v == args["type"].as_str().unwrap()),
                _ => {
                    let instruction = instruction(&args["node"]["instruction"]);
                    pool.members
                        .get(id)
                        .unwrap()
                        .iter()
                        .any(|entry| entry.matches_instruction(&instruction))
                }
            };
            equal(actual, row["output"].as_bool().unwrap())
        });
    }
    check_rows("MemberDefinition.matches", |row| {
        let definition = member(&row["args"]["definition"]);
        let input = &row["args"]["input"];
        let actual = if input.get("tag").is_some() {
            definition.matches_handle(&JvmConstant::MethodHandle {
                tag: input["tag"].as_u64().unwrap() as u8,
                owner: s(&input["owner"]),
                name: s(&input["name"]),
                descriptor: s(&input["descriptor"]),
                is_interface: input["isInterface"].as_bool().unwrap_or(false),
            })
        } else {
            definition.matches_instruction(&instruction(input))
        };
        equal(actual, row["output"].as_bool().unwrap())
    });
}
#[test]
fn original_match_requests() {
    check_rows("OfficialExpressionMatcher.match", replay_match_request);
}
#[test]
fn every_official_virtual_component_keeps_exact_type_decorations_and_capture_location() {
    check_rows(
        "OfficialExpressionMatcher.matchComponents",
        replay_match_request,
    );
}

#[test]
fn original_completion_requests() {
    check_rows("OfficialExpressionMatcher.completeReceiverMembers", |row| {
        if row.get("throwable").is_some() {
            return original_cancellation(row);
        }
        let args = &row["args"];
        let pool = pool(&args["identifierPool"]);
        let resolved = resolver(row);
        let resolve = |left: &str, right: &str| {
            resolved
                .get(&(left.into(), right.into()))
                .cloned()
                .flatten()
        };
        let context = MatchContext {
            context_type: context_kind(&args["contextType"]),
            identifier_pool: &pool,
            allow_incomplete_list_inputs: args["allowIncompleteListInputs"].as_bool().unwrap(),
            common_super_class: &resolve,
            cancellation: &NO_CANCELLATION,
        };
        let actual = complete_receiver_members(
            &bytes(row),
            args["methodName"].as_str().unwrap(),
            args["methodDescriptor"].as_str().unwrap(),
            args["receiverExpression"].as_str().unwrap(),
            args["memberPrefix"].as_str().unwrap(),
            &context,
        )
        .unwrap();
        let expected = &row["output"];
        if row["outputType"].as_str().unwrap().ends_with("$Available") {
            let expected = expected["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| MemberCandidate {
                    kind: if v["kind"] == "FIELD" {
                        MemberKind::Field
                    } else {
                        MemberKind::Method
                    },
                    owner_internal_name: s(&v["ownerInternalName"]),
                    name: s(&v["name"]),
                    descriptor: s(&v["descriptor"]),
                    is_interface: v["isInterface"].as_bool().unwrap_or(false),
                    original_instruction_opcode: v["originalInstructionOpcode"].as_u64().unwrap()
                        as u8,
                    original_instruction_index: v["originalInstructionIndex"].as_u64().unwrap()
                        as usize,
                    decorations: decorations(&v["decorations"]),
                })
                .collect();
            equal(actual, MemberCompletionResult::Available(expected))
        } else {
            match actual {
                MemberCompletionResult::Unavailable { reason, cause } => {
                    unavailable(&reason, cause, expected)
                }
                other => Err(format!(
                    "expected Unavailable {}, actual {other:?}",
                    expected["reason"]
                )),
            }
        }
    });
}
