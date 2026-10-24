use std::collections::VecDeque;

/// Java String.hashCode over UTF-16, retained for legacy changedtick fallbacks/debug.
pub fn java_text_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |value, unit| {
        value.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub document_uri: String,
    pub version: i64,
    pub text: String,
    pub text_hash: i32,
}
#[derive(Debug, Clone)]
pub struct SnapshotResult {
    pub snapshot: DocumentSnapshot,
    pub cache_hit: bool,
}
struct Entry {
    snapshot: DocumentSnapshot,
    created_at: i64,
}
pub struct DocumentSnapshotCache {
    entries: VecDeque<Entry>,
    max_entries: usize,
    max_age_millis: i64,
}
impl Default for DocumentSnapshotCache {
    fn default() -> Self {
        Self::new(64, 600_000)
    }
}
impl DocumentSnapshotCache {
    pub fn new(max_entries: usize, max_age_millis: i64) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            max_age_millis,
        }
    }
    pub fn get(&mut self, uri: &str, version: Option<i64>, text: &str, now: i64) -> SnapshotResult {
        let hash = java_text_hash(text);
        let version = version.unwrap_or(i64::from(hash));
        let position = self.entries.iter().position(|e| {
            e.snapshot.document_uri == uri
                && e.snapshot.version == version
                && e.snapshot.text_hash == hash
        });
        if let Some(position) = position {
            let entry = self.entries.remove(position).unwrap();
            if now.saturating_sub(entry.created_at) <= self.max_age_millis
                && entry.snapshot.text == text
            {
                let snapshot = entry.snapshot.clone();
                self.entries.push_back(entry);
                return SnapshotResult {
                    snapshot,
                    cache_hit: true,
                };
            }
        }
        let snapshot = DocumentSnapshot {
            document_uri: uri.into(),
            version,
            text: text.into(),
            text_hash: hash,
        };
        self.entries.push_back(Entry {
            snapshot: snapshot.clone(),
            created_at: now,
        });
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
        SnapshotResult {
            snapshot,
            cache_hit: false,
        }
    }
    pub fn invalidate(&mut self, uri: &str) {
        self.entries.retain(|e| e.snapshot.document_uri != uri);
    }
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }
}
struct NegativeEntry {
    key: String,
    text: String,
    reason: String,
    expires_at: i64,
}
#[derive(Default)]
pub struct NegativeCompletionCache {
    entries: VecDeque<NegativeEntry>,
}
impl NegativeCompletionCache {
    pub fn get(&mut self, key: &str, text: &str, now: i64) -> Option<String> {
        let at = self.entries.iter().position(|e| e.key == key)?;
        let entry = self.entries.remove(at)?;
        if now > entry.expires_at || entry.text != text {
            return None;
        }
        let reason = entry.reason.clone();
        self.entries.push_back(entry);
        Some(reason)
    }
    pub fn put(&mut self, key: String, text: String, reason: String, expires_at: i64) {
        self.entries.retain(|e| e.key != key);
        self.entries.push_back(NegativeEntry {
            key,
            text,
            reason,
            expires_at,
        });
        while self.entries.len() > 256 {
            self.entries.pop_front();
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
pub fn negative_ttl(reason: &str) -> i64 {
    match reason {
        "NO_COMPLETION_CONTEXT" => 500,
        "NO_MIXIN_TARGET" | "PROJECT_CONTEXT_EMPTY" => 2000,
        _ => 1000,
    }
}
