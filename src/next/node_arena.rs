use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

#[derive(Debug, Default, Clone)]
pub struct NodeArena {
    next_id: u64,
    key_to_node: HashMap<Vec<u8>, NodeId>,
    dirty_nodes: BTreeSet<NodeId>,
}

impl NodeArena {
    pub fn mark_dirty_key(&mut self, key: &[u8]) {
        let node_id = self.key_to_node.entry(key.to_vec()).or_insert_with(|| {
            let current = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            NodeId(current)
        });
        self.dirty_nodes.insert(*node_id);
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty_nodes.len()
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_nodes.clear();
    }
}
