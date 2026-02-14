use crate::core_api::{TrieEngine, TrieSnapshot};
use crate::core_trie::{SaveStats, Unitrie};
use crate::next::hashing::IncrementalHashState;
use crate::next::iter::collect_exact_size_keys;
use crate::next::mutation::MutationGeneration;
use crate::next::node_arena::NodeArena;
use crate::next::persistence::IncrementalPersistence;
use crate::next::storage_iteration_cache::StorageIterationCache;
use crate::node_ref::HASH_SIZE;
use crate::storage_keys_packed;
use crate::store_adapter::RawStoreAdapter;
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
pub struct NextUnitrie {
    inner: Unitrie,
    node_arena: NodeArena,
    hash_state: IncrementalHashState,
    persistence: IncrementalPersistence,
    storage_iteration_cache: StorageIterationCache,
    mutation_generation: MutationGeneration,
    last_save_stats: SaveStats,
}

impl NextUnitrie {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_persisted_root<T: RawStoreAdapter>(
        root_hash: &[u8],
        store: &mut T,
    ) -> Result<Self, String> {
        let inner = Unitrie::from_persisted_root(root_hash, store)?;
        let mut this = Self {
            inner,
            ..Self::default()
        };
        this.hash_state.update(this.inner.current_root_hash());
        Ok(this)
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.inner.get(key)
    }

    pub fn get_ref(&self, key: &[u8]) -> Option<&[u8]> {
        self.inner.get_ref(key)
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.bump_mutation_generation();
        self.node_arena.mark_dirty_key(&key);
        self.hash_state.invalidate();
        self.inner.put(key, value);
    }

    pub fn delete(&mut self, key: &[u8]) {
        self.bump_mutation_generation();
        self.node_arena.mark_dirty_key(key);
        self.hash_state.invalidate();
        self.inner.delete(key);
    }

    pub fn delete_recursive(&mut self, prefix: &[u8]) {
        self.bump_mutation_generation();
        self.node_arena.mark_dirty_key(prefix);
        self.hash_state.invalidate();
        self.inner.delete_recursive(prefix);
    }

    pub fn get_value_length(&self, key: &[u8]) -> Option<usize> {
        self.inner.get_value_length(key)
    }

    pub fn get_value_hash(&self, key: &[u8]) -> Option<[u8; HASH_SIZE]> {
        self.inner.get_value_hash(key)
    }

    pub fn collect_keys(&self, byte_size: usize) -> Vec<Vec<u8>> {
        collect_exact_size_keys(self.inner.keys(), byte_size)
    }

    pub fn get_storage_keys(&mut self, account_address: &[u8]) -> Vec<Vec<u8>> {
        self.storage_keys_bundle_for_account(account_address)
            .0
            .as_ref()
            .clone()
    }

    pub fn get_storage_keys_packed(&mut self, account_address: &[u8]) -> Arc<Vec<u8>> {
        self.storage_keys_bundle_for_account(account_address).1
    }

    pub fn root_hash(&mut self) -> [u8; HASH_SIZE] {
        if let Some(cached) = self.hash_state.root_hash() {
            return cached;
        }

        let root = self.inner.root_hash();
        self.hash_state.update(root);
        root
    }

    pub fn current_root_hash(&mut self) -> [u8; HASH_SIZE] {
        self.root_hash()
    }

    pub fn snapshot(&mut self) -> TrieSnapshot {
        TrieSnapshot {
            root: self.current_root_hash(),
            key_count: self.inner.key_count(),
        }
    }

    pub fn save_to_store<T: RawStoreAdapter>(&mut self, store: &mut T) {
        self.last_save_stats =
            self.persistence
                .save(&mut self.inner, store, self.node_arena.dirty_count());
        self.node_arena.clear_dirty();
        self.hash_state.update(self.inner.current_root_hash());
    }

    pub fn last_save_stats(&self) -> SaveStats {
        self.last_save_stats
    }

    fn storage_keys_bundle_for_account(
        &mut self,
        account_address: &[u8],
    ) -> (Arc<Vec<Vec<u8>>>, Arc<Vec<u8>>) {
        if let (Some(cached_keys), Some(cached_packed)) = (
            self.storage_iteration_cache
                .get_keys(account_address, self.mutation_generation.current()),
            self.storage_iteration_cache
                .get_packed(account_address, self.mutation_generation.current()),
        ) {
            return (cached_keys, cached_packed);
        }

        let keys = Arc::new(self.inner.get_storage_keys(account_address));
        let packed = Arc::new(storage_keys_packed::encode(keys.as_ref()));
        self.storage_iteration_cache.insert(
            account_address.to_vec(),
            self.mutation_generation.current(),
            keys,
            packed,
        )
    }

    fn bump_mutation_generation(&mut self) {
        self.mutation_generation.next();
    }
}

impl TrieEngine for NextUnitrie {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.get(key)
    }

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.put(key, value);
    }

    fn delete(&mut self, key: &[u8]) {
        self.delete(key);
    }

    fn delete_recursive(&mut self, prefix: &[u8]) {
        self.delete_recursive(prefix);
    }

    fn get_value_length(&self, key: &[u8]) -> Option<usize> {
        self.get_value_length(key)
    }

    fn get_value_hash(&self, key: &[u8]) -> Option<[u8; HASH_SIZE]> {
        self.get_value_hash(key)
    }

    fn collect_keys(&self, byte_size: usize) -> Vec<Vec<u8>> {
        self.collect_keys(byte_size)
    }

    fn get_storage_keys(&mut self, account_address: &[u8]) -> Vec<Vec<u8>> {
        self.get_storage_keys(account_address)
    }

    fn current_root_hash(&mut self) -> [u8; HASH_SIZE] {
        self.current_root_hash()
    }

    fn snapshot(&mut self) -> TrieSnapshot {
        self.snapshot()
    }
}
