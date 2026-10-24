use super::*;
use crate::descriptor::{parse_method_descriptor, to_descriptor};
use noak::reader::{
    attributes::{BootstrapMethods, Code, RawInstruction},
    cpool, Class,
};
use noak::AccessFlags;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub enum JvmConstant {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(Vec<u16>),
    Class(String),
    MethodType(String),
    MethodHandle {
        tag: u8,
        owner: String,
        name: String,
        descriptor: String,
        is_interface: bool,
    },
    Null,
}
#[derive(Debug, Clone, PartialEq)]
pub enum InstructionOperand {
    None,
    Local(usize),
    Increment {
        slot: usize,
        amount: i32,
    },
    Constant(JvmConstant),
    Member {
        owner: String,
        name: String,
        descriptor: String,
        is_interface: bool,
    },
    Type(String),
    NewArray(u8),
    MultiArray {
        descriptor: String,
        dimensions: usize,
    },
    Jump(usize),
    Switch(Vec<usize>),
    Dynamic {
        name: String,
        descriptor: String,
        bootstrap: JvmConstant,
        arguments: Vec<JvmConstant>,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct Instruction {
    pub opcode: u8,
    pub original_opcode: u8,
    pub original_index: usize,
    pub byte_offset: usize,
    pub operand: InstructionOperand,
}
impl Instruction {
    pub fn metadata(&self) -> Option<InstructionMetadata> {
        match &self.operand {
            InstructionOperand::Member {
                owner,
                name,
                descriptor,
                is_interface,
            } => Some(if matches!(self.original_opcode, 0xb2..=0xb5) {
                InstructionMetadata::FieldAccess {
                    owner_internal_name: owner.clone(),
                    name: name.clone(),
                    descriptor: descriptor.clone(),
                }
            } else {
                InstructionMetadata::MethodInvocation {
                    owner_internal_name: owner.clone(),
                    name: name.clone(),
                    descriptor: descriptor.clone(),
                    is_interface: *is_interface,
                }
            }),
            InstructionOperand::Type(value) => Some(InstructionMetadata::TypeOperation {
                type_internal_name: value.clone(),
            }),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariable {
    pub slot: usize,
    pub name: String,
    pub descriptor: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallType {
    Normal,
    Static,
    Super,
}
#[derive(Debug, Clone)]
pub(crate) struct ConcatInfo {
    pub first: bool,
    pub builder: bool,
    pub initial: usize,
    pub to_string: Option<usize>,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct FlowDecorations {
    pub array_creation: bool,
    pub instantiation: Option<usize>,
    pub complex_comparison_jump: Option<usize>,
    pub concat: Option<ConcatInfo>,
    pub call_type: Option<CallType>,
    pub lambda: Option<(JvmConstant, u8)>,
}
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub instruction: Option<Instruction>,
    pub ty: TypeConstraint,
    pub inputs: Vec<usize>,
    pub synthetic: bool,
    pub(crate) decorations: FlowDecorations,
    pub(crate) return_targets: BTreeSet<usize>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub locals: Vec<usize>,
    pub stack: Vec<usize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalysisPolicy {
    #[default]
    OfficialExpression,
    ExactLocalCapture,
}
#[derive(Debug, Clone, Default)]
pub struct FlowGraph {
    pub nodes: Vec<FlowNode>,
    pub(crate) by_instruction: BTreeMap<usize, usize>,
    dummies: BTreeMap<(TypeConstraint, BTreeSet<usize>), usize>,
    analysis_policy: AnalysisPolicy,
}
impl FlowGraph {
    pub(crate) fn dummy(&mut self, ty: TypeConstraint) -> usize {
        self.dummy_returns(ty, BTreeSet::new())
    }
    fn dummy_returns(&mut self, ty: TypeConstraint, returns: BTreeSet<usize>) -> usize {
        let ty = self.normalize_type(ty, None);
        let key = (ty.clone(), returns.clone());
        if let Some(&id) = self.dummies.get(&key) {
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(FlowNode {
            instruction: None,
            ty,
            inputs: Vec::new(),
            synthetic: true,
            decorations: FlowDecorations::default(),
            return_targets: returns,
        });
        self.dummies.insert(key, id);
        id
    }
    pub(crate) fn add(
        &mut self,
        instruction: Instruction,
        ty: TypeConstraint,
        inputs: Vec<usize>,
    ) -> usize {
        let ty = self.normalize_type(ty, Some(instruction.opcode));
        let id = self.nodes.len();
        self.nodes.push(FlowNode {
            instruction: Some(instruction),
            ty,
            inputs,
            synthetic: false,
            decorations: FlowDecorations::default(),
            return_targets: BTreeSet::new(),
        });
        id
    }
    fn record(
        &mut self,
        instruction: &Instruction,
        ty: TypeConstraint,
        inputs: Vec<usize>,
        context: &MatchContext<'_>,
    ) -> EngineResult<usize> {
        let ty = self.normalize_type(ty, Some(instruction.opcode));
        if let Some(&id) = self.by_instruction.get(&instruction.original_index) {
            if self.nodes[id].inputs.len() != inputs.len() {
                return Err(EngineError::Unavailable(
                    "inconsistent input arity at flow merge".into(),
                ));
            }
            for (index, input) in inputs.into_iter().enumerate() {
                let existing = self.nodes[id].inputs[index];
                self.nodes[id].inputs[index] = self.merge(existing, input, context)?;
            }
            self.nodes[id].ty = ty;
            Ok(id)
        } else {
            let id = self.add(instruction.clone(), ty, inputs);
            self.by_instruction.insert(instruction.original_index, id);
            Ok(id)
        }
    }
    fn merge(
        &mut self,
        left: usize,
        right: usize,
        context: &MatchContext<'_>,
    ) -> EngineResult<usize> {
        context.cancellation.check_cancelled()?;
        if left == right {
            return Ok(left);
        }
        let ty = if self.analysis_policy == AnalysisPolicy::ExactLocalCapture {
            if self.nodes[left].ty == self.nodes[right].ty {
                self.nodes[left].ty.clone()
            } else {
                TypeConstraint::exact("V")
            }
        } else {
            common_type(&self.nodes[left].ty, &self.nodes[right].ty, context)?
        };
        let returns = self.nodes[left]
            .return_targets
            .union(&self.nodes[right].return_targets)
            .copied()
            .collect();
        Ok(self.dummy_returns(ty, returns))
    }
    fn normalize_type(&self, ty: TypeConstraint, opcode: Option<u8>) -> TypeConstraint {
        if self.analysis_policy == AnalysisPolicy::ExactLocalCapture {
            if opcode == Some(0x32) {
                return TypeConstraint::exact("Ljava/lang/Object;");
            }
            if is_int_like(ty.descriptor()) {
                return TypeConstraint::exact("I");
            }
        }
        ty
    }
    pub(crate) fn children(&self, id: usize) -> Vec<(usize, usize)> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| !node.synthetic)
            .flat_map(|(child, node)| {
                node.inputs
                    .iter()
                    .enumerate()
                    .filter_map(move |(input, &parent)| (parent == id).then_some((child, input)))
            })
            .collect()
    }
}
#[derive(Debug, Clone)]
pub struct MethodFlow {
    pub class_name: String,
    pub method_name: String,
    pub descriptor: String,
    pub is_static: bool,
    pub max_stack: usize,
    pub instructions: Vec<Instruction>,
    pub graph: FlowGraph,
    pub frames: Vec<Option<Frame>>,
    pub local_variables: Vec<LocalVariable>,
    pub argument_slots: BTreeSet<usize>,
    pub local_types: BTreeMap<usize, TypeConstraint>,
}
#[derive(Debug)]
struct Handler {
    start: usize,
    end: usize,
    target: usize,
    descriptor: String,
}
fn text(value: &noak::MStr) -> String {
    value.chars_lossy().collect()
}
fn utf16(value: &noak::MStr) -> Vec<u16> {
    crate::bytecode::modified_utf8_units(value)
}
fn cp_member(
    pool: &cpool::ConstantPool<'_>,
    index: u16,
) -> EngineResult<(String, String, String, bool)> {
    let (class, nt, interface) = match pool.get(cpool::Index::<cpool::Item<'_>>::new(index)?)? {
        cpool::Item::FieldRef(value) => (value.class, value.name_and_type, false),
        cpool::Item::MethodRef(value) => (value.class, value.name_and_type, false),
        cpool::Item::InterfaceMethodRef(value) => (value.class, value.name_and_type, true),
        _ => return Err(EngineError::Unavailable("invalid member constant".into())),
    };
    let nt = pool.retrieve(nt)?;
    Ok((
        text(pool.retrieve(class)?.name),
        text(nt.name),
        text(nt.descriptor),
        interface,
    ))
}
fn cp_constant(pool: &cpool::ConstantPool<'_>, index: u16) -> EngineResult<JvmConstant> {
    Ok(
        match pool.get(cpool::Index::<cpool::Item<'_>>::new(index)?)? {
            cpool::Item::Integer(value) => JvmConstant::Int(value.value),
            cpool::Item::Long(value) => JvmConstant::Long(value.value),
            cpool::Item::Float(value) => JvmConstant::Float(value.value),
            cpool::Item::Double(value) => JvmConstant::Double(value.value),
            cpool::Item::String(value) => JvmConstant::String(utf16(pool.retrieve(value.string)?)),
            cpool::Item::Class(value) => JvmConstant::Class(text(pool.retrieve(value.name)?)),
            cpool::Item::MethodType(value) => {
                JvmConstant::MethodType(text(pool.retrieve(value.descriptor)?))
            }
            cpool::Item::MethodHandle(value) => {
                let (owner, name, descriptor, is_interface) =
                    cp_member(pool, value.reference.as_u16())?;
                JvmConstant::MethodHandle {
                    tag: value.kind as u8 + 1,
                    owner,
                    name,
                    descriptor,
                    is_interface,
                }
            }
            _ => return Err(EngineError::Unavailable("Illegal LDC constant".into())),
        },
    )
}
fn object_descriptor(name: &str) -> String {
    if name.starts_with('[') {
        name.into()
    } else {
        format!("L{name};")
    }
}
fn target_at(offset: i64, positions: &BTreeMap<usize, usize>) -> EngineResult<usize> {
    usize::try_from(offset)
        .ok()
        .and_then(|offset| positions.get(&offset).copied())
        .ok_or_else(|| {
            EngineError::Unavailable("branch target is not an instruction boundary".into())
        })
}
fn read_method(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    context: &MatchContext<'_>,
    validate_frames: bool,
) -> EngineResult<(MethodFlow, usize, Vec<Handler>)> {
    context.cancellation.check_cancelled()?;
    let class = Class::new(bytes)?;
    let pool = class.pool();
    let class_name = text(pool.retrieve(class.this_class())?.name);
    let mut bootstraps = Vec::new();
    for attribute in class.attributes() {
        let attribute = attribute?;
        if pool.retrieve(attribute.name())?.as_bytes() == b"BootstrapMethods" {
            let methods = BootstrapMethods::try_from(attribute.read_content(pool)?)?;
            for method in methods.methods() {
                let method = method?;
                let handle = cp_constant(pool, method.method_ref().as_u16())?;
                let args = method
                    .arguments()
                    .into_iter()
                    .map(|value| cp_constant(pool, value?.as_u16()))
                    .collect::<EngineResult<Vec<_>>>()?;
                bootstraps.push((handle, args));
            }
        }
    }
    for method in class.methods() {
        let method = method?;
        if text(pool.retrieve(method.name())?) != name
            || text(pool.retrieve(method.descriptor())?) != descriptor
        {
            continue;
        }
        for attribute in method.attributes() {
            let attribute = attribute?;
            if pool.retrieve(attribute.name())?.as_bytes() != b"Code" {
                continue;
            }
            let code = Code::try_from(attribute.read_content(pool)?)?;
            let raw = code.raw_instructions().collect::<Result<Vec<_>, _>>()?;
            let positions = raw
                .iter()
                .enumerate()
                .map(|(index, (offset, _))| (offset.as_u32() as usize, index))
                .collect::<BTreeMap<_, _>>();
            let content = attribute.content();
            let mut instructions = Vec::new();
            for (index, (offset, raw)) in raw.iter().enumerate() {
                context.cancellation.check_cancelled()?;
                let offset = offset.as_u32() as usize;
                let at = 8 + offset;
                let wire = content[at];
                let mut opcode = wire;
                let u16_at = |at: usize| u16::from_be_bytes([content[at], content[at + 1]]);
                let operand = match wire {
                    0x01 => InstructionOperand::Constant(JvmConstant::Null),
                    0x02..=0x08 => InstructionOperand::Constant(JvmConstant::Int(wire as i32 - 3)),
                    0x09..=0x0a => {
                        InstructionOperand::Constant(JvmConstant::Long((wire - 9) as i64))
                    }
                    0x0b..=0x0d => {
                        InstructionOperand::Constant(JvmConstant::Float((wire - 11) as f32))
                    }
                    0x0e..=0x0f => {
                        InstructionOperand::Constant(JvmConstant::Double((wire - 14) as f64))
                    }
                    0x10 => {
                        InstructionOperand::Constant(JvmConstant::Int(content[at + 1] as i8 as i32))
                    }
                    0x11 => {
                        InstructionOperand::Constant(JvmConstant::Int(u16_at(at + 1) as i16 as i32))
                    }
                    0x12..=0x14 => {
                        opcode = 0x12;
                        InstructionOperand::Constant(cp_constant(
                            pool,
                            if wire == 0x12 {
                                content[at + 1] as u16
                            } else {
                                u16_at(at + 1)
                            },
                        )?)
                    }
                    0x15..=0x19 | 0x36..=0x3a | 0xa9 => {
                        InstructionOperand::Local(content[at + 1] as usize)
                    }
                    0x1a..=0x2d => {
                        opcode = 0x15 + (wire - 0x1a) / 4;
                        InstructionOperand::Local(((wire - 0x1a) % 4) as usize)
                    }
                    0x3b..=0x4e => {
                        opcode = 0x36 + (wire - 0x3b) / 4;
                        InstructionOperand::Local(((wire - 0x3b) % 4) as usize)
                    }
                    0x84 => InstructionOperand::Increment {
                        slot: content[at + 1] as usize,
                        amount: content[at + 2] as i8 as i32,
                    },
                    0xb2..=0xb9 => {
                        let (owner, name, descriptor, is_interface) =
                            cp_member(pool, u16_at(at + 1))?;
                        InstructionOperand::Member {
                            owner,
                            name,
                            descriptor,
                            is_interface,
                        }
                    }
                    0xba => {
                        let dynamic = pool.retrieve(
                            cpool::Index::<cpool::InvokeDynamic<'_>>::new(u16_at(at + 1))?,
                        )?;
                        let (bootstrap, arguments) = bootstraps
                            .get(dynamic.bootstrap_method_attr as usize)
                            .cloned()
                            .ok_or_else(|| {
                                EngineError::Unavailable("missing bootstrap method".into())
                            })?;
                        InstructionOperand::Dynamic {
                            name: text(dynamic.name_and_type.name),
                            descriptor: text(dynamic.name_and_type.descriptor),
                            bootstrap,
                            arguments,
                        }
                    }
                    0xbb | 0xbd | 0xc0 | 0xc1 => InstructionOperand::Type(text(
                        pool.retrieve(cpool::Index::<cpool::Class<'_>>::new(u16_at(at + 1))?)?
                            .name,
                    )),
                    0xbc => InstructionOperand::NewArray(content[at + 1]),
                    0xc5 => InstructionOperand::MultiArray {
                        descriptor: text(
                            pool.retrieve(cpool::Index::<cpool::Class<'_>>::new(u16_at(at + 1))?)?
                                .name,
                        ),
                        dimensions: content[at + 3] as usize,
                    },
                    0x99..=0xa8 | 0xc6 | 0xc7 => InstructionOperand::Jump(target_at(
                        offset as i64 + u16_at(at + 1) as i16 as i64,
                        &positions,
                    )?),
                    0xc8 | 0xc9 => {
                        opcode = if wire == 0xc8 { 0xa7 } else { 0xa8 };
                        let delta = i32::from_be_bytes(content[at + 1..at + 5].try_into().unwrap());
                        InstructionOperand::Jump(target_at(
                            offset as i64 + delta as i64,
                            &positions,
                        )?)
                    }
                    0xaa | 0xab => {
                        let (default, deltas) = match raw {
                            RawInstruction::TableSwitch(table) => (
                                table.default_offset(),
                                table.pairs().map(|pair| pair.offset()).collect::<Vec<_>>(),
                            ),
                            RawInstruction::LookupSwitch(table) => (
                                table.default_offset(),
                                table.pairs().map(|pair| pair.offset()).collect::<Vec<_>>(),
                            ),
                            _ => unreachable!(),
                        };
                        let mut targets =
                            vec![target_at(offset as i64 + default as i64, &positions)?];
                        for delta in deltas {
                            targets.push(target_at(offset as i64 + delta as i64, &positions)?);
                        }
                        InstructionOperand::Switch(targets)
                    }
                    0xc4 => {
                        opcode = content[at + 1];
                        if opcode == 0x84 {
                            InstructionOperand::Increment {
                                slot: u16_at(at + 2) as usize,
                                amount: u16_at(at + 4) as i16 as i32,
                            }
                        } else {
                            InstructionOperand::Local(u16_at(at + 2) as usize)
                        }
                    }
                    _ => InstructionOperand::None,
                };
                instructions.push(Instruction {
                    opcode,
                    original_opcode: opcode,
                    original_index: index,
                    byte_offset: offset,
                    operand,
                });
            }
            let mut handlers = Vec::new();
            for handler in code.exception_handlers() {
                let start = handler.start().as_u32() as usize;
                let end = handler.end().as_u32() as usize;
                let target = target_at(handler.handler().as_u32() as i64, &positions)?;
                let descriptor = handler
                    .catch_type()
                    .map(|index| {
                        pool.retrieve(index)
                            .map(|value| object_descriptor(&text(value.name)))
                    })
                    .transpose()?
                    .unwrap_or_else(|| "Ljava/lang/Throwable;".into());
                handlers.push(Handler {
                    start,
                    end,
                    target,
                    descriptor,
                });
            }
            let mut locals = Vec::new();
            for attr in code.attributes() {
                let attr = attr?;
                if validate_frames && pool.retrieve(attr.name())?.as_bytes() == b"StackMapTable" {
                    let mut offsets = BTreeSet::new();
                    let code_length = u32::from_be_bytes(content[4..8].try_into().unwrap());
                    let data = crate::bytecode::classfile_tail(bytes, attr.content()).map_err(
                        |error| {
                            EngineError::Unavailable(format!(
                                "Failed to parse class bytecode: {error}"
                            ))
                        },
                    )?;
                    crate::bytecode::stack_map_offsets(data, &mut offsets, pool, Some(code_length))
                        .map_err(|error| {
                            EngineError::Unavailable(format!(
                                "Failed to parse class bytecode: {error}"
                            ))
                        })?;
                }
                if pool.retrieve(attr.name())?.as_bytes() == b"LocalVariableTable" {
                    let data = attr.content();
                    let mut chunks = data
                        .get(2..)
                        .ok_or_else(|| {
                            EngineError::Unavailable("truncated local variable table".into())
                        })?
                        .chunks_exact(10);
                    let count = u16::from_be_bytes(data[..2].try_into().unwrap()) as usize;
                    for _ in 0..count {
                        let chunk = chunks.next().ok_or_else(|| {
                            EngineError::Unavailable("truncated local variable table".into())
                        })?;
                        let val = |at| u16::from_be_bytes([chunk[at], chunk[at + 1]]) as usize;
                        locals.push(LocalVariable {
                            slot: val(8),
                            start: val(0),
                            end: val(0) + val(2),
                            name: text(
                                pool.retrieve(
                                    cpool::Index::<cpool::Utf8<'_>>::new(val(4) as u16)?,
                                )?,
                            ),
                            descriptor: text(
                                pool.retrieve(
                                    cpool::Index::<cpool::Utf8<'_>>::new(val(6) as u16)?,
                                )?,
                            ),
                        });
                    }
                }
            }
            return Ok((
                MethodFlow {
                    class_name,
                    method_name: name.into(),
                    descriptor: descriptor.into(),
                    is_static: method.access_flags().contains(AccessFlags::STATIC),
                    max_stack: code.max_stack() as usize,
                    instructions,
                    graph: FlowGraph::default(),
                    frames: Vec::new(),
                    local_variables: locals,
                    argument_slots: BTreeSet::new(),
                    local_types: BTreeMap::new(),
                },
                code.max_locals() as usize,
                handlers,
            ));
        }
        return Ok((
            MethodFlow {
                class_name,
                method_name: name.into(),
                descriptor: descriptor.into(),
                is_static: method.access_flags().contains(AccessFlags::STATIC),
                max_stack: 0,
                instructions: Vec::new(),
                graph: FlowGraph::default(),
                frames: Vec::new(),
                local_variables: Vec::new(),
                argument_slots: BTreeSet::new(),
                local_types: BTreeMap::new(),
            },
            0,
            Vec::new(),
        ));
    }
    Err(EngineError::Unavailable(format!(
        "Method not found: {name}{descriptor}"
    )))
}
pub(crate) fn common_type(
    left: &TypeConstraint,
    right: &TypeConstraint,
    context: &MatchContext<'_>,
) -> EngineResult<TypeConstraint> {
    if left == right || right.descriptor() == "Lnull;" {
        return Ok(left.clone());
    }
    if left.descriptor() == "Lnull;" {
        return Ok(right.clone());
    }
    let l = left.descriptor();
    let r = right.descriptor();
    if is_int_like(l) && is_int_like(r) {
        return Ok(TypeConstraint::IntLike);
    }
    if is_int_like(l) || is_int_like(r) {
        return Ok(TypeConstraint::exact("Lnull;"));
    }
    if l.starts_with('[') && r.starts_with('[') {
        let dl = l.bytes().take_while(|&c| c == b'[').count();
        let dr = r.bytes().take_while(|&c| c == b'[').count();
        let el = &l[dl..];
        let er = &r[dr..];
        if dl == dr {
            let elem: String = if el == er {
                el.into()
            } else if el.starts_with('L') && er.starts_with('L') {
                common_type(
                    &TypeConstraint::exact(el),
                    &TypeConstraint::exact(er),
                    context,
                )?
                .descriptor()
                .into()
            } else {
                return Ok(TypeConstraint::exact(format!(
                    "{}Ljava/lang/Object;",
                    "[".repeat(dl - 1)
                )));
            };
            return Ok(TypeConstraint::exact(format!("{}{elem}", "[".repeat(dl))));
        }
        let (small, shared) = if dl < dr { (el, dl - 1) } else { (er, dr - 1) };
        return Ok(TypeConstraint::exact(format!(
            "{}Ljava/lang/Object;",
            "[".repeat(shared + usize::from(small.starts_with('L')))
        )));
    }
    if l.starts_with('[') && r.starts_with('L') || r.starts_with('[') && l.starts_with('L') {
        return Ok(TypeConstraint::exact("Ljava/lang/Object;"));
    }
    if !(l.starts_with('L') && r.starts_with('L')) {
        return Ok(TypeConstraint::exact("Lnull;"));
    }
    context.cancellation.check_cancelled()?;
    (context.common_super_class)(l, r)
        .map(TypeConstraint::exact)
        .ok_or_else(|| {
            EngineError::Unavailable(format!(
                "Could not resolve common super class of {l} and {r}"
            ))
        })
}
fn primitive_array(atype: u8) -> EngineResult<&'static str> {
    Ok(match atype {
        4 => "Z",
        5 => "C",
        6 => "F",
        7 => "D",
        8 => "B",
        9 => "S",
        10 => "I",
        11 => "J",
        _ => {
            return Err(EngineError::Unavailable(format!(
                "Invalid array type {atype}"
            )))
        }
    })
}
fn operand_type(instruction: &Instruction) -> EngineResult<TypeConstraint> {
    Ok(match &instruction.operand {
        InstructionOperand::Constant(constant) => match constant {
            JvmConstant::Int(_) => TypeConstraint::IntLike,
            JvmConstant::Long(_) => TypeConstraint::exact("J"),
            JvmConstant::Float(_) => TypeConstraint::exact("F"),
            JvmConstant::Double(_) => TypeConstraint::exact("D"),
            JvmConstant::String(_) => TypeConstraint::exact("Ljava/lang/String;"),
            JvmConstant::Class(_) => TypeConstraint::exact("Ljava/lang/Class;"),
            JvmConstant::MethodType(_) => TypeConstraint::exact("Ljava/lang/invoke/MethodType;"),
            JvmConstant::MethodHandle { .. } => {
                TypeConstraint::exact("Ljava/lang/invoke/MethodHandle;")
            }
            JvmConstant::Null => TypeConstraint::exact("Lnull;"),
        },
        InstructionOperand::Member { descriptor, .. } => {
            if instruction.opcode <= 0xb5 {
                TypeConstraint::exact(descriptor)
            } else {
                TypeConstraint::exact(to_descriptor(
                    &parse_method_descriptor(descriptor)?.return_type,
                ))
            }
        }
        InstructionOperand::Dynamic { descriptor, .. } => TypeConstraint::exact(to_descriptor(
            &parse_method_descriptor(descriptor)?.return_type,
        )),
        InstructionOperand::Type(name) => TypeConstraint::exact(object_descriptor(name)),
        InstructionOperand::MultiArray { descriptor, .. } => TypeConstraint::exact(descriptor),
        _ => TypeConstraint::exact("V"),
    })
}
fn pop(frame: &mut Frame) -> EngineResult<usize> {
    frame
        .stack
        .pop()
        .ok_or_else(|| EngineError::Unavailable("operand stack underflow".into()))
}
fn pop_many(frame: &mut Frame, count: usize) -> EngineResult<Vec<usize>> {
    if frame.stack.len() < count {
        return Err(EngineError::Unavailable("operand stack underflow".into()));
    }
    Ok(frame.stack.split_off(frame.stack.len() - count))
}
fn unary_type(instruction: &Instruction) -> EngineResult<TypeConstraint> {
    let opcode = instruction.opcode;
    Ok(TypeConstraint::exact(match opcode {
        0x74 | 0x88 | 0x8b | 0x8e | 0xbe => "I",
        0x75 | 0x85 | 0x8c | 0x8f => "J",
        0x76 | 0x86 | 0x89 | 0x90 => "F",
        0x77 | 0x87 | 0x8a | 0x8d => "D",
        0x91 => "B",
        0x92 => "C",
        0x93 => "S",
        0xc1 => "Z",
        0xb4 => {
            if let InstructionOperand::Member { descriptor, .. } = &instruction.operand {
                return Ok(TypeConstraint::exact(descriptor));
            }
            unreachable!()
        }
        0xbc => {
            if let InstructionOperand::NewArray(atype) = instruction.operand {
                return Ok(TypeConstraint::exact(format!(
                    "[{}",
                    primitive_array(atype)?
                )));
            }
            unreachable!()
        }
        0xbd => {
            if let InstructionOperand::Type(name) = &instruction.operand {
                return Ok(TypeConstraint::exact(format!(
                    "[{}",
                    object_descriptor(name)
                )));
            }
            unreachable!()
        }
        0xc0 => return operand_type(instruction),
        _ => "V",
    }))
}
fn binary_type(opcode: u8, left: &TypeConstraint) -> EngineResult<TypeConstraint> {
    Ok(TypeConstraint::exact(match opcode {
        0x2f | 0x61 | 0x65 | 0x69 | 0x6d | 0x71 | 0x79 | 0x7b | 0x7d | 0x7f | 0x81 | 0x83 => "J",
        0x31 | 0x63 | 0x67 | 0x6b | 0x6f | 0x73 => "D",
        0x30 | 0x62 | 0x66 | 0x6a | 0x6e | 0x72 => "F",
        0x34 => "C",
        0x35 => "S",
        0x32 | 0x33 => {
            return Ok(if left.descriptor() == "Lnull;" {
                left.clone()
            } else {
                TypeConstraint::exact(left.descriptor().strip_prefix('[').ok_or_else(|| {
                    EngineError::Unavailable("array operand has no array type".into())
                })?)
            });
        }
        0x2e
        | 0x60
        | 0x64
        | 0x68
        | 0x6c
        | 0x70
        | 0x78
        | 0x7a
        | 0x7c
        | 0x7e
        | 0x80
        | 0x82
        | 0x94..=0x98 => "I",
        _ => "V",
    }))
}
fn box_primitive(owner: &str) -> Option<&'static str> {
    Some(match owner {
        "java/lang/Boolean" => "Z",
        "java/lang/Character" => "C",
        "java/lang/Byte" => "B",
        "java/lang/Short" => "S",
        "java/lang/Integer" => "I",
        "java/lang/Float" => "F",
        "java/lang/Long" => "J",
        "java/lang/Double" => "D",
        _ => return None,
    })
}
fn execute(
    instruction: &Instruction,
    frame: &mut Frame,
    graph: &mut FlowGraph,
    context: &MatchContext<'_>,
    method: &MethodFlow,
    strip_boxing: bool,
) -> EngineResult<()> {
    context.cancellation.check_phase(if strip_boxing {
        CancellationPhase::FlowInterpretation
    } else {
        CancellationPhase::LocalAnalysis
    })?;
    let opcode = instruction.opcode;
    if graph.analysis_policy == AnalysisPolicy::OfficialExpression && opcode == 0xa8 {
        return Err(EngineError::Unavailable(
            "MixinExtras 0.0.6 does not define a JSR expression value".into(),
        ));
    }
    let mut push = true;
    let (ty, inputs) = match opcode {
        0x00 | 0xa7 | 0xa9 | 0xb1 => return Ok(()),
        0x01..=0x14 | 0xb2 | 0xbb => (operand_type(instruction)?, Vec::new()),
        0x15..=0x19 => {
            let InstructionOperand::Local(slot) = instruction.operand else {
                unreachable!()
            };
            let source = *frame
                .locals
                .get(slot)
                .ok_or_else(|| EngineError::Unavailable("local slot exceeds max_locals".into()))?;
            let debug = method.local_variables.iter().rev().find(|local| {
                local.slot == slot
                    && local.start <= instruction.byte_offset
                    && instruction.byte_offset < local.end
            });
            let ty = if graph.analysis_policy == AnalysisPolicy::ExactLocalCapture {
                graph.nodes[source].ty.clone()
            } else {
                method
                    .local_types
                    .get(&instruction.original_index)
                    .cloned()
                    .or_else(|| debug.map(|value| TypeConstraint::exact(&value.descriptor)))
                    .unwrap_or_else(|| graph.nodes[source].ty.clone())
            };
            if ty.descriptor() == "V" && graph.analysis_policy != AnalysisPolicy::ExactLocalCapture
            {
                return Err(EngineError::Unavailable(
                    "load of uninitialized local".into(),
                ));
            }
            (ty, Vec::new())
        }
        0x36..=0x3a => {
            let InstructionOperand::Local(slot) = instruction.operand else {
                unreachable!()
            };
            let value = pop(frame)?;
            let size = graph.nodes[value].ty.size();
            if slot + size > frame.locals.len() {
                return Err(EngineError::Unavailable(
                    "local store exceeds max_locals".into(),
                ));
            }
            let dummy = graph.dummy_returns(
                graph.nodes[value].ty.clone(),
                graph.nodes[value].return_targets.clone(),
            );
            frame.locals[slot] = dummy;
            if size == 2 {
                frame.locals[slot + 1] = graph.dummy(TypeConstraint::exact("V"));
            }
            if slot > 0 && graph.nodes[frame.locals[slot - 1]].ty.size() == 2 {
                frame.locals[slot - 1] = graph.dummy(TypeConstraint::exact("V"));
            }
            push = false;
            (TypeConstraint::exact("V"), vec![value])
        }
        0x2e..=0x35 | 0x60..=0x73 | 0x78..=0x83 | 0x94..=0x98 | 0x9f..=0xa6 | 0xb5 => {
            let inputs = pop_many(frame, 2)?;
            let ty = if graph.analysis_policy == AnalysisPolicy::ExactLocalCapture && opcode == 0x32
            {
                TypeConstraint::exact("Ljava/lang/Object;")
            } else if graph.analysis_policy == AnalysisPolicy::ExactLocalCapture && opcode == 0x33 {
                TypeConstraint::exact("I")
            } else {
                binary_type(opcode, &graph.nodes[inputs[0]].ty)?
            };
            if ty.descriptor() == "V" {
                push = false;
            }
            (ty, inputs)
        }
        0x4f..=0x56 => {
            push = false;
            (TypeConstraint::exact("V"), pop_many(frame, 3)?)
        }
        0x57 => {
            let value = pop(frame)?;
            if graph.nodes[value].ty.size() != 1 {
                return Err(EngineError::Unavailable(
                    "POP applied to category 2 value".into(),
                ));
            }
            return Ok(());
        }
        0x58 => {
            let value = pop(frame)?;
            if graph.nodes[value].ty.size() == 1 {
                let second = pop(frame)?;
                if graph.nodes[second].ty.size() != 1 {
                    return Err(EngineError::Unavailable("invalid POP2 operands".into()));
                }
            }
            return Ok(());
        }
        0x59..=0x5f => return stack_shuffle(opcode, frame, graph),
        0x84 => {
            let InstructionOperand::Increment { slot, .. } = instruction.operand else {
                unreachable!()
            };
            if slot >= frame.locals.len() {
                return Err(EngineError::Unavailable("IINC exceeds max_locals".into()));
            }
            frame.locals[slot] = graph.dummy(TypeConstraint::exact("I"));
            push = false;
            (TypeConstraint::exact("V"), Vec::new())
        }
        0xa8 => {
            let id = graph.dummy_returns(
                TypeConstraint::exact("Lreturn-address;"),
                BTreeSet::from([instruction.original_index + 1]),
            );
            frame.stack.push(id);
            return Ok(());
        }
        0xaa | 0xab => {
            push = false;
            (TypeConstraint::exact("V"), vec![pop(frame)?])
        }
        0xb6..=0xba => {
            let descriptor = match &instruction.operand {
                InstructionOperand::Member { descriptor, .. }
                | InstructionOperand::Dynamic { descriptor, .. } => descriptor,
                _ => unreachable!(),
            };
            let parsed = parse_method_descriptor(descriptor)?;
            let count = parsed.parameters.len() + usize::from(opcode != 0xb8 && opcode != 0xba);
            let inputs = pop_many(frame, count)?;
            if let InstructionOperand::Member { owner, name, .. } = &instruction.operand {
                let unboxing_name = match owner.as_str() {
                    "java/lang/Boolean" => "booleanValue",
                    "java/lang/Character" => "charValue",
                    "java/lang/Byte" => "byteValue",
                    "java/lang/Short" => "shortValue",
                    "java/lang/Integer" => "intValue",
                    "java/lang/Float" => "floatValue",
                    "java/lang/Long" => "longValue",
                    "java/lang/Double" => "doubleValue",
                    _ => "",
                };
                if strip_boxing
                    && box_primitive(owner).is_some_and(|primitive| {
                        name == unboxing_name
                            || (name == "valueOf"
                                && parsed.parameters.len() == 1
                                && to_descriptor(&parsed.parameters[0]) == primitive)
                    })
                {
                    frame.stack.push(inputs[0]);
                    return Ok(());
                }
            }
            let ty = TypeConstraint::exact(to_descriptor(&parsed.return_type));
            push = ty.descriptor() != "V";
            (ty, inputs)
        }
        0xc5 => {
            let InstructionOperand::MultiArray { dimensions, .. } = instruction.operand else {
                unreachable!()
            };
            (operand_type(instruction)?, pop_many(frame, dimensions)?)
        }
        0x74..=0x77
        | 0x85..=0x93
        | 0x99..=0x9e
        | 0xac..=0xb0
        | 0xb3
        | 0xb4
        | 0xbc..=0xbf
        | 0xc0..=0xc3
        | 0xc6
        | 0xc7 => {
            let ty = unary_type(instruction)?;
            push = ty.descriptor() != "V";
            (ty, vec![pop(frame)?])
        }
        _ => {
            return Err(EngineError::Unavailable(format!(
                "unsupported JVM instruction {opcode:#x}"
            )))
        }
    };
    let id = graph.record(instruction, ty, inputs, context)?;
    if push {
        frame.stack.push(id);
    }
    Ok(())
}
fn stack_shuffle(opcode: u8, frame: &mut Frame, graph: &FlowGraph) -> EngineResult<()> {
    let v1 = pop(frame)?;
    let s1 = graph.nodes[v1].ty.size();
    let values = match opcode {
        0x59 if s1 == 1 => vec![v1, v1],
        0x5a if s1 == 1 => {
            let v2 = pop(frame)?;
            if graph.nodes[v2].ty.size() != 1 {
                return Err(EngineError::Unavailable("invalid DUP_X1".into()));
            }
            vec![v1, v2, v1]
        }
        0x5b if s1 == 1 => {
            let v2 = pop(frame)?;
            if graph.nodes[v2].ty.size() == 2 {
                vec![v1, v2, v1]
            } else {
                let v3 = pop(frame)?;
                if graph.nodes[v3].ty.size() != 1 {
                    return Err(EngineError::Unavailable("invalid DUP_X2".into()));
                }
                vec![v1, v3, v2, v1]
            }
        }
        0x5c => {
            if s1 == 2 {
                vec![v1, v1]
            } else {
                let v2 = pop(frame)?;
                if graph.nodes[v2].ty.size() != 1 {
                    return Err(EngineError::Unavailable("invalid DUP2".into()));
                }
                vec![v2, v1, v2, v1]
            }
        }
        0x5d => {
            if s1 == 2 {
                let v2 = pop(frame)?;
                if graph.nodes[v2].ty.size() != 1 {
                    return Err(EngineError::Unavailable("invalid DUP2_X1".into()));
                }
                vec![v1, v2, v1]
            } else {
                let v2 = pop(frame)?;
                let v3 = pop(frame)?;
                if graph.nodes[v2].ty.size() != 1 || graph.nodes[v3].ty.size() != 1 {
                    return Err(EngineError::Unavailable("invalid DUP2_X1".into()));
                }
                vec![v2, v1, v3, v2, v1]
            }
        }
        0x5e => {
            if s1 == 2 {
                let v2 = pop(frame)?;
                if graph.nodes[v2].ty.size() == 2 {
                    vec![v1, v2, v1]
                } else {
                    let v3 = pop(frame)?;
                    if graph.nodes[v3].ty.size() != 1 {
                        return Err(EngineError::Unavailable("invalid DUP2_X2".into()));
                    }
                    vec![v1, v3, v2, v1]
                }
            } else {
                let v2 = pop(frame)?;
                let v3 = pop(frame)?;
                if graph.nodes[v2].ty.size() != 1 {
                    return Err(EngineError::Unavailable("invalid DUP2_X2".into()));
                }
                if graph.nodes[v3].ty.size() == 2 {
                    vec![v2, v1, v3, v2, v1]
                } else {
                    let v4 = pop(frame)?;
                    if graph.nodes[v4].ty.size() != 1 {
                        return Err(EngineError::Unavailable("invalid DUP2_X2".into()));
                    }
                    vec![v2, v1, v4, v3, v2, v1]
                }
            }
        }
        0x5f if s1 == 1 => {
            let v2 = pop(frame)?;
            if graph.nodes[v2].ty.size() != 1 {
                return Err(EngineError::Unavailable("invalid SWAP".into()));
            }
            vec![v1, v2]
        }
        _ => return Err(EngineError::Unavailable("invalid stack shuffle".into())),
    };
    frame.stack.extend(values);
    Ok(())
}
fn merge_frame(
    graph: &mut FlowGraph,
    frames: &mut [Option<Frame>],
    target: usize,
    incoming: &Frame,
    context: &MatchContext<'_>,
) -> EngineResult<bool> {
    let Some(existing) = frames.get_mut(target) else {
        return Err(EngineError::Unavailable(
            "control flow falls outside method".into(),
        ));
    };
    if let Some(frame) = existing {
        if frame.stack.len() != incoming.stack.len() || frame.locals.len() != incoming.locals.len()
        {
            return Err(EngineError::Unavailable(
                "incompatible frame dimensions".into(),
            ));
        }
        let mut changed = false;
        for (index, &value) in incoming.locals.iter().enumerate() {
            let merged = graph.merge(frame.locals[index], value, context)?;
            changed |= merged != frame.locals[index];
            frame.locals[index] = merged;
        }
        for (index, &value) in incoming.stack.iter().enumerate() {
            if graph.nodes[frame.stack[index]].ty.size() != graph.nodes[value].ty.size() {
                return Err(EngineError::Unavailable(
                    "incompatible category at stack merge".into(),
                ));
            }
            let merged = graph.merge(frame.stack[index], value, context)?;
            changed |= merged != frame.stack[index];
            frame.stack[index] = merged;
        }
        Ok(changed)
    } else {
        *existing = Some(incoming.clone());
        Ok(true)
    }
}
fn analyze_once(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    context: &MatchContext<'_>,
    local_types: &BTreeMap<usize, TypeConstraint>,
    strip_boxing: bool,
    analysis_policy: AnalysisPolicy,
) -> EngineResult<MethodFlow> {
    let (mut method, max_locals, handlers) = read_method(
        bytes,
        name,
        descriptor,
        context,
        analysis_policy == AnalysisPolicy::OfficialExpression,
    )?;
    if method.instructions.is_empty() {
        return Err(EngineError::Unavailable(format!(
            "Method has no instructions: {name}{descriptor}"
        )));
    }
    method.local_types = local_types.clone();
    let mut graph = FlowGraph::default();
    graph.analysis_policy = analysis_policy;
    let uninit = graph.dummy(TypeConstraint::exact("V"));
    let mut initial = Frame {
        locals: vec![uninit; max_locals],
        stack: Vec::new(),
    };
    let mut slot = 0;
    if !method.is_static {
        if max_locals == 0 {
            return Err(EngineError::Unavailable(
                "instance method has no receiver local".into(),
            ));
        }
        initial.locals[0] =
            graph.dummy(TypeConstraint::exact(object_descriptor(&method.class_name)));
        method.argument_slots.insert(0);
        slot = 1;
    }
    for parameter in parse_method_descriptor(descriptor)?.parameters {
        let ty = TypeConstraint::exact(to_descriptor(&parameter));
        if slot + ty.size() > max_locals {
            return Err(EngineError::Unavailable(
                "arguments exceed max_locals".into(),
            ));
        }
        initial.locals[slot] = graph.dummy(ty.clone());
        method.argument_slots.insert(slot);
        slot += ty.size();
    }
    let mut frames = vec![None; method.instructions.len()];
    frames[0] = Some(initial);
    let mut queue = VecDeque::from([0]);
    let mut queued = BTreeSet::from([0]);
    while let Some(index) = queue.pop_front() {
        queued.remove(&index);
        context.cancellation.check_cancelled()?;
        let before = frames[index].as_ref().unwrap().clone();
        let mut after = before.clone();
        let insn = &method.instructions[index];
        execute(insn, &mut after, &mut graph, context, &method, strip_boxing)?;
        if after.stack.len() > method.max_stack {
            return Err(EngineError::Unavailable(
                "operand stack exceeds max_stack".into(),
            ));
        }
        let mut successors = match &insn.operand {
            InstructionOperand::Jump(target) => vec![*target],
            InstructionOperand::Switch(targets) => targets.clone(),
            _ => Vec::new(),
        };
        if insn.opcode == 0xa9 {
            let InstructionOperand::Local(slot) = insn.operand else {
                unreachable!()
            };
            let value = *after
                .locals
                .get(slot)
                .ok_or_else(|| EngineError::Unavailable("RET exceeds local frame".into()))?;
            successors.extend(graph.nodes[value].return_targets.iter().copied());
            if successors.is_empty() {
                return Err(EngineError::Unavailable(
                    "RET local is not a return address".into(),
                ));
            }
        }
        if !matches!(insn.opcode,0xa7..=0xab|0xac..=0xb1|0xbf) {
            successors.push(index + 1);
        }
        for target in successors {
            if merge_frame(&mut graph, &mut frames, target, &after, context)?
                && queued.insert(target)
            {
                queue.push_back(target);
            }
        }
        for handler in &handlers {
            if handler.start <= insn.byte_offset && insn.byte_offset < handler.end {
                let mut exceptional = before.clone();
                exceptional.stack = vec![graph.dummy(TypeConstraint::exact(&handler.descriptor))];
                if method.max_stack == 0 {
                    return Err(EngineError::Unavailable(
                        "exception stack exceeds max_stack".into(),
                    ));
                }
                if merge_frame(
                    &mut graph,
                    &mut frames,
                    handler.target,
                    &exceptional,
                    context,
                )? && queued.insert(handler.target)
                {
                    queue.push_back(handler.target);
                }
            }
        }
    }
    method.frames = frames;
    method.graph = graph;
    Ok(method)
}
pub(crate) fn analyze(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    context: &MatchContext<'_>,
) -> EngineResult<MethodFlow> {
    // Official LocalsCalculator runs before the flow interpreter erases boxing/unboxing nodes.
    let locals = analyze_once(
        bytes,
        name,
        descriptor,
        context,
        &BTreeMap::new(),
        false,
        AnalysisPolicy::OfficialExpression,
    )?;
    let local_types = locals
        .graph
        .by_instruction
        .iter()
        .filter_map(|(&index, &id)| {
            matches!(locals.instructions[index].opcode, 0x15..=0x19)
                .then(|| (index, locals.graph.nodes[id].ty.clone()))
        })
        .collect();
    let mut method = analyze_once(
        bytes,
        name,
        descriptor,
        context,
        &local_types,
        true,
        AnalysisPolicy::OfficialExpression,
    )?;
    for (index, frame) in locals.frames.into_iter().enumerate() {
        if let (Some(source), Some(target)) = (frame, method.frames[index].as_mut()) {
            target.locals = source
                .locals
                .into_iter()
                .map(|id| method.graph.dummy(locals.graph.nodes[id].ty.clone()))
                .collect();
        }
    }
    Ok(method)
}
pub fn analyze_method(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    context: &MatchContext<'_>,
) -> Result<Result<MethodFlow, String>, Cancellation> {
    match analyze(bytes, name, descriptor, context) {
        Ok(flow) => Ok(Ok(flow)),
        Err(EngineError::Unavailable(reason)) => Ok(Err(reason)),
        Err(EngineError::Cancelled(error)) => Err(error),
    }
}
/// Shares the decoder and CFG with expression matching while preserving the legacy
/// BasicInterpreter local-frame merge rules used by LocalCaptureExtractor.
pub fn analyze_method_with_policy(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    context: &MatchContext<'_>,
    policy: AnalysisPolicy,
) -> Result<Result<MethodFlow, String>, Cancellation> {
    let result = match policy {
        AnalysisPolicy::OfficialExpression => analyze(bytes, name, descriptor, context),
        AnalysisPolicy::ExactLocalCapture => analyze_once(
            bytes,
            name,
            descriptor,
            context,
            &BTreeMap::new(),
            false,
            policy,
        ),
    };
    match result {
        Ok(method) => Ok(Ok(method)),
        Err(EngineError::Unavailable(reason)) => Ok(Err(reason)),
        Err(EngineError::Cancelled(error)) => Err(error),
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeMethodError {
    CorruptClass { detail: String },
    MethodNotFound,
}
/// Decodes without interpreting instructions, allowing index validation and empty
/// requests to retain their original ordering before local-frame analysis.
pub fn decode_method(
    bytes: &[u8],
    name: &str,
    descriptor: &str,
    context: &MatchContext<'_>,
) -> Result<Result<MethodFlow, DecodeMethodError>, Cancellation> {
    match read_method(bytes, name, descriptor, context, false) {
        Ok((method, ..)) => Ok(Ok(method)),
        Err(EngineError::Cancelled(error)) => Err(error),
        Err(EngineError::Unavailable(reason)) => {
            Ok(Err(if reason.starts_with("Method not found:") {
                DecodeMethodError::MethodNotFound
            } else {
                DecodeMethodError::CorruptClass { detail: reason }
            }))
        }
    }
}
