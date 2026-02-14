pub fn collect_exact_size_keys<'a>(
    keys: impl Iterator<Item = &'a Vec<u8>>,
    byte_size: usize,
) -> Vec<Vec<u8>> {
    let collect_all = byte_size == i32::MAX as usize;
    keys.filter(|key| collect_all || key.len() == byte_size)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::collect_exact_size_keys;

    #[test]
    fn collect_exact_size_filters_keys() {
        let keys = vec![vec![1], vec![2, 3], vec![4]];
        assert_eq!(collect_exact_size_keys(keys.iter(), 1).len(), 2);
    }
}
