use crate::core_trie::{SaveStats, Unitrie};
use crate::node_ref::HASH_SIZE;
use crate::store_adapter::RawStoreAdapter;

#[derive(Debug, Default, Clone)]
pub struct IncrementalPersistence {
    last_saved_root: Option<[u8; HASH_SIZE]>,
}

impl IncrementalPersistence {
    pub fn save<T: RawStoreAdapter>(
        &mut self,
        trie: &mut Unitrie,
        store: &mut T,
        dirty_nodes: usize,
    ) -> SaveStats {
        let current_root = trie.current_root_hash();
        if dirty_nodes == 0 && self.last_saved_root == Some(current_root) {
            return SaveStats::default();
        }

        let save_stats = trie.save_to_store_with_stats(store);
        self.last_saved_root = Some(trie.current_root_hash());
        save_stats
    }
}
