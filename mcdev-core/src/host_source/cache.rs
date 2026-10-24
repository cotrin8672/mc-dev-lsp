use super::*;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CachedHostAnswer<T> {
    pub answer: HostAnswer<T>,
    pub cache_hit: bool,
}
#[derive(Clone)]
struct Entry<T> {
    answer: HostAnswer<T>,
    created: Instant,
    access: u64,
    generations: HostGenerations,
}
struct CacheState<K, T> {
    entries: HashMap<K, Entry<T>>,
    flights: HashMap<K, Weak<Mutex<()>>>,
    access: u64,
    epoch: u64,
}
/// Bounded owned-data cache. The validator runs for every reuse, including a
/// negative result; it must register these reads in the current evaluation.
/// Per-key exclusion coalesces concurrent builds without blocking unrelated keys.
pub struct HostFactCache<K, T> {
    state: Mutex<CacheState<K, T>>,
    capacity: usize,
    max_age: Option<Duration>,
}
impl<K: Eq + Hash + Clone, T: Clone> HostFactCache<K, T> {
    pub fn new(capacity: usize, max_age: Option<Duration>) -> Self {
        Self {
            state: Mutex::new(CacheState {
                entries: HashMap::new(),
                flights: HashMap::new(),
                access: 0,
                epoch: 0,
            }),
            capacity,
            max_age,
        }
    }
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.entries.clear();
        state.epoch = state.epoch.wrapping_add(1);
    }
    pub fn retain(&self, keep: impl Fn(&K) -> bool) {
        let mut state = self.state.lock().unwrap();
        state.entries.retain(|k, _| keep(k));
        state.epoch = state.epoch.wrapping_add(1);
    }
    pub fn get_or_compute(
        &self,
        key: K,
        generations: HostGenerations,
        validator: &dyn HostReadValidator,
        compute: impl FnOnce() -> HostSourceResult<T>,
    ) -> Result<CachedHostAnswer<T>, HostSourceError> {
        self.get_or_compute_cacheable(key, generations, validator, compute, |_| true)
    }
    pub fn get_or_compute_cacheable(
        &self,
        key: K,
        generations: HostGenerations,
        validator: &dyn HostReadValidator,
        compute: impl FnOnce() -> HostSourceResult<T>,
        cacheable: impl Fn(&T) -> bool,
    ) -> Result<CachedHostAnswer<T>, HostSourceError> {
        let flight = {
            let mut state = self.state.lock().unwrap();
            state.flights.retain(|_, flight| flight.strong_count() > 0);
            let flight = state
                .flights
                .get(&key)
                .and_then(Weak::upgrade)
                .unwrap_or_else(|| Arc::new(Mutex::new(())));
            state.flights.insert(key.clone(), Arc::downgrade(&flight));
            flight
        };
        let _exclusive = flight.lock().unwrap_or_else(|poison| poison.into_inner());
        let cached = self.state.lock().unwrap().entries.get(&key).cloned();
        if let Some(entry) = cached {
            if entry.generations == generations
                && self.max_age.is_none_or(|age| entry.created.elapsed() < age)
                && validator.validate_current_reads(generations, &entry.answer.reads)?
            {
                let mut state = self.state.lock().unwrap();
                state.access = state.access.wrapping_add(1);
                let access = state.access;
                if let Some(e) = state.entries.get_mut(&key) {
                    e.access = access;
                }
                return Ok(CachedHostAnswer {
                    answer: entry.answer,
                    cache_hit: true,
                });
            }
            self.state.lock().unwrap().entries.remove(&key);
        }
        let epoch = self.state.lock().unwrap().epoch;
        let answer = compute()?;
        if answer.fact.is_available()
            && answer.fact.known().is_none_or(cacheable)
            && self.capacity > 0
        {
            let mut state = self.state.lock().unwrap();
            if state.epoch != epoch {
                return Ok(CachedHostAnswer {
                    answer,
                    cache_hit: false,
                });
            }
            state.access = state.access.wrapping_add(1);
            let access = state.access;
            state.entries.insert(
                key,
                Entry {
                    answer: answer.clone(),
                    created: Instant::now(),
                    access,
                    generations,
                },
            );
            while state.entries.len() > self.capacity {
                let key = state
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.access)
                    .map(|(k, _)| k.clone())
                    .unwrap();
                state.entries.remove(&key);
            }
        }
        Ok(CachedHostAnswer {
            answer,
            cache_hit: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostProjectKey {
    /// Stable host project identity, not a pointer or evaluation-scoped token.
    pub project_id: String,
    pub generations: HostGenerations,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CandidateCacheDebug {
    pub cache_hit: bool,
    pub build_ms: u64,
}
thread_local! {
    static CANDIDATE_DEBUG: std::cell::RefCell<CandidateCacheDebug> = std::cell::RefCell::new(CandidateCacheDebug::default());
}
pub fn reset_candidate_cache_debug() {
    CANDIDATE_DEBUG.with(|d| *d.borrow_mut() = CandidateCacheDebug::default());
}
pub fn candidate_cache_debug() -> CandidateCacheDebug {
    CANDIDATE_DEBUG.with(|d| d.borrow().clone())
}
pub fn record_candidate_cache_debug(hit: bool, build_ms: u64) {
    CANDIDATE_DEBUG.with(|d| {
        let mut d = d.borrow_mut();
        d.cache_hit |= hit;
        d.build_ms = d.build_ms.saturating_add(build_ms);
    });
}
