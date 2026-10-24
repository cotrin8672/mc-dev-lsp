use super::flow::CallType;
use super::pool::{matches_field, matches_method};
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
thread_local! {static CANCELLATION:std::cell::RefCell<Vec<Arc<dyn CancellationChecker>>>=std::cell::RefCell::new(Vec::new());}
pub fn current_cancellation() -> Arc<dyn CancellationChecker> {
    static DEFAULT: std::sync::OnceLock<Arc<dyn CancellationChecker>> = std::sync::OnceLock::new();
    CANCELLATION
        .with(|stack| stack.borrow().last().cloned())
        .unwrap_or_else(|| DEFAULT.get_or_init(|| Arc::new(NoCancellation)).clone())
}
pub fn with_cancellation<T>(checker: Arc<dyn CancellationChecker>, block: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            CANCELLATION.with(|stack| {
                stack.borrow_mut().pop();
            });
        }
    }
    CANCELLATION.with(|stack| stack.borrow_mut().push(checker));
    let _guard = Guard;
    block()
}
#[derive(Default)]
struct Sink {
    captured: Vec<(usize, usize)>,
    decorations: BTreeMap<usize, Decorations>,
}
struct Matcher<'a, 'b> {
    method: &'a MethodFlow,
    context: &'b MatchContext<'b>,
    sink: Sink,
}
impl Matcher<'_, '_> {
    fn matches(&mut self, expression: &Expression, id: usize) -> EngineResult<bool> {
        self.context
            .cancellation
            .check_phase(CancellationPhase::Matching)?;
        if let ExprKind::Capture(inner) = &expression.kind {
            let matched = self.matches(inner, id)?;
            if matched {
                self.capture(inner, id)?;
            }
            return Ok(matched);
        }
        if matches!(expression.kind, ExprKind::Wildcard) {
            return Ok(true);
        }
        let node = &self.method.graph.nodes[id];
        if match &expression.kind {
            ExprKind::This => self.method.is_static,
            ExprKind::MethodCall { .. } => node.decorations.call_type != Some(CallType::Normal),
            ExprKind::StaticCall { .. } => node.decorations.call_type != Some(CallType::Static),
            ExprKind::SuperCall { .. } => node.decorations.call_type != Some(CallType::Super),
            ExprKind::New { .. } => node.decorations.instantiation.is_none(),
            ExprKind::ArrayLiteral { .. } => !node.decorations.array_creation,
            ExprKind::BoundMethodReference { .. } => {
                !matches!(node.decorations.lambda, Some((_, 1)))
            }
            ExprKind::FreeMethodReference(_) => !matches!(node.decorations.lambda, Some((_, 0))),
            ExprKind::ConstructorReference(_) => !matches!(node.decorations.lambda, Some((_, 2))),
            _ => false,
        } {
            return Ok(false);
        }
        match &expression.kind {
            ExprKind::Int(_) => {
                if !node.ty.matches("I") && !node.ty.matches("J") {
                    return Ok(false);
                }
            }
            ExprKind::Decimal(_) => {
                if !node.ty.matches("F") && !node.ty.matches("D") {
                    return Ok(false);
                }
            }
            ExprKind::Boolean(_) => {
                if !node.ty.matches("Z") {
                    return Ok(false);
                }
            }
            _ => {}
        }
        let Some(insn) = node.instruction.as_ref() else {
            return Err(EngineError::Unavailable("ComplexDataException".into()));
        };
        let opcode = insn.opcode;
        Ok(match &expression.kind {
            ExprKind::This => {
                !self.method.is_static
                    && opcode == 0x19
                    && insn.operand == InstructionOperand::Local(0)
            }
            ExprKind::Int(expected) => match &insn.operand {
                InstructionOperand::Constant(JvmConstant::Int(value)) => *value as i64 == *expected,
                InstructionOperand::Constant(JvmConstant::Long(value)) => value == expected,
                _ => false,
            },
            ExprKind::Decimal(expected) => match &insn.operand {
                InstructionOperand::Constant(JvmConstant::Float(value)) => {
                    crate::bytecode::constant_label(Some(
                        &crate::bytecode::ConstantValue::FloatValue(*value),
                    ))
                    .trim_end_matches('f')
                        == crate::bytecode::constant_label(Some(
                            &crate::bytecode::ConstantValue::DoubleValue(*expected),
                        ))
                }
                InstructionOperand::Constant(JvmConstant::Double(value)) => {
                    crate::bytecode::constant_label(Some(
                        &crate::bytecode::ConstantValue::DoubleValue(*value),
                    )) == crate::bytecode::constant_label(Some(
                        &crate::bytecode::ConstantValue::DoubleValue(*expected),
                    ))
                }
                _ => false,
            },
            ExprKind::Boolean(expected) => {
                matches!(&insn.operand,InstructionOperand::Constant(JvmConstant::Int(value))if *value==i32::from(*expected))
            }
            ExprKind::Null => opcode == 0x01,
            ExprKind::String(expected) => match &insn.operand {
                InstructionOperand::Constant(JvmConstant::String(value)) => {
                    *value == expected.encode_utf16().collect::<Vec<_>>()
                }
                InstructionOperand::Constant(JvmConstant::Int(value)) if node.ty.matches("C") => {
                    let mut chars = expected.encode_utf16();
                    let first = chars.next();
                    chars.next().is_none() && first.map(i32::from) == Some(*value)
                }
                _ => false,
            },
            ExprKind::StringUtf16(expected) => match &insn.operand {
                InstructionOperand::Constant(JvmConstant::String(value)) => value == expected,
                InstructionOperand::Constant(JvmConstant::Int(value)) if node.ty.matches("C") => {
                    expected.len() == 1 && i32::from(expected[0]) == *value
                }
                _ => false,
            },
            ExprKind::Identifier(name) => {
                matches!(opcode, 0x15..=0x19 | 0xb2)
                    && self.matches_member(&Name::Defined(name.clone()), id)?
            }
            ExprKind::MemberAccess { receiver, name } => {
                matches!(opcode, 0xb4 | 0xbe)
                    && self.matches_member(name, id)?
                    && self.inputs(id, 0, &[receiver.as_ref()], false)?
            }
            ExprKind::ArrayAccess { array, index } => {
                matches!(opcode, 0x2e..=0x35)
                    && self.inputs(id, 0, &[array.as_ref(), index.as_ref()], false)?
            }
            ExprKind::MethodCall {
                receiver,
                name,
                arguments,
            } => {
                if node.decorations.call_type != Some(CallType::Normal)
                    || !self.matches_member(name, id)?
                {
                    false
                } else {
                    let mut values = vec![receiver.as_ref()];
                    values.extend(arguments.iter());
                    self.inputs(id, 0, &values, self.context.allow_incomplete_list_inputs)?
                }
            }
            ExprKind::StaticCall { name, arguments } => {
                node.decorations.call_type == Some(CallType::Static)
                    && self.matches_member(name, id)?
                    && self.inputs(
                        id,
                        0,
                        &arguments.iter().collect::<Vec<_>>(),
                        self.context.allow_incomplete_list_inputs,
                    )?
            }
            ExprKind::SuperCall { name, arguments } => {
                node.decorations.call_type == Some(CallType::Super)
                    && self.matches_member(name, id)?
                    && self.inputs(
                        id,
                        0,
                        &arguments.iter().collect::<Vec<_>>(),
                        self.context.allow_incomplete_list_inputs,
                    )?
            }
            ExprKind::New { target, arguments } => {
                node.decorations.instantiation.is_some()
                    && self
                        .context
                        .identifier_pool
                        .matches_type(target, node.ty.descriptor())?
                    && self.inputs(
                        id,
                        0,
                        &arguments.iter().collect::<Vec<_>>(),
                        self.context.allow_incomplete_list_inputs,
                    )?
            }
            ExprKind::ArrayLiteral { element, values } => {
                node.decorations.array_creation
                    && self
                        .context
                        .identifier_pool
                        .matches_type(element, &self.array_element(insn)?)?
                    && self.inputs(
                        id,
                        0,
                        &values.iter().collect::<Vec<_>>(),
                        self.context.allow_incomplete_list_inputs,
                    )?
            }
            ExprKind::NewArray {
                element,
                dimensions,
                blank_dimensions,
            } => {
                if node.decorations.array_creation || !matches!(opcode, 0xbc | 0xbd | 0xc5) {
                    false
                } else {
                    let descriptor = node.ty.descriptor();
                    let total = descriptor.bytes().take_while(|&c| c == b'[').count();
                    total >= blank_dimensions + dimensions.len()
                        && self
                            .context
                            .identifier_pool
                            .matches_type(element, &descriptor[total..])?
                        && self.inputs(
                            id,
                            0,
                            &dimensions.iter().collect::<Vec<_>>(),
                            self.context.allow_incomplete_list_inputs,
                        )?
                }
            }
            ExprKind::ClassLiteral(target) => {
                if let Some(descriptor) = self.class_literal(insn) {
                    self.context
                        .identifier_pool
                        .matches_type(target, &descriptor)?
                } else {
                    false
                }
            }
            ExprKind::Cast { target, value } => {
                matches!(opcode, 0x85..=0x93 | 0xc0)
                    && self
                        .context
                        .identifier_pool
                        .matches_type(target, node.ty.descriptor())?
                    && self.inputs(id, 0, &[value.as_ref()], false)?
            }
            ExprKind::InstanceOf { value, target } => {
                opcode == 0xc1
                    && matches!(&insn.operand,InstructionOperand::Type(name)if self.context.identifier_pool.matches_type(target,&object_descriptor(name))?)
                    && self.inputs(id, 0, &[value.as_ref()], false)?
            }
            ExprKind::Unary { operator, value } => match operator {
                UnaryOperator::Negate if matches!(opcode, 0x74..=0x77) => {
                    matches!(opcode, 0x74..=0x77) && self.inputs(id, 0, &[value.as_ref()], false)?
                }
                // Official v0.0.6 MINUS falls through to BITWISE_NOT for other opcodes.
                UnaryOperator::Negate | UnaryOperator::BitNot => {
                    matches!(opcode, 0x82 | 0x83)
                        && self.inputs(
                            id,
                            0,
                            &[
                                value.as_ref(),
                                &Expression {
                                    span: expression.span,
                                    kind: ExprKind::Int(-1),
                                },
                            ],
                            false,
                        )?
                }
            },
            ExprKind::Binary {
                left,
                operator,
                right,
            } => {
                if is_comparison(*operator) {
                    self.comparison(id, *operator)?
                        && self.inputs(id, 0, &[left.as_ref(), right.as_ref()], false)?
                } else if binary_opcode(*operator, opcode)
                    && self.inputs(id, 0, &[left.as_ref(), right.as_ref()], false)?
                {
                    true
                } else if *operator == BinaryOperator::Add && node.decorations.concat.is_some() {
                    self.concat(id, left, right)?
                } else {
                    false
                }
            }
            ExprKind::AssignIdentifier { name, value } => {
                matches!(opcode, 0x36..=0x3a | 0xb3)
                    && self.matches_member(name, id)?
                    && self.inputs(id, 0, &[value.as_ref()], false)?
            }
            ExprKind::AssignMember {
                receiver,
                name,
                value,
            } => {
                opcode == 0xb5
                    && self.matches_member(name, id)?
                    && self.inputs(id, 0, &[receiver.as_ref(), value.as_ref()], false)?
            }
            ExprKind::ArrayStore {
                array,
                index,
                value,
            } => {
                matches!(opcode, 0x4f..=0x56)
                    && self.inputs(
                        id,
                        0,
                        &[array.as_ref(), index.as_ref(), value.as_ref()],
                        false,
                    )?
            }
            ExprKind::Return(value) => {
                matches!(opcode, 0xac..=0xb0) && self.inputs(id, 0, &[value.as_ref()], false)?
            }
            ExprKind::Throw(value) => {
                opcode == 0xbf && self.inputs(id, 0, &[value.as_ref()], false)?
            }
            ExprKind::BoundMethodReference { receiver, name } => {
                matches!(node.decorations.lambda, Some((_, 1)))
                    && self.matches_member(name, id)?
                    && if let Some(&input) = node.inputs.first() {
                        self.matches(receiver, input)?
                    } else {
                        false
                    }
            }
            ExprKind::FreeMethodReference(name) => {
                matches!(node.decorations.lambda, Some((_, 0))) && self.matches_member(name, id)?
            }
            ExprKind::ConstructorReference(target) => match &node.decorations.lambda {
                Some((JvmConstant::MethodHandle { owner, .. }, 2)) => self
                    .context
                    .identifier_pool
                    .matches_type(target, &object_descriptor(owner))?,
                _ => false,
            },
            ExprKind::Capture(_) | ExprKind::Wildcard => unreachable!(),
        })
    }
    fn inputs(
        &mut self,
        id: usize,
        start: usize,
        expressions: &[&Expression],
        incomplete: bool,
    ) -> EngineResult<bool> {
        let inputs = self.method.graph.nodes[id].inputs.clone();
        if inputs.len() < start {
            return Ok(false);
        }
        let required = inputs.len() - start;
        if !(incomplete && expressions.len() < required) && expressions.len() != required {
            return Ok(false);
        }
        for (expression, &input) in expressions.iter().zip(&inputs[start..]) {
            if !self.matches(expression, input)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn matches_member(&self, name: &Name, id: usize) -> EngineResult<bool> {
        self.context.cancellation.check_cancelled()?;
        let Name::Defined(name) = name else {
            return Ok(true);
        };
        let pool = self.context.identifier_pool;
        let definitions = pool.members.get(name).ok_or_else(|| {
            EngineError::Unavailable(format!("Use of undeclared identifier '{name}'"))
        })?;
        let node = &self.method.graph.nodes[id];
        let insn = node.instruction.as_ref().unwrap();
        for definition in definitions {
            let matched = match definition {
                MemberDefinition::ArrayLength => insn.opcode == 0xbe,
                MemberDefinition::LocalIndex(slot) => {
                    matches!(insn.operand,InstructionOperand::Local(value)if value as i32==*slot)
                }
                MemberDefinition::Field(selector) => match &insn.operand {
                    InstructionOperand::Member {
                        owner,
                        name,
                        descriptor,
                        ..
                    } if matches!(insn.opcode, 0xb2..=0xb5) => {
                        matches_field(selector, owner, name, descriptor)
                    }
                    _ => false,
                },
                MemberDefinition::Method(selector) => match &node.decorations.lambda {
                    Some((
                        JvmConstant::MethodHandle {
                            tag: 5 | 6 | 7 | 9,
                            owner,
                            name,
                            descriptor,
                            ..
                        },
                        _,
                    )) => matches_method(selector, owner, name, descriptor),
                    Some(_) => false,
                    None => match &insn.operand {
                        InstructionOperand::Member {
                            owner,
                            name,
                            descriptor,
                            ..
                        } if matches!(insn.opcode, 0xb6..=0xb9) => {
                            matches_method(selector, owner, name, descriptor)
                        }
                        _ => false,
                    },
                },
            };
            if matched {
                return Ok(true);
            }
        }
        for definition in pool
            .locals
            .iter()
            .filter(|definition| definition.id == *name)
        {
            if self.matches_local(definition, id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn matches_local(&self, definition: &LocalDefinition, id: usize) -> EngineResult<bool> {
        let node = &self.method.graph.nodes[id];
        let insn = node.instruction.as_ref().unwrap();
        let InstructionOperand::Local(slot) = insn.operand else {
            return Ok(false);
        };
        let position = insn.original_index + usize::from(matches!(insn.opcode, 0x36..=0x3a));
        let Some(frame) = self.method.frames.get(position).and_then(Option::as_ref) else {
            return Ok(false);
        };
        let mut ordinals = BTreeMap::<String, i32>::new();
        let mut candidates = Vec::new();
        let byte = self
            .method
            .instructions
            .get(position)
            .map(|insn| insn.byte_offset)
            .unwrap_or(usize::MAX);
        for (index, &value) in frame.locals.iter().enumerate() {
            self.context.cancellation.check_cancelled()?;
            let ty = self.method.graph.nodes[value].ty.descriptor();
            if matches!(ty, "V" | "Lnull;" | "Lreturn-address;") {
                continue;
            }
            let debug = self
                .method
                .local_variables
                .iter()
                .rev()
                .find(|local| local.slot == index && local.start <= byte && byte < local.end);
            let descriptor = debug
                .map(|local| local.descriptor.as_str())
                .unwrap_or(if ty == "Lint-like;" { "I" } else { ty });
            let ordinal = *ordinals.entry(descriptor.into()).or_default();
            *ordinals.get_mut(descriptor).unwrap() += 1;
            if descriptor == definition.descriptor
                && (!definition.spec.args_only || self.method.argument_slots.contains(&index))
            {
                candidates.push((index, ordinal, debug.map(|local| local.name.as_str())));
            }
        }
        let selected =
            if let Some(ordinal) = definition.spec.ordinal.filter(|ordinal| *ordinal >= 0) {
                candidates.iter().find(|candidate| candidate.1 == ordinal)
            } else if let Some(index) = definition.spec.index.filter(|index| *index >= 0) {
                candidates
                    .iter()
                    .find(|candidate| candidate.0 == index as usize)
            } else if !definition.spec.names.is_empty() {
                candidates.iter().find(|candidate| {
                    candidate
                        .2
                        .is_some_and(|name| definition.spec.names.contains(name))
                })
            } else if candidates.len() == 1 {
                candidates.first()
            } else {
                None
            };
        Ok(selected.is_some_and(|candidate| candidate.0 == slot))
    }
    fn class_literal(&self, insn: &Instruction) -> Option<String> {
        match &insn.operand {
            InstructionOperand::Constant(JvmConstant::Class(name)) => Some(object_descriptor(name)),
            InstructionOperand::Constant(JvmConstant::MethodType(descriptor)) => {
                Some(descriptor.clone())
            }
            InstructionOperand::Member {
                owner,
                name,
                descriptor,
                ..
            } if insn.opcode == 0xb2 && name == "TYPE" && descriptor == "Ljava/lang/Class;" => {
                Some(
                    match owner.as_str() {
                        "java/lang/Boolean" => "Z",
                        "java/lang/Character" => "C",
                        "java/lang/Byte" => "B",
                        "java/lang/Short" => "S",
                        "java/lang/Integer" => "I",
                        "java/lang/Float" => "F",
                        "java/lang/Long" => "J",
                        "java/lang/Double" => "D",
                        _ => return None,
                    }
                    .into(),
                )
            }
            _ => None,
        }
    }
    fn array_element(&self, insn: &Instruction) -> EngineResult<String> {
        match &insn.operand {
            InstructionOperand::Type(name) => Ok(object_descriptor(name)),
            InstructionOperand::NewArray(atype) => Ok(match atype {
                4 => "Z",
                5 => "C",
                6 => "F",
                7 => "D",
                8 => "B",
                9 => "S",
                10 => "I",
                11 => "J",
                _ => return Err(EngineError::Unavailable("invalid newarray type".into())),
            }
            .into()),
            _ => Err(EngineError::Unavailable(
                "array creation has no element type".into(),
            )),
        }
    }
    fn concat(&mut self, id: usize, left: &Expression, right: &Expression) -> EngineResult<bool> {
        let info = self.method.graph.nodes[id]
            .decorations
            .concat
            .as_ref()
            .unwrap()
            .clone();
        let current = if info.to_string == Some(id) {
            self.method.graph.nodes[id].inputs[0]
        } else {
            id
        };
        let inputs = &self.method.graph.nodes[current].inputs;
        if inputs.len() != 2 {
            return Ok(false);
        }
        let (left_input, right_input) = (inputs[0], inputs[1]);
        if !self.matches(right, right_input)? {
            return Ok(false);
        }
        if info.first {
            return self.matches(left, info.initial);
        }
        let mut inner = left;
        while let ExprKind::Capture(value) = &inner.kind {
            inner = value;
        }
        if matches!(inner.kind, ExprKind::Wildcard) {
            if matches!(left.kind, ExprKind::Capture(_)) {
                self.supports_concat()?;
                self.sink.decorations.entry(left_input).or_default().insert(
                    DecorationKey::IsStringConcatExpression,
                    DecorationValue::Flag(true),
                );
            }
            self.matches(left, left_input)
        } else if matches!(
            inner.kind,
            ExprKind::Binary {
                operator: BinaryOperator::Add,
                ..
            }
        ) {
            self.matches(left, left_input)
        } else {
            Ok(false)
        }
    }
    fn supports_concat(&self) -> EngineResult<()> {
        if matches!(
            self.context.context_type,
            MatchContextType::Slice
                | MatchContextType::Inject
                | MatchContextType::ModifyVariable
                | MatchContextType::ModifyExpressionValue
        ) {
            Ok(())
        } else {
            Err(EngineError::Unavailable(format!(
                "Expression context type {} does not support string concat!",
                self.context.context_type.as_official_name()
            )))
        }
    }
    fn comparison(&mut self, id: usize, operator: BinaryOperator) -> EngineResult<bool> {
        let node = &self.method.graph.nodes[id];
        if node.inputs.len() != 2 {
            return Ok(false);
        }
        let opcode = node.instruction.as_ref().unwrap().opcode;
        let (direct, inverted, object) = match operator {
            BinaryOperator::Equal => (0x9f, 0xa0, Some((0xa5, 0xa6))),
            BinaryOperator::NotEqual => (0xa0, 0x9f, Some((0xa6, 0xa5))),
            // v0.0.6 uses opcode zero as the unsupported object-comparison sentinel.
            // Expanded concat NOPs consequently retain this observable matching behavior.
            BinaryOperator::Less => (0xa1, 0xa2, Some((0, 0))),
            BinaryOperator::LessEqual => (0xa4, 0xa3, Some((0, 0))),
            BinaryOperator::Greater => (0xa3, 0xa4, Some((0, 0))),
            BinaryOperator::GreaterEqual => (0xa2, 0xa1, Some((0, 0))),
            _ => return Ok(false),
        };
        let ty = if object.is_some_and(|(a, b)| opcode == a || opcode == b) {
            TypeConstraint::exact("Ljava/lang/Object;")
        } else if opcode == direct || opcode == inverted {
            super::flow::common_type(
                &unbox(&self.method.graph.nodes[node.inputs[0]].ty),
                &unbox(&self.method.graph.nodes[node.inputs[1]].ty),
                self.context,
            )?
        } else if matches!(opcode, 0x94..=0x98) {
            let allowed = match operator {
                BinaryOperator::Less | BinaryOperator::LessEqual => {
                    matches!(opcode, 0x94 | 0x96 | 0x98)
                }
                BinaryOperator::Greater | BinaryOperator::GreaterEqual => {
                    matches!(opcode, 0x94 | 0x95 | 0x97)
                }
                _ => true,
            };
            if !allowed {
                return Ok(false);
            }
            let Some(jump) = node.decorations.complex_comparison_jump else {
                return Err(EngineError::Unavailable(
                    "complex comparison has no jump".into(),
                ));
            };
            let jump = self.method.graph.nodes[jump]
                .instruction
                .as_ref()
                .unwrap()
                .opcode;
            if jump != direct - 6 && jump != inverted - 6 {
                return Ok(false);
            }
            TypeConstraint::exact(match opcode {
                0x94 => "J",
                0x95 | 0x96 => "F",
                _ => "D",
            })
        } else {
            return Ok(false);
        };
        self.operation_decorations(
            id,
            vec![ty.clone(), ty],
            TypeConstraint::exact("Z"),
            &["left", "right"],
        );
        Ok(true)
    }
    fn operation_decorations(
        &mut self,
        id: usize,
        args: Vec<TypeConstraint>,
        result: TypeConstraint,
        names: &[&str],
    ) {
        let decorations = self.sink.decorations.entry(id).or_default();
        decorations.insert(
            DecorationKey::SimpleOperationArgs,
            DecorationValue::TypeConstraints(args),
        );
        decorations.insert(
            DecorationKey::SimpleOperationReturnType,
            DecorationValue::TypeConstraint(result),
        );
        decorations.insert(
            DecorationKey::SimpleOperationParamNames,
            DecorationValue::ParamNames(names.iter().map(|name| (*name).into()).collect()),
        );
    }
    fn capture(&mut self, expression: &Expression, mut id: usize) -> EngineResult<()> {
        self.context.cancellation.check_cancelled()?;
        let node = &self.method.graph.nodes[id];
        match &expression.kind {
            ExprKind::New { .. } => {
                if self.context.context_type == MatchContextType::Redirect {
                    return Err(EngineError::Unavailable("Factory redirects are not supported with expressions! Either switch to @WrapOperation or use the standard NEW injection point.".into()));
                }
                if matches!(
                    self.context.context_type,
                    MatchContextType::ModifyArg | MatchContextType::ModifyArgs
                ) {
                    id = node.decorations.instantiation.unwrap();
                }
            }
            ExprKind::Binary { operator, .. } if is_comparison(*operator) => {
                self.sink.decorations.entry(id).or_default().insert(
                    DecorationKey::SimpleExpressionType,
                    DecorationValue::TypeConstraint(TypeConstraint::exact("Z")),
                );
            }
            ExprKind::ArrayAccess { .. } => self.operation_decorations(
                id,
                vec![
                    self.method.graph.nodes[node.inputs[0]].ty.clone(),
                    TypeConstraint::exact("I"),
                ],
                node.ty.clone(),
                &["array", "index"],
            ),
            ExprKind::ArrayStore { .. } => {
                let array = self.method.graph.nodes[node.inputs[0]].ty.clone();
                let inner = array.descriptor().strip_prefix('[').unwrap_or("Lnull;");
                self.operation_decorations(
                    id,
                    vec![
                        array.clone(),
                        TypeConstraint::exact("I"),
                        TypeConstraint::exact(inner),
                    ],
                    TypeConstraint::exact("V"),
                    &["array", "index", "value"],
                );
            }
            ExprKind::Cast { .. }
                if node
                    .instruction
                    .as_ref()
                    .is_some_and(|insn| insn.opcode == 0xc0) =>
            {
                self.operation_decorations(
                    id,
                    vec![TypeConstraint::exact("Ljava/lang/Object;")],
                    node.ty.clone(),
                    &["object"],
                )
            }
            ExprKind::MemberAccess { .. }
                if node
                    .instruction
                    .as_ref()
                    .is_some_and(|insn| insn.opcode == 0xbe) =>
            {
                self.operation_decorations(
                    id,
                    vec![self.method.graph.nodes[node.inputs[0]].ty.clone()],
                    TypeConstraint::exact("I"),
                    &["array", "index"],
                )
            }
            ExprKind::Binary { .. } if node.decorations.concat.is_some() => {
                self.supports_concat()?;
                if node.decorations.concat.as_ref().unwrap().builder {
                    self.sink.decorations.entry(id).or_default().insert(
                        DecorationKey::IsStringConcatExpression,
                        DecorationValue::Flag(true),
                    );
                }
            }
            _ => {}
        }
        if is_simple(&expression.kind) {
            let ty = self.method.graph.nodes[id].ty.clone();
            if ty.descriptor() != "V" {
                if self.method.graph.nodes[id].instruction.is_none() {
                    return Err(EngineError::Unavailable("ComplexDataException".into()));
                }
                self.sink.decorations.entry(id).or_default().insert(
                    DecorationKey::SimpleExpressionType,
                    DecorationValue::TypeConstraint(if ty.descriptor() == "Lnull;" {
                        TypeConstraint::exact("Ljava/lang/Object;")
                    } else {
                        ty
                    }),
                );
            }
        }
        self.sink.captured.push((id, expression.span.start));
        Ok(())
    }
}
fn unbox(ty: &TypeConstraint) -> TypeConstraint {
    TypeConstraint::exact(match ty.descriptor() {
        "Ljava/lang/Boolean;" => "Z",
        "Ljava/lang/Character;" => "C",
        "Ljava/lang/Byte;" => "B",
        "Ljava/lang/Short;" => "S",
        "Ljava/lang/Integer;" => "I",
        "Ljava/lang/Float;" => "F",
        "Ljava/lang/Long;" => "J",
        "Ljava/lang/Double;" => "D",
        other => other,
    })
}
fn object_descriptor(name: &str) -> String {
    if name.starts_with('[') {
        name.into()
    } else {
        format!("L{name};")
    }
}
fn is_comparison(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual
    )
}
fn binary_opcode(operator: BinaryOperator, opcode: u8) -> bool {
    use BinaryOperator::*;
    match operator {
        Multiply => (0x68..=0x6b).contains(&opcode),
        Divide => (0x6c..=0x6f).contains(&opcode),
        Modulo => (0x70..=0x73).contains(&opcode),
        Add => (0x60..=0x63).contains(&opcode),
        Subtract => (0x64..=0x67).contains(&opcode),
        ShiftLeft => matches!(opcode, 0x78 | 0x79),
        ShiftRight => matches!(opcode, 0x7a | 0x7b),
        UnsignedShiftRight => matches!(opcode, 0x7c | 0x7d),
        BitAnd => matches!(opcode, 0x7e | 0x7f),
        BitXor => matches!(opcode, 0x82 | 0x83),
        BitOr => matches!(opcode, 0x80 | 0x81),
        _ => false,
    }
}
fn is_simple(kind: &ExprKind) -> bool {
    !matches!(
        kind,
        ExprKind::New { .. }
            | ExprKind::AssignIdentifier { .. }
            | ExprKind::AssignMember { .. }
            | ExprKind::ArrayStore { .. }
            | ExprKind::Return(_)
            | ExprKind::Throw(_)
    ) && !matches!(kind,ExprKind::Binary{operator,..}if is_comparison(*operator))
}
fn run_match(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    expressions: &[String],
    context: &MatchContext<'_>,
    keep_components: bool,
) -> EngineResult<Vec<ExpressionMatch>> {
    let mut parsed = Vec::new();
    for source in expressions {
        context.cancellation.check_cancelled()?;
        parsed.push(parse_expression(source).map_err(|failure| {
            EngineError::Unavailable(format!("Failed to parse expression: {failure}"))
        })?);
    }
    run_parsed_match(bytes, name, descriptor, &parsed, context, keep_components)
}
fn run_parsed_match(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    parsed: &[Expression],
    context: &MatchContext<'_>,
    keep_components: bool,
) -> EngineResult<Vec<ExpressionMatch>> {
    let mut method =
        super::flow::analyze(bytes, name, descriptor, context).map_err(analysis_error)?;
    super::postprocess::finish(&mut method, context).map_err(analysis_error)?;
    collect_matches_inner(&method, parsed, context, !keep_components)
}
fn collect_matches(
    method: &MethodFlow,
    parsed: &[Expression],
    context: &MatchContext<'_>,
) -> EngineResult<Vec<ExpressionMatch>> {
    collect_matches_inner(method, parsed, context, true)
}
fn collect_matches_inner(
    method: &MethodFlow,
    parsed: &[Expression],
    context: &MatchContext<'_>,
    deduplicate: bool,
) -> EngineResult<Vec<ExpressionMatch>> {
    let mut entries = method
        .graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| !node.synthetic && node.instruction.is_some())
        .map(|(id, node)| (node.instruction.as_ref().unwrap().original_index, id))
        .collect::<Vec<_>>();
    entries.sort_unstable();
    if entries.is_empty() {
        return Err(EngineError::Unavailable(format!(
            "Failed to build flow map for method: {}{}",
            method.method_name, method.descriptor
        )));
    }
    let mut matches = Vec::new();
    let mut seen = BTreeSet::new();
    for (expression_index, expression) in parsed.iter().enumerate() {
        for &(_, root) in &entries {
            context.cancellation.check_cancelled()?;
            let mut matcher = Matcher {
                method: &method,
                context,
                sink: Sink::default(),
            };
            let matched = match matcher.matches(expression, root) {
                Ok(value) => value,
                Err(EngineError::Unavailable(reason)) => {
                    context
                        .cancellation
                        .check_phase(CancellationPhase::MatchFailureRecheck)?;
                    return Err(EngineError::Unavailable(format!(
                        "Failed to match expression at index {expression_index}: {reason}"
                    )));
                }
                Err(error) => return Err(error),
            };
            if !matched {
                continue;
            }
            for (id, offset) in matcher.sink.captured {
                context.cancellation.check_cancelled()?;
                let node = &method.graph.nodes[id];
                let Some(virtual_insn) = node.instruction.as_ref() else {
                    continue;
                };
                let physical = &method.instructions[virtual_insn.original_index];
                if !seen.insert((expression_index, physical.original_index, offset)) && deduplicate
                {
                    continue;
                }
                matches.push(ExpressionMatch {
                    expression_index,
                    expression_start_offset: offset,
                    original_instruction_opcode: physical.opcode,
                    original_instruction_index: physical.original_index,
                    captured_type: node.ty.clone(),
                    decorations: matcher
                        .sink
                        .decorations
                        .get(&id)
                        .cloned()
                        .unwrap_or_default(),
                    instruction_metadata: physical.metadata(),
                });
            }
        }
    }
    matches.sort_by_key(|value| {
        (
            value.expression_index,
            value.original_instruction_index,
            value.expression_start_offset,
            value.original_instruction_opcode,
        )
    });
    Ok(matches)
}
pub fn match_expressions(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    expressions: &[String],
    context: &MatchContext<'_>,
) -> Result<MatchResult, Cancellation> {
    match run_match(bytes, name, descriptor, expressions, context, false) {
        Ok(matches) => Ok(MatchResult::Available(matches)),
        Err(EngineError::Unavailable(reason)) => Ok(MatchResult::Unavailable {
            cause: failure_cause(&reason),
            reason: failure_reason(reason),
        }),
        Err(EngineError::Cancelled(error)) => Err(error),
    }
}
/// Returns every matching virtual component before physical-instruction deduplication.
/// Normal matching selects the original node first, followed by expansion insertion
/// order; this inspection API makes that deterministic selection reviewable.
pub fn match_expression_components(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    expressions: &[String],
    context: &MatchContext<'_>,
) -> Result<MatchResult, Cancellation> {
    match run_match(bytes, name, descriptor, expressions, context, true) {
        Ok(matches) => Ok(MatchResult::Available(matches)),
        Err(EngineError::Unavailable(reason)) => Ok(MatchResult::Unavailable {
            cause: failure_cause(&reason),
            reason: failure_reason(reason),
        }),
        Err(EngineError::Cancelled(error)) => Err(error),
    }
}
pub fn match_expressions_utf16(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    expressions: &[Vec<u16>],
    context: &MatchContext<'_>,
) -> Result<MatchResult, Cancellation> {
    let result = (|| {
        let mut parsed = Vec::new();
        for source in expressions {
            context.cancellation.check_cancelled()?;
            parsed.push(parse_expression_utf16(source).map_err(|failure| {
                EngineError::Unavailable(format!("Failed to parse expression: {failure}"))
            })?);
        }
        run_parsed_match(bytes, name, descriptor, &parsed, context, false)
    })();
    match result {
        Ok(matches) => Ok(MatchResult::Available(matches)),
        Err(EngineError::Unavailable(reason)) => Ok(MatchResult::Unavailable {
            cause: failure_cause(&reason),
            reason: failure_reason(reason),
        }),
        Err(EngineError::Cancelled(error)) => Err(error),
    }
}
fn failure_cause(reason: &str) -> Option<String> {
    if reason.starts_with("Method not found:")
        || reason.starts_with("Method has no instructions:")
        || reason.starts_with("Failed to build flow map for method:")
    {
        None
    } else {
        Some(reason.into())
    }
}
fn failure_reason(reason: String) -> String {
    if reason.starts_with("Failed to parse class bytecode:") {
        "Failed to parse class bytecode".into()
    } else if reason.starts_with("Failed to analyze method bytecode flow:") {
        "Failed to analyze method bytecode flow".into()
    } else {
        reason
    }
}
fn analysis_error(error: EngineError) -> EngineError {
    match error {
        EngineError::Unavailable(reason)
            if !reason.starts_with("Failed to parse class bytecode")
                && !reason.starts_with("Method not found:")
                && !reason.starts_with("Method has no instructions:")
                && !reason.starts_with("Could not resolve common super class") =>
        {
            EngineError::Unavailable(format!("Failed to analyze method bytecode flow: {reason}"))
        }
        other => other,
    }
}
pub fn receiver_probes(receiver: &str, pool: &IdentifierPool) -> Option<Vec<(MemberKind, String)>> {
    receiver_probes_utf16(&receiver.encode_utf16().collect::<Vec<_>>(), pool).map(|probes| {
        probes
            .into_iter()
            .map(|(kind, source)| {
                (
                    kind,
                    String::from_utf16(&source)
                        .expect("UTF-8 receiver produces valid UTF-16 probes"),
                )
            })
            .collect()
    })
}
fn receiver_whitespace(ch: char) -> bool {
    (ch.is_whitespace() && ch != '\u{85}') || ('\u{1c}'..='\u{1f}').contains(&ch)
}
pub fn is_valid_receiver_expression(receiver: &str) -> bool {
    is_valid_receiver_expression_utf16(&receiver.encode_utf16().collect::<Vec<_>>())
}
pub fn is_valid_receiver_expression_utf16(receiver: &[u16]) -> bool {
    let source = super::parser::Utf16Source::new(receiver);
    let receiver = source.text.trim_matches(receiver_whitespace);
    if receiver == "super" {
        return true;
    }
    if receiver.is_empty()
        || matches!(receiver, "new" | "true" | "false" | "null")
        || receiver.contains("::")
        || receiver
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '\'' || ch == '"')
    {
        return false;
    }
    let leading = source.text.len() - source.text.trim_start_matches(receiver_whitespace).len();
    let mut units = "@(".encode_utf16().collect::<Vec<_>>();
    for (byte, ch) in receiver.char_indices() {
        source.append(&mut units, ch, leading + byte);
    }
    units.extend(".?)".encode_utf16());
    parse_expression_utf16(&units).is_ok()
}
pub fn receiver_probes_utf16(
    receiver: &[u16],
    pool: &IdentifierPool,
) -> Option<Vec<(MemberKind, Vec<u16>)>> {
    if !is_valid_receiver_expression_utf16(receiver) {
        return None;
    }
    let source = super::parser::Utf16Source::new(receiver);
    let receiver = source.text.trim_matches(receiver_whitespace);
    if receiver == "super" {
        return Some(vec![(
            MemberKind::Method,
            "@(super.?())".encode_utf16().collect(),
        )]);
    }
    if receiver.is_empty() {
        return None;
    }
    let leading = source.text.len() - source.text.trim_start_matches(receiver_whitespace).len();
    let mut result = Vec::new();
    let mut chars = receiver.char_indices().peekable();
    while let Some((byte, ch)) = chars.next() {
        if ch == '\'' || ch == '"' {
            source.append(&mut result, ch, leading + byte);
            let mut closed = false;
            while let Some((byte, next)) = chars.next() {
                source.append(&mut result, next, leading + byte);
                if next == '\\' {
                    if let Some((byte, escaped)) = chars.next() {
                        source.append(&mut result, escaped, leading + byte);
                    } else {
                        break;
                    }
                } else if next == ch {
                    closed = true;
                    break;
                }
            }
            if !closed {
                return None;
            }
        } else if ch.is_alphabetic() || matches!(ch, '_' | '$') {
            let mut word = String::from(ch);
            while chars
                .peek()
                .is_some_and(|(_, ch)| ch.is_alphanumeric() || matches!(ch, '_' | '$'))
            {
                word.push(chars.next().unwrap().1);
            }
            if pool.identifier_exists(&word)
                || matches!(
                    word.as_str(),
                    "new"
                        | "true"
                        | "false"
                        | "null"
                        | "this"
                        | "super"
                        | "instanceof"
                        | "return"
                        | "throw"
                )
            {
                result.extend(word.encode_utf16());
            } else {
                result.push(b'?' as u16);
            }
        } else {
            source.append(&mut result, ch, leading + byte);
        }
    }
    let wrap = |suffix: &str| {
        "@(("
            .encode_utf16()
            .chain(result.iter().copied())
            .chain(suffix.encode_utf16())
            .collect()
    };
    Some(vec![
        (MemberKind::Field, wrap(").?)")),
        (MemberKind::Method, wrap(").?())")),
    ])
}
pub fn complete_receiver_members(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    receiver: &str,
    prefix: &str,
    context: &MatchContext<'_>,
) -> Result<MemberCompletionResult, Cancellation> {
    context.cancellation.check_cancelled()?;
    let probes = receiver_probes_utf16(
        &receiver.encode_utf16().collect::<Vec<_>>(),
        context.identifier_pool,
    );
    complete_receiver_members_inner(bytes, name, descriptor, receiver, prefix, context, probes)
}
pub fn complete_receiver_members_utf16(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    receiver: &[u16],
    prefix: &str,
    context: &MatchContext<'_>,
) -> Result<MemberCompletionResult, Cancellation> {
    context.cancellation.check_cancelled()?;
    let probes = receiver_probes_utf16(receiver, context.identifier_pool);
    let display = String::from_utf16(receiver).unwrap_or_else(|_| {
        let quoted = crate::bytecode::java_string_literal_utf16(receiver);
        quoted[1..quoted.len() - 1].to_owned()
    });
    complete_receiver_members_inner(bytes, name, descriptor, &display, prefix, context, probes)
}
fn complete_receiver_members_inner(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    receiver: &str,
    prefix: &str,
    context: &MatchContext<'_>,
    probes: Option<Vec<(MemberKind, Vec<u16>)>>,
) -> Result<MemberCompletionResult, Cancellation> {
    let Some(probes) = probes else {
        return Ok(MemberCompletionResult::Unavailable {
            reason: format!(
                "Receiver expression cannot be represented in probe syntax: {receiver}"
            ),
            cause: None,
        });
    };
    let mut parsed = Vec::new();
    for (kind, source) in probes {
        context.cancellation.check_cancelled()?;
        match parse_expression_utf16(&source) {
            Ok(expression) => parsed.push((kind, expression)),
            Err(error) => {
                return Ok(MemberCompletionResult::Unavailable {
                    reason: format!("Failed to parse receiver probe expression: {error}"),
                    cause: Some(error.to_string()),
                })
            }
        }
    }
    let method = match super::flow::analyze(bytes, name, descriptor, context)
        .and_then(|mut method| {
            super::postprocess::finish(&mut method, context)?;
            Ok(method)
        })
        .map_err(analysis_error)
    {
        Ok(method) => method,
        Err(EngineError::Cancelled(error)) => return Err(error),
        Err(EngineError::Unavailable(reason)) => {
            return Ok(MemberCompletionResult::Unavailable {
                cause: failure_cause(&reason),
                reason: failure_reason(reason),
            })
        }
    };
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for (kind, expression) in parsed {
        context.cancellation.check_cancelled()?;
        match collect_matches(&method, &[expression], context) {
            Err(EngineError::Cancelled(error)) => return Err(error),
            Err(EngineError::Unavailable(reason)) => {
                let detail = reason
                    .strip_prefix("Failed to match expression at index 0: ")
                    .unwrap_or(&reason);
                return Ok(MemberCompletionResult::Unavailable {
                    reason: format!(
                        "Failed to match {} receiver probe: {detail}",
                        if kind == MemberKind::Field {
                            "field"
                        } else {
                            "method"
                        }
                    ),
                    cause: Some(reason),
                });
            }
            Ok(matches) => {
                for matched in matches {
                    let candidate = match matched.instruction_metadata {
                        Some(InstructionMetadata::FieldAccess {
                            owner_internal_name,
                            name,
                            descriptor,
                        }) if kind == MemberKind::Field => Some(MemberCandidate {
                            kind,
                            owner_internal_name,
                            name,
                            descriptor,
                            is_interface: false,
                            original_instruction_opcode: matched.original_instruction_opcode,
                            original_instruction_index: matched.original_instruction_index,
                            decorations: matched.decorations,
                        }),
                        Some(InstructionMetadata::MethodInvocation {
                            owner_internal_name,
                            name,
                            descriptor,
                            is_interface,
                        }) if kind == MemberKind::Method => Some(MemberCandidate {
                            kind,
                            owner_internal_name,
                            name,
                            descriptor,
                            is_interface,
                            original_instruction_opcode: matched.original_instruction_opcode,
                            original_instruction_index: matched.original_instruction_index,
                            decorations: matched.decorations,
                        }),
                        _ => None,
                    };
                    if let Some(candidate) = candidate {
                        if candidate.name.starts_with(prefix)
                            && seen.insert((
                                kind,
                                candidate.owner_internal_name.clone(),
                                candidate.name.clone(),
                                candidate.descriptor.clone(),
                            ))
                        {
                            candidates.push(candidate);
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        (
            a.original_instruction_index,
            a.original_instruction_opcode,
            a.kind,
            &a.name,
            &a.descriptor,
        )
            .cmp(&(
                b.original_instruction_index,
                b.original_instruction_opcode,
                b.kind,
                &b.name,
                &b.descriptor,
            ))
    });
    Ok(MemberCompletionResult::Available(candidates))
}
