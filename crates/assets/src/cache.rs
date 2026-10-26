use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct CachedBytes {
    pub data: Arc<[u8]>,
    pub bytes: usize,
    pub last_used: u64,
}

#[derive(Debug)]
pub(super) struct ByteCache {
    entries: HashMap<String, CachedBytes>,
    usage_counter: u64,
    current_bytes: usize,
    max_bytes: usize,
}

impl ByteCache {
    pub(super) fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            usage_counter: 0,
            current_bytes: 0,
            max_bytes,
        }
    }

    pub(super) fn get(&mut self, key: &str) -> Option<Arc<[u8]>> {
        self.usage_counter = self.usage_counter.wrapping_add(1);
        self.entries.get_mut(key).map(|entry| {
            entry.last_used = self.usage_counter;
            Arc::clone(&entry.data)
        })
    }

    pub(super) fn insert(&mut self, key: String, data: Arc<[u8]>) {
        let bytes = data.len();
        if let Some(old) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(old.bytes);
        }
        // Empty payloads still consume map/key memory. Do not retain them,
        // and remove stale entries even when their replacement cannot fit.
        if bytes == 0 || bytes > self.max_bytes {
            return;
        }
        self.usage_counter = self.usage_counter.wrapping_add(1);

        while self.current_bytes > self.max_bytes - bytes || self.entries.len() >= 4096 {
            let Some((evict_key, evict_bytes)) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, entry)| (key.clone(), entry.bytes))
            else {
                break;
            };
            self.entries.remove(&evict_key);
            self.current_bytes = self.current_bytes.saturating_sub(evict_bytes);
        }

        self.entries.insert(
            key,
            CachedBytes {
                data,
                bytes,
                last_used: self.usage_counter,
            },
        );
        self.current_bytes = self.current_bytes.saturating_add(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_files_do_not_accumulate_with_any_budget() {
        for budget in [0, 1024] {
            let mut cache = ByteCache::new(budget);
            for i in 0..10_000 {
                cache.insert(format!("empty-{i}"), Arc::from([]));
            }
            assert!(cache.entries.is_empty());
            assert_eq!(cache.current_bytes, 0);
        }
    }

    #[test]
    fn oversized_replacement_removes_stale_payload() {
        let mut cache = ByteCache::new(4);
        cache.insert("asset".into(), Arc::from([1, 2]));
        cache.insert("asset".into(), Arc::from([3; 5]));
        assert!(cache.get("asset").is_none());
        assert_eq!(cache.current_bytes, 0);
    }

    #[test]
    fn tiny_files_have_a_bounded_entry_count() {
        let mut cache = ByteCache::new(usize::MAX);
        for i in 0..5000 {
            cache.insert(format!("tiny-{i}"), Arc::from([1]));
        }
        assert_eq!(cache.entries.len(), 4096);
        assert_eq!(cache.current_bytes, 4096);
        assert!(cache.get("tiny-0").is_none());
        assert!(cache.get("tiny-4999").is_some());
    }
}
