//! Count the same strict two-coordinate order as the source nesting exclusion.

pub(super) fn non_nested_count(mut spans: Vec<(usize, u32, u32)>) -> usize {
    spans.sort_unstable();
    let mut count = 0;
    let mut start = 0;
    while start < spans.len() {
        let end = start + spans[start..].partition_point(|span| span.0 == spans[start].0);
        // Different files never exclude each other.
        count += start * (end - start);
        count += within_file(&spans[start..end]);
        start = end;
    }
    count
}

fn within_file(spans: &[(usize, u32, u32)]) -> usize {
    if spans
        .windows(2)
        .all(|pair| pair[0].1 < pair[1].1 && pair[0].2 < pair[1].2)
    {
        return spans.len() * spans.len().saturating_sub(1) / 2;
    }
    let mut ends = spans.iter().map(|span| span.2).collect::<Vec<_>>();
    ends.sort_unstable();
    ends.dedup();
    let mut counts = vec![0usize; ends.len() + 1];
    let mut count = 0;
    let mut start = 0;
    while start < spans.len() {
        let end = start + spans[start..].partition_point(|span| span.1 == spans[start].1);
        // Query before inserting this batch: equal starts are always nested.
        for span in &spans[start..end] {
            let mut index = ends.binary_search(&span.2).unwrap();
            while index != 0 {
                count += counts[index];
                index &= index - 1;
            }
        }
        for span in &spans[start..end] {
            let mut index = ends.binary_search(&span.2).unwrap() + 1;
            while index < counts.len() {
                counts[index] += 1;
                index += index & index.wrapping_neg();
            }
        }
        start = end;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_count_matches_literal_source_exclusions() {
        for files in [1, 3, 17] {
            for count in [0, 1, 2, 31, 32, 33, 255, 256, 257, 513] {
                let mut spans = (0..count)
                    .map(|i| (i % files, ((i * 13) % 41) as u32, ((i * 7) % 43) as u32))
                    .collect::<Vec<_>>();
                let expected = spans
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        spans[i + 1..]
                            .iter()
                            .filter(|b| {
                                a.0 != b.0
                                    || !((a.1 <= b.1 && a.2 >= b.2) || (b.1 <= a.1 && b.2 >= a.2))
                            })
                            .count()
                    })
                    .sum::<usize>();
                assert_eq!(non_nested_count(spans.clone()), expected);
                spans.reverse();
                assert_eq!(non_nested_count(spans), expected);
            }
        }
        assert_eq!(non_nested_count(vec![(0, 1, 5), (0, 2, 6), (0, 3, 7)]), 3);
        assert_eq!(non_nested_count(vec![(0, 1, 5), (0, 1, 6), (0, 2, 5)]), 0);
    }
}
