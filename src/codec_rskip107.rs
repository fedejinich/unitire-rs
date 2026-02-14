use crate::node_ref::{NodeReference, SharedPath, TrieNode, ValueRef, HASH_SIZE};
use crate::path::shared_path_serializer;
use crate::varint;

const VERSION_FLAG: u8 = 0b0100_0000;
const VERSION_MASK: u8 = 0b1100_0000;
const LONG_VALUE_FLAG: u8 = 0b0010_0000;
const SHARED_PREFIX_FLAG: u8 = 0b0001_0000;
const LEFT_PRESENT_FLAG: u8 = 0b0000_1000;
const RIGHT_PRESENT_FLAG: u8 = 0b0000_0100;
const LEFT_EMBEDDED_FLAG: u8 = 0b0000_0010;
const RIGHT_EMBEDDED_FLAG: u8 = 0b0000_0001;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ChildEncoding {
    Empty,
    Embedded(Vec<u8>),
    Hashed([u8; HASH_SIZE]),
}

impl ChildEncoding {
    pub fn is_present(&self) -> bool {
        !matches!(self, ChildEncoding::Empty)
    }

    pub fn is_embedded(&self) -> bool {
        matches!(self, ChildEncoding::Embedded(_))
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct Rskip107Codec;

impl Rskip107Codec {
    pub fn is_rskip107_message(payload: &[u8]) -> bool {
        !payload.is_empty() && (payload[0] & VERSION_MASK) == VERSION_FLAG
    }

    pub fn decode_node(payload: &[u8]) -> Result<TrieNode, String> {
        if payload.is_empty() {
            return Err("RSKIP107 node payload is empty".to_string());
        }

        let flags = payload[0];
        let has_long_value = (flags & LONG_VALUE_FLAG) == LONG_VALUE_FLAG;
        let shared_prefix_present = (flags & SHARED_PREFIX_FLAG) == SHARED_PREFIX_FLAG;
        let left_present = (flags & LEFT_PRESENT_FLAG) == LEFT_PRESENT_FLAG;
        let right_present = (flags & RIGHT_PRESENT_FLAG) == RIGHT_PRESENT_FLAG;
        let left_embedded = (flags & LEFT_EMBEDDED_FLAG) == LEFT_EMBEDDED_FLAG;
        let right_embedded = (flags & RIGHT_EMBEDDED_FLAG) == RIGHT_EMBEDDED_FLAG;

        let mut offset = 1usize;
        let shared_bits = shared_path_serializer::deserialize_from_slice(
            payload,
            &mut offset,
            shared_prefix_present,
        )?;
        let shared_path = SharedPath::from_bits(shared_bits)?;

        let left = if left_present {
            Self::decode_reference(payload, &mut offset, left_embedded)?
        } else {
            NodeReference::Empty
        };

        let right = if right_present {
            Self::decode_reference(payload, &mut offset, right_embedded)?
        } else {
            NodeReference::Empty
        };

        if left_present || right_present {
            let _children_size = varint::decode_from_slice(payload, &mut offset)?;
        }

        let value = if has_long_value {
            let hash = read_hash(payload, &mut offset)?;
            let value_length = read_u24(payload, &mut offset)?;
            ValueRef::hashed(hash, Some(value_length))
        } else if offset < payload.len() {
            let inline = payload[offset..].to_vec();
            offset = payload.len();
            ValueRef::inline(inline)
        } else {
            ValueRef::empty()
        };

        if offset != payload.len() {
            return Err("RSKIP107 node payload has trailing data".to_string());
        }

        Ok(TrieNode::new(shared_path, value, left, right))
    }

    pub fn encode_node(
        node: &TrieNode,
        left: &ChildEncoding,
        right: &ChildEncoding,
        children_size: Option<u64>,
    ) -> Result<Vec<u8>, String> {
        let has_long_value = node.has_long_value();
        let left_present = left.is_present();
        let right_present = right.is_present();

        if (left_present || right_present) && children_size.is_none() {
            return Err("childrenSize is required for non-terminal node".to_string());
        }

        let mut flags = VERSION_FLAG;
        if has_long_value {
            flags |= LONG_VALUE_FLAG;
        }
        if !node.shared_path.is_empty() {
            flags |= SHARED_PREFIX_FLAG;
        }
        if left_present {
            flags |= LEFT_PRESENT_FLAG;
        }
        if right_present {
            flags |= RIGHT_PRESENT_FLAG;
        }
        if left.is_embedded() {
            flags |= LEFT_EMBEDDED_FLAG;
        }
        if right.is_embedded() {
            flags |= RIGHT_EMBEDDED_FLAG;
        }

        let mut encoded = Vec::new();
        encoded.push(flags);
        shared_path_serializer::serialize_into(node.shared_path.as_bits(), &mut encoded);

        Self::encode_reference(left, &mut encoded)?;
        Self::encode_reference(right, &mut encoded)?;

        if left_present || right_present {
            encoded.extend_from_slice(&varint::encode(children_size.unwrap_or(0)));
        }

        if has_long_value {
            let hash = node
                .value
                .hash()
                .ok_or_else(|| "long value node missing value hash".to_string())?;
            let length = node
                .value
                .len()
                .ok_or_else(|| "long value node missing value length".to_string())?;
            encoded.extend_from_slice(&hash);
            encoded.extend_from_slice(&encode_u24(length)?);
        } else if let Some(inline) = node.value.inline_bytes() {
            encoded.extend_from_slice(inline);
        }

        Ok(encoded)
    }

    fn decode_reference(
        payload: &[u8],
        offset: &mut usize,
        embedded: bool,
    ) -> Result<NodeReference, String> {
        if embedded {
            if *offset >= payload.len() {
                return Err("embedded node length is truncated".to_string());
            }
            let length = payload[*offset] as usize;
            *offset += 1;

            let end = *offset + length;
            if end > payload.len() {
                return Err("embedded node payload is truncated".to_string());
            }

            let node_payload = &payload[*offset..end];
            *offset = end;
            let embedded_node = Self::decode_node(node_payload)?;
            Ok(NodeReference::embedded(embedded_node))
        } else {
            let hash = read_hash(payload, offset)?;
            Ok(NodeReference::hashed(hash))
        }
    }

    fn encode_reference(reference: &ChildEncoding, output: &mut Vec<u8>) -> Result<(), String> {
        match reference {
            ChildEncoding::Empty => Ok(()),
            ChildEncoding::Embedded(serialized_node) => {
                if serialized_node.len() > u8::MAX as usize {
                    return Err(format!(
                        "embedded node length {} does not fit in uint8",
                        serialized_node.len()
                    ));
                }
                output.push(serialized_node.len() as u8);
                output.extend_from_slice(serialized_node);
                Ok(())
            }
            ChildEncoding::Hashed(hash) => {
                output.extend_from_slice(hash);
                Ok(())
            }
        }
    }
}

fn read_hash(payload: &[u8], offset: &mut usize) -> Result<[u8; HASH_SIZE], String> {
    let end = *offset + HASH_SIZE;
    if end > payload.len() {
        return Err("hash payload is truncated".to_string());
    }

    let mut hash = [0u8; HASH_SIZE];
    hash.copy_from_slice(&payload[*offset..end]);
    *offset = end;
    Ok(hash)
}

fn read_u24(payload: &[u8], offset: &mut usize) -> Result<usize, String> {
    let end = *offset + 3;
    if end > payload.len() {
        return Err("uint24 payload is truncated".to_string());
    }

    let value = ((payload[*offset] as usize) << 16)
        | ((payload[*offset + 1] as usize) << 8)
        | payload[*offset + 2] as usize;
    *offset = end;
    Ok(value)
}

fn encode_u24(value: usize) -> Result<[u8; 3], String> {
    if value > 0x00ff_ffff {
        return Err("value does not fit in uint24".to_string());
    }

    Ok([
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::{ChildEncoding, Rskip107Codec, LONG_VALUE_FLAG, VERSION_FLAG};
    use crate::node_ref::{NodeReference, SharedPath, TrieNode, ValueRef};

    #[test]
    fn decode_rejects_empty_payload() {
        assert!(Rskip107Codec::decode_node(&[]).is_err());
    }

    #[test]
    fn encode_decode_round_trip_terminal_short_value() {
        let node = TrieNode::new(
            SharedPath::from_bits(vec![1, 0, 1]).unwrap(),
            ValueRef::inline(vec![1, 2, 3, 4]),
            NodeReference::empty(),
            NodeReference::empty(),
        );

        let encoded =
            Rskip107Codec::encode_node(&node, &ChildEncoding::Empty, &ChildEncoding::Empty, None)
                .unwrap();
        let decoded = Rskip107Codec::decode_node(&encoded).unwrap();
        assert_eq!(decoded, node);
    }

    #[test]
    fn encode_sets_version_and_long_value_bits() {
        let node = TrieNode::new(
            SharedPath::empty(),
            ValueRef::inline(vec![7u8; 40]),
            NodeReference::empty(),
            NodeReference::empty(),
        );

        let encoded =
            Rskip107Codec::encode_node(&node, &ChildEncoding::Empty, &ChildEncoding::Empty, None)
                .unwrap();
        assert_eq!(encoded[0] & VERSION_FLAG, VERSION_FLAG);
        assert_eq!(encoded[0] & LONG_VALUE_FLAG, LONG_VALUE_FLAG);
    }

    #[test]
    fn encode_requires_children_size_when_children_are_present() {
        let node = TrieNode::new(
            SharedPath::empty(),
            ValueRef::empty(),
            NodeReference::empty(),
            NodeReference::empty(),
        );

        let result = Rskip107Codec::encode_node(
            &node,
            &ChildEncoding::Hashed([3u8; 32]),
            &ChildEncoding::Empty,
            None,
        );
        assert!(result.is_err());
    }
}
