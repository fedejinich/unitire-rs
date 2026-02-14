#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PathBits {
    bytes: Vec<u8>,
    bit_len: usize,
}

impl PathBits {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_key_bytes(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
            bit_len: bytes.len().saturating_mul(8),
        }
    }

    pub fn from_bits(bits: &[u8]) -> Result<Self, String> {
        if bits.iter().any(|bit| *bit > 1) {
            return Err("path bits must contain only 0 or 1".to_string());
        }

        if bits.is_empty() {
            return Ok(Self::empty());
        }

        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (index, bit) in bits.iter().enumerate() {
            if *bit == 1 {
                let byte_index = index / 8;
                let bit_index = 7 - (index % 8);
                bytes[byte_index] |= 1u8 << bit_index;
            }
        }

        Ok(Self {
            bytes,
            bit_len: bits.len(),
        })
    }

    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    pub fn is_empty(&self) -> bool {
        self.bit_len == 0
    }

    pub fn get_bit(&self, bit_index: usize) -> Option<u8> {
        if bit_index >= self.bit_len {
            return None;
        }

        let byte_index = bit_index / 8;
        let shift = 7 - (bit_index % 8);
        Some((self.bytes[byte_index] >> shift) & 1)
    }

    pub fn to_bits_vec(&self) -> Vec<u8> {
        (0..self.bit_len)
            .map(|index| self.get_bit(index).unwrap_or(0))
            .collect()
    }

    pub fn to_packed_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::PathBits;

    #[test]
    fn path_bits_round_trip_from_bits() {
        let bits = vec![1, 0, 1, 1, 0, 1, 0, 0, 1];
        let path = PathBits::from_bits(&bits).expect("valid bits");
        assert_eq!(path.bit_len(), bits.len());
        assert_eq!(path.to_bits_vec(), bits);
    }

    #[test]
    fn path_bits_from_key_bytes_reads_msb_first() {
        let path = PathBits::from_key_bytes(&[0b1010_0000]);
        assert_eq!(path.bit_len(), 8);
        assert_eq!(path.to_bits_vec(), vec![1, 0, 1, 0, 0, 0, 0, 0]);
    }
}
