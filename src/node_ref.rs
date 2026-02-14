use crate::hash::keccak256;
use crate::path::shared_path_serializer;

pub const HASH_SIZE: usize = 32;
pub const LONG_VALUE_THRESHOLD: usize = 32;
pub const MAX_EMBEDDED_NODE_SIZE_IN_BYTES: usize = 44;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CodecMode {
    Rskip107,
    Orchid,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SharedPath {
    bits: Vec<u8>,
}

impl SharedPath {
    pub fn empty() -> Self {
        Self { bits: Vec::new() }
    }

    pub fn from_bits(bits: Vec<u8>) -> Result<Self, String> {
        if bits.iter().any(|bit| *bit > 1) {
            return Err("shared path must contain only 0/1 bits".to_string());
        }

        Ok(Self { bits })
    }

    pub fn len(&self) -> usize {
        self.bits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    pub fn as_bits(&self) -> &[u8] {
        &self.bits
    }

    pub fn encoded(&self) -> Vec<u8> {
        shared_path_serializer::encode(&self.bits)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ValueRef {
    Empty,
    Inline(Vec<u8>),
    Hashed {
        hash: [u8; HASH_SIZE],
        length: Option<usize>,
    },
}

impl ValueRef {
    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn inline(value: Vec<u8>) -> Self {
        if value.is_empty() {
            return Self::Empty;
        }
        Self::Inline(value)
    }

    pub fn hashed(hash: [u8; HASH_SIZE], length: Option<usize>) -> Self {
        Self::Hashed { hash, length }
    }

    pub fn len(&self) -> Option<usize> {
        match self {
            ValueRef::Empty => Some(0),
            ValueRef::Inline(value) => Some(value.len()),
            ValueRef::Hashed { length, .. } => *length,
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, ValueRef::Empty)
            || matches!(self, ValueRef::Inline(value) if value.is_empty())
    }

    pub fn has_value(&self) -> bool {
        match self {
            ValueRef::Empty => false,
            ValueRef::Inline(value) => !value.is_empty(),
            ValueRef::Hashed { .. } => true,
        }
    }

    pub fn has_long_value(&self) -> bool {
        match self.len() {
            Some(length) => length > LONG_VALUE_THRESHOLD,
            None => true,
        }
    }

    pub fn hash(&self) -> Option<[u8; HASH_SIZE]> {
        match self {
            ValueRef::Empty => None,
            ValueRef::Inline(value) => Some(keccak256(value)),
            ValueRef::Hashed { hash, .. } => Some(*hash),
        }
    }

    pub fn inline_bytes(&self) -> Option<&[u8]> {
        match self {
            ValueRef::Inline(value) => Some(value.as_slice()),
            _ => None,
        }
    }

    pub fn with_known_length(self, length: usize) -> Self {
        match self {
            ValueRef::Hashed { hash, .. } => ValueRef::Hashed {
                hash,
                length: Some(length),
            },
            other => other,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NodeReference {
    Empty,
    Embedded(Box<TrieNode>),
    Hashed([u8; HASH_SIZE]),
}

impl NodeReference {
    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn embedded(node: TrieNode) -> Self {
        Self::Embedded(Box::new(node))
    }

    pub fn hashed(hash: [u8; HASH_SIZE]) -> Self {
        Self::Hashed(hash)
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, NodeReference::Empty)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TrieNode {
    pub shared_path: SharedPath,
    pub value: ValueRef,
    pub left: NodeReference,
    pub right: NodeReference,
}

impl TrieNode {
    pub fn new(
        shared_path: SharedPath,
        value: ValueRef,
        left: NodeReference,
        right: NodeReference,
    ) -> Self {
        Self {
            shared_path,
            value,
            left,
            right,
        }
    }

    pub fn empty() -> Self {
        Self::new(
            SharedPath::empty(),
            ValueRef::empty(),
            NodeReference::empty(),
            NodeReference::empty(),
        )
    }

    pub fn is_terminal(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }

    pub fn has_value(&self) -> bool {
        self.value.has_value()
    }

    pub fn value_length(&self) -> usize {
        self.value.len().unwrap_or(0)
    }

    pub fn has_long_value(&self) -> bool {
        self.value.has_long_value()
    }

    pub fn is_empty_trie(&self) -> bool {
        !self.has_value() && self.left.is_empty() && self.right.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{NodeReference, SharedPath, TrieNode, ValueRef, HASH_SIZE};

    #[test]
    fn shared_path_rejects_invalid_bit_values() {
        assert!(SharedPath::from_bits(vec![0, 1, 2]).is_err());
    }

    #[test]
    fn inline_value_hash_matches_expected_size() {
        let value = ValueRef::inline(vec![1, 2, 3]);
        assert_eq!(value.hash().map(|hash| hash.len()), Some(HASH_SIZE));
    }

    #[test]
    fn trie_node_empty_detection_matches_java_semantics() {
        let node = TrieNode::new(
            SharedPath::empty(),
            ValueRef::empty(),
            NodeReference::empty(),
            NodeReference::empty(),
        );
        assert!(node.is_empty_trie());
    }
}
