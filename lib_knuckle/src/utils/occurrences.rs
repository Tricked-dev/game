pub(crate) fn count_occurrences(arr: &[u32]) -> std::collections::HashMap<u32, u32> {
    let mut map = std::collections::HashMap::new();
    for &item in arr {
        *map.entry(item).or_insert(0) += 1;
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_occurrences() {
        let arr = vec![1, 2, 3, 4, 1, 2, 3, 4];
        let map = count_occurrences(&arr);
        assert_eq!(map[&1], 2);
        assert_eq!(map[&2], 2);
        assert_eq!(map[&3], 2);
        assert_eq!(map[&4], 2);
    }
}
