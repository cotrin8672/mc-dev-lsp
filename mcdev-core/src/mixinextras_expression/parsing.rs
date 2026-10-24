use crate::extras_model::*;
use crate::mixin_source::*;
use crate::ClassIndex;
use std::ops::Range;

pub struct ClassLiteralTypeNameResolver<'a> {
    imports: Option<JavaSourceImports>,
    class_index: Option<&'a dyn ClassIndex>,
}
impl<'a> ClassLiteralTypeNameResolver<'a> {
    pub const FAIL_CLOSED: Self = Self {
        imports: None,
        class_index: None,
    };
    pub fn for_source(source: &str, class_index: &'a dyn ClassIndex) -> Self {
        Self::for_imports(
            &JavaTypeDescriptorResolver::imports_for(source),
            class_index,
        )
    }
    pub fn for_imports(imports: &JavaSourceImports, class_index: &'a dyn ClassIndex) -> Self {
        Self {
            imports: Some(imports.clone()),
            class_index: Some(class_index),
        }
    }
    pub fn resolve(&self, name: &str) -> Option<String> {
        if name.trim().is_empty() || is_method_descriptor_like(name) {
            return None;
        }
        let mut base = if self.imports.is_some() {
            name.trim()
        } else {
            name
        };
        let mut dimensions = 0;
        while let Some(rest) = base.strip_suffix("[]") {
            base = if self.imports.is_some() {
                rest.trim()
            } else {
                rest
            };
            dimensions += 1;
        }
        let descriptor = if let Some(primitive) = primitive(base) {
            primitive.to_string()
        } else if base.starts_with('[') || base.starts_with('L') && base.ends_with(';') {
            if !is_valid_class_literal_resolved_type(base) {
                return None;
            }
            base.to_string()
        } else if let (Some(imports), Some(index)) = (&self.imports, self.class_index) {
            if base.contains('/') {
                format!("L{};", index.find_class(base)?.internal_name)
            } else if imports.is_kotlin {
                if base.contains('.') || imports.explicit.contains_key(base) {
                    if let Some(owner) = resolve_source_class(base, imports, index) {
                        format!("L{owner};")
                    } else {
                        return self.kotlin_descriptor(base, imports, index, dimensions);
                    }
                } else {
                    return self.kotlin_descriptor(base, imports, index, dimensions);
                }
            } else {
                format!("L{};", resolve_source_class(base, imports, index)?)
            }
        } else {
            if base.starts_with('L') && !base.ends_with(';') {
                return None;
            }
            if !base.contains(['.', '/']) {
                return None;
            }
            format!("L{};", base.replace('.', "/"))
        };
        if dimensions > 0 && descriptor == "V" {
            return None;
        }
        Some("[".repeat(dimensions) + &descriptor)
    }
    fn kotlin_descriptor(
        &self,
        base: &str,
        imports: &JavaSourceImports,
        index: &dyn ClassIndex,
        dimensions: usize,
    ) -> Option<String> {
        let lookup = ClassIndexJavaTypeLookup { class_index: index };
        let context = JavaTypeResolutionContext {
            imports,
            lookup: Some(&lookup),
        };
        let descriptor = JavaTypeDescriptorResolver::descriptor_or_null(base, &context)?;
        if dimensions > 0 && descriptor == "V" {
            return None;
        }
        Some("[".repeat(dimensions) + &descriptor)
    }
}
fn primitive(name: &str) -> Option<&'static str> {
    Some(match name {
        "void" => "V",
        "boolean" => "Z",
        "byte" => "B",
        "char" => "C",
        "short" => "S",
        "int" => "I",
        "long" => "J",
        "float" => "F",
        "double" => "D",
        _ => return None,
    })
}
fn resolve_source_class(
    name: &str,
    imports: &JavaSourceImports,
    index: &dyn ClassIndex,
) -> Option<String> {
    let first = name.split('.').next()?;
    if imports.ambiguous_explicit.contains_key(first) {
        return None;
    }
    if let Some(explicit) = imports.explicit.get(name) {
        return resolve_qualified(explicit, index);
    }
    if let Some(explicit) = imports.explicit.get(first) {
        if let Some(rest) = name.strip_prefix(&format!("{first}.")) {
            if let Some(found) = resolve_qualified(&format!("{explicit}.{rest}"), index) {
                return Some(found);
            }
        }
    }
    if name.contains('.') {
        if let Some(found) = resolve_qualified(name, index) {
            return Some(found);
        }
    }
    if let Some(package) = &imports.package_name {
        if let Some(found) = resolve_qualified(&format!("{package}.{name}"), index) {
            return Some(found);
        }
    }
    let mut candidates = std::collections::BTreeSet::new();
    for package in imports
        .wildcard_packages
        .iter()
        .map(String::as_str)
        .chain(["java.lang"])
    {
        for value in resolve_qualified_all(&format!("{package}.{name}"), index) {
            candidates.insert(value);
        }
    }
    if candidates.len() == 1 {
        return candidates.into_iter().next();
    }
    if candidates.len() > 1 {
        return None;
    }
    if !name.contains('.') {
        resolve_qualified(name, index)
    } else {
        None
    }
}
fn resolve_qualified_all(name: &str, index: &dyn ClassIndex) -> Vec<String> {
    let mut candidates = std::collections::BTreeSet::new();
    if let Some(c) = index.find_class_by_fqn(name) {
        candidates.insert(c.internal_name);
    }
    if name.contains('/') {
        if let Some(c) = index.find_class(name) {
            candidates.insert(c.internal_name);
        }
    }
    let parts: Vec<_> = name.split('.').collect();
    for start in 1..parts.len() {
        let nested = format!("{}.{}", parts[..start].join("."), parts[start..].join("$"));
        if let Some(c) = index.find_class_by_fqn(&nested) {
            candidates.insert(c.internal_name);
        }
    }
    candidates.into_iter().collect()
}
fn resolve_qualified(name: &str, index: &dyn ClassIndex) -> Option<String> {
    let values = resolve_qualified_all(name, index);
    if values.len() == 1 {
        values.into_iter().next()
    } else {
        None
    }
}
pub fn is_method_descriptor_like(name: &str) -> bool {
    name.starts_with('(')
        || name
            .find(';')
            .is_some_and(|at| name[at + 1..].contains('('))
}
pub fn is_valid_class_literal_resolved_type(descriptor: &str) -> bool {
    descriptor == "V" || crate::descriptor::parse_field_descriptor(descriptor).is_ok()
}
pub fn class_literal_type_resolution_failure_message(name: &str) -> &'static str {
    if is_method_descriptor_like(name) {
        "method descriptor is not a type"
    } else {
        let base = name.split('[').next().unwrap_or("");
        if base.starts_with(['[', 'L']) || base.len() == 1 && "BCDFIJSVL".contains(base) {
            "invalid type descriptor"
        } else {
            "unresolved type name"
        }
    }
}

pub struct DefinitionAnnotationParser;
pub struct DefinitionsAnnotationParser;
pub struct ExpressionAnnotationParser;
pub struct ExpressionsAnnotationParser;

pub(super) fn whitespace(source: &str, mut at: usize) -> usize {
    while at < source.len() {
        let ch = source[at..].chars().next().unwrap();
        if ch.is_whitespace() {
            at += ch.len_utf8();
        } else if source[at..].starts_with("//") {
            at = source[at..].find('\n').map_or(source.len(), |n| at + n + 1);
        } else if source[at..].starts_with("/*") {
            at = source[at + 2..]
                .find("*/")
                .map_or(source.len(), |n| at + n + 4);
        } else {
            break;
        }
    }
    at
}
fn identifier(source: &str, at: usize) -> Option<usize> {
    let first = source.get(at..)?.chars().next()?;
    if !(first.is_alphabetic() || first == '_' || first == '$') {
        return None;
    }
    Some(
        at + source[at..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .map(char::len_utf8)
            .sum::<usize>(),
    )
}
pub(super) fn matching_close(source: &str, at: usize, kotlin: bool) -> Option<usize> {
    let tokens = source_tokens(source, kotlin);
    let token = tokens.iter().position(|t| t.start == at)?;
    matching(&tokens, token).map(|n| tokens[n].start)
}
pub(super) fn comma_parts(source: &str, kotlin: bool) -> Vec<Range<usize>> {
    let tokens = source_tokens(source, kotlin);
    let mut depth = 0;
    let mut out = vec![];
    let mut start = 0;
    for t in tokens {
        match t.text.as_str() {
            "(" | "{" | "[" | "<" => depth += 1,
            ")" | "}" | "]" | ">" => depth = (depth - 1).max(0),
            "," if depth == 0 => {
                out.push(start..t.start);
                start = t.end;
            }
            _ => {}
        }
    }
    out.push(start..source.len());
    out
}
fn string_literal(
    source: &str,
    at: usize,
    kotlin: bool,
    expression: bool,
) -> Option<(String, usize, Range<usize>)> {
    let (units, end, range) = string_literal_units(source, at, kotlin, expression)?;
    Some((String::from_utf16(&units).ok()?, end, range))
}
fn string_literal_units(
    source: &str,
    at: usize,
    kotlin: bool,
    expression: bool,
) -> Option<(Vec<u16>, usize, Range<usize>)> {
    if source.as_bytes().get(at) != Some(&b'"') {
        return None;
    }
    if kotlin && source[at..].starts_with("\"\"\"") {
        let start = at + 3;
        let end = start + source[start..].find("\"\"\"")?;
        return Some((
            source[start..end].encode_utf16().collect(),
            end + 3,
            start..end,
        ));
    }
    let mut value = Vec::new();
    let mut index = at + 1;
    while index < source.len() {
        let c = source[index..].chars().next()?;
        if c == '"' {
            return Some((value, index + 1, at + 1..index));
        }
        if c == '\\' {
            if expression && source.as_bytes().get(index + 1) == Some(&b'u') {
                let e = JavaStringContentDecoder::decode_unicode_escape_utf16(
                    source,
                    index,
                    source.len(),
                )?;
                if e.units.iter().any(|u| matches!(*u, 34 | 92 | 10 | 13)) {
                    return None;
                }
                value.extend(e.units);
                index = e.next_index;
            } else {
                let escape = JavaStringContentDecoder::decode_escape(source, index, source.len())?;
                let mut units = [0; 2];
                value.extend_from_slice(escape.r#char.encode_utf16(&mut units));
                index = escape.next_index;
            }
        } else {
            let mut units = [0; 2];
            value.extend_from_slice(c.encode_utf16(&mut units));
            index += c.len_utf8();
        }
    }
    None
}
fn skip_value(source: &str, start: usize, kotlin: bool) -> usize {
    let at = whitespace(source, start);
    if at >= source.len() {
        return at;
    }
    let tokens = source_tokens(&source[at..], kotlin);
    let mut depth = 0;
    let mut end = at;
    for t in tokens {
        match t.text.as_str() {
            "[" if source.as_bytes().get(at) == Some(&b'[') => {}
            "(" | "{" | "[" | "<" => depth += 1,
            ")" | "}" | "]" | ">" if depth > 0 => depth -= 1,
            "," | ")" if depth == 0 => return at + t.start,
            _ => {}
        }
        end = at + t.end;
    }
    end.max(source.len())
}
fn strings(
    source: &str,
    start: usize,
    kotlin: bool,
    expression: bool,
) -> Option<(Vec<String>, usize)> {
    let (units, end) = strings_units(source, start, kotlin, expression)?;
    Some((
        units
            .iter()
            .map(|u| String::from_utf16(u))
            .collect::<Result<Vec<_>, _>>()
            .ok()?,
        end,
    ))
}
fn strings_units(
    source: &str,
    start: usize,
    kotlin: bool,
    expression: bool,
) -> Option<(Vec<Vec<u16>>, usize)> {
    let at = whitespace(source, start);
    if source.as_bytes().get(at) == Some(&b'"') {
        let (v, end, _) = string_literal_units(source, at, kotlin, expression)?;
        return Some((vec![v], end));
    }
    if source.as_bytes().get(at) != Some(&b'{')
        && !(kotlin && source.as_bytes().get(at) == Some(&b'['))
    {
        return None;
    }
    let close = matching_close(source, at, kotlin)?;
    let body = &source[at + 1..close];
    let parts = comma_parts(body, kotlin);
    let mut values = vec![];
    for (i, r) in parts.iter().enumerate() {
        let part = &body[r.clone()];
        let p = whitespace(part, 0);
        if p == part.len() {
            if !expression && parts.len() > 1 && (!kotlin || i + 1 < parts.len())
                || expression && kotlin && i + 1 < parts.len()
            {
                return None;
            }
            continue;
        }
        let (v, end, _) = string_literal_units(part, p, kotlin, expression)?;
        if whitespace(part, end) != part.len() {
            return None;
        }
        values.push(v);
    }
    Some((values, close + 1))
}
fn class_literal(source: &str, start: usize, kotlin: bool) -> Option<(String, usize)> {
    let at = whitespace(source, start);
    let tokens = source_tokens(&source[at..], kotlin);
    let mut name = String::new();
    let mut n = 0;
    while n < tokens.len() {
        let t = &tokens[n];
        if (!kotlin && t.text == "." && tokens.get(n + 1).is_some_and(|t| t.text == "class"))
            || (kotlin
                && t.text == ":"
                && tokens.get(n + 1).is_some_and(|t| t.text == ":")
                && tokens.get(n + 2).is_some_and(|t| t.text == "class"))
        {
            let suffix = if kotlin { 2 } else { 1 };
            let end = at + tokens[n + suffix].end;
            return valid_type_name(&name).then_some((name, end));
        }
        if t.text == "."
            || t.text == "["
            || t.text == "]"
            || identifier(&t.text, 0) == Some(t.text.len())
        {
            name.push_str(&t.text);
        } else {
            return None;
        }
        n += 1;
    }
    None
}
fn valid_type_name(name: &str) -> bool {
    let base = name.split('[').next().unwrap_or("");
    let suffix = &name[base.len()..];
    if suffix.len() % 2 != 0 || !suffix.as_bytes().chunks(2).all(|v| v == b"[]") {
        return false;
    }
    if primitive(base).is_some() {
        return true;
    }
    base.split(['.', '$']).all(|s| {
        !s.is_empty()
            && s.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            && s.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    })
}
fn array_or_scalar<T>(
    source: &str,
    start: usize,
    kotlin: bool,
    parse: impl Fn(&str, usize) -> Option<(T, usize)>,
) -> Option<(Vec<T>, usize)> {
    let at = whitespace(source, start);
    if source.as_bytes().get(at) != Some(&b'{')
        && !(kotlin && source.as_bytes().get(at) == Some(&b'['))
    {
        let (v, end) = parse(source, at)?;
        return Some((vec![v], end));
    }
    let close = matching_close(source, at, kotlin)?;
    let body = &source[at + 1..close];
    let parts = comma_parts(body, kotlin);
    let mut out = vec![];
    for (i, r) in parts.iter().enumerate() {
        let part = &body[r.clone()];
        let p = whitespace(part, 0);
        if p == part.len() {
            if parts.len() > 1 && (!kotlin || i + 1 < parts.len()) {
                return None;
            }
            continue;
        }
        let (v, end) = parse(part, p)?;
        if whitespace(part, end) != part.len() {
            return None;
        }
        out.push(v);
    }
    Some((out, close + 1))
}
fn bool_literal(source: &str, start: usize) -> Option<(bool, usize)> {
    let at = whitespace(source, start);
    for (name, value) in [("true", true), ("false", false)] {
        if source[at..].starts_with(name)
            && source[at + name.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$')
        {
            return Some((value, at + name.len()));
        }
    }
    None
}
fn annotation_name(
    source: &str,
    start: usize,
    expected: &str,
    imports: Option<&JavaSourceImports>,
) -> Option<(usize, usize)> {
    let kotlin = imports.is_some_and(|i| i.is_kotlin);
    let at = whitespace(source, start);
    let mut index = at;
    if source.as_bytes().get(index) == Some(&b'@') {
        index += 1;
    } else if !kotlin {
        return None;
    }
    let name_start = index;
    while index < source.len() {
        let c = source[index..].chars().next()?;
        if c.is_alphanumeric() || "._$".contains(c) {
            index += c.len_utf8();
        } else {
            break;
        }
    }
    let name = &source[name_start..index];
    let package = if expected == "Local" {
        "com.llamalad7.mixinextras.sugar"
    } else {
        "com.llamalad7.mixinextras.expression"
    };
    let official = format!("{package}.{expected}");
    let allowed = if kotlin {
        let i = imports.unwrap();
        !i.ambiguous_explicit.contains_key(name)
            && if let Some(import) = i.explicit.get(name) {
                import == &official
            } else {
                name == official
                    || name == expected && i.wildcard_packages.iter().any(|p| p == package)
            }
    } else {
        name == expected || name == official
    };
    if !allowed {
        return None;
    }
    Some((index, at))
}
fn local(
    source: &str,
    start: usize,
    imports: Option<&JavaSourceImports>,
) -> Option<(LocalSugarSpec, usize)> {
    let kotlin = imports.is_some_and(|i| i.is_kotlin);
    let (index, _) = annotation_name(source, start, "Local", imports)?;
    let index = whitespace(source, index);
    let mut result = LocalSugarSpec::default();
    if source.as_bytes().get(index) != Some(&b'(') {
        return Some((result, index));
    }
    let end = matching_close(source, index, kotlin)?;
    let body = &source[index + 1..end];
    for range in comma_parts(body, kotlin) {
        let part = &body[range];
        let Some(eq) = part.find('=') else {
            continue;
        };
        let key = part[..eq].trim();
        let value_start = whitespace(part, eq + 1);
        match key {
            "argsOnly" => result.args_only = bool_literal(part, value_start).is_some_and(|v| v.0),
            "print" => result.print = bool_literal(part, value_start).is_some_and(|v| v.0),
            "index" | "ordinal" => {
                let raw = &part[value_start..];
                let len = raw
                    .char_indices()
                    .take_while(|(i, c)| c.is_ascii_digit() || *i == 0 && *c == '-')
                    .last()
                    .map_or(0, |(i, c)| i + c.len_utf8());
                let value = raw[..len]
                    .parse::<i32>()
                    .ok()
                    .filter(|v| *v != -1)
                    .filter(|_| {
                        raw[len..]
                            .chars()
                            .next()
                            .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$')
                    });
                if key == "index" {
                    result.index = value;
                } else {
                    result.ordinal = value;
                }
            }
            "name" => {
                if let Some((values, _)) = strings(part, value_start, kotlin, false) {
                    result.names.extend(values);
                }
            }
            "type" => {
                result.type_class_name = class_literal(part, value_start, kotlin).map(|v| v.0)
            }
            _ => {}
        }
    }
    Some((result, end + 1))
}
impl DefinitionAnnotationParser {
    pub fn parse(body: &str, imports: Option<&JavaSourceImports>) -> MixinExtrasDefinition {
        let kotlin = imports.is_some_and(|i| i.is_kotlin);
        let mut out = MixinExtrasDefinition {
            body_length: body.len(),
            ..Default::default()
        };
        let mut index = whitespace(body, 0);
        while index < body.len() {
            index = whitespace(body, index);
            if index >= body.len() {
                break;
            }
            let attribute_start = index;
            let Some(name_end) = identifier(body, index) else {
                break;
            };
            let name = &body[index..name_end];
            index = whitespace(body, name_end);
            if body.as_bytes().get(index) != Some(&b'=') {
                let end = skip_value(body, index, kotlin);
                out.parse_issues.push(DefinitionAnnotationParseIssue {
                    attribute: name.into(),
                    raw_value: body[index..end].trim().into(),
                    message: "expected '=' after attribute".into(),
                    body_range: attribute_start..end,
                });
                index = end;
                if body.as_bytes().get(index) == Some(&b',') {
                    index += 1;
                }
                continue;
            }
            index = whitespace(body, index + 1);
            let value_start = index;
            let mut error = None;
            match name {
                "id" => {
                    if let Some((value, end, range)) = string_literal(body, index, kotlin, false) {
                        out.id = Some(value);
                        out.id_content_range = Some(range);
                        index = end;
                    } else {
                        error = Some("expected a string literal");
                    }
                }
                "method" | "field" => {
                    if let Some((values, end)) = strings(body, index, kotlin, false) {
                        if name == "method" {
                            out.raw_method_references.extend(values);
                        } else {
                            out.raw_field_references.extend(values);
                        }
                        index = end;
                    } else {
                        error = Some("expected a string or string array");
                    }
                }
                "type" => {
                    if let Some((values, end)) =
                        array_or_scalar(body, index, kotlin, |s, p| class_literal(s, p, kotlin))
                    {
                        out.class_literal_type_names.extend(values);
                        index = end;
                    } else {
                        error = Some("expected a class literal or class literal array");
                    }
                }
                "remap" => {
                    if let Some((value, end)) = bool_literal(body, index) {
                        out.remap = Some(value);
                        index = end;
                    } else {
                        error = Some("expected a boolean literal");
                    }
                }
                "local" => {
                    if let Some((values, end)) =
                        array_or_scalar(body, index, kotlin, |s, p| local(s, p, imports))
                    {
                        out.local_specs.extend(values);
                        index = end;
                    } else {
                        error = Some("expected @Local or @Local array");
                    }
                }
                _ => index = skip_value(body, index, kotlin),
            }
            if let Some(message) = error {
                index = skip_value(body, value_start, kotlin);
                out.parse_issues.push(DefinitionAnnotationParseIssue {
                    attribute: name.into(),
                    raw_value: body[value_start..index].trim().into(),
                    message: message.into(),
                    body_range: value_start..index,
                });
            }
            index = whitespace(body, index);
            if body.as_bytes().get(index) == Some(&b',') {
                index += 1;
            }
        }
        out
    }
}
impl ExpressionAnnotationParser {
    pub fn parse(body: &str, imports: Option<&JavaSourceImports>) -> Option<MixinExtrasExpression> {
        let kotlin = imports.is_some_and(|i| i.is_kotlin);
        let mut index = whitespace(body, 0);
        if index >= body.len() {
            return None;
        }
        if matches!(body.as_bytes()[index], b'"' | b'{') || kotlin && body.as_bytes()[index] == b'['
        {
            let (values, end) = strings_units(body, index, kotlin, true)?;
            let end = trailing(body, end, kotlin);
            return (end == body.len())
                .then(|| MixinExtrasExpression::from_utf16(String::new(), values));
        }
        let mut id = None;
        let mut values = None;
        while index < body.len() {
            index = whitespace(body, index);
            if index == body.len() {
                break;
            }
            let name_end = identifier(body, index)?;
            let name = &body[index..name_end];
            index = whitespace(body, name_end);
            if body.as_bytes().get(index) != Some(&b'=') {
                return None;
            }
            index = whitespace(body, index + 1);
            match name {
                "id" => {
                    if id.is_some() {
                        return None;
                    }
                    let (value, end, _) = string_literal(body, index, kotlin, true)?;
                    id = Some(value);
                    index = end;
                }
                "value" => {
                    if values.is_some() {
                        return None;
                    }
                    let (value, end) = strings_units(body, index, kotlin, true)?;
                    values = Some(value);
                    index = end;
                }
                _ => return None,
            }
            index = whitespace(body, index);
            if body.as_bytes().get(index) == Some(&b',') {
                index += 1;
            }
        }
        Some(MixinExtrasExpression::from_utf16(
            id.unwrap_or_default(),
            values?,
        ))
    }
}
fn trailing(source: &str, end: usize, kotlin: bool) -> usize {
    let mut end = whitespace(source, end);
    if kotlin && source.as_bytes().get(end) == Some(&b',') {
        end = whitespace(source, end + 1);
    }
    end
}
impl DefinitionsAnnotationParser {
    pub fn parse_definition_at(
        source: &str,
        offset: usize,
        base: usize,
        imports: Option<&JavaSourceImports>,
    ) -> Option<MixinExtrasDefinition> {
        read_definition(source, offset, base, imports).map(|v| v.0)
    }
    pub fn parse(
        body: &str,
        base: usize,
        imports: Option<&JavaSourceImports>,
    ) -> MixinExtrasDefinitionIndex {
        MixinExtrasDefinitionIndex {
            definitions: container(
                body,
                imports,
                |s, p| read_definition(s, p, base, imports),
                true,
            )
            .unwrap_or_default(),
        }
    }
}
fn read_definition(
    source: &str,
    start: usize,
    base: usize,
    imports: Option<&JavaSourceImports>,
) -> Option<(MixinExtrasDefinition, usize)> {
    let kotlin = imports.is_some_and(|i| i.is_kotlin);
    let (index, _) = annotation_name(source, start, "Definition", imports)?;
    let index = whitespace(source, index);
    if source.as_bytes().get(index) != Some(&b'(') {
        return None;
    }
    let end = matching_close(source, index, kotlin)?;
    Some((
        DefinitionAnnotationParser::parse(&source[index + 1..end], imports)
            .with_id_source_range(base + index + 1),
        whitespace(source, end + 1),
    ))
}
impl ExpressionsAnnotationParser {
    pub fn parse(body: &str, imports: Option<&JavaSourceImports>) -> MixinExtrasExpressionIndex {
        MixinExtrasExpressionIndex {
            expressions: container(
                body,
                imports,
                |s, p| {
                    let kotlin = imports.is_some_and(|i| i.is_kotlin);
                    let (index, _) = annotation_name(s, p, "Expression", imports)?;
                    let index = whitespace(s, index);
                    if s.as_bytes().get(index) != Some(&b'(') {
                        return None;
                    }
                    let end = matching_close(s, index, kotlin)?;
                    let expression =
                        ExpressionAnnotationParser::parse(&s[index + 1..end], imports)?;
                    if trailing(s, end + 1, kotlin) < s.len() {
                        return None;
                    }
                    Some((expression, whitespace(s, end + 1)))
                },
                false,
            )
            .unwrap_or_default(),
        }
    }
}
fn container<T>(
    body: &str,
    imports: Option<&JavaSourceImports>,
    parse: impl Fn(&str, usize) -> Option<(T, usize)>,
    definitions: bool,
) -> Option<Vec<T>> {
    let kotlin = imports.is_some_and(|i| i.is_kotlin);
    let mut index = whitespace(body, 0);
    if index >= body.len() {
        return Some(vec![]);
    }
    if body[index..].starts_with("value") {
        let end = identifier(body, index)?;
        index = whitespace(body, end);
        if body.as_bytes().get(index) != Some(&b'=') {
            return None;
        }
        index = whitespace(body, index + 1);
    }
    let mut values = vec![];
    let end;
    if body.as_bytes().get(index) == Some(&b'{')
        || kotlin && body.as_bytes().get(index) == Some(&b'[')
    {
        let close = matching_close(body, index, kotlin)?;
        if definitions {
            let mut at = index + 1;
            while at < close {
                at = whitespace(body, at);
                if at >= close {
                    break;
                }
                if kotlin && body.as_bytes().get(at) == Some(&b',') {
                    return None;
                }
                if !kotlin && body.as_bytes().get(at) != Some(&b'@') {
                    at += body[at..].chars().next()?.len_utf8();
                    continue;
                }
                if let Some((value, parsed_end)) = parse(body, at) {
                    let after = whitespace(body, parsed_end);
                    if after >= close || body.as_bytes().get(after) == Some(&b',') {
                        values.push(value);
                        at = if body.as_bytes().get(after) == Some(&b',') {
                            after + 1
                        } else {
                            after
                        };
                    } else {
                        at = skip_value(body, parsed_end, kotlin).min(close);
                        if body.as_bytes().get(at) == Some(&b',') {
                            at += 1;
                        }
                    }
                } else {
                    at = if kotlin {
                        skip_value(body, at, kotlin).min(close)
                    } else {
                        at + 1
                    };
                    if kotlin && body.as_bytes().get(at) == Some(&b',') {
                        at += 1;
                    }
                }
            }
        } else {
            let content = &body[index + 1..close];
            let ranges = comma_parts(content, kotlin);
            for (i, range) in ranges.iter().enumerate() {
                let part = &content[range.clone()];
                let at = whitespace(part, 0);
                if at == part.len() {
                    if kotlin && i + 1 < ranges.len() {
                        return None;
                    }
                    continue;
                }
                if let Some((value, _)) = parse(part, at) {
                    values.push(value);
                }
            }
        }
        end = close + 1;
    } else {
        let (value, e) = parse(body, index)?;
        values.push(value);
        end = e;
    }
    if trailing(body, end, kotlin) != body.len() {
        return None;
    }
    Some(values)
}
