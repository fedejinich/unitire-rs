use crate::varint;

pub fn calculate_encoded_length(key_length: usize) -> usize {
    key_length / 8 + usize::from(!key_length.is_multiple_of(8))
}

pub fn encode(path: &[u8]) -> Vec<u8> {
    let mut encoded = vec![0u8; calculate_encoded_length(path.len())];
    for (idx, bit) in path.iter().enumerate() {
        if *bit == 0 {
            continue;
        }
        let byte_index = idx / 8;
        let offset = idx % 8;
        encoded[byte_index] |= 0x80 >> offset;
    }
    encoded
}

pub fn decode(encoded: &[u8], bit_length: usize) -> Vec<u8> {
    let mut path = vec![0u8; bit_length];
    for (idx, bit) in path.iter_mut().enumerate().take(bit_length) {
        let byte_index = idx / 8;
        let offset = idx % 8;
        if ((encoded[byte_index] >> (7 - offset)) & 0x01) != 0 {
            *bit = 1;
        }
    }
    path
}

pub fn calculate_varint_size(bit_length: usize) -> usize {
    if (1..=32).contains(&bit_length) || (160..=382).contains(&bit_length) {
        return 1;
    }

    1 + varint::size_of(bit_length as u64)
}

pub fn serialized_length(shared_path_bits: &[u8]) -> usize {
    if shared_path_bits.is_empty() {
        return 0;
    }

    calculate_varint_size(shared_path_bits.len()) + calculate_encoded_length(shared_path_bits.len())
}

pub fn serialize_into(shared_path_bits: &[u8], output: &mut Vec<u8>) {
    if shared_path_bits.is_empty() {
        return;
    }

    let bit_length = shared_path_bits.len();
    if (1..=32).contains(&bit_length) {
        output.push((bit_length - 1) as u8);
    } else if (160..=382).contains(&bit_length) {
        output.push((bit_length - 128) as u8);
    } else {
        output.push(0xff);
        output.extend_from_slice(&varint::encode(bit_length as u64));
    }

    output.extend_from_slice(&encode(shared_path_bits));
}

pub fn deserialize_from_slice(
    input: &[u8],
    offset: &mut usize,
    shared_prefix_present: bool,
) -> Result<Vec<u8>, String> {
    if !shared_prefix_present {
        return Ok(Vec::new());
    }

    let bit_length = read_path_bit_length(input, offset)?;
    let encoded_length = calculate_encoded_length(bit_length);
    let end = *offset + encoded_length;
    if end > input.len() {
        return Err("shared path encoded bytes are truncated".to_string());
    }

    let encoded = &input[*offset..end];
    *offset = end;
    Ok(decode(encoded, bit_length))
}

pub fn read_path_bit_length(input: &[u8], offset: &mut usize) -> Result<usize, String> {
    if *offset >= input.len() {
        return Err("shared path prefix is truncated".to_string());
    }

    let first = input[*offset];
    *offset += 1;

    let length = if first <= 31 {
        usize::from(first) + 1
    } else if first <= 254 {
        usize::from(first) + 128
    } else {
        varint::decode_from_slice(input, offset)? as usize
    };

    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_varint_size, decode, deserialize_from_slice, encode, read_path_bit_length,
        serialize_into, serialized_length,
    };

    #[test]
    fn bit_pack_round_trip() {
        let path = vec![1, 0, 1, 1, 0, 0, 1, 0, 1];
        let encoded = encode(&path);
        let decoded = decode(&encoded, path.len());
        assert_eq!(decoded, path);
    }

    #[test]
    fn serializes_compact_header_for_short_paths() {
        let path = vec![1; 8];
        let mut output = Vec::new();
        serialize_into(&path, &mut output);
        assert_eq!(output[0], 7);
    }

    #[test]
    fn serializes_varint_header_for_mid_range_gap() {
        let path = vec![1; 120];
        let mut output = Vec::new();
        serialize_into(&path, &mut output);
        assert_eq!(output[0], 0xff);
        assert_eq!(read_path_bit_length(&output, &mut 0usize).unwrap(), 120);
    }

    #[test]
    fn serialized_length_matches_payload() {
        let path = vec![0, 1, 1, 0, 1, 0, 0, 1, 1, 1];
        let mut output = Vec::new();
        serialize_into(&path, &mut output);
        assert_eq!(serialized_length(&path), output.len());
    }

    #[test]
    fn deserialize_returns_empty_when_not_present() {
        let mut offset = 0usize;
        let decoded = deserialize_from_slice(&[], &mut offset, false).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn calculates_varint_size_compatible_ranges() {
        assert_eq!(calculate_varint_size(1), 1);
        assert_eq!(calculate_varint_size(32), 1);
        assert_eq!(calculate_varint_size(160), 1);
        assert_eq!(calculate_varint_size(382), 1);
        assert!(calculate_varint_size(120) > 1);
    }
}
