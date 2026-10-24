use super::*;
use crate::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry};
use std::sync::{Arc, Mutex};
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceAuthority {
    Complete,
    Partial,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceQueryResult<T> {
    pub value: T,
    pub authority: SourceAuthority,
    pub reads: HostReadSet,
}
impl<T> SourceQueryResult<T> {
    pub fn new(value: T, authority: SourceAuthority) -> Self {
        Self {
            value,
            authority,
            reads: vec![],
        }
    }
}
fn recover_query<T: Default>(
    result: Result<SourceQueryResult<T>, HostSourceError>,
) -> Result<SourceQueryResult<T>, HostSourceError> {
    match result {
        Err(HostSourceError::Failure(_)) => Ok(SourceQueryResult::new(
            T::default(),
            SourceAuthority::Unavailable,
        )),
        other => other,
    }
}
/// Authority belongs to each operation, never a mutable last-error/thread-local flag.
pub trait ProjectSourceQuery: Send + Sync {
    fn find_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError>;
    fn find_class(
        &self,
        internal: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError>;
    fn find_class_by_fqn(
        &self,
        fqn: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError>;
    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError>;
    fn get_fields(
        &self,
        _owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        Ok(SourceQueryResult::new(vec![], SourceAuthority::Partial))
    }
    fn covers_project_dependencies(&self) -> bool {
        false
    }
}
pub struct SourceBackedClassIndex {
    delegate: Arc<dyn ClassIndex>,
    source: Arc<dyn ProjectSourceQuery>,
    reads: Mutex<HostReadSet>,
}
impl SourceBackedClassIndex {
    pub fn new(delegate: Arc<dyn ClassIndex>, source: Arc<dyn ProjectSourceQuery>) -> Self {
        Self {
            delegate,
            source,
            reads: Mutex::new(vec![]),
        }
    }
    pub fn read_set(&self) -> HostReadSet {
        self.reads.lock().unwrap().clone()
    }
    fn answer<T: Default>(
        &self,
        result: Result<SourceQueryResult<T>, HostSourceError>,
    ) -> SourceQueryResult<T> {
        match result {
            Ok(answer) => {
                self.reads.lock().unwrap().extend(answer.reads.clone());
                answer
            }
            Err(HostSourceError::Failure(_)) => {
                SourceQueryResult::new(T::default(), SourceAuthority::Unavailable)
            }
            Err(error) => std::panic::panic_any(error),
        }
    }
}
impl ClassIndex for SourceBackedClassIndex {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        let source = self.answer(self.source.find_classes(prefix, limit));
        if source.authority == SourceAuthority::Complete {
            return source.value.into_iter().take(limit).collect();
        }
        unique(
            source
                .value
                .into_iter()
                .chain(self.delegate.find_classes(prefix, limit)),
            |c| c.internal_name.clone(),
        )
        .into_iter()
        .take(limit)
        .collect()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        let source = self.answer(self.source.find_class(name));
        if source.authority == SourceAuthority::Complete {
            source.value
        } else {
            source.value.or_else(|| self.delegate.find_class(name))
        }
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        let source = self.answer(self.source.find_class_by_fqn(name));
        if source.authority == SourceAuthority::Complete {
            source.value
        } else {
            source
                .value
                .or_else(|| self.delegate.find_class_by_fqn(name))
        }
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        let source = self.answer(self.source.get_methods(owner));
        if source.authority == SourceAuthority::Complete {
            return source.value;
        }
        unique(
            source
                .value
                .into_iter()
                .chain(self.delegate.get_methods(owner)),
            |m| (m.name.clone(), m.descriptor.clone()),
        )
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        let source = self.answer(self.source.get_fields(owner));
        if source.authority == SourceAuthority::Complete {
            return source.value;
        }
        unique(
            source
                .value
                .into_iter()
                .chain(self.delegate.get_fields(owner)),
            |m| (m.name.clone(), m.descriptor.clone()),
        )
    }
}
fn unique<T, K: PartialEq>(values: impl IntoIterator<Item = T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut seen = vec![];
    let mut result = vec![];
    for value in values {
        let k = key(&value);
        if !seen.contains(&k) {
            seen.push(k);
            result.push(value);
        }
    }
    result
}
pub struct CompositeProjectSourceQuery {
    delegates: Vec<Arc<dyn ProjectSourceQuery>>,
}
impl CompositeProjectSourceQuery {
    pub fn new(delegates: Vec<Arc<dyn ProjectSourceQuery>>) -> Self {
        Self { delegates }
    }
    fn lookup(
        &self,
        query: impl Fn(
            &dyn ProjectSourceQuery,
        ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError>,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        let mut reads = vec![];
        let mut authority = SourceAuthority::Partial;
        for delegate in &self.delegates {
            let answer = recover_query(query(delegate.as_ref()))?;
            reads.extend(answer.reads);
            if answer.authority == SourceAuthority::Complete {
                authority = SourceAuthority::Complete;
            }
            if answer.value.is_some() {
                return Ok(SourceQueryResult {
                    value: answer.value,
                    authority: if self.covers_project_dependencies() {
                        SourceAuthority::Complete
                    } else {
                        authority
                    },
                    reads,
                });
            }
        }
        Ok(SourceQueryResult {
            value: None,
            authority,
            reads,
        })
    }
    fn members<T, K: PartialEq>(
        &self,
        query: impl Fn(&dyn ProjectSourceQuery) -> Result<SourceQueryResult<Vec<T>>, HostSourceError>,
        key: impl Fn(&T) -> K,
    ) -> Result<SourceQueryResult<Vec<T>>, HostSourceError> {
        let mut values = vec![];
        let mut reads = vec![];
        let mut authority = SourceAuthority::Partial;
        for delegate in &self.delegates {
            let answer = recover_query(query(delegate.as_ref()))?;
            values.extend(answer.value);
            reads.extend(answer.reads);
            if answer.authority == SourceAuthority::Complete {
                authority = SourceAuthority::Complete;
            }
        }
        Ok(SourceQueryResult {
            value: unique(values, key),
            authority,
            reads,
        })
    }
}
impl ProjectSourceQuery for CompositeProjectSourceQuery {
    fn find_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        if limit == 0 {
            return Ok(SourceQueryResult::new(
                vec![],
                if self.covers_project_dependencies() {
                    SourceAuthority::Complete
                } else {
                    SourceAuthority::Partial
                },
            ));
        }
        let mut entries = vec![];
        let mut reads = vec![];
        let mut authority = SourceAuthority::Partial;
        for delegate in &self.delegates {
            if entries.len() >= limit {
                break;
            }
            let answer = recover_query(delegate.find_classes(prefix, limit))?;
            reads.extend(answer.reads);
            if answer.authority == SourceAuthority::Complete {
                authority = SourceAuthority::Complete;
            }
            entries = unique(
                entries.into_iter().chain(answer.value),
                |c: &ClassIndexEntry| c.internal_name.clone(),
            );
            entries.truncate(limit);
        }
        if entries.len() >= limit && self.covers_project_dependencies() {
            authority = SourceAuthority::Complete;
        }
        Ok(SourceQueryResult {
            value: entries,
            authority,
            reads,
        })
    }
    fn find_class(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.lookup(|d| d.find_class(name))
    }
    fn find_class_by_fqn(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.lookup(|d| d.find_class_by_fqn(name))
    }
    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        self.members(
            |d| d.get_methods(owner),
            |m| (m.name.clone(), m.descriptor.clone()),
        )
    }
    fn get_fields(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        self.members(
            |d| d.get_fields(owner),
            |m| (m.name.clone(), m.descriptor.clone()),
        )
    }
    fn covers_project_dependencies(&self) -> bool {
        self.delegates
            .iter()
            .any(|d| d.covers_project_dependencies())
    }
}
