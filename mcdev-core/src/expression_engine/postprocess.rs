use super::flow::{CallType, ConcatInfo};
use super::*;

fn opcode(graph: &FlowGraph, id: usize) -> Option<u8> {
    graph.nodes[id].instruction.as_ref().map(|insn| insn.opcode)
}
fn virtual_instruction(
    method: &MethodFlow,
    id: usize,
    opcode: u8,
    operand: InstructionOperand,
) -> Instruction {
    let mut insn = method.graph.nodes[id].instruction.as_ref().unwrap().clone();
    insn.opcode = opcode;
    insn.operand = operand;
    insn
}
fn member(insn: &Instruction, owner: &str, name: &str, descriptor: Option<&str>) -> bool {
    matches!(&insn.operand,InstructionOperand::Member{owner:o,name:n,descriptor:d,..}if o==owner&&n==name&&descriptor.is_none_or(|descriptor|d==descriptor))
}
fn constant_int(graph: &FlowGraph, id: usize) -> Option<i32> {
    match graph.nodes[id].instruction.as_ref()?.operand {
        InstructionOperand::Constant(JvmConstant::Int(value)) => Some(value),
        _ => None,
    }
}
fn all_children(graph: &FlowGraph) -> Vec<Vec<(usize, usize)>> {
    (0..graph.nodes.len())
        .map(|id| graph.children(id))
        .collect()
}
fn add_virtual(
    method: &mut MethodFlow,
    root: usize,
    opcode: u8,
    operand: InstructionOperand,
    ty: TypeConstraint,
    inputs: Vec<usize>,
) -> usize {
    let insn = virtual_instruction(method, root, opcode, operand);
    method.graph.add(insn, ty, inputs)
}
pub(crate) fn finish(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    // Preserve the exact 0.0.6 postprocessor order; each stage sees the previous stage's graph.
    arrays(method, context)?;
    increments(method, context)?;
    comparisons(method, context)?;
    indy_concat(method, context)?;
    constructors(method, context)?;
    builder_concat(method, context)?;
    calls(method, context)?;
    lambdas(method, context)?;
    split_nodes(method, context)?;
    Ok(())
}
fn arrays(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let children = all_children(&method.graph);
    let count = method.graph.nodes.len();
    for id in 0..count {
        context.cancellation.check_cancelled()?;
        if method.graph.nodes[id].synthetic
            || !matches!(opcode(&method.graph, id), Some(0xbc | 0xbd))
        {
            continue;
        }
        let Some(&size_node) = method.graph.nodes[id].inputs.first() else {
            continue;
        };
        let Some(size) =
            constant_int(&method.graph, size_node).and_then(|value| usize::try_from(value).ok())
        else {
            continue;
        };
        if size == 0 {
            continue;
        }
        let mut next = children[id].iter().map(|&(id, _)| id).collect::<Vec<_>>();
        next.sort_by_key(|&id| {
            method.graph.nodes[id]
                .instruction
                .as_ref()
                .map(|insn| insn.original_index)
        });
        if next.len() < size {
            continue;
        }
        let stores = &next[..size];
        if !stores.iter().enumerate().all(|(index, &store)| {
            matches!(opcode(&method.graph, store), Some(0x4f..=0x56))
                && method.graph.nodes[store].inputs.first() == Some(&id)
                && method.graph.nodes[store]
                    .inputs
                    .get(1)
                    .and_then(|&id| constant_int(&method.graph, id))
                    == Some(index as i32)
        }) {
            continue;
        }
        let values = stores
            .iter()
            .map(|&store| method.graph.nodes[store].inputs[2])
            .collect();
        method.graph.nodes[size_node].synthetic = true;
        for &store in stores {
            let index = method.graph.nodes[store].inputs[1];
            method.graph.nodes[index].synthetic = true;
            method.graph.nodes[store].synthetic = true;
        }
        method.graph.nodes[id].inputs = values;
        method.graph.nodes[id].decorations.array_creation = true;
    }
    Ok(())
}
fn increments(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    for id in 0..method.graph.nodes.len() {
        context.cancellation.check_cancelled()?;
        let Some(insn) = method.graph.nodes[id].instruction.clone() else {
            continue;
        };
        if insn.opcode != 0x84 {
            continue;
        }
        let InstructionOperand::Increment { slot, amount } = insn.operand else {
            unreachable!()
        };
        let load = add_virtual(
            method,
            id,
            0x15,
            InstructionOperand::Local(slot),
            TypeConstraint::exact("I"),
            Vec::new(),
        );
        let cst_opcode = if (-1..=5).contains(&amount) {
            (amount + 3) as u8
        } else if i8::try_from(amount).is_ok() {
            0x10
        } else if i16::try_from(amount).is_ok() {
            0x11
        } else {
            0x12
        };
        let cst = add_virtual(
            method,
            id,
            cst_opcode,
            InstructionOperand::Constant(JvmConstant::Int(amount)),
            TypeConstraint::exact("I"),
            Vec::new(),
        );
        let add = add_virtual(
            method,
            id,
            0x60,
            InstructionOperand::None,
            TypeConstraint::exact("I"),
            vec![load, cst],
        );
        method.graph.nodes[id].instruction = Some(virtual_instruction(
            method,
            id,
            0x36,
            InstructionOperand::Local(slot),
        ));
        method.graph.nodes[id].inputs = vec![add];
    }
    Ok(())
}
fn comparisons(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let count = method.graph.nodes.len();
    for id in 0..count {
        context.cancellation.check_cancelled()?;
        let Some(op) = opcode(&method.graph, id) else {
            continue;
        };
        if !matches!(op, 0x99..=0x9e | 0xc6 | 0xc7) {
            continue;
        }
        let Some(&left) = method.graph.nodes[id].inputs.first() else {
            continue;
        };
        if matches!(opcode(&method.graph, left), Some(0x94..=0x98)) {
            method.graph.nodes[left].decorations.complex_comparison_jump = Some(id);
            method.graph.nodes[id].synthetic = true;
            continue;
        }
        let constant = if op >= 0xc6 {
            JvmConstant::Null
        } else {
            JvmConstant::Int(0)
        };
        let ty = if op >= 0xc6 {
            TypeConstraint::exact("Lnull;")
        } else {
            TypeConstraint::IntLike
        };
        let cst = add_virtual(
            method,
            id,
            if op >= 0xc6 { 0x01 } else { 0x03 },
            InstructionOperand::Constant(constant),
            ty,
            Vec::new(),
        );
        let expanded = if op >= 0xc6 {
            op - (0xc6 - 0xa5)
        } else {
            op + (0x9f - 0x99)
        };
        method.graph.nodes[id].instruction.as_mut().unwrap().opcode = expanded;
        method.graph.nodes[id].inputs = vec![left, cst];
    }
    Ok(())
}
fn indy_concat(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let count = method.graph.nodes.len();
    for id in 0..count {
        context.cancellation.check_cancelled()?;
        let Some(insn) = method.graph.nodes[id].instruction.clone() else {
            continue;
        };
        let InstructionOperand::Dynamic {
            bootstrap,
            arguments,
            ..
        } = &insn.operand
        else {
            continue;
        };
        let JvmConstant::MethodHandle { owner, name, .. } = bootstrap else {
            continue;
        };
        if owner != "java/lang/invoke/StringConcatFactory"
            || !matches!(name.as_str(), "makeConcat" | "makeConcatWithConstants")
        {
            continue;
        }
        let inputs = method.graph.nodes[id].inputs.clone();
        let template = if name == "makeConcat" {
            vec![1u16; inputs.len()]
        } else {
            let Some(JvmConstant::String(value)) = arguments.first() else {
                return Err(EngineError::Unavailable(
                    "invalid StringConcatFactory recipe".into(),
                ));
            };
            value.clone()
        };
        let mut parts = Vec::new();
        let mut text = Vec::new();
        let mut argument = 0;
        let mut constant = 1;
        for character in template {
            if matches!(character, 1 | 2) && !text.is_empty() {
                parts.push(Err(JvmConstant::String(std::mem::take(&mut text))));
            }
            match character {
                1 => {
                    parts.push(Ok(*inputs.get(argument).ok_or_else(|| {
                        EngineError::Unavailable(
                            "StringConcatFactory argument count mismatch".into(),
                        )
                    })?));
                    argument += 1;
                }
                2 => {
                    parts.push(Err(arguments.get(constant).cloned().ok_or_else(|| {
                        EngineError::Unavailable(
                            "StringConcatFactory constant count mismatch".into(),
                        )
                    })?));
                    constant += 1;
                }
                _ => text.push(character),
            }
        }
        if !text.is_empty() {
            parts.push(Err(JvmConstant::String(text)));
        }
        if parts.len() < 2 {
            continue;
        }
        let mut current = None;
        let mut initial = 0;
        let total = parts.len();
        let mut first = true;
        for (index, part) in parts.into_iter().enumerate() {
            context.cancellation.check_cancelled()?;
            let component = match part {
                Ok(value) => {
                    let value_of =
                        method.graph.nodes[value]
                            .instruction
                            .as_ref()
                            .is_some_and(|insn| {
                                insn.opcode == 0xb8
                                    && member(
                                        insn,
                                        "java/lang/String",
                                        "valueOf",
                                        Some("(Ljava/lang/Object;)Ljava/lang/String;"),
                                    )
                            });
                    if value_of {
                        method.graph.nodes[value].synthetic = true;
                        method.graph.nodes[value].inputs[0]
                    } else {
                        value
                    }
                }
                Err(constant) => {
                    let ty = match &constant {
                        JvmConstant::String(_) => TypeConstraint::exact("Ljava/lang/String;"),
                        JvmConstant::Int(_) => TypeConstraint::IntLike,
                        JvmConstant::Long(_) => TypeConstraint::exact("J"),
                        JvmConstant::Float(_) => TypeConstraint::exact("F"),
                        JvmConstant::Double(_) => TypeConstraint::exact("D"),
                        JvmConstant::Class(_) => TypeConstraint::exact("Ljava/lang/Class;"),
                        _ => {
                            return Err(EngineError::Unavailable(
                                "unsupported StringConcatFactory constant type".into(),
                            ))
                        }
                    };
                    add_virtual(
                        method,
                        id,
                        0x12,
                        InstructionOperand::Constant(constant),
                        ty,
                        Vec::new(),
                    )
                }
            };
            if index == 0 {
                current = Some(component);
                initial = component;
                continue;
            }
            let parents = vec![current.unwrap(), component];
            let target = if index + 1 == total {
                method.graph.nodes[id].instruction = Some(virtual_instruction(
                    method,
                    id,
                    0x00,
                    InstructionOperand::None,
                ));
                method.graph.nodes[id].inputs = parents;
                id
            } else {
                add_virtual(
                    method,
                    id,
                    0x00,
                    InstructionOperand::None,
                    TypeConstraint::exact("Ljava/lang/StringBuilder;"),
                    parents,
                )
            };
            method.graph.nodes[target].decorations.concat = Some(ConcatInfo {
                first,
                builder: index + 1 < total,
                initial,
                to_string: None,
            });
            first = false;
            current = Some(target);
        }
    }
    Ok(())
}
fn constructors(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let children = all_children(&method.graph);
    for id in 0..children.len() {
        context.cancellation.check_cancelled()?;
        if opcode(&method.graph, id) != Some(0xbb) {
            continue;
        }
        let init=children[id].iter().find_map(|&(child,input)|{let node=&method.graph.nodes[child];(input==0&&node.instruction.as_ref().is_some_and(|insn|insn.opcode==0xb7&&matches!(&insn.operand,InstructionOperand::Member{name,..}if name=="<init>"))).then_some(child)}).ok_or_else(||EngineError::Unavailable("Could not find <init> call for NEW".into()))?;
        method.graph.nodes[id].inputs = method.graph.nodes[init].inputs[1..].to_vec();
        method.graph.nodes[id].decorations.instantiation = Some(init);
        method.graph.nodes[init].synthetic = true;
    }
    Ok(())
}
fn builder_concat(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let children = all_children(&method.graph);
    for id in 0..children.len() {
        context.cancellation.check_cancelled()?;
        let Some(init) = method.graph.nodes[id].decorations.instantiation else {
            continue;
        };
        if method.graph.nodes[id].ty.descriptor() != "Ljava/lang/StringBuilder;"
            || !method.graph.nodes[init]
                .instruction
                .as_ref()
                .is_some_and(|insn| member(insn, "java/lang/StringBuilder", "<init>", Some("()V")))
        {
            continue;
        }
        let mut chain = Vec::new();
        let mut current = id;
        let mut seen = std::collections::BTreeSet::new();
        let to_string = loop {
            if !seen.insert(current) || children[current].len() != 1 {
                break None;
            }
            let (next, input) = children[current][0];
            if input != 0 {
                break None;
            }
            let Some(insn) = method.graph.nodes[next].instruction.as_ref() else {
                break None;
            };
            if insn.opcode == 0xb6
                && member(insn, "java/lang/StringBuilder", "append", None)
                && method.graph.nodes[next].inputs.len() == 2
            {
                chain.push(next);
                current = next;
            } else if insn.opcode == 0xb6
                && member(insn, "java/lang/StringBuilder", "toString", None)
            {
                break Some(next);
            } else {
                break None;
            }
        };
        let Some(to_string) = to_string.filter(|_| chain.len() >= 2) else {
            continue;
        };
        let initial = method.graph.nodes[chain[0]].inputs[1];
        for (index, &append) in chain.iter().enumerate().skip(1).take(chain.len() - 2) {
            method.graph.nodes[append].decorations.concat = Some(ConcatInfo {
                first: index == 1,
                builder: true,
                initial,
                to_string: Some(to_string),
            });
        }
        method.graph.nodes[to_string].decorations.concat = Some(ConcatInfo {
            first: chain.len() == 2,
            builder: false,
            initial,
            to_string: Some(to_string),
        });
    }
    Ok(())
}
fn calls(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    for id in 0..method.graph.nodes.len() {
        context.cancellation.check_cancelled()?;
        let Some(insn) = method.graph.nodes[id].instruction.clone() else {
            continue;
        };
        let InstructionOperand::Member { owner, name, .. } = &insn.operand else {
            continue;
        };
        let ty = match insn.opcode {
            0xb6 | 0xb9 => Some(CallType::Normal),
            0xb8 => Some(CallType::Static),
            0xb7 if name != "<init>" => {
                if owner == &method.class_name {
                    Some(CallType::Normal)
                } else {
                    let receiver = method.graph.nodes[id].inputs.first().copied();
                    if !method.is_static
                        && receiver.is_some_and(|receiver| {
                            method.graph.nodes[receiver]
                                .instruction
                                .as_ref()
                                .is_some_and(|insn| {
                                    insn.opcode == 0x19
                                        && insn.operand == InstructionOperand::Local(0)
                                })
                        })
                    {
                        Some(CallType::Super)
                    } else {
                        None
                    }
                }
            }
            _ => None,
        };
        if ty == Some(CallType::Super) {
            let receiver = method.graph.nodes[id].inputs.remove(0);
            method.graph.nodes[receiver].synthetic = true;
        }
        method.graph.nodes[id].decorations.call_type = ty;
    }
    Ok(())
}
fn lambdas(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let children = all_children(&method.graph);
    for id in 0..children.len() {
        context.cancellation.check_cancelled()?;
        let Some(insn) = method.graph.nodes[id].instruction.clone() else {
            continue;
        };
        let InstructionOperand::Dynamic {
            bootstrap,
            arguments,
            ..
        } = &insn.operand
        else {
            continue;
        };
        let JvmConstant::MethodHandle {
            tag,
            owner,
            name,
            descriptor,
            is_interface,
        } = bootstrap
        else {
            continue;
        };
        let correct_descriptor=match name.as_str(){"metafactory"=>"(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodHandle;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;","altMetafactory"=>"(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;[Ljava/lang/Object;)Ljava/lang/invoke/CallSite;",_=>continue};
        if *tag != 6
            || owner != "java/lang/invoke/LambdaMetafactory"
            || descriptor != correct_descriptor
            || *is_interface
        {
            continue;
        }
        let Some(implementation @ JvmConstant::MethodHandle { tag, owner, .. }) = arguments.get(1)
        else {
            return Err(EngineError::Unavailable(
                "invalid LambdaMetafactory implementation handle".into(),
            ));
        };
        let bound = !method.graph.nodes[id].inputs.is_empty();
        let ty = match *tag {
            8 if !bound => 2,
            7 if owner != &method.class_name => continue,
            5 | 7 | 9 => {
                if bound {
                    1
                } else {
                    0
                }
            }
            6 => 0,
            _ => continue,
        };
        method.graph.nodes[id].decorations.lambda = Some((implementation.clone(), ty));
        if ty == 1 {
            let receiver = method.graph.nodes[id].inputs[0];
            for &(child, input) in &children[receiver] {
                if child == id
                    || input != 0
                    || method.graph.nodes[child].inputs.len() != 1
                    || !children[child].is_empty()
                {
                    continue;
                }
                let synthetic =
                    method.graph.nodes[child]
                        .instruction
                        .as_ref()
                        .is_some_and(|insn| {
                            (insn.opcode == 0xb6
                                && member(
                                    insn,
                                    "java/lang/Object",
                                    "getClass",
                                    Some("()Ljava/lang/Class;"),
                                ))
                                || (insn.opcode == 0xb8
                                    && member(
                                        insn,
                                        "java/util/Objects",
                                        "requireNonNull",
                                        Some("(Ljava/lang/Object;)Ljava/lang/Object;"),
                                    ))
                        });
                if synthetic {
                    method.graph.nodes[child].synthetic = true;
                }
            }
        }
    }
    Ok(())
}
fn split_nodes(method: &mut MethodFlow, context: &MatchContext<'_>) -> EngineResult<()> {
    let children = all_children(&method.graph);
    for (id, next) in children.iter().enumerate() {
        context.cancellation.check_cancelled()?;
        if next.len() <= 1 {
            continue;
        }
        let replacement = method.graph.dummy(method.graph.nodes[id].ty.clone());
        for &(child, input) in next {
            method.graph.nodes[child].inputs[input] = replacement;
        }
    }
    Ok(())
}
