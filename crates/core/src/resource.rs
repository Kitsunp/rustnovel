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
