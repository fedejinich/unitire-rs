pub mod codec_orchid;
pub mod codec_rskip107;
pub mod core_api;
pub mod core_trie;
pub mod hash;
pub mod next;
pub mod node_ref;
pub mod path;
pub mod storage_keys_packed;
pub mod store_adapter;
pub mod varint;

use std::fmt;

use crate::core_api::TrieSnapshot;
use crate::core_trie::{SaveStats, Unitrie};
use crate::next::core_trie::NextUnitrie;
use crate::node_ref::HASH_SIZE;

pub use crate::store_adapter::RawStoreAdapter;

pub type TrieRoot = [u8; HASH_SIZE];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnitrieImplementation {
    LegacyV1,
    Next,
}

impl UnitrieImplementation {
    pub fn from_config(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "legacy-v1" => Ok(Self::LegacyV1),
            "next" => Ok(Self::Next),
            other => Err(format!(
                "unsupported unitrie implementation '{other}', expected one of: legacy-v1, next"
            )),
        }
    }

    pub fn as_config_name(self) -> &'static str {
        match self {
            Self::LegacyV1 => "legacy-v1",
            Self::Next => "next",
        }
    }
}

impl fmt::Display for UnitrieImplementation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_name())
    }
}

#[derive(Debug, Clone)]
enum UnitrieCoreInner {
    Legacy(Unitrie),
    Next(NextUnitrie),
}

#[derive(Debug, Clone)]
pub struct UnitrieCore {
    implementation: UnitrieImplementation,
    inner: UnitrieCoreInner,
}

impl UnitrieCore {
    pub fn new(implementation: UnitrieImplementation) -> Self {
        let inner = match implementation {
            UnitrieImplementation::LegacyV1 => UnitrieCoreInner::Legacy(Unitrie::new()),
            UnitrieImplementation::Next => UnitrieCoreInner::Next(NextUnitrie::new()),
        };

        Self {
            implementation,
            inner,
        }
    }

    pub fn from_persisted_root<T: RawStoreAdapter>(
        implementation: UnitrieImplementation,
        root_hash: &[u8],
        store: &mut T,
    ) -> Result<Self, String> {
        let inner = match implementation {
            UnitrieImplementation::LegacyV1 => {
                UnitrieCoreInner::Legacy(Unitrie::from_persisted_root(root_hash, store)?)
            }
            UnitrieImplementation::Next => {
                UnitrieCoreInner::Next(NextUnitrie::from_persisted_root(root_hash, store)?)
            }
        };

        Ok(Self {
            implementation,
            inner,
        })
    }

    pub fn implementation(&self) -> UnitrieImplementation {
        self.implementation
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match &self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.get(key),
            UnitrieCoreInner::Next(trie) => trie.get(key),
        }
    }

    pub fn get_ref(&self, key: &[u8]) -> Option<&[u8]> {
        match &self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.get_ref(key),
            UnitrieCoreInner::Next(trie) => trie.get_ref(key),
        }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.put(key, value),
            UnitrieCoreInner::Next(trie) => trie.put(key, value),
        }
    }

    pub fn delete(&mut self, key: &[u8]) {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.delete(key),
            UnitrieCoreInner::Next(trie) => trie.delete(key),
        }
    }

    pub fn delete_recursive(&mut self, key: &[u8]) {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.delete_recursive(key),
            UnitrieCoreInner::Next(trie) => trie.delete_recursive(key),
        }
    }

    pub fn get_value_length(&self, key: &[u8]) -> Option<usize> {
        match &self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.get_value_length(key),
            UnitrieCoreInner::Next(trie) => trie.get_value_length(key),
        }
    }

    pub fn get_value_hash(&self, key: &[u8]) -> Option<TrieRoot> {
        match &self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.get_value_hash(key),
            UnitrieCoreInner::Next(trie) => trie.get_value_hash(key),
        }
    }

    pub fn collect_keys(&self, byte_size: usize) -> Vec<Vec<u8>> {
        match &self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.collect_keys(byte_size),
            UnitrieCoreInner::Next(trie) => trie.collect_keys(byte_size),
        }
    }

    pub fn get_storage_keys(&mut self, account_address: &[u8]) -> Vec<Vec<u8>> {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.get_storage_keys(account_address),
            UnitrieCoreInner::Next(trie) => trie.get_storage_keys(account_address),
        }
    }

    pub fn root_hash(&mut self) -> TrieRoot {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.root_hash(),
            UnitrieCoreInner::Next(trie) => trie.root_hash(),
        }
    }

    pub fn current_root_hash(&mut self) -> TrieRoot {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.current_root_hash(),
            UnitrieCoreInner::Next(trie) => trie.current_root_hash(),
        }
    }

    pub fn save_to_store<T: RawStoreAdapter>(&mut self, store: &mut T) {
        let _ = self.save_to_store_with_stats(store);
    }

    pub fn save_to_store_with_stats<T: RawStoreAdapter>(&mut self, store: &mut T) -> SaveStats {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => trie.save_to_store_with_stats(store),
            UnitrieCoreInner::Next(trie) => {
                trie.save_to_store(store);
                trie.last_save_stats()
            }
        }
    }

    pub fn snapshot(&mut self) -> TrieSnapshot {
        match &mut self.inner {
            UnitrieCoreInner::Legacy(trie) => TrieSnapshot {
                root: trie.current_root_hash(),
                key_count: trie.key_count(),
            },
            UnitrieCoreInner::Next(trie) => trie.snapshot(),
        }
    }
}
