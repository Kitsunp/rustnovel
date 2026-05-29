use std::sync::Arc;
use visual_novel_engine::LruCache;

#[test]
fn lru_cache_evicts_oldest_entries_within_byte_budget() {
    let mut cache = LruCache::<u32>::new(10);

    cache.insert(1, vec![1, 2, 3, 4]);
    assert_eq!(cache.current_bytes(), 4);
    assert_eq!(cache.len(), 1);

    cache.insert(2, vec![5, 6, 7, 8]);
    assert_eq!(cache.current_bytes(), 8);
    assert_eq!(cache.len(), 2);

    cache.insert(3, vec![9, 10, 11, 12]);
    assert_eq!(cache.current_bytes(), 8);
    assert_eq!(cache.len(), 2);
    assert!(cache.get(&1).is_none());
    assert!(cache.get(&2).is_some());
    assert!(cache.get(&3).is_some());

    let _ = cache.get(&2);
    cache.insert(4, vec![13, 14, 15, 16, 17]);

    assert_eq!(cache.current_bytes(), 9);
    assert_eq!(cache.len(), 2);
    assert!(cache.get(&2).is_some());
    assert!(cache.get(&3).is_none());
    assert!(cache.get(&4).is_some());
}

#[test]
fn lru_cache_returns_shared_bytes_without_cloning_payloads() {
    let mut cache = LruCache::<u32>::new(10);
    cache.insert(1, vec![1, 2, 3, 4]);

    let first = cache.get(&1).expect("first hit");
    let second = cache.get(&1).expect("second hit");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.as_ref(), &[1, 2, 3, 4]);
}
