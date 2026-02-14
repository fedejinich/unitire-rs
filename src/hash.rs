use tiny_keccak::{Hasher, Keccak};

pub const EMPTY_TRIE_RLP: [u8; 1] = [0x80];

pub fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(input);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

pub fn empty_trie_hash() -> [u8; 32] {
    keccak256(&EMPTY_TRIE_RLP)
}

#[cfg(test)]
mod tests {
    use super::{empty_trie_hash, keccak256};

    #[test]
    fn empty_hash_is_stable() {
        let hash = empty_trie_hash();
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn keccak_is_stable_for_input() {
        let first = keccak256(b"rsk");
        let second = keccak256(b"rsk");
        assert_eq!(first, second);
    }
}
