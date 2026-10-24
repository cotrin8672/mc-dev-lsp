use mcdev_core::host_source::*;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Mutex,
    },
};
fn class(name: &str, method: &str) -> HostClassFact {
    HostClassFact {
        internal_name: format!("com/example/{name}"),
        simple_name: name.into(),
        package_name: "com.example".into(),
        methods: vec![HostMethodFact {
            name: method.into(),
            is_constructor: Some(false),
            access_flags: Some(1),
            parameter_types: vec![],
            return_type: HostTypeFact::Primitive(HostPrimitive::Void),
        }],
        fields: vec![],
        methods_complete: true,
        fields_complete: true,
    }
}
#[derive(Default)]
struct Documents {
    documents: Mutex<BTreeMap<String, (u64, Vec<HostClassFact>)>>,
    catalog: AtomicU64,
    reads: AtomicUsize,
}
impl Documents {
    fn set(&self, name: &str, method: &str) {
        let uri = format!("file:///src/com/example/{name}.java");
        let mut documents = self.documents.lock().unwrap();
        let revision = documents.get(&uri).map_or(1, |(revision, _)| revision + 1);
        documents.insert(uri, (revision, vec![class(name, method)]));
        self.catalog.fetch_add(1, Ordering::SeqCst);
    }
    fn delete(&self, name: &str) {
        self.documents
            .lock()
            .unwrap()
            .remove(&format!("file:///src/com/example/{name}.java"));
        self.catalog.fetch_add(1, Ordering::SeqCst);
    }
}
impl HostReadValidator for Documents {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        let docs = self.documents.lock().unwrap();
        Ok(reads.iter().all(|read| {
            docs.get(&read.key)
                .is_some_and(|(generation, _)| *generation == read.generation)
        }))
    }
}
impl HostSourceDocumentsProvider for Documents {
    fn source_documents(
        &self,
        _: &HostProjectKey,
    ) -> HostSourceResult<Vec<HostSourceDocumentStamp>> {
        let documents = self.documents.lock().unwrap();
        Ok(HostAnswer::known(
            documents
                .iter()
                .map(|(uri, (revision, _))| HostSourceDocumentStamp {
                    uri: uri.clone(),
                    revision: revision.to_string(),
                })
                .collect(),
            vec![HostReadStamp {
                kind: "sourceCatalog".into(),
                key: "root".into(),
                generation: self.catalog.load(Ordering::SeqCst),
                missing: false,
            }],
        ))
    }
    fn declarations(
        &self,
        _: &HostProjectKey,
        document: &HostSourceDocumentStamp,
    ) -> HostSourceResult<Vec<HostClassFact>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let docs = self.documents.lock().unwrap();
        let (revision, classes) = docs.get(&document.uri).unwrap();
        assert_eq!(revision.to_string(), document.revision);
        Ok(HostAnswer::known(
            classes.clone(),
            vec![HostReadStamp {
                kind: "document".into(),
                key: document.uri.clone(),
                generation: *revision,
                missing: false,
            }],
        ))
    }
}
fn project(id: &str) -> HostProjectKey {
    HostProjectKey {
        project_id: id.into(),
        generations: HostGenerations::default(),
    }
}
fn snapshot(
    cache: &HostSourceSnapshotCache,
    host: &Documents,
    buffer_method: &str,
    metrics: &mut HostSourceSnapshotMetrics,
) -> SnapshotSourceQuery {
    snapshot_for(cache, host, "Current", buffer_method, metrics)
}
fn snapshot_for(
    cache: &HostSourceSnapshotCache,
    host: &Documents,
    name: &str,
    buffer_method: &str,
    metrics: &mut HostSourceSnapshotMetrics,
) -> SnapshotSourceQuery {
    let overlay = [class(name, buffer_method)];
    let uri = format!("file:///src/com/example/{name}.java");
    let answer = cache
        .snapshot(&project("root"), host, Some((&uri, &overlay)))
        .unwrap();
    let HostFact::Known(snapshot) = answer.fact else {
        panic!("known source snapshot required")
    };
    metrics.document_reads += snapshot.metrics.document_reads;
    metrics.document_projections += snapshot.metrics.document_projections;
    metrics.overlay_projections += snapshot.metrics.overlay_projections;
    SnapshotSourceQuery::from_snapshot(snapshot)
}
#[test]
fn per_keystroke_overlay_does_not_reread_unchanged_sources() {
    let host = Documents::default();
    host.set("Alpha", "alphaRun");
    host.set("Beta", "betaRun");
    let cache = HostSourceSnapshotCache::default();
    let mut metrics = HostSourceSnapshotMetrics::default();
    for version in 0..30 {
        let query = snapshot(&cache, &host, &format!("keystroke{version}"), &mut metrics);
        for name in ["Current", "Alpha", "Beta"] {
            assert_eq!(
                query
                    .find_class_by_fqn(&format!("com.example.{name}"))
                    .unwrap()
                    .value
                    .unwrap()
                    .internal_name,
                format!("com/example/{name}")
            );
        }
    }
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: 2,
            document_projections: 2,
            overlay_projections: 30
        }
    );
    assert_eq!(host.reads.load(Ordering::SeqCst), 2);
}
#[test]
fn modifying_one_source_document_increments_only_that_file_read_and_projection() {
    let host = Documents::default();
    host.set("Alpha", "alphaRun");
    host.set("Beta", "betaRun");
    let cache = HostSourceSnapshotCache::default();
    let mut metrics = HostSourceSnapshotMetrics::default();
    snapshot(&cache, &host, "first", &mut metrics);
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: 2,
            document_projections: 2,
            overlay_projections: 1
        }
    );
    host.set("Alpha", "alphaUpdated");
    let query = snapshot(&cache, &host, "second", &mut metrics);
    assert!(query
        .get_methods("com/example/Alpha")
        .unwrap()
        .value
        .iter()
        .any(|m| m.name == "alphaUpdated"));
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: 3,
            document_projections: 3,
            overlay_projections: 2
        }
    );
}
#[test]
fn deleting_source_document_removes_its_class() {
    let host = Documents::default();
    host.set("Alpha", "alphaRun");
    let cache = HostSourceSnapshotCache::default();
    let mut metrics = HostSourceSnapshotMetrics::default();
    let before = snapshot(&cache, &host, "first", &mut metrics);
    assert_eq!(
        before
            .find_class_by_fqn("com.example.Alpha")
            .unwrap()
            .value
            .unwrap()
            .internal_name,
        "com/example/Alpha"
    );
    host.delete("Alpha");
    let after = snapshot(&cache, &host, "second", &mut metrics);
    assert_eq!(
        after.find_class_by_fqn("com.example.Alpha").unwrap().value,
        None
    );
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: 1,
            document_projections: 1,
            overlay_projections: 2
        }
    );
}
#[test]
fn source_snapshot_cache_stays_bounded_to_explicit_max() {
    assert_eq!(HostSourceSnapshotCache::DEFAULT_PROJECT_CAPACITY, 8);
    let host = Documents::default();
    for i in 0..12 {
        host.set(&format!("Cache{i}"), &format!("cache{i}"));
    }
    let cache = HostSourceSnapshotCache::default();
    let mut metrics = HostSourceSnapshotMetrics::default();
    snapshot_for(&cache, &host, "Overlay", "overlay", &mut metrics);
    assert!(cache.len() <= 8);
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: 12,
            document_projections: 12,
            overlay_projections: 1
        }
    );
    for i in 0..12 {
        cache
            .snapshot(&project(&format!("project-{i}")), &host, None)
            .unwrap();
    }
    assert_eq!(cache.len(), 8);
}
#[test]
fn large_source_tree_does_not_fully_reread_unchanged_documents_on_second_snapshot() {
    let count = 1000;
    let host = Documents::default();
    for i in 0..count {
        host.set(&format!("Cache{i}"), &format!("cache{i}"));
    }
    let cache = HostSourceSnapshotCache::default();
    let mut metrics = HostSourceSnapshotMetrics::default();
    let first = snapshot_for(&cache, &host, "Overlay", "first", &mut metrics);
    assert_eq!(
        first
            .find_class_by_fqn("com.example.Cache0")
            .unwrap()
            .value
            .unwrap()
            .internal_name,
        "com/example/Cache0"
    );
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: count,
            document_projections: count,
            overlay_projections: 1
        }
    );
    let second = snapshot_for(&cache, &host, "Overlay", "second", &mut metrics);
    assert_eq!(
        second
            .find_class_by_fqn("com.example.Cache0")
            .unwrap()
            .value
            .unwrap()
            .internal_name,
        "com/example/Cache0"
    );
    assert_eq!(
        metrics,
        HostSourceSnapshotMetrics {
            document_reads: count,
            document_projections: count,
            overlay_projections: 2
        }
    );
    assert_eq!(host.reads.load(Ordering::SeqCst), count);
}
#[test]
fn unsaved_overlay_skips_selected_disk_document_and_closed_cache_capacity_zero_rebuilds() {
    let host = Documents::default();
    host.set("Current", "stale");
    let cache = HostSourceSnapshotCache::new(0);
    let mut metrics = HostSourceSnapshotMetrics::default();
    let first = snapshot(&cache, &host, "latest", &mut metrics);
    assert_eq!(
        first.get_methods("com/example/Current").unwrap().value[0].name,
        "latest"
    );
    assert_eq!(host.reads.load(Ordering::SeqCst), 0);
    assert_eq!(cache.len(), 0);
}
