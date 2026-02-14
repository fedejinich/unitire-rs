use crate::node_ref::HASH_SIZE;

#[derive(Debug, Default, Clone)]
pub struct HashCache {
    cached_root: Option<[u8; HASH_SIZE]>,
}

impl HashCache {
    pub fn invalidate(&mut self) {
        self.cached_root = None;
    }

    pub fn update_root(&mut self, root_hash: [u8; HASH_SIZE]) {
        self.cached_root = Some(root_hash);
    }

    pub fn root_hash(&self) -> Option<[u8; HASH_SIZE]> {
        self.cached_root
    }
}
