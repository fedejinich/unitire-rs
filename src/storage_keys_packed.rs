use crate::varint;

pub fn encode(values: &[Vec<u8>]) -> Vec<u8> {
    let mut payload_size = varint::size_of(values.len() as u64);
    for value in values {
        payload_size += varint::size_of(value.len() as u64) + value.len();
    }

    let mut output = Vec::with_capacity(payload_size);
    varint::encode_into(values.len() as u64, &mut output);
    for value in values {
        varint::encode_into(value.len() as u64, &mut output);
        output.extend_from_slice(value);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::encode;
    use crate::varint::decode_from_slice;

    #[test]
    fn round_trip() {
        let values = vec![vec![0x01], vec![0xaa, 0xbb], vec![0x10; 260]];
        let encoded = encode(&values);

        let mut offset = 0usize;
        let count = decode_from_slice(&encoded, &mut offset).expect("count varint");
        assert_eq!(count as usize, values.len());

        let mut decoded = Vec::new();
        for _ in 0..count {
            let len = decode_from_slice(&encoded, &mut offset).expect("len varint") as usize;
            let end = offset + len;
            decoded.push(encoded[offset..end].to_vec());
            offset = end;
        }

        assert_eq!(decoded, values);
        assert_eq!(offset, encoded.len());
    }
}
