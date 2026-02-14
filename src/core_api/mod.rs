use crate::node_ref::HASH_SIZE;

pub type TrieRoot = [u8; HASH_SIZE];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TrieSnapshot {
    pub root: TrieRoot,
    pub key_count: usize,
}

pub trait TrieStoreReader {
    fn load_raw_node(&mut self, hash: &[u8]) -> Option<Vec<u8>>;

    fn load_raw_value(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
        self.load_raw_node(hash)
    }
}

pub trait TrieStoreWriter {
    fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]);

    fn save_raw_value(&mut self, hash: &[u8], value: &[u8]);
}

pub trait TrieEngine {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>);

    fn delete(&mut self, key: &[u8]);

    fn delete_recursive(&mut self, prefix: &[u8]);

    fn get_value_length(&self, key: &[u8]) -> Option<usize>;

    fn get_value_hash(&self, key: &[u8]) -> Option<[u8; HASH_SIZE]>;

    fn collect_keys(&self, byte_size: usize) -> Vec<Vec<u8>>;

    fn get_storage_keys(&mut self, account_address: &[u8]) -> Vec<Vec<u8>>;

    fn current_root_hash(&mut self) -> TrieRoot;

    fn snapshot(&mut self) -> TrieSnapshot;
}
