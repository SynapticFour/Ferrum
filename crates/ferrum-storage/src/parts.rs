//! Byte-range splitting for multipart uploads (pure logic, no AWS).

/// Half-open byte ranges `[start, end)` covering `[0, total_len)`.
///
/// - If `total_len == 0`, returns a single empty range `(0, 0)`.
/// - `part_size` must be greater than zero.
pub fn split_into_part_ranges(total_len: usize, part_size: usize) -> Vec<(usize, usize)> {
    assert!(part_size > 0, "part_size must be positive");
    if total_len == 0 {
        return vec![(0, 0)];
    }
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while start < total_len {
        let end = (start + part_size).min(total_len);
        ranges.push((start, end));
        start = end;
    }
    ranges
}

/// Same as [`split_into_part_ranges`] for `u64` lengths (TB-scale file sizes).
pub fn split_file_part_ranges(total_len: u64, part_size: u64) -> Vec<(u64, u64)> {
    assert!(part_size > 0, "part_size must be positive");
    if total_len == 0 {
        return vec![(0, 0)];
    }
    let mut ranges = Vec::new();
    let mut start = 0u64;
    while start < total_len {
        let end = (start + part_size).min(total_len);
        ranges.push((start, end));
        start = end;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_total() {
        assert_eq!(split_into_part_ranges(0, 5 * 1024 * 1024), vec![(0, 0)]);
    }

    #[test]
    fn one_part_exact() {
        let ps = 5 * 1024 * 1024;
        assert_eq!(split_into_part_ranges(ps, ps), vec![(0, ps)]);
    }

    #[test]
    fn one_part_smaller_than_part_size() {
        assert_eq!(split_into_part_ranges(100, 256), vec![(0, 100)]);
    }

    #[test]
    fn two_parts_last_smaller() {
        let ps = 5 * 1024 * 1024;
        let total = ps + 1;
        assert_eq!(
            split_into_part_ranges(total, ps),
            vec![(0, ps), (ps, total)]
        );
    }

    #[test]
    fn many_parts() {
        let ranges = split_into_part_ranges(10, 3);
        assert_eq!(ranges, vec![(0, 3), (3, 6), (6, 9), (9, 10)]);
    }

    #[test]
    fn file_ranges_64mb_parts() {
        let ps = 64 * 1024 * 1024u64;
        let total = ps + 100;
        assert_eq!(
            split_file_part_ranges(total, ps),
            vec![(0, ps), (ps, total)]
        );
    }
}
