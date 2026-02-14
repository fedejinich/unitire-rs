use crate::node_ref::{NodeReference, SharedPath, TrieNode, ValueRef, HASH_SIZE};
use crate::path::shared_path_serializer;

const ARITY: u8 = 2;
const MESSAGE_HEADER_LENGTH: usize = 6;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct OrchidCodec;

impl OrchidCodec {
    pub fn is_orchid_message(payload: &[u8]) -> bool {
        payload.first().copied() == Some(ARITY)
    }

    pub fn decode_node(payload: &[u8]) -> Result<TrieNode, String> {
        if payload.len() < MESSAGE_HEADER_LENGTH {
            return Err("orchid payload is too short".to_string());
        }

        let mut offset = 0usize;
        let arity = payload[offset];
        offset += 1;
        if arity != ARITY {
            return Err("invalid orchid arity".to_string());
        }

        let flags = payload[offset];
        offset += 1;
        let has_long_value = (flags & 0x02) == 0x02;

        let bhashes = read_u16(payload, &mut offset)?;
        let shared_path_bits_length = read_u16(payload, &mut offset)? as usize;
        let encoded_shared_path_length =
            shared_path_serializer::calculate_encoded_length(shared_path_bits_length);

        let shared_path_bits = if encoded_shared_path_length > 0 {
            let end = offset + encoded_shared_path_length;
            if end > payload.len() {
                return Err("orchid payload shared path is truncated".to_string());
            }
            let encoded = &payload[offset..end];
            offset = end;
            shared_path_serializer::decode(encoded, shared_path_bits_length)
        } else {
            Vec::new()
        };
        let shared_path = SharedPath::from_bits(shared_path_bits)?;

        let left = if (bhashes & 0b01) != 0 {
            NodeReference::hashed(read_hash(payload, &mut offset)?)
        } else {
            NodeReference::empty()
        };

        let right = if (bhashes & 0b10) != 0 {
            NodeReference::hashed(read_hash(payload, &mut offset)?)
        } else {
            NodeReference::empty()
        };

        let value = if has_long_value {
            ValueRef::hashed(read_hash(payload, &mut offset)?, None)
        } else if offset < payload.len() {
            ValueRef::inline(payload[offset..].to_vec())
        } else {
            ValueRef::empty()
        };

        Ok(TrieNode::new(shared_path, value, left, right))
    }

    pub fn encode_node(
        node: &TrieNode,
        left_hash: Option<[u8; HASH_SIZE]>,
        right_hash: Option<[u8; HASH_SIZE]>,
        secure: bool,
    ) -> Result<Vec<u8>, String> {
        let has_long_value = node.has_long_value();

        let mut flags = 0u8;
        if secure {
            flags |= 0x01;
        }
        if has_long_value {
            flags |= 0x02;
        }

        let mut bhashes: u16 = 0;
        if left_hash.is_some() {
            bhashes |= 0b01;
        }
        if right_hash.is_some() {
            bhashes |= 0b10;
        }

        let shared_path_len = node.shared_path.len();
        if shared_path_len > u16::MAX as usize {
            return Err("orchid shared path length does not fit in uint16".to_string());
        }

        let mut encoded = Vec::with_capacity(
            MESSAGE_HEADER_LENGTH
                + shared_path_serializer::calculate_encoded_length(shared_path_len)
                + if left_hash.is_some() { HASH_SIZE } else { 0 }
                + if right_hash.is_some() { HASH_SIZE } else { 0 }
                + if has_long_value {
                    HASH_SIZE
                } else {
                    node.value.len().unwrap_or(0)
                },
        );

        encoded.push(ARITY);
        encoded.push(flags);
        encoded.extend_from_slice(&bhashes.to_be_bytes());
        encoded.extend_from_slice(&(shared_path_len as u16).to_be_bytes());
        if shared_path_len > 0 {
            encoded.extend_from_slice(&node.shared_path.encoded());
        }

        if let Some(hash) = left_hash {
            encoded.extend_from_slice(&hash);
        }
        if let Some(hash) = right_hash {
            encoded.extend_from_slice(&hash);
        }

        if has_long_value {
            let hash = node
                .value
                .hash()
                .ok_or_else(|| "long orchid value is missing hash".to_string())?;
            encoded.extend_from_slice(&hash);
        } else if let Some(inline) = node.value.inline_bytes() {
            encoded.extend_from_slice(inline);
        }

        Ok(encoded)
    }
}

fn read_hash(payload: &[u8], offset: &mut usize) -> Result<[u8; HASH_SIZE], String> {
    let end = *offset + HASH_SIZE;
    if end > payload.len() {
        return Err("orchid hash field is truncated".to_string());
    }

    let mut hash = [0u8; HASH_SIZE];
    hash.copy_from_slice(&payload[*offset..end]);
    *offset = end;
    Ok(hash)
}

fn read_u16(payload: &[u8], offset: &mut usize) -> Result<u16, String> {
    let end = *offset + 2;
    if end > payload.len() {
        return Err("orchid uint16 field is truncated".to_string());
    }

    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&payload[*offset..end]);
    *offset = end;
    Ok(u16::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::OrchidCodec;
    use crate::node_ref::{NodeReference, SharedPath, TrieNode, ValueRef};

    #[test]
    fn decode_rejects_wrong_arity() {
        let payload = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(OrchidCodec::decode_node(&payload).is_err());
    }

    #[test]
    fn encode_decode_round_trip() {
        let node = TrieNode::new(
            SharedPath::from_bits(vec![1, 0, 1, 0]).unwrap(),
            ValueRef::inline(vec![1, 2, 3, 4]),
            NodeReference::empty(),
            NodeReference::empty(),
        );
        let encoded = OrchidCodec::encode_node(&node, None, None, false).unwrap();
        let decoded = OrchidCodec::decode_node(&encoded).unwrap();
        assert_eq!(decoded, node);
    }

    #[test]
    fn long_value_flag_is_respected() {
        let node = TrieNode::new(
            SharedPath::empty(),
            ValueRef::inline(vec![9u8; 40]),
            NodeReference::empty(),
            NodeReference::empty(),
        );
        let encoded = OrchidCodec::encode_node(&node, None, None, true).unwrap();
        assert_eq!(encoded[0], 2);
        assert_eq!(encoded[1] & 0x02, 0x02);
        assert_eq!(encoded[1] & 0x01, 0x01);
    }
}
