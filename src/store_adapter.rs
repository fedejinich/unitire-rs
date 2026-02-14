use crate::core_api::{TrieStoreReader, TrieStoreWriter};

pub trait RawStoreAdapter {
    fn load_raw_node(&mut self, _hash: &[u8]) -> Option<Vec<u8>> {
        None
    }

    fn load_raw_value(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        RawStoreAdapter::load_raw_node(self, hash)
    }

    fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]);

    fn save_raw_value(&mut self, hash: &[u8], value: &[u8]);
}

impl<T> TrieStoreReader for T
where
    T: RawStoreAdapter + ?Sized,
{
    fn load_raw_node(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        RawStoreAdapter::load_raw_node(self, hash)
    }

    fn load_raw_value(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        RawStoreAdapter::load_raw_value(self, hash)
    }
}

impl<T> TrieStoreWriter for T
where
    T: RawStoreAdapter + ?Sized,
{
    fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]) {
        RawStoreAdapter::save_raw_node(self, hash, serialized_node);
    }

    fn save_raw_value(&mut self, hash: &[u8], value: &[u8]) {
        RawStoreAdapter::save_raw_value(self, hash, value);
    }
}

#[cfg(test)]
mod tests {
    use super::RawStoreAdapter;
    use std::collections::HashMap;

    #[derive(Default)]
    struct InMemoryStore {
        nodes: HashMap<Vec<u8>, Vec<u8>>,
        values: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl RawStoreAdapter for InMemoryStore {
        fn load_raw_node(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
            self.nodes.get(hash).cloned()
        }

        fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]) {
            self.nodes.insert(hash.to_vec(), serialized_node.to_vec());
        }

        fn save_raw_value(&mut self, hash: &[u8], value: &[u8]) {
            self.values.insert(hash.to_vec(), value.to_vec());
        }
    }

    #[test]
    fn in_memory_store_round_trip_node() {
        let mut store = InMemoryStore::default();
        store.save_raw_node(&[1, 2, 3], &[9, 9, 9]);
        assert_eq!(
            store.load_raw_node(&[1, 2, 3]).as_deref(),
            Some([9, 9, 9].as_ref())
        );
    }
}
