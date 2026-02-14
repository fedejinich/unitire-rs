pub fn size_of(value: u64) -> usize {
    if value < 0xfd {
        1
    } else if value <= 0xffff {
        3
    } else if value <= 0xffff_ffff {
        5
    } else {
        9
    }
}

pub fn encode(value: u64) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(size_of(value));
    encode_into(value, &mut encoded);
    encoded
}

pub fn encode_into(value: u64, encoded: &mut Vec<u8>) {
    if value < 0xfd {
        encoded.push(value as u8);
    } else if value <= 0xffff {
        encoded.push(0xfd);
        encoded.extend_from_slice(&(value as u16).to_le_bytes());
    } else if value <= 0xffff_ffff {
        encoded.push(0xfe);
        encoded.extend_from_slice(&(value as u32).to_le_bytes());
    } else {
        encoded.push(0xff);
        encoded.extend_from_slice(&value.to_le_bytes());
    }
}

pub fn decode_from_slice(input: &[u8], offset: &mut usize) -> Result<u64, String> {
    if *offset >= input.len() {
        return Err("varint is truncated".to_string());
    }

    let first = input[*offset];
    *offset += 1;

    match first {
        0xfd => decode_u16(input, offset).map(u64::from),
        0xfe => decode_u32(input, offset).map(u64::from),
        0xff => decode_u64(input, offset),
        value => Ok(u64::from(value)),
    }
}

fn decode_u16(input: &[u8], offset: &mut usize) -> Result<u16, String> {
    let end = *offset + 2;
    if end > input.len() {
        return Err("varint u16 is truncated".to_string());
    }

    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&input[*offset..end]);
    *offset = end;
    Ok(u16::from_le_bytes(bytes))
}

fn decode_u32(input: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = *offset + 4;
    if end > input.len() {
        return Err("varint u32 is truncated".to_string());
    }

    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&input[*offset..end]);
    *offset = end;
    Ok(u32::from_le_bytes(bytes))
}

fn decode_u64(input: &[u8], offset: &mut usize) -> Result<u64, String> {
    let end = *offset + 8;
    if end > input.len() {
        return Err("varint u64 is truncated".to_string());
    }

    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[*offset..end]);
    *offset = end;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::{decode_from_slice, encode, encode_into, size_of};

    #[test]
    fn size_matches_encoding_boundaries() {
        assert_eq!(size_of(252), 1);
        assert_eq!(size_of(253), 3);
        assert_eq!(size_of(65_535), 3);
        assert_eq!(size_of(65_536), 5);
    }

    #[test]
    fn round_trip() {
        let values = [0, 1, 252, 253, 65_535, 65_536, u32::MAX as u64 + 1];
        for value in values {
            let encoded = encode(value);
            let mut offset = 0usize;
            let decoded = decode_from_slice(&encoded, &mut offset).expect("varint should decode");
            assert_eq!(decoded, value);
            assert_eq!(offset, encoded.len());
        }
    }

    #[test]
    fn encode_into_matches_encode() {
        let values = [0, 1, 252, 253, 65_535, 65_536, u32::MAX as u64 + 1];
        for value in values {
            let direct = encode(value);
            let mut reused = Vec::new();
            encode_into(value, &mut reused);
            assert_eq!(direct, reused);
        }
    }
}
