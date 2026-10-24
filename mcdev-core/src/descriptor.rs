use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JvmType {
    ByteType,
    CharType,
    DoubleType,
    FloatType,
    IntType,
    LongType,
    ShortType,
    BooleanType,
    VoidType,
    ObjectType(String),
    ArrayType(Box<JvmType>),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodDescriptor {
    pub parameters: Vec<JvmType>,
    pub return_type: JvmType,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorParseError {
    pub message: String,
    pub offset: usize,
}
impl std::fmt::Display for DescriptorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.message, self.offset)
    }
}
impl std::error::Error for DescriptorParseError {}
pub type DescriptorParseResult<T> = Result<T, DescriptorParseError>;
fn failure<T>(message: &str, offset: usize) -> DescriptorParseResult<T> {
    Err(DescriptorParseError {
        message: message.into(),
        offset,
    })
}

pub struct DescriptorParser<'a> {
    input: &'a str,
    index: usize,
}
impl<'a> DescriptorParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }
    pub fn parse_field_descriptor(mut self) -> DescriptorParseResult<JvmType> {
        let value = self.parse_type(false).ok_or_else(|| DescriptorParseError {
            message: "expected field descriptor".into(),
            offset: self.index,
        })?;
        if self.index != self.input.len() {
            return failure("unexpected trailing input", self.index);
        }
        Ok(value)
    }
    pub fn parse_method_descriptor(mut self) -> DescriptorParseResult<MethodDescriptor> {
        if !self.consume(b'(') {
            return failure("expected '('", self.index);
        }
        let mut parameters = Vec::new();
        while self.input.as_bytes().get(self.index) != Some(&b')') {
            if self.index >= self.input.len() {
                return failure("unterminated parameter list", self.index);
            }
            parameters.push(self.parse_type(false).ok_or_else(|| DescriptorParseError {
                message: "expected parameter descriptor".into(),
                offset: self.index,
            })?);
        }
        self.index += 1;
        let return_type = self.parse_type(true).ok_or_else(|| DescriptorParseError {
            message: "expected return descriptor".into(),
            offset: self.index,
        })?;
        if self.index != self.input.len() {
            return failure("unexpected trailing input", self.index);
        }
        Ok(MethodDescriptor {
            parameters,
            return_type,
        })
    }
    fn consume(&mut self, ch: u8) -> bool {
        if self.input.as_bytes().get(self.index) == Some(&ch) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn parse_type(&mut self, allow_void: bool) -> Option<JvmType> {
        let start = self.index;
        let mut dimensions = 0;
        while self.consume(b'[') {
            dimensions += 1;
        }
        let marker = *self.input.as_bytes().get(self.index)?;
        self.index += 1;
        let mut ty = match marker {
            b'B' => JvmType::ByteType,
            b'C' => JvmType::CharType,
            b'D' => JvmType::DoubleType,
            b'F' => JvmType::FloatType,
            b'I' => JvmType::IntType,
            b'J' => JvmType::LongType,
            b'S' => JvmType::ShortType,
            b'Z' => JvmType::BooleanType,
            b'V' if allow_void && dimensions == 0 => JvmType::VoidType,
            b'V' => return None,
            b'L' => {
                let object_start = self.index;
                let Some(end) = self.input[object_start..]
                    .find(';')
                    .map(|n| object_start + n)
                else {
                    self.index = object_start - 1;
                    return None;
                };
                let name = &self.input[object_start..end];
                self.index = end + 1;
                if name.trim().is_empty() || name.contains(['.', '[']) {
                    self.index = object_start - 1;
                    return None;
                }
                JvmType::ObjectType(name.into())
            }
            _ => {
                self.index = start + dimensions;
                return None;
            }
        };
        for _ in 0..dimensions {
            ty = JvmType::ArrayType(Box::new(ty));
        }
        Some(ty)
    }
}
pub fn parse_field_descriptor(input: &str) -> DescriptorParseResult<JvmType> {
    DescriptorParser::new(input).parse_field_descriptor()
}
pub fn parse_method_descriptor(input: &str) -> DescriptorParseResult<MethodDescriptor> {
    DescriptorParser::new(input).parse_method_descriptor()
}
pub fn render(ty: &JvmType) -> String {
    match ty {
        JvmType::ByteType => "byte".into(),
        JvmType::CharType => "char".into(),
        JvmType::DoubleType => "double".into(),
        JvmType::FloatType => "float".into(),
        JvmType::IntType => "int".into(),
        JvmType::LongType => "long".into(),
        JvmType::ShortType => "short".into(),
        JvmType::BooleanType => "boolean".into(),
        JvmType::VoidType => "void".into(),
        JvmType::ObjectType(name) => name.rsplit('/').next().unwrap_or(name).into(),
        JvmType::ArrayType(component) => format!("{}[]", render(component)),
    }
}
pub fn render_method(value: &MethodDescriptor) -> String {
    format!(
        "({}): {}",
        value
            .parameters
            .iter()
            .map(render)
            .collect::<Vec<_>>()
            .join(", "),
        render(&value.return_type)
    )
}
pub fn to_descriptor(ty: &JvmType) -> String {
    match ty {
        JvmType::ByteType => "B".into(),
        JvmType::CharType => "C".into(),
        JvmType::DoubleType => "D".into(),
        JvmType::FloatType => "F".into(),
        JvmType::IntType => "I".into(),
        JvmType::LongType => "J".into(),
        JvmType::ShortType => "S".into(),
        JvmType::BooleanType => "Z".into(),
        JvmType::VoidType => "V".into(),
        JvmType::ObjectType(name) => format!("L{name};"),
        JvmType::ArrayType(component) => format!("[{}", to_descriptor(component)),
    }
}
pub fn method_to_descriptor(value: &MethodDescriptor) -> String {
    format!(
        "({}){}",
        value
            .parameters
            .iter()
            .map(to_descriptor)
            .collect::<String>(),
        to_descriptor(&value.return_type)
    )
}
pub struct DescriptorRenderer;
impl DescriptorRenderer {
    pub fn render(ty: &JvmType) -> String {
        render(ty)
    }
    pub fn render_method(value: &MethodDescriptor) -> String {
        render_method(value)
    }
    pub fn to_descriptor(ty: &JvmType) -> String {
        to_descriptor(ty)
    }
    pub fn method_to_descriptor(value: &MethodDescriptor) -> String {
        method_to_descriptor(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberTarget {
    Method {
        owner: String,
        name: String,
        descriptor: MethodDescriptor,
    },
    Field {
        owner: String,
        name: String,
        descriptor: JvmType,
    },
}
pub struct MemberTargetParser;
impl MemberTargetParser {
    pub fn parse(input: &str) -> DescriptorParseResult<MemberTarget> {
        parse_member_target(input)
    }
}
pub fn parse_member_target(input: &str) -> DescriptorParseResult<MemberTarget> {
    if !input.starts_with('L') {
        return failure("target must start with owner marker 'L'", 0);
    }
    let owner_end = input
        .find(';')
        .filter(|n| *n > 1)
        .ok_or_else(|| DescriptorParseError {
            message: "missing owner internal name".into(),
            offset: 1,
        })?;
    let owner = input[1..owner_end].to_owned();
    let member_start = owner_end + 1;
    if member_start >= input.len() {
        return failure("missing member name", member_start);
    }
    let member = &input[member_start..];
    if let Some(n) = member.find('(').filter(|n| *n > 0) {
        return Ok(MemberTarget::Method {
            owner,
            name: member[..n].into(),
            descriptor: parse_method_descriptor(&member[n..])?,
        });
    }
    if let Some(n) = member.find(':').filter(|n| *n > 0) {
        return Ok(MemberTarget::Field {
            owner,
            name: member[..n].into(),
            descriptor: parse_field_descriptor(&member[n + 1..])?,
        });
    }
    failure("missing member descriptor", member_start)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern<T> {
    Any,
    Exact(T),
}
impl<T: PartialEq> Pattern<T> {
    pub fn matches(&self, value: &T) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(v) => v == value,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodSelector {
    pub owner: Pattern<String>,
    pub name: Pattern<String>,
    pub descriptor: Pattern<MethodDescriptor>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSelector {
    pub owner: Pattern<String>,
    pub name: Pattern<String>,
    pub descriptor: Pattern<JvmType>,
}
fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_' || ch == '$')
        && chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '$')
}
fn validate_owner(owner: &str, offset: usize, internal: bool) -> DescriptorParseResult<()> {
    if owner.contains('*') {
        return failure(
            "wildcards are not supported in owner",
            offset + owner.find('*').unwrap(),
        );
    }
    if internal && owner.contains('.') {
        return failure("owner internal name must use slash separators", offset);
    }
    if owner.contains('.') && owner.contains('/') {
        return failure("owner name must not mix dot and slash separators", offset);
    }
    for segment in owner.split(if internal || owner.contains('/') {
        '/'
    } else {
        '.'
    }) {
        if segment.is_empty() {
            return failure("empty owner segment", offset);
        }
        if !identifier(segment) {
            return failure("invalid owner segment", offset);
        }
    }
    Ok(())
}
fn selector_parts(
    input: &str,
    delimiter: char,
) -> DescriptorParseResult<(Pattern<String>, &str, usize)> {
    if input.is_empty() {
        return failure(
            if delimiter == '(' {
                "empty method selector"
            } else {
                "empty field selector"
            },
            0,
        );
    }
    if input.starts_with('L') {
        if let Some(end) = input.find(';').filter(|n| *n > 0) {
            if input.find(delimiter).is_none_or(|n| end < n) {
                if end <= 1 {
                    return failure("missing owner internal name", 1);
                }
                let owner = &input[1..end];
                validate_owner(owner, 1, true)?;
                return Ok((Pattern::Exact(owner.into()), &input[end + 1..], end + 1));
            }
        }
    }
    let descriptor_start = input.find(delimiter).unwrap_or(input.len());
    if let Some(dot) = input[..descriptor_start].rfind('.') {
        let owner = &input[..dot];
        validate_owner(owner, 0, false)?;
        Ok((
            Pattern::Exact(owner.replace('.', "/")),
            &input[dot + 1..],
            dot + 1,
        ))
    } else {
        Ok((Pattern::Any, input, 0))
    }
}
fn selector_name(
    name: &str,
    offset: usize,
    method: bool,
    has_descriptor: bool,
) -> DescriptorParseResult<Pattern<String>> {
    let kind = if method { "method" } else { "field" };
    if name.is_empty() {
        return failure(&format!("missing {kind} name"), offset);
    }
    if name == "*" {
        return Ok(Pattern::Any);
    }
    let terminal = name.strip_suffix('*');
    if has_descriptor && terminal.is_some() {
        return failure(
            &format!("malformed {kind} name wildcard"),
            offset + name.len() - 1,
        );
    }
    let value = terminal.unwrap_or(name);
    if value.contains('*') {
        return failure(
            &format!("malformed {kind} name wildcard"),
            offset + value.find('*').unwrap(),
        );
    }
    let special = method && matches!(value, "<init>" | "<clinit>");
    if !special && !identifier(value) {
        return failure(
            &format!(
                "invalid {kind} name{}",
                if terminal.is_some() { " prefix" } else { "" }
            ),
            offset,
        );
    }
    Ok(Pattern::Exact(value.into()))
}
pub fn parse_method_selector(input: &str) -> DescriptorParseResult<MethodSelector> {
    let input = input.replace(' ', "");
    if input == "*" {
        return Ok(MethodSelector {
            owner: Pattern::Any,
            name: Pattern::Any,
            descriptor: Pattern::Any,
        });
    }
    let (owner, member, offset) = selector_parts(&input, '(')?;
    if member.is_empty() && input.ends_with(';') {
        return Ok(MethodSelector {
            owner,
            name: Pattern::Any,
            descriptor: Pattern::Any,
        });
    }
    if let Some(n) = member.find(':') {
        return failure(
            "field selector syntax is not supported for methods",
            offset + n,
        );
    }
    let start = member.find('(');
    let name_text = &member[..start.unwrap_or(member.len())];
    let name = selector_name(name_text, offset, true, start.is_some())?;
    let descriptor = if let Some(n) = start {
        let value = parse_method_descriptor(&member[n..])?;
        if name_text == "<init>" && value.return_type != JvmType::VoidType {
            return failure("constructor descriptor must return void", offset + n);
        }
        if name_text == "<clinit>"
            && (!value.parameters.is_empty() || value.return_type != JvmType::VoidType)
        {
            return failure("class initializer descriptor must be ()V", offset + n);
        }
        Pattern::Exact(value)
    } else {
        Pattern::Any
    };
    Ok(MethodSelector {
        owner,
        name,
        descriptor,
    })
}
pub fn parse_field_selector(input: &str) -> DescriptorParseResult<FieldSelector> {
    let input = input.replace(' ', "");
    if input == "*" {
        return Ok(FieldSelector {
            owner: Pattern::Any,
            name: Pattern::Any,
            descriptor: Pattern::Any,
        });
    }
    let (owner, member, offset) = selector_parts(&input, ':')?;
    if member.is_empty() && input.ends_with(';') {
        return Ok(FieldSelector {
            owner,
            name: Pattern::Any,
            descriptor: Pattern::Any,
        });
    }
    if let Some(n) = member.find('(') {
        return failure(
            "method selector syntax is not supported for fields",
            offset + n,
        );
    }
    let start = member.find(':');
    let name = selector_name(
        &member[..start.unwrap_or(member.len())],
        offset,
        false,
        start.is_some(),
    )?;
    let descriptor = if let Some(n) = start {
        if n + 1 == member.len() {
            return failure("missing field descriptor", offset + n + 1);
        }
        Pattern::Exact(parse_field_descriptor(&member[n + 1..])?)
    } else {
        Pattern::Any
    };
    Ok(FieldSelector {
        owner,
        name,
        descriptor,
    })
}
