use super::*;
use crate::descriptor::{render, render_method, to_descriptor, JvmType, MethodDescriptor};
use crate::{ClassIndexEntry, FieldIndexEntry, MethodIndexEntry};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostClassFact {
    pub internal_name: String,
    /// Owned display names from the host, preserving nested source names.
    pub simple_name: String,
    pub package_name: String,
    pub methods: Vec<HostMethodFact>,
    pub fields: Vec<HostFieldFact>,
    pub methods_complete: bool,
    pub fields_complete: bool,
}
impl HostClassFact {
    pub fn entry(&self) -> ClassIndexEntry {
        ClassIndexEntry {
            internal_name: self.internal_name.clone(),
            simple_name: self.simple_name.clone(),
            package_name: self.package_name.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostMethodFact {
    pub name: String,
    pub is_constructor: Option<bool>,
    pub access_flags: Option<u32>,
    pub parameter_types: Vec<HostTypeFact>,
    pub return_type: HostTypeFact,
}
impl HostMethodFact {
    pub fn entry(&self) -> Option<MethodIndexEntry> {
        let constructor = self.is_constructor?;
        let flags = self.access_flags?;
        let parameters: Vec<JvmType> = self
            .parameter_types
            .iter()
            .map(HostTypeFact::jvm_type)
            .collect::<Option<_>>()?;
        if parameters.contains(&JvmType::VoidType) {
            return None;
        }
        let return_type = if constructor {
            JvmType::VoidType
        } else {
            self.return_type.jvm_type()?
        };
        let descriptor = MethodDescriptor {
            parameters,
            return_type,
        };
        let name = if constructor { "<init>" } else { &self.name };
        Some(MethodIndexEntry {
            name: name.into(),
            descriptor: format!(
                "({}){}",
                descriptor
                    .parameters
                    .iter()
                    .map(to_descriptor)
                    .collect::<String>(),
                to_descriptor(&descriptor.return_type)
            ),
            is_static: flags & 8 != 0,
            readable_signature: format!("{name}{}", render_method(&descriptor)),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostFieldFact {
    pub name: String,
    pub access_flags: Option<u32>,
    pub field_type: HostTypeFact,
}
impl HostFieldFact {
    pub fn entry(&self) -> Option<FieldIndexEntry> {
        let ty = self.field_type.jvm_type()?;
        if ty == JvmType::VoidType {
            return None;
        }
        Some(FieldIndexEntry {
            name: self.name.clone(),
            descriptor: to_descriptor(&ty),
            is_static: self.access_flags? & 8 != 0,
            readable_type: render(&ty),
        })
    }
}
/// Host callbacks own source/dependency selection, imports, aliases, JVM names and
/// inherited resolution. MC neither scans roots nor guesses a second authority.
pub trait HostProjectSourceProvider: Send + Sync {
    fn search_classes(&self, prefix: &str, limit: usize) -> HostSourceResult<Vec<ClassIndexEntry>>;
    fn class_by_internal_name(&self, internal: &str) -> HostSourceResult<HostClassFact>;
    fn class_by_fqn(&self, fqn: &str) -> HostSourceResult<HostClassFact>;
    fn search_is_complete(&self) -> bool;
}
pub struct HostProjectSourceQuery {
    provider: Arc<dyn HostProjectSourceProvider>,
}
impl HostProjectSourceQuery {
    pub fn new(provider: Arc<dyn HostProjectSourceProvider>) -> Self {
        Self { provider }
    }
    fn class_result(
        answer: HostAnswer<HostClassFact>,
    ) -> SourceQueryResult<Option<ClassIndexEntry>> {
        let (value, authority) = match answer.fact {
            HostFact::Known(class) => (Some(class.entry()), SourceAuthority::Complete),
            HostFact::Missing => (None, SourceAuthority::Complete),
            HostFact::Unavailable { .. } => (None, SourceAuthority::Unavailable),
        };
        SourceQueryResult {
            value,
            authority,
            reads: answer.reads,
        }
    }
}
impl ProjectSourceQuery for HostProjectSourceQuery {
    fn covers_project_dependencies(&self) -> bool {
        self.provider.search_is_complete()
    }
    fn find_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        if limit == 0 {
            return Ok(SourceQueryResult::new(
                vec![],
                if self.provider.search_is_complete() {
                    SourceAuthority::Complete
                } else {
                    SourceAuthority::Partial
                },
            ));
        }
        let answer = self.provider.search_classes(prefix, limit)?;
        let (mut value, authority) = match answer.fact {
            HostFact::Known(value) => (
                value,
                if self.provider.search_is_complete() {
                    SourceAuthority::Complete
                } else {
                    SourceAuthority::Partial
                },
            ),
            HostFact::Missing => (
                vec![],
                if self.provider.search_is_complete() {
                    SourceAuthority::Complete
                } else {
                    SourceAuthority::Partial
                },
            ),
            HostFact::Unavailable { .. } => (vec![], SourceAuthority::Unavailable),
        };
        value.sort_by(|a, b| a.internal_name.cmp(&b.internal_name));
        value.dedup_by(|a, b| a.internal_name == b.internal_name);
        value.truncate(limit);
        Ok(SourceQueryResult {
            value,
            authority,
            reads: answer.reads,
        })
    }
    fn find_class(
        &self,
        internal: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        Ok(Self::class_result(
            self.provider.class_by_internal_name(internal)?,
        ))
    }
    fn find_class_by_fqn(
        &self,
        fqn: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        Ok(Self::class_result(self.provider.class_by_fqn(fqn)?))
    }
    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        let answer = self.provider.class_by_internal_name(owner)?;
        let (value, authority) = match answer.fact {
            HostFact::Known(class) => {
                let entries: Vec<_> = class
                    .methods
                    .iter()
                    .filter_map(HostMethodFact::entry)
                    .collect();
                let complete = class.methods_complete && entries.len() == class.methods.len();
                (
                    entries,
                    if complete {
                        SourceAuthority::Complete
                    } else {
                        SourceAuthority::Partial
                    },
                )
            }
            HostFact::Missing => (vec![], SourceAuthority::Complete),
            HostFact::Unavailable { .. } => (vec![], SourceAuthority::Unavailable),
        };
        Ok(SourceQueryResult {
            value,
            authority,
            reads: answer.reads,
        })
    }
    fn get_fields(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        let answer = self.provider.class_by_internal_name(owner)?;
        let (value, authority) = match answer.fact {
            HostFact::Known(class) => {
                let entries: Vec<_> = class
                    .fields
                    .iter()
                    .filter_map(HostFieldFact::entry)
                    .collect();
                let complete = class.fields_complete && entries.len() == class.fields.len();
                (
                    entries,
                    if complete {
                        SourceAuthority::Complete
                    } else {
                        SourceAuthority::Partial
                    },
                )
            }
            HostFact::Missing => (vec![], SourceAuthority::Complete),
            HostFact::Unavailable { .. } => (vec![], SourceAuthority::Unavailable),
        };
        Ok(SourceQueryResult {
            value,
            authority,
            reads: answer.reads,
        })
    }
}
