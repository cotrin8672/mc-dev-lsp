use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Name {
    Defined(String),
    Wildcard,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeName {
    pub name: Name,
    pub dimensions: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    Multiply,
    Divide,
    Modulo,
    Add,
    Subtract,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    BitAnd,
    BitXor,
    BitOr,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Negate,
    BitNot,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    pub span: SourceSpan,
    pub kind: ExprKind,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExprKind {
    Capture(Box<Expression>),
    Wildcard,
    This,
    Int(i64),
    Decimal(f64),
    Boolean(bool),
    Null,
    String(String),
    StringUtf16(Vec<u16>),
    Identifier(String),
    ClassLiteral(TypeName),
    ArrayAccess {
        array: Box<Expression>,
        index: Box<Expression>,
    },
    MemberAccess {
        receiver: Box<Expression>,
        name: Name,
    },
    MethodCall {
        receiver: Box<Expression>,
        name: Name,
        arguments: Vec<Expression>,
    },
    StaticCall {
        name: Name,
        arguments: Vec<Expression>,
    },
    SuperCall {
        name: Name,
        arguments: Vec<Expression>,
    },
    BoundMethodReference {
        receiver: Box<Expression>,
        name: Name,
    },
    FreeMethodReference(Name),
    ConstructorReference(TypeName),
    Unary {
        operator: UnaryOperator,
        value: Box<Expression>,
    },
    New {
        target: TypeName,
        arguments: Vec<Expression>,
    },
    ArrayLiteral {
        element: TypeName,
        values: Vec<Expression>,
    },
    NewArray {
        element: TypeName,
        dimensions: Vec<Expression>,
        blank_dimensions: usize,
    },
    Cast {
        target: TypeName,
        value: Box<Expression>,
    },
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    InstanceOf {
        value: Box<Expression>,
        target: TypeName,
    },
    AssignIdentifier {
        name: Name,
        value: Box<Expression>,
    },
    AssignMember {
        receiver: Box<Expression>,
        name: Name,
        value: Box<Expression>,
    },
    ArrayStore {
        array: Box<Expression>,
        index: Box<Expression>,
        value: Box<Expression>,
    },
    Return(Box<Expression>),
    Throw(Box<Expression>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxFailure {
    pub message: String,
    pub offset: usize,
    pub line: usize,
    pub column: usize,
    pub cause: SyntaxCause,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxCause {
    pub message: String,
}
impl fmt::Display for SyntaxCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SyntaxCause {}
impl fmt::Display for SyntaxFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SyntaxFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Word(String),
    Int(u64),
    Decimal(f64),
    String(String),
    StringUtf16(Vec<u16>),
    Symbol(&'static str),
    Eof,
}
#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
    byte_start: usize,
}
fn error(source: &str, byte: usize, detail: impl fmt::Display) -> SyntaxFailure {
    let prefix = &source[..byte];
    let line = prefix.chars().filter(|&c| c == '\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count();
    let message =
        format!("Failed to parse expression \"{source}\": line {line}:{column}: {detail}");
    SyntaxFailure {
        cause: SyntaxCause {
            message: message.clone(),
        },
        message,
        offset: prefix.chars().count(),
        line,
        column,
    }
}
pub(crate) struct Utf16Source {
    pub text: String,
    pub surrogates: std::collections::BTreeMap<usize, u16>,
}
impl Utf16Source {
    pub fn new(units: &[u16]) -> Self {
        let mut text = String::new();
        let mut surrogates = std::collections::BTreeMap::new();
        for value in char::decode_utf16(units.iter().copied()) {
            match value {
                Ok(ch) => text.push(ch),
                Err(value) => {
                    surrogates.insert(text.len(), value.unpaired_surrogate());
                    text.push('\u{fffd}');
                }
            }
        }
        Self { text, surrogates }
    }
    pub fn append(&self, output: &mut Vec<u16>, ch: char, byte: usize) {
        if let Some(&unit) = self.surrogates.get(&byte) {
            output.push(unit);
        } else {
            output.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
        }
    }
}
fn lex(
    source: &str,
    surrogates: &std::collections::BTreeMap<usize, u16>,
) -> Result<Vec<Token>, SyntaxFailure> {
    let mut result = Vec::new();
    let mut byte = 0;
    let mut offset = 0;
    while byte < source.len() {
        let rest = &source[byte..];
        let first = rest.chars().next().unwrap();
        let start = byte;
        let start_offset = offset;
        if first == ' ' || first == '\t' || first == '\n' || rest.starts_with("\r\n") {
            let width = if rest.starts_with("\r\n") { 2 } else { 1 };
            byte += width;
            offset += width;
            continue;
        }
        let kind = if first == '\'' {
            byte += 1;
            offset += 1;
            let mut value = Vec::new();
            let mut closed = false;
            while byte < source.len() {
                let ch = source[byte..].chars().next().unwrap();
                let char_byte = byte;
                byte += ch.len_utf8();
                offset += 1;
                if ch == '\'' {
                    closed = true;
                    break;
                }
                if ch == '\\' {
                    let Some(next) = source[byte..].chars().next() else {
                        return Err(error(source, start, "unterminated string literal"));
                    };
                    if next != '\'' && next != '\\' {
                        return Err(error(source, byte, "invalid string escape"));
                    }
                    byte += 1;
                    offset += 1;
                    value.push(next as u16);
                } else {
                    if let Some(&unit) = surrogates.get(&char_byte) {
                        value.push(unit);
                    } else {
                        value.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
                    }
                }
            }
            if !closed {
                return Err(error(source, start, "unterminated string literal"));
            }
            match String::from_utf16(&value) {
                Ok(value) => TokenKind::String(value),
                Err(_) => TokenKind::StringUtf16(value),
            }
        } else if first.is_ascii_alphabetic() || first == '_' {
            byte += 1;
            offset += 1;
            while byte < source.len()
                && (source.as_bytes()[byte].is_ascii_alphanumeric()
                    || source.as_bytes()[byte] == b'_')
            {
                byte += 1;
                offset += 1;
            }
            TokenKind::Word(source[start..byte].into())
        } else if first.is_ascii_digit()
            || (first == '.' && rest.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
        {
            if rest.starts_with("0x") && rest.as_bytes().get(2).is_some_and(u8::is_ascii_hexdigit) {
                byte += 2;
                while byte < source.len() && source.as_bytes()[byte].is_ascii_hexdigit() {
                    byte += 1;
                }
                let value = i32::from_str_radix(&source[start + 2..byte], 16)
                    .map_err(|e| error(source, start, e))?;
                offset += byte - start;
                TokenKind::Int(value as u64)
            } else {
                while byte < source.len() && source.as_bytes()[byte].is_ascii_digit() {
                    byte += 1;
                }
                if source.as_bytes().get(byte) == Some(&b'.')
                    && source
                        .as_bytes()
                        .get(byte + 1)
                        .is_some_and(u8::is_ascii_digit)
                {
                    byte += 1;
                    while byte < source.len() && source.as_bytes()[byte].is_ascii_digit() {
                        byte += 1;
                    }
                    offset += byte - start;
                    TokenKind::Decimal(
                        source[start..byte]
                            .parse()
                            .map_err(|e| error(source, start, e))?,
                    )
                } else {
                    offset += byte - start;
                    TokenKind::Int(
                        source[start..byte]
                            .parse()
                            .map_err(|e| error(source, start, e))?,
                    )
                }
            }
        } else {
            let symbol = [
                ">>>", "<<", ">>", "<=", ">=", "==", "!=", "::", "++", "--", "?", "+", "-", "*",
                "/", "%", "~", ".", ",", "(", ")", "[", "]", "{", "}", "@", "<", ">", "&", "^",
                "|", "=",
            ]
            .into_iter()
            .find(|symbol| rest.starts_with(symbol))
            .ok_or_else(|| {
                error(
                    source,
                    byte,
                    format!("token recognition error at: '{first}'"),
                )
            })?;
            byte += symbol.len();
            offset += symbol.len();
            TokenKind::Symbol(symbol)
        };
        result.push(Token {
            kind,
            start: start_offset,
            end: offset,
            byte_start: start,
        });
    }
    result.push(Token {
        kind: TokenKind::Eof,
        start: offset,
        end: offset,
        byte_start: byte,
    });
    Ok(result)
}
struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    at: usize,
    explicit_capture: bool,
    depth: usize,
}
impl Parser<'_> {
    fn peek(&self) -> &TokenKind {
        &self.tokens[self.at].kind
    }
    fn symbol(&self, symbol: &str) -> bool {
        matches!(self.peek(),TokenKind::Symbol(value)if *value==symbol)
    }
    fn word(&self, word: &str) -> bool {
        matches!(self.peek(),TokenKind::Word(value)if value==word)
    }
    fn take(&mut self) -> Token {
        let token = self.tokens[self.at].clone();
        if token.kind != TokenKind::Eof {
            self.at += 1;
        }
        token
    }
    fn consume(&mut self, symbol: &str) -> bool {
        if self.symbol(symbol) {
            self.take();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, symbol: &str) -> Result<(), SyntaxFailure> {
        if self.consume(symbol) {
            Ok(())
        } else {
            Err(self.failure(format!("expected '{symbol}'")))
        }
    }
    fn failure(&self, detail: impl fmt::Display) -> SyntaxFailure {
        error(self.source, self.tokens[self.at].byte_start, detail)
    }
    fn name(&mut self) -> Result<Name, SyntaxFailure> {
        if self.consume("?") {
            return Ok(Name::Wildcard);
        }
        let TokenKind::Word(value) = self.peek() else {
            return Err(self.failure("expected identifier or '?'"));
        };
        if is_keyword(value) {
            return Err(self.failure(format!("reserved identifier '{value}'")));
        }
        let value = value.clone();
        self.take();
        Ok(Name::Defined(value))
    }
    fn type_name(&mut self) -> Result<TypeName, SyntaxFailure> {
        let name = self.name()?;
        let mut dimensions = 0;
        while self.symbol("[")
            && self
                .tokens
                .get(self.at + 1)
                .is_some_and(|token| token.kind == TokenKind::Symbol("]"))
        {
            self.take();
            self.take();
            dimensions += 1;
        }
        Ok(TypeName { name, dimensions })
    }
    fn arguments(&mut self, close: &str, nonempty: bool) -> Result<Vec<Expression>, SyntaxFailure> {
        let mut args = Vec::new();
        if !nonempty && self.consume(close) {
            return Ok(args);
        }
        loop {
            args.push(self.expression(0)?);
            if self.consume(close) {
                return Ok(args);
            }
            self.expect(",")?;
        }
    }
    fn statement(&mut self) -> Result<Expression, SyntaxFailure> {
        let start = self.tokens[self.at].start;
        if self.word("return") || self.word("throw") {
            let is_return = self.word("return");
            self.take();
            let value = self.expression(0)?;
            return Ok(Expression {
                span: SourceSpan {
                    start,
                    end: value.span.end,
                },
                kind: if is_return {
                    ExprKind::Return(Box::new(value))
                } else {
                    ExprKind::Throw(Box::new(value))
                },
            });
        }
        let left = self.expression(0)?;
        let assignment_token = self.at;
        if !self.consume("=") {
            return Ok(left);
        }
        if assignment_token > 0 && self.tokens[assignment_token - 1].kind == TokenKind::Symbol(")")
        {
            return Err(self.failure("invalid parenthesized assignment target"));
        }
        let right = self.expression(0)?;
        let span = SourceSpan {
            start,
            end: right.span.end,
        };
        let kind = match left.kind {
            ExprKind::Identifier(value) => ExprKind::AssignIdentifier {
                name: Name::Defined(value),
                value: Box::new(right),
            },
            ExprKind::Wildcard => ExprKind::AssignIdentifier {
                name: Name::Wildcard,
                value: Box::new(right),
            },
            ExprKind::MemberAccess { receiver, name } => ExprKind::AssignMember {
                receiver,
                name,
                value: Box::new(right),
            },
            ExprKind::ArrayAccess { array, index } => ExprKind::ArrayStore {
                array,
                index,
                value: Box::new(right),
            },
            _ => return Err(self.failure("invalid assignment target")),
        };
        Ok(Expression { span, kind })
    }
    fn expression(&mut self, minimum: u8) -> Result<Expression, SyntaxFailure> {
        self.depth += 1;
        if self.depth > 512 {
            return Err(self.failure("expression nesting exceeds safe parser depth"));
        }
        let result = self.expression_inner(minimum);
        self.depth -= 1;
        result
    }
    fn expression_inner(&mut self, minimum: u8) -> Result<Expression, SyntaxFailure> {
        let syntax_start = self.tokens[self.at].start;
        let mut left = self.prefix()?;
        loop {
            let start = syntax_start;
            let next = if minimum <= 23 && self.consume("[") {
                let index = self.expression(0)?;
                self.expect("]")?;
                ExprKind::ArrayAccess {
                    array: Box::new(left),
                    index: Box::new(index),
                }
            } else if minimum <= 22 && self.consume(".") {
                if self.word("class") {
                    if self.at >= 2 && self.tokens[self.at - 2].kind == TokenKind::Symbol(")") {
                        return Err(self.failure("invalid parenthesized class literal type"));
                    }
                    self.take();
                    let target = match left.kind {
                        ExprKind::Identifier(value) => TypeName {
                            name: Name::Defined(value),
                            dimensions: 0,
                        },
                        ExprKind::Wildcard => TypeName {
                            name: Name::Wildcard,
                            dimensions: 0,
                        },
                        _ => return Err(self.failure("invalid class literal type")),
                    };
                    ExprKind::ClassLiteral(target)
                } else {
                    let name = self.name()?;
                    if self.consume("(") {
                        ExprKind::MethodCall {
                            receiver: Box::new(left),
                            name,
                            arguments: self.arguments(")", false)?,
                        }
                    } else {
                        ExprKind::MemberAccess {
                            receiver: Box::new(left),
                            name,
                        }
                    }
                }
            } else if minimum <= 18 && self.consume("::") {
                if self.word("new") {
                    if self.at >= 2 && self.tokens[self.at - 2].kind == TokenKind::Symbol(")") {
                        return Err(
                            self.failure("invalid parenthesized constructor reference type")
                        );
                    }
                    self.take();
                    let target = match left.kind {
                        ExprKind::Identifier(value) => TypeName {
                            name: Name::Defined(value),
                            dimensions: 0,
                        },
                        ExprKind::Wildcard => TypeName {
                            name: Name::Wildcard,
                            dimensions: 0,
                        },
                        _ => return Err(self.failure("invalid constructor reference type")),
                    };
                    ExprKind::ConstructorReference(target)
                } else {
                    ExprKind::BoundMethodReference {
                        receiver: Box::new(left),
                        name: self.name()?,
                    }
                }
            } else if minimum <= 6 && self.word("instanceof") {
                self.take();
                ExprKind::InstanceOf {
                    value: Box::new(left),
                    target: self.type_name()?,
                }
            } else if let Some((operator, precedence)) = binary(self.peek()) {
                if precedence < minimum {
                    break;
                }
                self.take();
                let right = self.expression(precedence + 1)?;
                ExprKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                }
            } else {
                break;
            };
            left = Expression {
                span: SourceSpan {
                    start,
                    end: self.tokens[self.at - 1].end,
                },
                kind: next,
            };
        }
        Ok(left)
    }
    fn prefix(&mut self) -> Result<Expression, SyntaxFailure> {
        let start = self.tokens[self.at].start;
        let kind = if self.consume("@") {
            self.expect("(")?;
            self.explicit_capture = true;
            let value = self.expression(0)?;
            self.expect(")")?;
            ExprKind::Capture(Box::new(value))
        } else if self.consume("::") {
            ExprKind::FreeMethodReference(self.name()?)
        } else if self.word("super") {
            self.take();
            self.expect(".")?;
            let name = self.name()?;
            self.expect("(")?;
            ExprKind::SuperCall {
                name,
                arguments: self.arguments(")", false)?,
            }
        } else if self.word("new") {
            self.take();
            let name = self.name()?;
            let target = TypeName {
                name,
                dimensions: 0,
            };
            if self.consume("(") {
                ExprKind::New {
                    target,
                    arguments: self.arguments(")", false)?,
                }
            } else {
                let mut dims = Vec::new();
                let mut blank = 0;
                while self.consume("[") {
                    if self.consume("]") {
                        blank += 1;
                    } else {
                        if blank > 0 {
                            return Err(self.failure("specified dimension follows empty dimension"));
                        }
                        dims.push(self.expression(0)?);
                        self.expect("]")?;
                    }
                }
                if dims.is_empty() && blank > 0 && self.consume("{") {
                    ExprKind::ArrayLiteral {
                        element: TypeName {
                            name: target.name,
                            dimensions: blank - 1,
                        },
                        values: self.arguments("}", true)?,
                    }
                } else if !dims.is_empty() {
                    ExprKind::NewArray {
                        element: target,
                        dimensions: dims,
                        blank_dimensions: blank,
                    }
                } else {
                    return Err(self.failure("expected constructor arguments or array dimensions"));
                }
            }
        } else if self.symbol("-") || self.symbol("~") {
            let negative = self.consume("-");
            if !negative {
                self.expect("~")?;
            }
            if negative && matches!(self.peek(), TokenKind::Int(_) | TokenKind::Decimal(_)) {
                match self.take().kind {
                    TokenKind::Int(value) => ExprKind::Int(if value == i64::MAX as u64 + 1 {
                        i64::MIN
                    } else {
                        -i64::try_from(value).map_err(|e| self.failure(e))?
                    }),
                    TokenKind::Decimal(value) => ExprKind::Decimal(-value),
                    _ => unreachable!(),
                }
            } else {
                ExprKind::Unary {
                    operator: if negative {
                        UnaryOperator::Negate
                    } else {
                        UnaryOperator::BitNot
                    },
                    value: Box::new(self.expression(15)?),
                }
            }
        } else if self.consume("(") {
            let save = self.at;
            let explicit = self.explicit_capture;
            let tentative = self.type_name().ok().filter(|_| self.consume(")"));
            if let Some(target) = tentative.filter(|_| starts_expression(self.peek())) {
                match self.expression(11) {
                    Ok(value) => {
                        return Ok(Expression {
                            span: SourceSpan {
                                start,
                                end: value.span.end,
                            },
                            kind: ExprKind::Cast {
                                target,
                                value: Box::new(value),
                            },
                        })
                    }
                    Err(_) => {
                        self.at = save;
                        self.explicit_capture = explicit;
                    }
                }
            } else {
                self.at = save;
                self.explicit_capture = explicit;
            }
            let value = self.expression(0)?;
            self.expect(")")?;
            return Ok(value);
        } else {
            let token = self.take();
            match token.kind {
                TokenKind::Int(value) => {
                    ExprKind::Int(i64::try_from(value).map_err(|e| self.failure(e))?)
                }
                TokenKind::Decimal(value) => ExprKind::Decimal(value),
                TokenKind::String(value) => ExprKind::String(value),
                TokenKind::StringUtf16(value) => ExprKind::StringUtf16(value),
                TokenKind::Word(value) if value == "true" || value == "false" => {
                    ExprKind::Boolean(value == "true")
                }
                TokenKind::Word(value) if value == "null" => ExprKind::Null,
                TokenKind::Word(value) if value == "this" => ExprKind::This,
                TokenKind::Word(value) if !is_keyword(&value) => {
                    if self.consume("(") {
                        ExprKind::StaticCall {
                            name: Name::Defined(value),
                            arguments: self.arguments(")", false)?,
                        }
                    } else if self.symbol("[")
                        && self
                            .tokens
                            .get(self.at + 1)
                            .is_some_and(|token| token.kind == TokenKind::Symbol("]"))
                    {
                        let mut dimensions = 0;
                        while self.consume("[") {
                            self.expect("]")?;
                            dimensions += 1;
                        }
                        self.expect(".")?;
                        if !self.word("class") {
                            return Err(self.failure("expected 'class'"));
                        }
                        self.take();
                        ExprKind::ClassLiteral(TypeName {
                            name: Name::Defined(value),
                            dimensions,
                        })
                    } else {
                        ExprKind::Identifier(value)
                    }
                }
                TokenKind::Symbol("?") => {
                    if self.consume("(") {
                        ExprKind::StaticCall {
                            name: Name::Wildcard,
                            arguments: self.arguments(")", false)?,
                        }
                    } else if self.symbol("[")
                        && self
                            .tokens
                            .get(self.at + 1)
                            .is_some_and(|token| token.kind == TokenKind::Symbol("]"))
                    {
                        let mut dimensions = 0;
                        while self.consume("[") {
                            self.expect("]")?;
                            dimensions += 1;
                        }
                        self.expect(".")?;
                        if !self.word("class") {
                            return Err(self.failure("expected 'class'"));
                        }
                        self.take();
                        ExprKind::ClassLiteral(TypeName {
                            name: Name::Wildcard,
                            dimensions,
                        })
                    } else {
                        ExprKind::Wildcard
                    }
                }
                _ => return Err(error(self.source, token.byte_start, "expected expression")),
            }
        };
        Ok(Expression {
            span: SourceSpan {
                start,
                end: self.tokens[self.at.saturating_sub(1)].end,
            },
            kind,
        })
    }
}
fn starts_expression(token: &TokenKind) -> bool {
    match token {
        TokenKind::Int(_)
        | TokenKind::Decimal(_)
        | TokenKind::String(_)
        | TokenKind::StringUtf16(_) => true,
        TokenKind::Word(value) => {
            !is_keyword(value)
                || matches!(
                    value.as_str(),
                    "this" | "new" | "super" | "true" | "false" | "null"
                )
        }
        TokenKind::Symbol("@" | "?" | "::" | "-" | "~" | "(") => true,
        _ => false,
    }
}
fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "new"
            | "instanceof"
            | "true"
            | "false"
            | "null"
            | "return"
            | "throw"
            | "this"
            | "super"
            | "class"
            | "assert"
            | "break"
            | "case"
            | "catch"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "else"
            | "finally"
            | "for"
            | "goto"
            | "if"
            | "switch"
            | "synchronized"
            | "try"
            | "while"
            | "yield"
            | "_"
    )
}
fn binary(token: &TokenKind) -> Option<(BinaryOperator, u8)> {
    use BinaryOperator::*;
    Some(match token {
        TokenKind::Symbol("*") => (Multiply, 10),
        TokenKind::Symbol("/") => (Divide, 10),
        TokenKind::Symbol("%") => (Modulo, 10),
        TokenKind::Symbol("+") => (Add, 9),
        TokenKind::Symbol("-") => (Subtract, 9),
        TokenKind::Symbol("<<") => (ShiftLeft, 8),
        TokenKind::Symbol(">>") => (ShiftRight, 8),
        TokenKind::Symbol(">>>") => (UnsignedShiftRight, 8),
        TokenKind::Symbol("<") => (Less, 7),
        TokenKind::Symbol("<=") => (LessEqual, 7),
        TokenKind::Symbol(">") => (Greater, 7),
        TokenKind::Symbol(">=") => (GreaterEqual, 7),
        TokenKind::Symbol("==") => (Equal, 5),
        TokenKind::Symbol("!=") => (NotEqual, 5),
        TokenKind::Symbol("&") => (BitAnd, 4),
        TokenKind::Symbol("^") => (BitXor, 3),
        TokenKind::Symbol("|") => (BitOr, 2),
        _ => return None,
    })
}
pub fn parse_expression(source: &str) -> Result<Expression, SyntaxFailure> {
    parse_expression_inner(source, &std::collections::BTreeMap::new())
}
/// Parses a decoded JVM string without replacing isolated UTF-16 code units.
/// Spans retain the official code-point convention (implicit capture end is UTF-16 length).
pub fn parse_expression_utf16(units: &[u16]) -> Result<Expression, SyntaxFailure> {
    let source = Utf16Source::new(units);
    parse_expression_inner(&source.text, &source.surrogates)
}
pub fn codepoint_offset_to_utf16(units: &[u16], codepoints: usize) -> Option<usize> {
    let mut decoded = char::decode_utf16(units.iter().copied());
    let mut offset = 0;
    for _ in 0..codepoints {
        offset += match decoded.next()? {
            Ok(ch) => ch.len_utf16(),
            Err(_) => 1,
        };
    }
    Some(offset)
}
fn parse_expression_inner(
    source: &str,
    surrogates: &std::collections::BTreeMap<usize, u16>,
) -> Result<Expression, SyntaxFailure> {
    let mut parser = Parser {
        source,
        tokens: lex(source, surrogates)?,
        at: 0,
        explicit_capture: false,
        depth: 0,
    };
    let expression = parser.statement()?;
    if parser.peek() != &TokenKind::Eof {
        return Err(parser.failure("expected end of expression"));
    }
    if parser.explicit_capture {
        Ok(expression)
    } else {
        Ok(Expression {
            span: SourceSpan {
                start: 0,
                end: source.encode_utf16().count(),
            },
            kind: ExprKind::Capture(Box::new(expression)),
        })
    }
}
