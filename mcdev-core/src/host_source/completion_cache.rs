use super::*;
use crate::bytecode::{AtTargetCandidate, BytecodeIndex};
use crate::mixinextras_expression::ExpressionMemberCompletionCache;
use crate::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry};
use std::sync::Arc;
use std::time::Instant;

/// A request-scoped provider. Only its owned read set is retained by caches.
pub trait HostCacheReadScope: HostReadValidator {
    fn current_read_set(&self) -> HostReadSet;
    /// False when an underlying read was unavailable. In particular an empty
    /// legacy ClassIndex result must not turn unavailable host facts into absence.
    fn current_reads_are_authoritative(&self) -> bool;
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IndexKey {
    project: HostProjectKey,
    delegate: String,
    owner: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AtKey {
    index: IndexKey,
    method: String,
    descriptor: Option<String>,
    at_value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExpressionKey {
    project: HostProjectKey,
    class_delegate: String,
    bytecode_delegate: String,
}
pub struct CompletionIndexCaches {
    methods: HostFactCache<IndexKey, Vec<MethodIndexEntry>>,
    fields: HostFactCache<IndexKey, Vec<FieldIndexEntry>>,
    at_targets: HostFactCache<AtKey, Vec<AtTargetCandidate>>,
    expression_members: HostFactCache<ExpressionKey, Arc<ExpressionMemberCompletionCache>>,
}
impl Default for CompletionIndexCaches {
    fn default() -> Self {
        Self::new()
    }
}
impl CompletionIndexCaches {
    pub fn new() -> Self {
        Self {
            methods: HostFactCache::new(256, None),
            fields: HostFactCache::new(256, None),
            at_targets: HostFactCache::new(256, None),
            expression_members: HostFactCache::new(16, None),
        }
    }
    /// `delegate_id` is a stable provider-selected index identity, never a handle
    /// or an Arc address that could be reused after the delegate is dropped.
    pub fn class_index(
        self: &Arc<Self>,
        delegate: Arc<dyn ClassIndex>,
        delegate_id: String,
        project: HostProjectKey,
        scope: Arc<dyn HostCacheReadScope>,
    ) -> Arc<dyn ClassIndex> {
        Arc::new(CachedClasses {
            caches: self.clone(),
            delegate,
            delegate_id,
            project,
            scope,
        })
    }
    pub fn bytecode_index(
        self: &Arc<Self>,
        delegate: Arc<dyn BytecodeIndex>,
        delegate_id: String,
        project: HostProjectKey,
        scope: Arc<dyn HostCacheReadScope>,
    ) -> Arc<dyn BytecodeIndex> {
        Arc::new(CachedBytecode {
            caches: self.clone(),
            delegate,
            delegate_id,
            project,
            scope,
        })
    }
    pub fn expression_member_completion_cache(
        &self,
        class_delegate: String,
        bytecode_delegate: String,
        project: HostProjectKey,
        scope: &dyn HostCacheReadScope,
    ) -> Result<Arc<ExpressionMemberCompletionCache>, HostSourceError> {
        let answer = self.expression_members.get_or_compute_cacheable(
            ExpressionKey {
                project: project.clone(),
                class_delegate,
                bytecode_delegate,
            },
            project.generations,
            scope,
            || {
                Ok(HostAnswer::known(
                    Arc::new(ExpressionMemberCompletionCache::default()),
                    scope.current_read_set(),
                ))
            },
            |_| scope.current_reads_are_authoritative(),
        )?;
        match answer.answer.fact {
            HostFact::Known(cache) => Ok(cache),
            _ => unreachable!("cache factory always produces a known owned cache"),
        }
    }
    pub fn reset_debug(&self) {
        reset_candidate_cache_debug();
    }
    pub fn debug(&self) -> CandidateCacheDebug {
        candidate_cache_debug()
    }
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.methods.len(),
            self.fields.len(),
            self.at_targets.len(),
            self.expression_members.len(),
        )
    }
}
fn known<T>(result: Result<CachedHostAnswer<T>, HostSourceError>) -> T {
    match result {
        Ok(CachedHostAnswer {
            answer:
                HostAnswer {
                    fact: HostFact::Known(value),
                    ..
                },
            ..
        }) => value,
        Err(error) => std::panic::panic_any(error),
        _ => unreachable!("index factories always produce known owned values"),
    }
}
struct CachedClasses {
    caches: Arc<CompletionIndexCaches>,
    delegate: Arc<dyn ClassIndex>,
    delegate_id: String,
    project: HostProjectKey,
    scope: Arc<dyn HostCacheReadScope>,
}
impl CachedClasses {
    fn key(&self, owner: &str) -> IndexKey {
        IndexKey {
            project: self.project.clone(),
            delegate: self.delegate_id.clone(),
            owner: owner.into(),
        }
    }
}
impl ClassIndex for CachedClasses {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.delegate.find_classes(prefix, limit)
    }
    fn find_class(&self, internal: &str) -> Option<ClassIndexEntry> {
        self.delegate.find_class(internal)
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.delegate.find_class_by_fqn(fqn)
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        known(self.caches.methods.get_or_compute_cacheable(
            self.key(owner),
            self.project.generations,
            self.scope.as_ref(),
            || {
                let result = self.delegate.get_methods(owner);
                Ok(HostAnswer::known(result, self.scope.current_read_set()))
            },
            |_| self.scope.current_reads_are_authoritative(),
        ))
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        known(self.caches.fields.get_or_compute_cacheable(
            self.key(owner),
            self.project.generations,
            self.scope.as_ref(),
            || {
                let result = self.delegate.get_fields(owner);
                Ok(HostAnswer::known(result, self.scope.current_read_set()))
            },
            |_| self.scope.current_reads_are_authoritative(),
        ))
    }
}
struct CachedBytecode {
    caches: Arc<CompletionIndexCaches>,
    delegate: Arc<dyn BytecodeIndex>,
    delegate_id: String,
    project: HostProjectKey,
    scope: Arc<dyn HostCacheReadScope>,
}
impl BytecodeIndex for CachedBytecode {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
        at_value: &str,
    ) -> Vec<AtTargetCandidate> {
        let key = AtKey {
            index: IndexKey {
                project: self.project.clone(),
                delegate: self.delegate_id.clone(),
                owner: owner.into(),
            },
            method: method.into(),
            descriptor: descriptor.map(Into::into),
            at_value: at_value.into(),
        };
        let started = Instant::now();
        let result = self.caches.at_targets.get_or_compute_cacheable(
            key,
            self.project.generations,
            self.scope.as_ref(),
            || {
                let result = self
                    .delegate
                    .get_at_target_candidates(owner, method, descriptor, at_value);
                Ok(HostAnswer::known(result, self.scope.current_read_set()))
            },
            |_| self.scope.current_reads_are_authoritative(),
        );
        if let Ok(result) = &result {
            record_candidate_cache_debug(
                result.cache_hit,
                if result.cache_hit {
                    0
                } else {
                    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
                },
            );
        }
        known(result)
    }
    fn get_return_ordinal_count(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
    ) -> usize {
        self.delegate
            .get_return_ordinal_count(owner, method, descriptor)
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        self.delegate.get_class_bytes(owner)
    }
    fn get_class_bytes_ref(&self, owner: &str) -> Option<&[u8]> {
        self.delegate.get_class_bytes_ref(owner)
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.delegate.resolve_common_super_class(left, right)
    }
}
