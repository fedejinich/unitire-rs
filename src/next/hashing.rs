use crate::node_ref::HASH_SIZE;

#[derive(Debug, Default, Clone)]
pub struct IncrementalHashState {
    root_hash: Option<[u8; HASH_SIZE]>,
}

impl IncrementalHashState {
    pub fn invalidate(&mut self) {
        self.root_hash = None;
    }

    pub fn update(&mut self, root_hash: [u8; HASH_SIZE]) {
        self.root_hash = Some(root_hash);
    }

    pub fn root_hash(&self) -> Option<[u8; HASH_SIZE]> {
        self.root_hash
    }
}
