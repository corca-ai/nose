//! Exact union of overlapping row neighborhoods, with bitsets for dense buckets.
struct Bucket {
    members: Vec<usize>,
    bits: Option<Vec<u64>>,
}

pub(super) struct Neighborhoods {
    buckets: Vec<Bucket>,
    last_members: Vec<usize>,
}

pub(super) struct Scratch {
    seen: Vec<usize>,
    bits: Vec<u64>,
}

impl Scratch {
    pub(super) fn new(rows: usize) -> Self {
        Self {
            seen: vec![usize::MAX; rows],
            bits: vec![0; rows.div_ceil(64)],
        }
    }
}

impl Neighborhoods {
    pub(super) fn new(last_members: Vec<usize>, buckets: Vec<Vec<usize>>) -> Self {
        let words = last_members.len().div_ceil(64);
        let buckets = buckets
            .into_iter()
            .map(|members| {
                let bits = (members.len() > words * 4).then(|| {
                    let mut bits = vec![0; words];
                    for &right in &members {
                        bits[right / 64] |= 1 << (right % 64);
                    }
                    bits
                });
                Bucket { members, bits }
            })
            .collect();
        Self {
            buckets,
            last_members,
        }
    }

    /// `left` is the unique first source member of this row, also its visit tag.
    pub(super) fn collect(
        &self,
        left: usize,
        memberships: &[usize],
        scratch: &mut Scratch,
        out: &mut Vec<usize>,
    ) {
        out.clear();
        if !memberships
            .iter()
            .any(|&id| self.buckets[id].bits.is_some())
        {
            for &id in memberships {
                for &right in &self.buckets[id].members {
                    if scratch.seen[right] != left {
                        scratch.seen[right] = left;
                        if left < self.last_members[right] {
                            out.push(right);
                        }
                    }
                }
            }
            return;
        }
        scratch.bits.fill(0);
        for &id in memberships {
            let bucket = &self.buckets[id];
            if let Some(bits) = &bucket.bits {
                for (target, source) in scratch.bits.iter_mut().zip(bits) {
                    *target |= source;
                }
            } else {
                for &right in &bucket.members {
                    scratch.bits[right / 64] |= 1 << (right % 64);
                }
            }
        }
        for (word, &bits) in scratch.bits.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let right = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if left < self.last_members[right] {
                    out.push(right);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn dense_sparse_and_mixed_unions_match_every_source_admitted_neighbor() {
        for size in [0, 1, 63, 64, 65, 257] {
            let last = (0..size).map(|i| i * 3 + 2).collect::<Vec<_>>();
            let buckets = vec![
                (0..size).collect::<Vec<_>>(),
                (0..size).step_by(2).collect(),
                (0..size).step_by(3).collect(),
                (0..size).take(2).collect(),
            ];
            let index = Neighborhoods::new(last.clone(), buckets.clone());
            for memberships in [vec![], vec![0], vec![1, 2], vec![0, 1, 2, 3], vec![3]] {
                let mut scratch = Scratch::new(size);
                let mut actual = Vec::new();
                for left in (0..size * 3 + 1).step_by(7) {
                    let expected = memberships
                        .iter()
                        .flat_map(|&id| buckets[id].iter().copied())
                        .filter(|&right| left < last[right])
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    index.collect(left, &memberships, &mut scratch, &mut actual);
                    actual.sort_unstable();
                    assert_eq!(
                        actual, expected,
                        "size={size}, left={left}, memberships={memberships:?}"
                    );
                }
            }
        }
    }
}
