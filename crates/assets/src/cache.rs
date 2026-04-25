use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct CachedBytes {
    pub data: Vec<u8>,
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

    pub(super) fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.usage_counter = self.usage_counter.wrapping_add(1);
        self.entries.get_mut(key).map(|entry| {
            entry.last_used = self.usage_counter;
            entry.data.clone()
        })
    }

    pub(super) fn insert(&mut self, key: String, data: Vec<u8>) {
        let bytes = data.len();
        if bytes > self.max_bytes {
            return;
        }

        self.usage_counter = self.usage_counter.wrapping_add(1);

        if let Some(old) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(old.bytes);
        }

        while self.current_bytes + bytes > self.max_bytes {
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
