use crate::next::path_bits::PathBits;
use crate::node_ref::{HASH_SIZE, LONG_VALUE_THRESHOLD};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NextChildRef {
    Empty,
    InMemory(u64),
    Hashed([u8; HASH_SIZE]),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NextValueRef {
    Empty,
    Inline(Vec<u8>),
    Hashed {
        hash: [u8; HASH_SIZE],
        length: usize,
    },
}

impl NextValueRef {
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Inline(value) => value.len(),
            Self::Hashed { length, .. } => *length,
        }
    }

    pub fn has_long_value(&self) -> bool {
        self.len() > LONG_VALUE_THRESHOLD
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NextNode {
    pub shared_path: PathBits,
    pub value: NextValueRef,
    pub left: NextChildRef,
    pub right: NextChildRef,
}

impl NextNode {
    pub fn empty() -> Self {
        Self {
            shared_path: PathBits::empty(),
            value: NextValueRef::Empty,
            left: NextChildRef::Empty,
            right: NextChildRef::Empty,
        }
    }
}
