pub mod codec_orchid;
pub mod codec_rskip107;
pub mod core_api;
pub mod core_trie;
pub mod hash;
pub mod node_ref;
pub mod path;
pub mod storage_keys_packed;
pub mod store_adapter;
pub mod varint;

use std::fmt;

use crate::core_api::TrieSnapshot;
use crate::core_trie::{SaveStats, Unitrie};
use crate::node_ref::HASH_SIZE;

pub use crate::store_adapter::RawStoreAdapter;

pub type TrieRoot = [u8; HASH_SIZE];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnitrieImplementation {
    LegacyV1,
}

impl UnitrieImplementation {
    pub fn from_config(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "legacy-v1" => Ok(Self::LegacyV1),
            other => Err(format!(
                "unsupported unitrie implementation '{other}', expected one of: legacy-v1"
            )),
        }
    }

    pub fn as_config_name(self) -> &'static str {
        match self {
            Self::LegacyV1 => "legacy-v1",
        }
    }
}

impl fmt::Display for UnitrieImplementation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_name())
    }
}

#[derive(Debug, Clone)]
pub struct UnitrieCore {
    implementation: UnitrieImplementation,
    inner: Unitrie,
}

impl UnitrieCore {
    pub fn new(implementation: UnitrieImplementation) -> Self {
        let inner = match implementation {
            UnitrieImplementation::LegacyV1 => Unitrie::new(),
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
            UnitrieImplementation::LegacyV1 => Unitrie::from_persisted_root(root_hash, store)?,
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
        self.inner.get(key)
    }

    pub fn get_ref(&self, key: &[u8]) -> Option<&[u8]> {
        self.inner.get_ref(key)
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.inner.put(key, value)
    }

    pub fn delete(&mut self, key: &[u8]) {
        self.inner.delete(key)
    }

    pub fn delete_recursive(&mut self, key: &[u8]) {
        self.inner.delete_recursive(key)
    }

    pub fn get_value_length(&self, key: &[u8]) -> Option<usize> {
        self.inner.get_value_length(key)
    }

    pub fn get_value_hash(&self, key: &[u8]) -> Option<TrieRoot> {
        self.inner.get_value_hash(key)
    }

    pub fn collect_keys(&self, byte_size: usize) -> Vec<Vec<u8>> {
        self.inner.collect_keys(byte_size)
    }

    pub fn get_storage_keys(&mut self, account_address: &[u8]) -> Vec<Vec<u8>> {
        self.inner.get_storage_keys(account_address)
    }

    pub fn root_hash(&mut self) -> TrieRoot {
        self.inner.root_hash()
    }

    pub fn current_root_hash(&mut self) -> TrieRoot {
        self.inner.current_root_hash()
    }

    pub fn save_to_store<T: RawStoreAdapter>(&mut self, store: &mut T) {
        let _ = self.save_to_store_with_stats(store);
    }

    pub fn save_to_store_with_stats<T: RawStoreAdapter>(&mut self, store: &mut T) -> SaveStats {
        self.inner.save_to_store_with_stats(store)
    }

    pub fn snapshot(&mut self) -> TrieSnapshot {
        TrieSnapshot {
            root: self.inner.current_root_hash(),
            key_count: self.inner.key_count(),
        }
    }
}
