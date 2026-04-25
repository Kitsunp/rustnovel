use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

#[derive(Clone, Copy, Debug)]
pub struct ResourceLimiter {
    pub max_events: usize,
    pub max_text_length: usize,
    pub max_label_length: usize,
    pub max_asset_length: usize,
    pub max_characters: usize,
    pub max_script_bytes: usize,
}

impl Default for ResourceLimiter {
    fn default() -> Self {
        Self {
            max_events: 10_000,
            max_text_length: 4_096,
            max_label_length: 64,
            max_asset_length: 128,
            max_characters: 32,
            max_script_bytes: 512 * 1024,
        }
    }
}

/// Trait for calculating the string budget (size in bytes) of a resource.
pub trait StringBudget {
    fn string_bytes(&self) -> usize;
}

impl StringBudget for String {
    fn string_bytes(&self) -> usize {
        self.len()
    }
}

impl<T: StringBudget> StringBudget for Option<T> {
    fn string_bytes(&self) -> usize {
        match self {
            Some(inner) => inner.string_bytes(),
            None => 0,
        }
    }
}

impl<T: StringBudget> StringBudget for Vec<T> {
    fn string_bytes(&self) -> usize {
        self.iter().map(|item| item.string_bytes()).sum()
    }
}

#[derive(Debug)]
pub struct LruCache<K>
where
    K: Eq + Hash + Clone,
{
    map: HashMap<K, Vec<u8>>,
    order: VecDeque<K>,
    current_bytes: usize,
    max_bytes: usize,
}

impl<K> LruCache<K>
where
    K: Eq + Hash + Clone,
{
    pub fn new(max_bytes: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            current_bytes: 0,
            max_bytes,
        }
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn get(&mut self, key: &K) -> Option<&Vec<u8>> {
        if self.map.contains_key(key) {
            self.touch(key);
        }
        self.map.get(key)
    }

    pub fn insert(&mut self, key: K, value: Vec<u8>) {
        if let Some(existing) = self.map.get(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(existing.len());
        }
        self.map.insert(key.clone(), value);
        self.touch(&key);
        if let Some(stored) = self.map.get(&key) {
            self.current_bytes = self.current_bytes.saturating_add(stored.len());
        }
        self.evict_overflow();
    }

    fn touch(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|entry| entry == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.clone());
    }

    fn evict_overflow(&mut self) {
        while self.current_bytes > self.max_bytes && !self.order.is_empty() {
            let Some(lru_key) = self.order.pop_front() else {
                break;
            };
            if let Some(value) = self.map.remove(&lru_key) {
                self.current_bytes = self.current_bytes.saturating_sub(value.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_eviction() {
        // Engineer Manifesto: Resource Budgeting.
        // Verify that the cache strictly respects the byte budget.

        let mut cache = LruCache::<u32>::new(10); // 10 bytes max

        // Insert 4 bytes
        cache.insert(1, vec![1, 2, 3, 4]);
        assert_eq!(cache.current_bytes(), 4);
        assert_eq!(cache.len(), 1);

        // Insert another 4 bytes -> total 8
        cache.insert(2, vec![5, 6, 7, 8]);
        assert_eq!(cache.current_bytes(), 8);
        assert_eq!(cache.len(), 2);

        // Insert 4 bytes -> total 12 (overflows 10)
        // Should evict key 1 (LRU)
        cache.insert(3, vec![9, 10, 11, 12]);

        // Key 1 (4 bytes) evicted. Remaining: Key 2 (4 bytes) + Key 3 (4 bytes) = 8 bytes.
        assert_eq!(cache.current_bytes(), 8);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&1).is_none());
        assert!(cache.get(&2).is_some());
        assert!(cache.get(&3).is_some());

        // Access key 2 to make it MRU
        cache.touch(&2);

        // Insert 5 bytes. Total would be 8 + 5 = 13.
        // Should evict LRU (Key 3, 4 bytes) -> 4 left + 5 new = 9 bytes.
        cache.insert(4, vec![13, 14, 15, 16, 17]);

        assert_eq!(cache.current_bytes(), 9);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&2).is_some()); // Kept (MRU)
        assert!(cache.get(&3).is_none()); // Evicted (LRU)
        assert!(cache.get(&4).is_some());
    }
}
