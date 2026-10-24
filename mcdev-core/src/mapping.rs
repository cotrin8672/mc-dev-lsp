use crate::descriptor::{self, JvmType};
use crate::model::MappingNamespace;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassMapping {
    pub names: BTreeMap<MappingNamespace, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    pub owner: ClassMapping,
    pub descriptor: String,
    pub names: BTreeMap<MappingNamespace, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodMapping {
    pub owner: ClassMapping,
    pub descriptor: String,
    pub names: BTreeMap<MappingNamespace, String>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingSet {
    pub namespaces: Vec<MappingNamespace>,
    pub classes: Vec<ClassMapping>,
    pub fields: Vec<FieldMapping>,
    pub methods: Vec<MethodMapping>,
}
impl MappingSet {
    pub fn class_name(
        &self,
        name: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> Option<String> {
        // Mapping files are ordered: match the first row before looking up its
        // destination, including when that first row has no destination name.
        self.classes
            .iter()
            .find(|c| c.names.get(&from).is_some_and(|v| v == name))
            .and_then(|c| c.names.get(&to))
            .cloned()
    }
    pub fn method_name(
        &self,
        owner: &str,
        name: &str,
        descriptor: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> Option<String> {
        self.methods
            .iter()
            .find(|m| {
                m.owner.names.get(&from).is_some_and(|v| v == owner)
                    && m.names.get(&from).is_some_and(|v| v == name)
                    && m.descriptor == descriptor
            })
            .and_then(|m| m.names.get(&to))
            .cloned()
    }
    pub fn field_name(
        &self,
        owner: &str,
        name: &str,
        descriptor: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> Option<String> {
        self.fields
            .iter()
            .find(|m| {
                m.owner.names.get(&from).is_some_and(|v| v == owner)
                    && m.names.get(&from).is_some_and(|v| v == name)
                    && m.descriptor == descriptor
            })
            .and_then(|m| m.names.get(&to))
            .cloned()
    }
    pub fn as_resolver(self) -> MappingSetResolver {
        MappingSetResolver::new(self)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingParseError {
    pub line: usize,
    pub message: String,
}
impl std::fmt::Display for MappingParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at line {}", self.message, self.line)
    }
}
impl std::error::Error for MappingParseError {}
pub type MappingParseResult = Result<MappingSet, MappingParseError>;
fn parse_failure(line: usize, message: &str) -> MappingParseResult {
    Err(MappingParseError {
        line,
        message: message.into(),
    })
}
pub fn parse_namespace(value: &str) -> Option<MappingNamespace> {
    use MappingNamespace::*;
    match value.to_lowercase().as_str() {
        "named" => Some(Named),
        "intermediary" => Some(Intermediary),
        "official" => Some(Official),
        "srg" => Some(Srg),
        "mcp" => Some(Mcp),
        _ => None,
    }
}
pub struct TinyV2Parser;
impl TinyV2Parser {
    pub fn parse(text: &str) -> MappingParseResult {
        parse_tiny_v2(text)
    }
}
pub fn parse_tiny_v2(text: &str) -> MappingParseResult {
    let mut lines = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    let first = lines.next().unwrap_or("");
    let header = first.split('\t').collect::<Vec<_>>();
    if header.len() < 4 || header[0] != "tiny" || header[1] != "2" {
        return parse_failure(1, "expected Tiny v2 header");
    }
    let mut namespaces = Vec::new();
    for (i, value) in header[3..].iter().enumerate() {
        let Some(ns) = parse_namespace(value) else {
            return parse_failure(
                1,
                &format!("unknown namespace '{value}' at column {}", i + 4),
            );
        };
        namespaces.push(ns);
    }
    let mut result = MappingSet {
        namespaces: namespaces.clone(),
        ..Default::default()
    };
    let mut current = None;
    for (i, line) in lines.enumerate() {
        let n = i + 2;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let indent = line.bytes().take_while(|c| *c == b'\t').count();
        let parts = line[indent..].split('\t').collect::<Vec<_>>();
        match (indent, parts[0]) {
            (0, "c") => {
                if parts.len() != namespaces.len() + 1 {
                    return parse_failure(n, "class mapping has wrong number of names");
                }
                let class = ClassMapping {
                    names: namespaces
                        .iter()
                        .copied()
                        .zip(parts[1..].iter().map(|v| (*v).into()))
                        .collect(),
                };
                result.classes.push(class.clone());
                current = Some(class);
            }
            (1, "m" | "f") => {
                let method = parts[0] == "m";
                let Some(owner) = current.clone() else {
                    return parse_failure(
                        n,
                        if method {
                            "method mapping without class"
                        } else {
                            "field mapping without class"
                        },
                    );
                };
                if parts.len() != namespaces.len() + 2 {
                    return parse_failure(
                        n,
                        if method {
                            "method mapping has wrong number of names"
                        } else {
                            "field mapping has wrong number of names"
                        },
                    );
                }
                let names = namespaces
                    .iter()
                    .copied()
                    .zip(parts[2..].iter().map(|v| (*v).into()))
                    .collect();
                if method {
                    result.methods.push(MethodMapping {
                        owner,
                        descriptor: parts[1].into(),
                        names,
                    });
                } else {
                    result.fields.push(FieldMapping {
                        owner,
                        descriptor: parts[1].into(),
                        names,
                    });
                }
            }
            _ => return parse_failure(n, "unsupported Tiny v2 row"),
        }
    }
    Ok(result)
}
pub struct SrgParser;
impl SrgParser {
    pub fn parse(text: &str) -> MappingParseResult {
        parse_srg(text)
    }
}
fn split_member(value: &str) -> Option<(&str, &str)> {
    let (owner, name) = value.rsplit_once('/')?;
    (!owner.is_empty() && !name.is_empty()).then_some((owner, name))
}
pub fn parse_srg(text: &str) -> MappingParseResult {
    use MappingNamespace::{Official, Srg};
    let mut result = MappingSet {
        namespaces: vec![Official, Srg],
        ..Default::default()
    };
    let mut classes = BTreeMap::<String, ClassMapping>::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let p = line.split_whitespace().collect::<Vec<_>>();
        match p[0] {
            "CL:" => {
                if p.len() != 3 {
                    return parse_failure(n, "invalid CL row");
                }
                let class = ClassMapping {
                    names: BTreeMap::from([(Official, p[1].into()), (Srg, p[2].into())]),
                };
                classes.insert(p[1].into(), class.clone());
                result.classes.push(class);
            }
            "FD:" | "MD:" => {
                let method = p[0] == "MD:";
                if p.len() != if method { 5 } else { 3 } {
                    return parse_failure(
                        n,
                        if method {
                            "invalid MD row"
                        } else {
                            "invalid FD row"
                        },
                    );
                }
                let Some((from_owner, from_name)) = split_member(p[1]) else {
                    return parse_failure(
                        n,
                        if method {
                            "invalid source method"
                        } else {
                            "invalid source field"
                        },
                    );
                };
                let Some((_, to_name)) = split_member(p[if method { 3 } else { 2 }]) else {
                    return parse_failure(
                        n,
                        if method {
                            "invalid target method"
                        } else {
                            "invalid target field"
                        },
                    );
                };
                let owner = classes
                    .get(from_owner)
                    .cloned()
                    .unwrap_or_else(|| ClassMapping {
                        names: BTreeMap::from([(Official, from_owner.into())]),
                    });
                let names = BTreeMap::from([(Official, from_name.into()), (Srg, to_name.into())]);
                if method {
                    result.methods.push(MethodMapping {
                        owner,
                        descriptor: p[2].into(),
                        names,
                    });
                } else {
                    result.fields.push(FieldMapping {
                        owner,
                        descriptor: String::new(),
                        names,
                    });
                }
            }
            _ => return parse_failure(n, "unsupported SRG row"),
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassRef {
    pub internal_name: String,
    pub namespace: MappingNamespace,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldRef {
    pub owner: String,
    pub name: String,
    pub descriptor: String,
    pub namespace: MappingNamespace,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodRef {
    pub owner: String,
    pub name: String,
    pub descriptor: String,
    pub namespace: MappingNamespace,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingSubject {
    Class,
    Field,
    Method,
    Descriptor,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingLookupResult<T> {
    Found(T),
    Missing {
        subject: MappingSubject,
        owner: Option<String>,
        name: Option<String>,
        descriptor: Option<String>,
        from: MappingNamespace,
        to: MappingNamespace,
    },
    InvalidDescriptor {
        descriptor: String,
        message: String,
        offset: usize,
    },
}
impl<T> MappingLookupResult<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> MappingLookupResult<U> {
        match self {
            Self::Found(v) => MappingLookupResult::Found(f(v)),
            Self::Missing {
                subject,
                owner,
                name,
                descriptor,
                from,
                to,
            } => MappingLookupResult::Missing {
                subject,
                owner,
                name,
                descriptor,
                from,
                to,
            },
            Self::InvalidDescriptor {
                descriptor,
                message,
                offset,
            } => MappingLookupResult::InvalidDescriptor {
                descriptor,
                message,
                offset,
            },
        }
    }
    pub fn found(self) -> Option<T> {
        if let Self::Found(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
fn missing<T>(
    subject: MappingSubject,
    owner: Option<&str>,
    name: Option<&str>,
    descriptor: Option<&str>,
    from: MappingNamespace,
    to: MappingNamespace,
) -> MappingLookupResult<T> {
    MappingLookupResult::Missing {
        subject,
        owner: owner.map(str::to_owned),
        name: name.map(str::to_owned),
        descriptor: descriptor.map(str::to_owned),
        from,
        to,
    }
}
pub trait MappingResolver: Send + Sync {
    fn namespaces(&self) -> BTreeSet<MappingNamespace>;
    fn remap_class(
        &self,
        reference: &ClassRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<ClassRef>;
    fn remap_field(
        &self,
        reference: &FieldRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<FieldRef>;
    fn remap_method(
        &self,
        reference: &MethodRef,
        to: MappingNamespace,
    ) -> MappingLookupResult<MethodRef>;
    fn remap_descriptor(
        &self,
        descriptor: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> MappingLookupResult<String>;
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMappingContext {
    pub source_namespace: MappingNamespace,
    pub runtime_namespace: MappingNamespace,
    pub aw_namespace: Option<MappingNamespace>,
    pub at_namespace: Option<MappingNamespace>,
    pub available_namespaces: BTreeSet<MappingNamespace>,
    #[serde(skip, default = "empty_resolver")]
    pub resolver: Arc<dyn MappingResolver>,
}
fn empty_resolver() -> Arc<dyn MappingResolver> {
    Arc::new(EmptyMappingResolver)
}
impl std::fmt::Debug for ProjectMappingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectMappingContext")
            .field("source_namespace", &self.source_namespace)
            .field("runtime_namespace", &self.runtime_namespace)
            .field("aw_namespace", &self.aw_namespace)
            .field("at_namespace", &self.at_namespace)
            .field("available_namespaces", &self.available_namespaces)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone)]
pub struct MappingSetResolver {
    pub mappings: MappingSet,
}
impl MappingSetResolver {
    pub fn new(mappings: MappingSet) -> Self {
        Self { mappings }
    }
}
impl MappingResolver for MappingSetResolver {
    fn namespaces(&self) -> BTreeSet<MappingNamespace> {
        self.mappings.namespaces.iter().copied().collect()
    }
    fn remap_class(&self, r: &ClassRef, to: MappingNamespace) -> MappingLookupResult<ClassRef> {
        if r.namespace == to {
            return MappingLookupResult::Found(r.clone());
        }
        match self.mappings.class_name(&r.internal_name, r.namespace, to) {
            Some(internal_name) => MappingLookupResult::Found(ClassRef {
                internal_name,
                namespace: to,
            }),
            None => missing(
                MappingSubject::Class,
                Some(&r.internal_name),
                None,
                None,
                r.namespace,
                to,
            ),
        }
    }
    fn remap_field(&self, r: &FieldRef, to: MappingNamespace) -> MappingLookupResult<FieldRef> {
        if r.namespace == to {
            return MappingLookupResult::Found(r.clone());
        }
        let owner = match self.remap_class(
            &ClassRef {
                internal_name: r.owner.clone(),
                namespace: r.namespace,
            },
            to,
        ) {
            MappingLookupResult::Found(v) => v.internal_name,
            other => return other.map(|_| unreachable!()),
        };
        let descriptor = match self.remap_descriptor(&r.descriptor, r.namespace, to) {
            MappingLookupResult::Found(v) => v,
            MappingLookupResult::Missing { .. } => r.descriptor.clone(),
            other => return other.map(|_| unreachable!()),
        };
        match self
            .mappings
            .field_name(&r.owner, &r.name, &r.descriptor, r.namespace, to)
        {
            Some(name) => MappingLookupResult::Found(FieldRef {
                owner,
                name,
                descriptor,
                namespace: to,
            }),
            None => missing(
                MappingSubject::Field,
                Some(&r.owner),
                Some(&r.name),
                Some(&r.descriptor),
                r.namespace,
                to,
            ),
        }
    }
    fn remap_method(&self, r: &MethodRef, to: MappingNamespace) -> MappingLookupResult<MethodRef> {
        if r.namespace == to {
            return MappingLookupResult::Found(r.clone());
        }
        let owner = match self.remap_class(
            &ClassRef {
                internal_name: r.owner.clone(),
                namespace: r.namespace,
            },
            to,
        ) {
            MappingLookupResult::Found(v) => v.internal_name,
            other => return other.map(|_| unreachable!()),
        };
        let descriptor = match self.remap_descriptor(&r.descriptor, r.namespace, to) {
            MappingLookupResult::Found(v) => v,
            MappingLookupResult::Missing { .. } => r.descriptor.clone(),
            other => return other.map(|_| unreachable!()),
        };
        match self
            .mappings
            .method_name(&r.owner, &r.name, &r.descriptor, r.namespace, to)
        {
            Some(name) => MappingLookupResult::Found(MethodRef {
                owner,
                name,
                descriptor,
                namespace: to,
            }),
            None => missing(
                MappingSubject::Method,
                Some(&r.owner),
                Some(&r.name),
                Some(&r.descriptor),
                r.namespace,
                to,
            ),
        }
    }
    fn remap_descriptor(
        &self,
        value: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> MappingLookupResult<String> {
        if from == to {
            return MappingLookupResult::Found(value.into());
        }
        remap_descriptor(value, from, to, self)
    }
}
fn remap_type(
    ty: JvmType,
    from: MappingNamespace,
    to: MappingNamespace,
    resolver: &dyn MappingResolver,
) -> MappingLookupResult<JvmType> {
    match ty {
        JvmType::ObjectType(internal_name) => resolver
            .remap_class(
                &ClassRef {
                    internal_name,
                    namespace: from,
                },
                to,
            )
            .map(|r| JvmType::ObjectType(r.internal_name)),
        JvmType::ArrayType(component) => {
            remap_type(*component, from, to, resolver).map(|v| JvmType::ArrayType(Box::new(v)))
        }
        other => MappingLookupResult::Found(other),
    }
}
pub fn remap_descriptor(
    value: &str,
    from: MappingNamespace,
    to: MappingNamespace,
    resolver: &dyn MappingResolver,
) -> MappingLookupResult<String> {
    if value.starts_with('(') {
        let mut method = match descriptor::parse_method_descriptor(value) {
            Ok(v) => v,
            Err(e) => {
                return MappingLookupResult::InvalidDescriptor {
                    descriptor: value.into(),
                    message: e.message,
                    offset: e.offset,
                }
            }
        };
        for parameter in &mut method.parameters {
            match remap_type(parameter.clone(), from, to, resolver) {
                MappingLookupResult::Found(v) => *parameter = v,
                other => return other.map(|_| unreachable!()),
            }
        }
        method.return_type = match remap_type(method.return_type, from, to, resolver) {
            MappingLookupResult::Found(v) => v,
            other => return other.map(|_| unreachable!()),
        };
        MappingLookupResult::Found(descriptor::method_to_descriptor(&method))
    } else {
        match descriptor::parse_field_descriptor(value) {
            Ok(v) => remap_type(v, from, to, resolver).map(|ty| descriptor::to_descriptor(&ty)),
            Err(e) => MappingLookupResult::InvalidDescriptor {
                descriptor: value.into(),
                message: e.message,
                offset: e.offset,
            },
        }
    }
}
pub struct DescriptorRemapper;
impl DescriptorRemapper {
    pub fn remap_field_descriptor(
        value: &str,
        from: MappingNamespace,
        to: MappingNamespace,
        resolver: &dyn MappingResolver,
    ) -> MappingLookupResult<String> {
        match descriptor::parse_field_descriptor(value) {
            Ok(v) => remap_type(v, from, to, resolver).map(|ty| descriptor::to_descriptor(&ty)),
            Err(e) => MappingLookupResult::InvalidDescriptor {
                descriptor: value.into(),
                message: e.message,
                offset: e.offset,
            },
        }
    }
    pub fn remap_method_descriptor(
        value: &str,
        from: MappingNamespace,
        to: MappingNamespace,
        resolver: &dyn MappingResolver,
    ) -> MappingLookupResult<String> {
        if let Err(e) = descriptor::parse_method_descriptor(value) {
            return MappingLookupResult::InvalidDescriptor {
                descriptor: value.into(),
                message: e.message,
                offset: e.offset,
            };
        }
        remap_descriptor(value, from, to, resolver)
    }
}
pub struct CompositeMappingResolver {
    resolvers: Vec<Arc<dyn MappingResolver>>,
}
impl CompositeMappingResolver {
    pub fn new(resolvers: Vec<Arc<dyn MappingResolver>>) -> Result<Self, String> {
        if resolvers.is_empty() {
            Err("composite resolver requires at least one delegate".into())
        } else {
            Ok(Self { resolvers })
        }
    }
    fn first_found<T>(
        &self,
        action: impl Fn(&dyn MappingResolver) -> MappingLookupResult<T>,
    ) -> MappingLookupResult<T> {
        let mut missing = None;
        for resolver in &self.resolvers {
            match action(resolver.as_ref()) {
                value @ MappingLookupResult::Missing { .. } => missing = Some(value),
                other => return other,
            }
        }
        missing.expect("nonempty resolver invariant")
    }
}
impl MappingResolver for CompositeMappingResolver {
    fn namespaces(&self) -> BTreeSet<MappingNamespace> {
        self.resolvers.iter().flat_map(|r| r.namespaces()).collect()
    }
    fn remap_class(&self, r: &ClassRef, to: MappingNamespace) -> MappingLookupResult<ClassRef> {
        self.first_found(|v| v.remap_class(r, to))
    }
    fn remap_field(&self, r: &FieldRef, to: MappingNamespace) -> MappingLookupResult<FieldRef> {
        self.first_found(|v| v.remap_field(r, to))
    }
    fn remap_method(&self, r: &MethodRef, to: MappingNamespace) -> MappingLookupResult<MethodRef> {
        self.first_found(|v| v.remap_method(r, to))
    }
    fn remap_descriptor(
        &self,
        value: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> MappingLookupResult<String> {
        self.first_found(|v| v.remap_descriptor(value, from, to))
    }
}
pub fn as_composite_resolver(sets: Vec<MappingSet>) -> Result<Arc<dyn MappingResolver>, String> {
    match sets.len() {
        0 => Err("cannot build resolver from empty mapping sets".into()),
        1 => Ok(Arc::new(sets.into_iter().next().unwrap().as_resolver())),
        _ => Ok(Arc::new(CompositeMappingResolver::new(
            sets.into_iter()
                .map(|s| Arc::new(s.as_resolver()) as Arc<dyn MappingResolver>)
                .collect(),
        )?)),
    }
}
#[derive(Debug, Default, Clone)]
pub struct EmptyMappingResolver;
impl MappingResolver for EmptyMappingResolver {
    fn namespaces(&self) -> BTreeSet<MappingNamespace> {
        BTreeSet::new()
    }
    fn remap_class(&self, r: &ClassRef, to: MappingNamespace) -> MappingLookupResult<ClassRef> {
        missing(
            MappingSubject::Class,
            Some(&r.internal_name),
            None,
            None,
            r.namespace,
            to,
        )
    }
    fn remap_field(&self, r: &FieldRef, to: MappingNamespace) -> MappingLookupResult<FieldRef> {
        missing(
            MappingSubject::Field,
            Some(&r.owner),
            Some(&r.name),
            Some(&r.descriptor),
            r.namespace,
            to,
        )
    }
    fn remap_method(&self, r: &MethodRef, to: MappingNamespace) -> MappingLookupResult<MethodRef> {
        missing(
            MappingSubject::Method,
            Some(&r.owner),
            Some(&r.name),
            Some(&r.descriptor),
            r.namespace,
            to,
        )
    }
    fn remap_descriptor(
        &self,
        _value: &str,
        from: MappingNamespace,
        to: MappingNamespace,
    ) -> MappingLookupResult<String> {
        missing(MappingSubject::Descriptor, None, None, None, from, to)
    }
}
