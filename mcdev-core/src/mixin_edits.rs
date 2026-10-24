use crate::mixin_source::{
    matching, source_tokens, AnnotationContextExtractor, JavaTypeDescriptorResolver,
    SourceTokenKind,
};
use crate::McTextEdit;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeReference {
    pub text: String,
    pub import_fqn: Option<String>,
}
pub struct MixinImportEditBuilder;
impl MixinImportEditBuilder {
    pub fn reference_for_internal_name(
        source: &str,
        internal: &str,
        additional: &BTreeSet<String>,
    ) -> TypeReference {
        let class = internal.rsplit('/').next().unwrap_or(internal);
        if class.is_empty() {
            return TypeReference {
                text: internal.into(),
                import_fqn: None,
            };
        }
        let package = internal
            .rsplit_once('/')
            .map(|(p, _)| p.replace('/', "."))
            .unwrap_or_default();
        let name = class.replace('$', ".");
        let fqn = if package.is_empty() {
            name.clone()
        } else {
            format!("{package}.{name}")
        };
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if imports.is_kotlin {
            if let Some((alias, _)) = imports.explicit.iter().find(|(_, value)| **value == fqn) {
                return TypeReference {
                    text: alias.clone(),
                    import_fqn: None,
                };
            }
        }
        let bound = name.split('.').next().unwrap_or(&name);
        let conflict = imports.explicit.get(bound).is_some_and(|v| *v != fqn)
            || imports.ambiguous_explicit.contains_key(bound)
            || additional.iter().any(|n| {
                n != internal
                    && n.rsplit('/')
                        .next()
                        .unwrap_or(n)
                        .replace('$', ".")
                        .split('.')
                        .next()
                        == Some(bound)
            })
            || imports.wildcard_packages.iter().any(|p| *p != package);
        if class.contains('$') || conflict {
            return TypeReference {
                text: fqn,
                import_fqn: None,
            };
        }
        if imports.explicit.values().any(|v| *v == fqn)
            || imports.package_name.as_deref() == Some(package.as_str())
            || package == "java.lang"
        {
            TypeReference {
                text: name,
                import_fqn: None,
            }
        } else {
            TypeReference {
                text: name,
                import_fqn: Some(fqn),
            }
        }
    }
    pub fn needs_import(source: &str, fqn: &str) -> bool {
        Self::build_import_edit(source, fqn).is_some()
    }
    pub fn build_import_edit(source: &str, fqn: &str) -> Option<McTextEdit> {
        if fqn.trim().is_empty() || !fqn.contains('.') {
            return None;
        }
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        if imports.explicit.values().any(|v| v == fqn) {
            return None;
        }
        let package = fqn.rsplit_once('.')?.0;
        if imports.package_name.as_deref() == Some(package)
            || (imports.is_kotlin && imports.wildcard_packages.iter().any(|p| p == package))
        {
            return None;
        }
        let code = AnnotationContextExtractor::mask_non_code(source);
        let ts = source_tokens(source, imports.is_kotlin);
        let mut package_end = None;
        let mut import_end = None;
        for (i, token) in ts.iter().enumerate() {
            if token.kind != SourceTokenKind::Identifier
                || !matches!(token.text.as_str(), "package" | "import")
            {
                continue;
            }
            let line_start = source[..token.start].rfind('\n').map_or(0, |n| n + 1);
            if !code[line_start..token.start].trim().is_empty() {
                continue;
            }
            let end = if imports.is_kotlin {
                let line_end = source[token.end..]
                    .find('\n')
                    .map_or(source.len(), |n| token.end + n);
                let last = ts
                    .iter()
                    .skip(i + 1)
                    .take_while(|t| t.start < line_end)
                    .last()
                    .map_or(token.end, |t| t.end);
                if line_end < source.len() && code[last..line_end].trim().is_empty() {
                    line_end + 1
                } else {
                    last
                }
            } else {
                let Some(semicolon) = ts.iter().skip(i + 1).find(|t| t.text == ";") else {
                    continue;
                };
                semicolon.end
            };
            if token.text == "import" {
                import_end = Some(end);
            } else {
                package_end = Some(end);
            }
        }
        let mut offset = import_end.or(package_end).unwrap_or(0);
        if offset == 0 && imports.is_kotlin {
            let mut i = 0;
            while i + 3 < ts.len() {
                if ts[i].text == "@" && ts[i + 1].text == "file" && ts[i + 2].text == ":" {
                    let value = i + 3;
                    let end = if ts[value].text == "[" {
                        matching(&ts, value).map(|e| ts[e].end)?
                    } else {
                        let mut n = value;
                        while n + 2 < ts.len() && ts[n + 1].text == "." {
                            n += 2;
                        }
                        if ts.get(n + 1).is_some_and(|t| t.text == "(") {
                            matching(&ts, n + 1).map(|e| ts[e].end)?
                        } else {
                            ts[n].end
                        }
                    };
                    offset = end;
                    let line_end = source[end..].find('\n').map(|n| end + n);
                    if let Some(line_end) = line_end {
                        if code[end..line_end].trim().is_empty() {
                            offset = line_end + 1;
                        }
                    }
                    i = ts.partition_point(|t| t.start < end);
                    continue;
                }
                i += 1;
            }
        }
        let newline = if imports.is_kotlin && source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let leading = if offset > 0 && source.as_bytes().get(offset - 1) != Some(&b'\n') {
            newline
        } else {
            ""
        };
        Some(McTextEdit {
            start_offset: offset,
            end_offset: offset,
            new_text: format!(
                "{leading}import {fqn}{}{newline}",
                if imports.is_kotlin { "" } else { ";" }
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MixinSide {
    Client,
    Server,
}
pub struct MixinSideResolver;
impl MixinSideResolver {
    pub fn explicit_side_from_source(source: &str) -> Option<MixinSide> {
        let tokens = source_tokens(source, true);
        let mut depth = 0;
        let mut boundary = tokens.len();
        for (i, t) in tokens.iter().enumerate() {
            if t.kind != SourceTokenKind::Symbol && t.kind != SourceTokenKind::Identifier {
                continue;
            }
            if depth == 0
                && matches!(t.text.as_str(), "class" | "interface" | "enum" | "record")
                && tokens
                    .get(i + 1)
                    .is_some_and(|n| n.kind == SourceTokenKind::Identifier)
            {
                boundary = i;
                break;
            }
            if t.text == "{" {
                depth += 1;
            } else if t.text == "}" {
                depth -= 1;
            }
        }
        let mut sides = BTreeSet::new();
        let mut i = 0;
        while i < boundary {
            if tokens[i].text != "@" {
                i += 1;
                continue;
            }
            let mut end = i + 1;
            let mut name = String::new();
            while end < boundary
                && (tokens[end].kind == SourceTokenKind::Identifier || tokens[end].text == ".")
            {
                name.push_str(&tokens[end].text);
                end += 1;
            }
            let recognized = matches!(
                name.as_str(),
                "Environment"
                    | "net.fabricmc.api.Environment"
                    | "OnlyIn"
                    | "net.minecraftforge.api.distmarker.OnlyIn"
                    | "net.neoforged.api.distmarker.OnlyIn"
                    | "SideOnly"
                    | "cpw.mods.fml.relauncher.SideOnly"
                    | "net.minecraftforge.fml.relauncher.SideOnly"
            );
            if !recognized {
                i += 1;
                continue;
            }
            if !tokens.get(end).is_some_and(|t| t.text == "(") {
                return None;
            }
            let close = matching(&tokens, end)?;
            if close >= boundary {
                return None;
            }
            let mut body = tokens[end + 1..close]
                .iter()
                .map(|t| t.text.as_str())
                .collect::<String>();
            if body.starts_with("value") {
                body = body.split_once('=')?.1.into();
            }
            let value = body.trim_end_matches(',');
            let (enum_type, constant) = value.rsplit_once('.').unwrap_or(("", ""));
            let side = match (enum_type, constant) {
                (
                    "EnvType"
                    | "net.fabricmc.api.EnvType"
                    | "Side"
                    | "cpw.mods.fml.relauncher.Side"
                    | "net.minecraftforge.fml.relauncher.Side",
                    "CLIENT",
                ) => Some(MixinSide::Client),
                (
                    "EnvType"
                    | "net.fabricmc.api.EnvType"
                    | "Side"
                    | "cpw.mods.fml.relauncher.Side"
                    | "net.minecraftforge.fml.relauncher.Side",
                    "SERVER",
                ) => Some(MixinSide::Server),
                (
                    "Dist"
                    | "net.minecraftforge.api.distmarker.Dist"
                    | "net.neoforged.api.distmarker.Dist",
                    "CLIENT",
                ) => Some(MixinSide::Client),
                (
                    "Dist"
                    | "net.minecraftforge.api.distmarker.Dist"
                    | "net.neoforged.api.distmarker.Dist",
                    "DEDICATED_SERVER",
                ) => Some(MixinSide::Server),
                _ => None,
            };
            if let Some(side) = side {
                sides.insert(side);
            }
            i = close + 1;
        }
        if sides.len() == 1 {
            sides.first().copied()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinConfigEntry {
    pub path: String,
    pub package_name: Option<String>,
    pub mixins: Vec<String>,
    pub client: Vec<String>,
    pub server: Vec<String>,
    pub common: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinConfigEditDelta {
    pub start_offset: usize,
    pub end_offset: usize,
    pub new_text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinConfigEditResult {
    pub content: String,
    pub added: bool,
    pub array_name: String,
    pub delta: Option<MixinConfigEditDelta>,
}
pub struct MixinConfigEditor;
impl MixinConfigEditor {
    pub fn parse(content: &str, path: &str) -> Result<MixinConfigEntry, String> {
        let value = parse_config_json(content)?;
        for name in ["mixins", "client", "server", "common"] {
            if value.get(name).is_some_and(|v| !v.is_array()) {
                return Err(format!("{name} must be an array"));
            }
        }
        let strings = |v: Option<&serde_json::Value>| {
            v.and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(MixinConfigEntry {
            path: path.into(),
            package_name: value
                .get("package")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            mixins: if value.is_array() {
                strings(Some(&value))
            } else {
                strings(value.get("mixins"))
            },
            client: strings(value.get("client")),
            server: strings(value.get("server")),
            common: strings(value.get("common")),
        })
    }
    pub fn contains_entry(content: &str, name: &str) -> Result<bool, String> {
        let config = Self::parse(content, "")?;
        Ok(config
            .mixins
            .iter()
            .chain(&config.client)
            .chain(&config.server)
            .chain(&config.common)
            .any(|n| n == name))
    }
    pub fn list_mixin_classes(content: &str) -> Result<Vec<String>, String> {
        let c = Self::parse(content, "")?;
        Ok(c.mixins
            .into_iter()
            .chain(c.client)
            .chain(c.server)
            .chain(c.common)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }
    pub fn add_entry(
        content: &str,
        name: &str,
        array: &str,
    ) -> Result<MixinConfigEditResult, String> {
        if matches!(array, "mixins" | "client" | "server") {
            if let Some(result) = Self::lossless_append(content, name, array)? {
                return Ok(result);
            }
        }
        let mut value = parse_config_json(content)?;
        if value.is_array() {
            value = serde_json::json!({"mixins":value});
        }
        if !value.is_object() {
            value = serde_json::json!({});
        }
        if value.get(array).is_some_and(|v| !v.is_array()) {
            return Err(format!("{array} must be an array"));
        }
        let mut entries = value
            .get(array)
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if entries.iter().any(|s| s == name) {
            return Ok(Self::unchanged(content, array));
        }
        entries.push(name.into());
        entries.sort();
        value[array] = serde_json::json!(entries);
        Ok(MixinConfigEditResult {
            content: serde_json::to_string_pretty(&value).map_err(|e| e.to_string())? + "\n",
            added: true,
            array_name: array.into(),
            delta: None,
        })
    }
    fn unchanged(content: &str, array: &str) -> MixinConfigEditResult {
        MixinConfigEditResult {
            content: content.into(),
            added: false,
            array_name: array.into(),
            delta: None,
        }
    }
    fn insert(content: &str, offset: usize, text: String, array: &str) -> MixinConfigEditResult {
        MixinConfigEditResult {
            content: format!("{}{}{}", &content[..offset], text, &content[offset..]),
            added: true,
            array_name: array.into(),
            delta: Some(MixinConfigEditDelta {
                start_offset: offset,
                end_offset: offset,
                new_text: text,
            }),
        }
    }
    fn lossless_append(
        content: &str,
        name: &str,
        array: &str,
    ) -> Result<Option<MixinConfigEditResult>, String> {
        let tokens = json_tokens(content)?;
        if !matches!(
            tokens.first().map(|t| &t.kind),
            Some(JsonTokenKind::Punct(b'{'))
        ) {
            return Ok(None);
        }
        let pairs = json_pairs(&tokens)?;
        let close = *pairs.get(&0).ok_or("missing config object close")?;
        let mut i = 1;
        let mut first_key = None;
        let mut has_properties = false;
        let escaped = serde_json::to_string(name).map_err(|e| e.to_string())?;
        while i < close {
            if tokens[i].punct(b',') {
                i += 1;
                continue;
            }
            let key = match &tokens[i].kind {
                JsonTokenKind::String(key) => key.as_str(),
                JsonTokenKind::Other => &content[tokens[i].start..tokens[i].end],
                _ => return Ok(None),
            };
            first_key.get_or_insert(tokens[i].start);
            if !tokens.get(i + 1).is_some_and(|t| t.punct(b':')) {
                return Ok(None);
            }
            let start = i + 2;
            let value = tokens.get(start).ok_or("missing property value")?;
            if key == array {
                if !value.punct(b'[') {
                    return Ok(None);
                }
                let end = *pairs.get(&start).ok_or("missing array closing bracket")?;
                let mut entries = vec![];
                let mut cursor = start + 1;
                let mut has_values = false;
                while cursor < end {
                    if tokens[cursor].punct(b',') {
                        cursor += 1;
                        continue;
                    }
                    has_values = true;
                    if let JsonTokenKind::String(s) = &tokens[cursor].kind {
                        entries.push(s.as_str());
                    }
                    cursor = pairs.get(&cursor).copied().unwrap_or(cursor) + 1;
                }
                if entries.contains(&name) {
                    return Ok(Some(Self::unchanged(content, array)));
                }
                let (offset, text) = if !has_values {
                    (value.end, escaped)
                } else if tokens
                    .get(end.wrapping_sub(1))
                    .is_some_and(|t| t.punct(b','))
                {
                    (tokens[end].start, format!("{escaped},"))
                } else {
                    (tokens[end].start, format!(", {escaped}"))
                };
                return Ok(Some(Self::insert(content, offset, text, array)));
            }
            has_properties = true;
            i = pairs.get(&start).copied().unwrap_or(start) + 1;
        }
        let line_ending = if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let indent = first_key
            .map(|start| {
                let before = &content[..start];
                let line = before.rfind(['\n', '\r']).map_or(0, |n| n + 1);
                before[line..]
                    .chars()
                    .rev()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            })
            .unwrap_or_else(|| "  ".into());
        let close_offset = tokens[close].start;
        let mut offset = close_offset;
        while offset > 0 && content.as_bytes()[offset - 1].is_ascii_whitespace() {
            offset -= 1;
        }
        let comma = if has_properties
            && !tokens
                .get(close.wrapping_sub(1))
                .is_some_and(|t| t.punct(b','))
        {
            ","
        } else {
            ""
        };
        let text=format!("{comma}{line_ending}{indent}\"{array}\": [{line_ending}{indent}  {escaped}{line_ending}{indent}]{}",if offset==close_offset{line_ending}else{""});
        Ok(Some(Self::insert(content, offset, text, array)))
    }
}
#[derive(Debug)]
enum JsonTokenKind {
    String(String),
    Punct(u8),
    Other,
}
#[derive(Debug)]
struct JsonToken {
    kind: JsonTokenKind,
    start: usize,
    end: usize,
}
impl JsonToken {
    fn punct(&self, c: u8) -> bool {
        matches!(self.kind,JsonTokenKind::Punct(v)if v==c)
    }
}
fn json_tokens(source: &str) -> Result<Vec<JsonToken>, String> {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut out = vec![];
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if source[i..].starts_with("//") {
            i = source[i..]
                .find(['\n', '\r'])
                .map_or(bytes.len(), |n| i + n);
            continue;
        }
        if source[i..].starts_with("/*") {
            i = source[i + 2..]
                .find("*/")
                .map(|n| i + n + 4)
                .ok_or("unterminated config comment")?;
            continue;
        }
        let start = i;
        let kind = if matches!(bytes[i], b'"' | b'\'') {
            let quote = bytes[i];
            i += 1;
            let mut rendered = String::from("\"");
            let mut closed = false;
            while i < bytes.len() {
                let c = bytes[i];
                if c == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                if c == b'\\' {
                    let escaped = *bytes.get(i + 1).ok_or("incomplete string escape")?;
                    if escaped == b'u' {
                        let hex = source
                            .get(i + 2..i + 6)
                            .ok_or("incomplete Unicode escape")?;
                        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                            return Err("invalid Unicode escape".into());
                        }
                        rendered.push_str(&source[i..i + 6]);
                        i += 6;
                    } else if escaped == b'\'' && quote == b'\'' {
                        rendered.push('\'');
                        i += 2;
                    } else {
                        rendered.push('\\');
                        rendered.push(char::from(escaped));
                        i += 2;
                    }
                } else {
                    let ch = source[i..].chars().next().unwrap();
                    if ch == '"' {
                        rendered.push('\\');
                    }
                    rendered.push(ch);
                    i += ch.len_utf8();
                }
            }
            if !closed {
                return Err("unterminated config string".into());
            }
            rendered.push('"');
            JsonTokenKind::String(serde_json::from_str(&rendered).map_err(|e| e.to_string())?)
        } else if matches!(bytes[i], b'{' | b'}' | b'[' | b']' | b':' | b',') {
            i += 1;
            JsonTokenKind::Punct(bytes[start])
        } else {
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && !matches!(bytes[i], b'{' | b'}' | b'[' | b']' | b':' | b',')
            {
                let ch = source[i..].chars().next().unwrap();
                i += ch.len_utf8();
            }
            JsonTokenKind::Other
        };
        out.push(JsonToken {
            kind,
            start,
            end: i,
        });
    }
    Ok(out)
}
fn json_pairs(tokens: &[JsonToken]) -> Result<std::collections::BTreeMap<usize, usize>, String> {
    let mut stack = vec![];
    let mut out = std::collections::BTreeMap::new();
    for (i, t) in tokens.iter().enumerate() {
        match t.kind {
            JsonTokenKind::Punct(b'{' | b'[') => stack.push(i),
            JsonTokenKind::Punct(b'}' | b']') => {
                let open = stack.pop().ok_or("unexpected closing delimiter")?;
                if !(tokens[open].punct(b'{') && t.punct(b'}')
                    || tokens[open].punct(b'[') && t.punct(b']'))
                {
                    return Err("mismatched config delimiter".into());
                }
                out.insert(open, i);
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("unterminated config object or array".into());
    }
    Ok(out)
}
pub(crate) fn parse_config_json(source: &str) -> Result<serde_json::Value, String> {
    let tokens = json_tokens(source)?;
    let mut normalized = String::new();
    for (i, token) in tokens.iter().enumerate() {
        match &token.kind {
            JsonTokenKind::Punct(b',')
                if tokens
                    .get(i + 1)
                    .is_some_and(|t| t.punct(b']') || t.punct(b'}')) => {}
            JsonTokenKind::String(value) => {
                normalized.push_str(&serde_json::to_string(value).map_err(|e| e.to_string())?)
            }
            JsonTokenKind::Other if tokens.get(i + 1).is_some_and(|t| t.punct(b':')) => normalized
                .push_str(
                    &serde_json::to_string(&source[token.start..token.end])
                        .map_err(|e| e.to_string())?,
                ),
            _ => normalized.push_str(&source[token.start..token.end]),
        }
    }
    serde_json::from_str(&normalized).map_err(|e| e.to_string())
}
