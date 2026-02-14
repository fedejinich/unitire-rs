use crate::codec_orchid::OrchidCodec;
use crate::codec_rskip107::{ChildEncoding, Rskip107Codec};
use crate::hash::{empty_trie_hash, keccak256};
use crate::node_ref::{
    NodeReference, SharedPath, TrieNode, ValueRef, HASH_SIZE, LONG_VALUE_THRESHOLD,
    MAX_EMBEDDED_NODE_SIZE_IN_BYTES,
};
use crate::path::shared_path_serializer;
use crate::store_adapter::RawStoreAdapter;
use std::collections::{BTreeMap, HashMap, HashSet};

const SECURE_KEY_SIZE: usize = 10;
const DOMAIN_PREFIX: [u8; 1] = [0x00];
const STORAGE_PREFIX: [u8; 1] = [0x00];

#[derive(Debug, Clone)]
struct MaterializedTrie {
    root_node: Option<TrieNode>,
    root_hash: [u8; HASH_SIZE],
}

#[derive(Debug, Clone)]
struct NodeMetadata {
    hash: [u8; HASH_SIZE],
    serialized: Vec<u8>,
    reference_size: u64,
    embeddable: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SaveStats {
    pub nodes_visited: u64,
    pub nodes_written: u64,
    pub values_written: u64,
}

#[derive(Debug, Default, Clone)]
pub struct Unitrie {
    entries: BTreeMap<Vec<u8>, Vec<u8>>,
    materialized: Option<MaterializedTrie>,
    persisted_node_hashes: HashSet<[u8; HASH_SIZE]>,
    persisted_value_hashes: HashSet<[u8; HASH_SIZE]>,
}

impl Unitrie {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_persisted_root<T: RawStoreAdapter>(
        root_hash: &[u8],
        store: &mut T,
    ) -> Result<Self, String> {
        if root_hash.len() != HASH_SIZE {
            return Err(format!(
                "root hash must be {HASH_SIZE} bytes, got {}",
                root_hash.len()
            ));
        }

        let mut fixed_root = [0u8; HASH_SIZE];
        fixed_root.copy_from_slice(root_hash);
        if fixed_root == empty_trie_hash() {
            return Ok(Self::new());
        }

        let root_payload = store
            .load_raw_node(root_hash)
            .ok_or_else(|| "root hash not found in store adapter".to_string())?;
        let root_node = decode_persisted_node(&root_payload)?;

        let mut node_cache = HashMap::new();
        let mut persisted_node_hashes = HashSet::new();
        persisted_node_hashes.insert(fixed_root);
        let mut persisted_value_hashes = HashSet::new();
        let mut entries = BTreeMap::new();
        collect_entries_from_node(
            &root_node,
            Vec::new(),
            store,
            &mut node_cache,
            &mut entries,
            &mut persisted_node_hashes,
            &mut persisted_value_hashes,
        )?;

        Ok(Self {
            entries,
            materialized: None,
            persisted_node_hashes,
            persisted_value_hashes,
        })
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.entries.get(key).cloned()
    }

    pub fn get_ref(&self, key: &[u8]) -> Option<&[u8]> {
        self.entries.get(key).map(Vec::as_slice)
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        if value.is_empty() {
            self.entries.remove(&key);
        } else {
            self.entries.insert(key, value);
        }
        self.materialized = None;
    }

    pub fn delete(&mut self, key: &[u8]) {
        self.entries.remove(key);
        self.materialized = None;
    }

    pub fn delete_recursive(&mut self, prefix: &[u8]) {
        if self.entries.is_empty() {
            return;
        }

        if prefix.is_empty() {
            self.entries.clear();
            self.materialized = None;
            return;
        }

        // Remove lexicographic window [prefix, prefix_upper_bound) which is exactly
        // the key set that starts with `prefix`.
        let mut tail = self.entries.split_off(prefix);
        let removed_any = if let Some(upper_bound) = prefix_upper_bound(prefix) {
            let mut suffix = tail.split_off(&upper_bound);
            let removed = !tail.is_empty();
            self.entries.append(&mut suffix);
            removed
        } else {
            // Prefix made of 0xff bytes has no finite upper bound; everything in tail matches.
            !tail.is_empty()
        };

        if !removed_any {
            // Restore original map when nothing was removed.
            self.entries.append(&mut tail);
            return;
        }

        self.materialized = None;
    }

    pub fn get_value_length(&self, key: &[u8]) -> Option<usize> {
        self.entries.get(key).map(Vec::len)
    }

    pub fn get_value_hash(&self, key: &[u8]) -> Option<[u8; HASH_SIZE]> {
        self.entries.get(key).map(|value| keccak256(value))
    }

    // Matches Java semantics: collect keys with exactly `byte_size` bytes.
    // Integer.MAX_VALUE (from JNI) means collect all keys.
    pub fn collect_keys(&self, byte_size: usize) -> Vec<Vec<u8>> {
        let collect_all = byte_size == i32::MAX as usize;
        self.entries
            .keys()
            .filter(|key| collect_all || key.len() == byte_size)
            .cloned()
            .collect()
    }

    pub fn keys(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.entries.keys()
    }

    // Matches MutableTrieImpl storage key extraction:
    // accountStoragePrefixKey = [0x00] + secure(addr)[0..10] + addr + [0x00]
    // storage key payload starts after the secure subkey prefix (10 bytes).
    pub fn get_storage_keys(&self, account_address: &[u8]) -> Vec<Vec<u8>> {
        let account_storage_prefix_key = account_storage_prefix_key(account_address);

        self.entries
            .keys()
            .filter_map(|key| {
                if !key.starts_with(&account_storage_prefix_key) {
                    return None;
                }

                let storage_key_payload = &key[account_storage_prefix_key.len()..];
                if storage_key_payload.len() < SECURE_KEY_SIZE {
                    return None;
                }

                Some(storage_key_payload[SECURE_KEY_SIZE..].to_vec())
            })
            .collect()
    }

    pub fn root_hash(&mut self) -> [u8; HASH_SIZE] {
        self.materialize().root_hash
    }

    pub fn current_root_hash(&mut self) -> [u8; HASH_SIZE] {
        self.root_hash()
    }

    pub fn key_count(&self) -> usize {
        self.entries.len()
    }

    pub fn save_to_store<T: RawStoreAdapter>(&mut self, store: &mut T) {
        let _ = self.save_to_store_with_stats(store);
    }

    pub fn save_to_store_with_stats<T: RawStoreAdapter>(&mut self, store: &mut T) -> SaveStats {
        if self.entries.is_empty() {
            let empty_node_serialized = Rskip107Codec::encode_node(
                &TrieNode::empty(),
                &ChildEncoding::Empty,
                &ChildEncoding::Empty,
                None,
            )
            .expect("empty trie node encoding should never fail");
            let empty_hash = empty_trie_hash();
            store.save_raw_node(&empty_hash, &empty_node_serialized);
            self.persisted_node_hashes.insert(empty_hash);
            self.materialized = Some(MaterializedTrie {
                root_node: None,
                root_hash: empty_hash,
            });
            return SaveStats {
                nodes_visited: 1,
                nodes_written: 1,
                values_written: 0,
            };
        }

        let root_node = self
            .materialize()
            .root_node
            .as_ref()
            .expect("non-empty trie must have root node")
            .clone();

        let (root_metadata, save_stats) = persist_node_recursive(
            &root_node,
            store,
            &mut self.persisted_node_hashes,
            &mut self.persisted_value_hashes,
            true,
        )
        .expect("persisting node generated from in-memory entries should not fail");
        self.materialized = Some(MaterializedTrie {
            root_node: Some(root_node),
            root_hash: root_metadata.hash,
        });
        save_stats
    }

    fn materialize(&mut self) -> &MaterializedTrie {
        if self.materialized.is_none() {
            let root_node = build_root_node(&self.entries);
            let root_hash = match root_node.as_ref() {
                None => empty_trie_hash(),
                Some(node) => {
                    compute_node_metadata(node)
                        .expect("materialized node generated from entries should be encodable")
                        .hash
                }
            };

            self.materialized = Some(MaterializedTrie {
                root_node,
                root_hash,
            });
        }

        self.materialized.as_ref().expect("materialized trie")
    }
}

fn decode_persisted_node(payload: &[u8]) -> Result<TrieNode, String> {
    if OrchidCodec::is_orchid_message(payload) {
        return OrchidCodec::decode_node(payload);
    }

    Rskip107Codec::decode_node(payload)
}

fn collect_entries_from_node<T: RawStoreAdapter>(
    node: &TrieNode,
    prefix_bits: Vec<u8>,
    store: &mut T,
    node_cache: &mut HashMap<[u8; HASH_SIZE], TrieNode>,
    entries: &mut BTreeMap<Vec<u8>, Vec<u8>>,
    persisted_node_hashes: &mut HashSet<[u8; HASH_SIZE]>,
    persisted_value_hashes: &mut HashSet<[u8; HASH_SIZE]>,
) -> Result<(), String> {
    let mut full_bits = prefix_bits;
    full_bits.extend_from_slice(node.shared_path.as_bits());

    if node.value.has_value() {
        if let ValueRef::Hashed { hash, .. } = &node.value {
            persisted_value_hashes.insert(*hash);
        }
        let value = resolve_node_value(&node.value, store)?;
        entries.insert(shared_path_serializer::encode(&full_bits), value);
    }

    collect_child_entries(
        &node.left,
        0,
        &full_bits,
        store,
        node_cache,
        entries,
        persisted_node_hashes,
        persisted_value_hashes,
    )?;
    collect_child_entries(
        &node.right,
        1,
        &full_bits,
        store,
        node_cache,
        entries,
        persisted_node_hashes,
        persisted_value_hashes,
    )?;
    Ok(())
}

fn collect_child_entries<T: RawStoreAdapter>(
    reference: &NodeReference,
    implicit_bit: u8,
    parent_bits: &[u8],
    store: &mut T,
    node_cache: &mut HashMap<[u8; HASH_SIZE], TrieNode>,
    entries: &mut BTreeMap<Vec<u8>, Vec<u8>>,
    persisted_node_hashes: &mut HashSet<[u8; HASH_SIZE]>,
    persisted_value_hashes: &mut HashSet<[u8; HASH_SIZE]>,
) -> Result<(), String> {
    let child = match reference {
        NodeReference::Empty => return Ok(()),
        NodeReference::Embedded(node) => node.as_ref().clone(),
        NodeReference::Hashed(hash) => {
            persisted_node_hashes.insert(*hash);
            load_node_by_hash(hash, store, node_cache)?
        }
    };

    let mut child_prefix = parent_bits.to_vec();
    child_prefix.push(implicit_bit);
    collect_entries_from_node(
        &child,
        child_prefix,
        store,
        node_cache,
        entries,
        persisted_node_hashes,
        persisted_value_hashes,
    )
}

fn load_node_by_hash<T: RawStoreAdapter>(
    hash: &[u8; HASH_SIZE],
    store: &mut T,
    node_cache: &mut HashMap<[u8; HASH_SIZE], TrieNode>,
) -> Result<TrieNode, String> {
    if let Some(node) = node_cache.get(hash) {
        return Ok(node.clone());
    }

    let payload = store
        .load_raw_node(hash)
        .ok_or_else(|| format!("referenced node {} was not found in store", hex(hash)))?;
    let node = decode_persisted_node(&payload)?;
    node_cache.insert(*hash, node.clone());
    Ok(node)
}

fn resolve_node_value<T: RawStoreAdapter>(
    value: &ValueRef,
    store: &mut T,
) -> Result<Vec<u8>, String> {
    match value {
        ValueRef::Empty => Ok(Vec::new()),
        ValueRef::Inline(bytes) => Ok(bytes.clone()),
        ValueRef::Hashed { hash, .. } => store
            .load_raw_value(hash)
            .ok_or_else(|| format!("long value {} was not found in store", hex(hash))),
    }
}

fn build_root_node(entries: &BTreeMap<Vec<u8>, Vec<u8>>) -> Option<TrieNode> {
    if entries.is_empty() {
        return None;
    }

    let bit_entries: Vec<(Vec<u8>, Vec<u8>)> = entries
        .iter()
        .map(|(key, value)| {
            (
                shared_path_serializer::decode(key, key.len() * 8),
                value.clone(),
            )
        })
        .collect();

    Some(build_node(bit_entries, 0))
}

fn build_node(entries: Vec<(Vec<u8>, Vec<u8>)>, depth: usize) -> TrieNode {
    let shared_len = longest_common_suffix_length(&entries, depth);
    let node_depth = depth + shared_len;

    let shared_path_bits = entries
        .first()
        .map(|(bits, _)| bits[depth..node_depth].to_vec())
        .unwrap_or_default();

    let mut value: Option<Vec<u8>> = None;
    let mut left_entries = Vec::new();
    let mut right_entries = Vec::new();

    for (bits, node_value) in entries {
        if bits.len() == node_depth {
            value = Some(node_value);
            continue;
        }

        let next_bit = bits[node_depth];
        if next_bit == 0 {
            left_entries.push((bits, node_value));
        } else {
            right_entries.push((bits, node_value));
        }
    }

    let left_reference = if left_entries.is_empty() {
        NodeReference::empty()
    } else {
        NodeReference::embedded(build_node(left_entries, node_depth + 1))
    };

    let right_reference = if right_entries.is_empty() {
        NodeReference::empty()
    } else {
        NodeReference::embedded(build_node(right_entries, node_depth + 1))
    };

    TrieNode::new(
        SharedPath::from_bits(shared_path_bits).expect("generated path bits must be binary"),
        ValueRef::inline(value.unwrap_or_default()),
        left_reference,
        right_reference,
    )
}

fn longest_common_suffix_length(entries: &[(Vec<u8>, Vec<u8>)], depth: usize) -> usize {
    if entries.is_empty() {
        return 0;
    }

    let first = &entries[0].0;
    if depth >= first.len() {
        return 0;
    }

    let max_common_len = entries
        .iter()
        .map(|(bits, _)| bits.len().saturating_sub(depth))
        .min()
        .unwrap_or(0);

    for idx in 0..max_common_len {
        let bit = first[depth + idx];
        if entries.iter().any(|(bits, _)| bits[depth + idx] != bit) {
            return idx;
        }
    }

    max_common_len
}

fn compute_node_metadata(node: &TrieNode) -> Result<NodeMetadata, String> {
    let (left_encoding, left_size) = compute_child_encoding(&node.left)?;
    let (right_encoding, right_size) = compute_child_encoding(&node.right)?;

    let children_size = if node.is_terminal() {
        None
    } else {
        Some(left_size + right_size)
    };

    let serialized =
        Rskip107Codec::encode_node(node, &left_encoding, &right_encoding, children_size)?;
    let hash = keccak256(&serialized);
    let external_value_size = if node.has_long_value() {
        node.value_length() as u64
    } else {
        0
    };

    let reference_size = children_size.unwrap_or(0) + external_value_size + serialized.len() as u64;

    Ok(NodeMetadata {
        hash,
        serialized: serialized.clone(),
        reference_size,
        embeddable: node.is_terminal() && serialized.len() <= MAX_EMBEDDED_NODE_SIZE_IN_BYTES,
    })
}

fn compute_child_encoding(reference: &NodeReference) -> Result<(ChildEncoding, u64), String> {
    match reference {
        NodeReference::Empty => Ok((ChildEncoding::Empty, 0)),
        NodeReference::Embedded(child) => {
            let metadata = compute_node_metadata(child)?;
            if metadata.embeddable {
                Ok((
                    ChildEncoding::Embedded(metadata.serialized),
                    metadata.reference_size,
                ))
            } else {
                Ok((
                    ChildEncoding::Hashed(metadata.hash),
                    metadata.reference_size,
                ))
            }
        }
        NodeReference::Hashed(_) => {
            Err("cannot compute node metadata with unresolved hashed node reference".to_string())
        }
    }
}

fn persist_node_recursive<T: RawStoreAdapter>(
    node: &TrieNode,
    store: &mut T,
    persisted_node_hashes: &mut HashSet<[u8; HASH_SIZE]>,
    persisted_value_hashes: &mut HashSet<[u8; HASH_SIZE]>,
    is_root: bool,
) -> Result<(NodeMetadata, SaveStats), String> {
    let (left_encoding, left_size, left_stats) = persist_child_reference(
        &node.left,
        store,
        persisted_node_hashes,
        persisted_value_hashes,
    )?;
    let (right_encoding, right_size, right_stats) = persist_child_reference(
        &node.right,
        store,
        persisted_node_hashes,
        persisted_value_hashes,
    )?;

    let children_size = if node.is_terminal() {
        None
    } else {
        Some(left_size + right_size)
    };
    let serialized =
        Rskip107Codec::encode_node(node, &left_encoding, &right_encoding, children_size)?;
    let hash = keccak256(&serialized);
    let mut save_stats = SaveStats {
        nodes_visited: 1 + left_stats.nodes_visited + right_stats.nodes_visited,
        nodes_written: left_stats.nodes_written + right_stats.nodes_written,
        values_written: left_stats.values_written + right_stats.values_written,
    };

    if let Some(inline_value) = node.value.inline_bytes() {
        if inline_value.len() > LONG_VALUE_THRESHOLD {
            let value_hash = keccak256(inline_value);
            if persisted_value_hashes.insert(value_hash) {
                store.save_raw_value(&value_hash, inline_value);
                save_stats.values_written = save_stats.values_written.saturating_add(1);
            }
        }
    }

    let embeddable = node.is_terminal() && serialized.len() <= MAX_EMBEDDED_NODE_SIZE_IN_BYTES;
    if is_root || !embeddable {
        let should_write = if is_root {
            true
        } else {
            persisted_node_hashes.insert(hash)
        };

        if should_write {
            store.save_raw_node(&hash, &serialized);
            save_stats.nodes_written = save_stats.nodes_written.saturating_add(1);
        }

        if is_root {
            persisted_node_hashes.insert(hash);
        }
    }

    let external_value_size = if node.has_long_value() {
        node.value_length() as u64
    } else {
        0
    };
    let reference_size = children_size.unwrap_or(0) + external_value_size + serialized.len() as u64;

    Ok((
        NodeMetadata {
            hash,
            serialized,
            reference_size,
            embeddable,
        },
        save_stats,
    ))
}

fn persist_child_reference<T: RawStoreAdapter>(
    reference: &NodeReference,
    store: &mut T,
    persisted_node_hashes: &mut HashSet<[u8; HASH_SIZE]>,
    persisted_value_hashes: &mut HashSet<[u8; HASH_SIZE]>,
) -> Result<(ChildEncoding, u64, SaveStats), String> {
    match reference {
        NodeReference::Empty => Ok((ChildEncoding::Empty, 0, SaveStats::default())),
        NodeReference::Embedded(child) => {
            let (child_metadata, child_stats) = persist_node_recursive(
                child,
                store,
                persisted_node_hashes,
                persisted_value_hashes,
                false,
            )?;
            if child_metadata.embeddable {
                Ok((
                    ChildEncoding::Embedded(child_metadata.serialized),
                    child_metadata.reference_size,
                    child_stats,
                ))
            } else {
                Ok((
                    ChildEncoding::Hashed(child_metadata.hash),
                    child_metadata.reference_size,
                    child_stats,
                ))
            }
        }
        NodeReference::Hashed(hash) => {
            persisted_node_hashes.insert(*hash);
            Ok((ChildEncoding::Hashed(*hash), 0, SaveStats::default()))
        }
    }
}

fn account_storage_prefix_key(account_address: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(
        DOMAIN_PREFIX.len() + SECURE_KEY_SIZE + account_address.len() + STORAGE_PREFIX.len(),
    );
    key.extend_from_slice(&DOMAIN_PREFIX);
    key.extend_from_slice(&secure_key_prefix(account_address));
    key.extend_from_slice(account_address);
    key.extend_from_slice(&STORAGE_PREFIX);
    key
}

fn secure_key_prefix(key: &[u8]) -> [u8; SECURE_KEY_SIZE] {
    let hash = keccak256(key);
    let mut prefix = [0u8; SECURE_KEY_SIZE];
    prefix.copy_from_slice(&hash[..SECURE_KEY_SIZE]);
    prefix
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

fn prefix_upper_bound(prefix: &[u8]) -> Option<Vec<u8>> {
    if prefix.is_empty() {
        return None;
    }

    let mut upper = prefix.to_vec();
    for index in (0..upper.len()).rev() {
        if upper[index] != u8::MAX {
            upper[index] = upper[index].saturating_add(1);
            upper.truncate(index + 1);
            return Some(upper);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::Unitrie;
    use crate::hash::empty_trie_hash;
    use crate::store_adapter::RawStoreAdapter;
    use std::collections::HashMap;

    #[derive(Default)]
    struct InMemoryStore {
        nodes: HashMap<Vec<u8>, Vec<u8>>,
        values: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl RawStoreAdapter for InMemoryStore {
        fn load_raw_node(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
            self.nodes.get(hash).cloned()
        }

        fn load_raw_value(&mut self, hash: &[u8]) -> Option<Vec<u8>> {
            self.values.get(hash).cloned()
        }

        fn save_raw_node(&mut self, hash: &[u8], serialized_node: &[u8]) {
            self.nodes.insert(hash.to_vec(), serialized_node.to_vec());
        }

        fn save_raw_value(&mut self, hash: &[u8], value: &[u8]) {
            self.values.insert(hash.to_vec(), value.to_vec());
        }
    }

    #[test]
    fn get_put_delete_round_trip() {
        let mut trie = Unitrie::new();
        trie.put(b"hello".to_vec(), b"world".to_vec());
        assert_eq!(trie.get(b"hello").as_deref(), Some(b"world".as_slice()));

        trie.delete(b"hello");
        assert!(trie.get(b"hello").is_none());
    }

    #[test]
    fn delete_recursive_removes_prefixed_keys_only() {
        let mut trie = Unitrie::new();
        trie.put(b"acct:1:aa".to_vec(), b"v1".to_vec());
        trie.put(b"acct:1:bb".to_vec(), b"v2".to_vec());
        trie.put(b"acct:2:aa".to_vec(), b"v3".to_vec());

        trie.delete_recursive(b"acct:1:");

        assert!(trie.get(b"acct:1:aa").is_none());
        assert!(trie.get(b"acct:1:bb").is_none());
        assert_eq!(trie.get(b"acct:2:aa").as_deref(), Some(b"v3".as_slice()));
    }

    #[test]
    fn delete_recursive_handles_boundary_prefixes() {
        let mut trie = Unitrie::new();
        trie.put(vec![0xff, 0x10], vec![0x01]);
        trie.put(vec![0xff, 0x20], vec![0x02]);
        trie.put(vec![0xfe, 0x01], vec![0x03]);

        trie.delete_recursive(&[0xff]);

        assert!(trie.get(&[0xff, 0x10]).is_none());
        assert!(trie.get(&[0xff, 0x20]).is_none());
        assert_eq!(trie.get(&[0xfe, 0x01]).as_deref(), Some([0x03].as_slice()));
    }

    #[test]
    fn delete_recursive_range_matches_naive_behavior() {
        let keys = vec![
            vec![],
            vec![0x00],
            vec![0x00, 0x01],
            vec![0x00, 0xff],
            vec![0x01],
            vec![0x01, 0x00],
            vec![0x01, 0x80],
            vec![0x10, 0x20, 0x30],
            vec![0xff],
            vec![0xff, 0x00],
            vec![0xff, 0xff],
        ];

        let prefixes = vec![
            vec![],
            vec![0x00],
            vec![0x00, 0x01],
            vec![0x01],
            vec![0x01, 0x80],
            vec![0x02],
            vec![0xff],
            vec![0xff, 0xff],
            vec![0xff, 0xff, 0xff],
        ];

        for prefix in prefixes {
            let mut trie = Unitrie::new();
            let mut naive = HashMap::<Vec<u8>, Vec<u8>>::new();

            for key in &keys {
                let value = vec![key.len() as u8];
                trie.put(key.clone(), value.clone());
                naive.insert(key.clone(), value);
            }

            trie.delete_recursive(&prefix);
            naive.retain(|key, _| !key.starts_with(&prefix));

            let mut actual = HashMap::<Vec<u8>, Vec<u8>>::new();
            for key in &keys {
                if let Some(value) = trie.get(key) {
                    actual.insert(key.clone(), value);
                }
            }

            assert_eq!(actual, naive, "mismatch for prefix {:?}", prefix);
        }
    }

    #[test]
    fn empty_root_hash_matches_expected_semantics() {
        let mut trie = Unitrie::new();
        assert_eq!(trie.root_hash(), empty_trie_hash());
    }

    #[test]
    fn root_hash_is_stable_for_same_content() {
        let mut first = Unitrie::new();
        first.put(b"k1".to_vec(), b"v1".to_vec());
        first.put(b"k2".to_vec(), b"v2".to_vec());

        let mut second = Unitrie::new();
        second.put(b"k2".to_vec(), b"v2".to_vec());
        second.put(b"k1".to_vec(), b"v1".to_vec());

        assert_eq!(first.root_hash(), second.root_hash());
    }

    #[test]
    fn collect_keys_matches_java_contract_by_key_length() {
        let mut trie = Unitrie::new();
        trie.put(vec![0x01], vec![0xaa]);
        trie.put(vec![0x02], vec![0xbb]);
        trie.put(vec![0x03, 0x04], vec![0xcc]);

        let single_byte_keys = trie.collect_keys(1);
        assert_eq!(single_byte_keys.len(), 2);
        let all_keys = trie.collect_keys(i32::MAX as usize);
        assert_eq!(all_keys.len(), 3);
    }

    #[test]
    fn save_and_load_from_persisted_root_round_trip() {
        let mut trie = Unitrie::new();
        trie.put(vec![0xaa], vec![0x01, 0x02, 0x03]);
        trie.put(vec![0xab], vec![0x09; 40]);

        let root_hash = trie.root_hash();
        let mut store = InMemoryStore::default();
        trie.save_to_store(&mut store);

        let mut loaded = Unitrie::from_persisted_root(&root_hash, &mut store).unwrap();
        assert_eq!(
            loaded.get(&[0xaa]).as_deref(),
            Some([0x01, 0x02, 0x03].as_ref())
        );
        assert_eq!(
            loaded.get(&[0xab]).as_deref(),
            Some(vec![0x09; 40].as_slice())
        );
        assert_eq!(loaded.root_hash(), root_hash);
    }
}
