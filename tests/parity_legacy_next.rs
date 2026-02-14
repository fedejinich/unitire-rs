use std::collections::HashMap;
use unitrie_rs::hash::keccak256;
use unitrie_rs::{RawStoreAdapter, UnitrieCore, UnitrieImplementation};

#[derive(Default)]
struct InMemoryStore {
    nodes: HashMap<Vec<u8>, Vec<u8>>,
    values: HashMap<Vec<u8>, Vec<u8>>,
}

impl RawStoreAdapter for InMemoryStore {
    fn load_raw_node(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        self.nodes.get(hash).cloned()
    }

    fn load_raw_value(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        self.values.get(hash).cloned()
    }

    fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]) {
        self.nodes.insert(hash.to_vec(), serialized_node.to_vec());
    }

    fn save_raw_value(&mut self, hash: &[u8], value: &[u8]) {
        self.values.insert(hash.to_vec(), value.to_vec());
    }
}

#[test]
fn legacy_and_next_match_on_deterministic_operations() {
    let mut legacy = UnitrieCore::new(UnitrieImplementation::LegacyV1);
    let mut next = UnitrieCore::new(UnitrieImplementation::Next);

    let account = [0x11u8; 20];
    let storage_key_a = vec![0x01, 0x02, 0x03];
    let storage_key_b = vec![0xaau8; 32];

    let storage_prefixed_a = storage_full_key(&account, &storage_key_a);
    let storage_prefixed_b = storage_full_key(&account, &storage_key_b);

    let operations: Vec<(Vec<u8>, Vec<u8>)> = vec![
        (b"aa".to_vec(), b"v1".to_vec()),
        (b"ab".to_vec(), vec![0x99; 32]),
        (b"ab".to_vec(), vec![0x98; 33]),
        (b"abc".to_vec(), b"v3".to_vec()),
        (storage_prefixed_a.clone(), b"sv-a".to_vec()),
        (storage_prefixed_b.clone(), b"sv-b".to_vec()),
    ];

    for (key, value) in operations {
        legacy.put(key.clone(), value.clone());
        next.put(key.clone(), value.clone());
        assert_step_parity(&mut legacy, &mut next, &key);
    }

    legacy.delete(b"aa");
    next.delete(b"aa");
    assert_step_parity(&mut legacy, &mut next, b"aa");

    legacy.delete_recursive(b"ab");
    next.delete_recursive(b"ab");
    assert_step_parity(&mut legacy, &mut next, b"ab");

    let legacy_storage = legacy.get_storage_keys(&account);
    let next_storage = next.get_storage_keys(&account);
    assert_eq!(legacy_storage, next_storage);

    let legacy_keys = legacy.collect_keys(i32::MAX as usize);
    let next_keys = next.collect_keys(i32::MAX as usize);
    assert_eq!(legacy_keys, next_keys);
}

#[test]
fn next_and_legacy_are_cross_read_write_compatible() {
    let mut legacy = UnitrieCore::new(UnitrieImplementation::LegacyV1);
    let mut next = UnitrieCore::new(UnitrieImplementation::Next);

    legacy.put(b"k1".to_vec(), b"legacy".to_vec());
    legacy.put(b"k2".to_vec(), vec![0x42; 33]);

    next.put(b"k1".to_vec(), b"legacy".to_vec());
    next.put(b"k2".to_vec(), vec![0x42; 33]);

    let mut legacy_store = InMemoryStore::default();
    let mut next_store = InMemoryStore::default();

    let legacy_root = legacy.current_root_hash();
    let next_root = next.current_root_hash();
    assert_eq!(legacy_root, next_root);

    legacy.save_to_store(&mut legacy_store);
    next.save_to_store(&mut next_store);

    let mut next_from_legacy = UnitrieCore::from_persisted_root(
        UnitrieImplementation::Next,
        &legacy_root,
        &mut legacy_store,
    )
    .expect("next should load persisted legacy root");
    let mut legacy_from_next = UnitrieCore::from_persisted_root(
        UnitrieImplementation::LegacyV1,
        &next_root,
        &mut next_store,
    )
    .expect("legacy should load persisted next root");

    assert_eq!(next_from_legacy.get(b"k1"), Some(b"legacy".to_vec()));
    assert_eq!(legacy_from_next.get(b"k1"), Some(b"legacy".to_vec()));
    assert_eq!(next_from_legacy.get_value_length(b"k2"), Some(33));
    assert_eq!(legacy_from_next.get_value_length(b"k2"), Some(33));

    assert_eq!(next_from_legacy.current_root_hash(), legacy_root);
    assert_eq!(legacy_from_next.current_root_hash(), next_root);
}

fn assert_step_parity(legacy: &mut UnitrieCore, next: &mut UnitrieCore, key: &[u8]) {
    assert_eq!(legacy.get(key), next.get(key));
    assert_eq!(legacy.get_value_length(key), next.get_value_length(key));
    assert_eq!(legacy.get_value_hash(key), next.get_value_hash(key));
    assert_eq!(legacy.current_root_hash(), next.current_root_hash());
}

fn storage_full_key(account_address: &[u8], storage_key: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.push(0x00);
    output.extend_from_slice(&secure_prefix(account_address));
    output.extend_from_slice(account_address);
    output.push(0x00);
    output.extend_from_slice(&secure_prefix(storage_key));
    output.extend_from_slice(storage_key);
    output
}

fn secure_prefix(input: &[u8]) -> [u8; 10] {
    let hash = keccak256(input);
    let mut prefix = [0u8; 10];
    prefix.copy_from_slice(&hash[..10]);
    prefix
}
