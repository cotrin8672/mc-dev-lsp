use super::*;
use crate::{ClassIndexEntry, FieldIndexEntry, MethodIndexEntry};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostSourceDocumentStamp {
    pub uri: String,
    pub revision: String,
}
pub trait HostSourceDocumentsProvider: HostReadValidator {
    /// The host chooses roots, exclusions, languages and generated/dependency
    /// sources. These are owned document identities, not filesystem search hints.
    fn source_documents(
        &self,
        project: &HostProjectKey,
    ) -> HostSourceResult<Vec<HostSourceDocumentStamp>>;
    fn declarations(
        &self,
        project: &HostProjectKey,
        document: &HostSourceDocumentStamp,
    ) -> HostSourceResult<Vec<HostClassFact>>;
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostSourceSnapshotMetrics {
    pub document_reads: usize,
    pub document_projections: usize,
    pub overlay_projections: usize,
}
pub struct HostSourceSnapshot {
    pub classes: Vec<HostClassFact>,
    pub reads: HostReadSet,
    pub metrics: HostSourceSnapshotMetrics,
}
type DocumentCache = HostFactCache<HostSourceDocumentStamp, Vec<HostClassFact>>;
pub struct HostSourceSnapshotCache {
    capacity: usize,
    projects: Mutex<VecDeque<(HostProjectKey, Arc<DocumentCache>)>>,
}
impl Default for HostSourceSnapshotCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_PROJECT_CAPACITY)
    }
}
impl HostSourceSnapshotCache {
    pub const DEFAULT_PROJECT_CAPACITY: usize = 8;
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            projects: Mutex::new(VecDeque::new()),
        }
    }
    pub fn len(&self) -> usize {
        self.projects.lock().unwrap().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn clear(&self) {
        self.projects.lock().unwrap().clear();
    }
    fn documents(&self, key: &HostProjectKey) -> Arc<DocumentCache> {
        let mut entries = self.projects.lock().unwrap();
        let cache = if let Some(i) = entries.iter().position(|(k, _)| k == key) {
            entries.remove(i).unwrap().1
        } else {
            Arc::new(DocumentCache::new(usize::MAX, None))
        };
        if self.capacity > 0 {
            entries.push_back((key.clone(), cache.clone()));
            while entries.len() > self.capacity {
                entries.pop_front();
            }
        }
        cache
    }
    pub fn snapshot(
        &self,
        project: &HostProjectKey,
        provider: &dyn HostSourceDocumentsProvider,
        overlay: Option<(&str, &[HostClassFact])>,
    ) -> HostSourceResult<HostSourceSnapshot> {
        let catalog = provider.source_documents(project)?;
        let documents = match catalog.fact {
            HostFact::Known(documents) => documents,
            HostFact::Missing => vec![],
            HostFact::Unavailable { reason } => {
                return Ok(HostAnswer {
                    fact: HostFact::Unavailable { reason },
                    reads: catalog.reads,
                })
            }
        };
        let cache = self.documents(project);
        cache.retain(|key| documents.contains(key));
        let mut reads = catalog.reads;
        let mut classes = vec![];
        let mut metrics = HostSourceSnapshotMetrics::default();
        for document in documents {
            if overlay.is_some_and(|(uri, _)| uri == document.uri) {
                continue;
            }
            let cached =
                cache.get_or_compute(document.clone(), project.generations, provider, || {
                    metrics.document_reads += 1;
                    metrics.document_projections += 1;
                    provider.declarations(project, &document)
                })?;
            reads.extend(cached.answer.reads);
            match cached.answer.fact {
                HostFact::Known(values) => classes.extend(values),
                HostFact::Missing => {}
                HostFact::Unavailable { reason } => {
                    return Ok(HostAnswer {
                        fact: HostFact::Unavailable { reason },
                        reads,
                    })
                }
            }
        }
        if let Some((_, values)) = overlay {
            metrics.overlay_projections = 1;
            classes.extend_from_slice(values);
        }
        Ok(HostAnswer::known(
            HostSourceSnapshot {
                classes,
                reads: reads.clone(),
                metrics,
            },
            reads,
        ))
    }
}
/// Immutable source overlay/query from host-selected normalized declarations.
/// Catalog completeness is deliberately partial: binary dependencies still layer
/// underneath it through SourceBackedClassIndex.
pub struct SnapshotSourceQuery {
    pub classes: Vec<HostClassFact>,
    pub reads: HostReadSet,
}
impl SnapshotSourceQuery {
    pub fn from_snapshot(snapshot: HostSourceSnapshot) -> Self {
        Self {
            classes: snapshot.classes,
            reads: snapshot.reads,
        }
    }
    pub fn from_buffer(classes: Vec<HostClassFact>, reads: HostReadSet) -> Self {
        Self { classes, reads }
    }
    fn answer<T>(&self, value: T) -> Result<SourceQueryResult<T>, HostSourceError> {
        Ok(SourceQueryResult {
            value,
            authority: SourceAuthority::Partial,
            reads: self.reads.clone(),
        })
    }
}
impl ProjectSourceQuery for SnapshotSourceQuery {
    fn find_classes(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        let prefix = prefix.to_lowercase();
        self.answer(
            self.classes
                .iter()
                .map(HostClassFact::entry)
                .filter(|e| {
                    e.simple_name.to_lowercase().starts_with(&prefix)
                        || e.fqn().to_lowercase().starts_with(&prefix)
                })
                .take(limit)
                .collect(),
        )
    }
    fn find_class(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.answer(
            self.classes
                .iter()
                .find(|c| c.internal_name == name)
                .map(HostClassFact::entry),
        )
    }
    fn find_class_by_fqn(
        &self,
        name: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        self.answer(
            self.classes
                .iter()
                .find(|c| c.entry().fqn() == name)
                .map(HostClassFact::entry),
        )
    }
    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        self.answer(
            self.classes
                .iter()
                .find(|c| c.internal_name == owner)
                .map(|c| c.methods.iter().filter_map(HostMethodFact::entry).collect())
                .unwrap_or_default(),
        )
    }
    fn get_fields(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        self.answer(
            self.classes
                .iter()
                .find(|c| c.internal_name == owner)
                .map(|c| c.fields.iter().filter_map(HostFieldFact::entry).collect())
                .unwrap_or_default(),
        )
    }
}
