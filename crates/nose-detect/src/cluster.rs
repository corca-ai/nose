//! Union-find clustering of accepted clone pairs into clone groups.

pub(crate) struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    pub(crate) fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    pub(crate) fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.parent[r] != r {
            r = self.parent[r];
        }
        // path compression
        let mut c = x;
        while self.parent[c] != r {
            let next = self.parent[c];
            self.parent[c] = r;
            c = next;
        }
        r
    }

    pub(crate) fn union(&mut self, a: usize, b: usize) {
        let root = self.find(a);
        self.union_from_root(root, b);
    }

    /// Join from a current root and return the root chosen by the same rank rule.
    /// Callers must replace their root with the returned value after every join.
    pub(crate) fn union_from_root(&mut self, ra: usize, b: usize) -> usize {
        debug_assert_eq!(self.parent[ra], ra);
        let rb = self.find(b);
        if ra == rb {
            return ra;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => {
                self.parent[ra] = rb;
                rb
            }
            std::cmp::Ordering::Greater => {
                self.parent[rb] = ra;
                ra
            }
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
                ra
            }
        }
    }

    /// Groups of size ≥ 2, each a list of member indices.
    pub(crate) fn groups(&mut self, n: usize) -> Vec<Vec<usize>> {
        use rustc_hash::FxHashMap;
        let mut by_root: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
        for i in 0..n {
            let r = self.find(i);
            by_root.entry(r).or_default().push(i);
        }
        by_root.into_values().filter(|g| g.len() >= 2).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_root_tracks_rank_changes_and_preserves_representatives() {
        let mut actual = UnionFind::new(16);
        let mut reference = UnionFind::new(16);
        for (a, b) in [
            (1, 2),
            (3, 4),
            (1, 3),
            (6, 7),
            (8, 9),
            (10, 11),
            (12, 13),
            (6, 8),
            (10, 12),
            (6, 10),
        ] {
            actual.union(a, b);
            reference.union(a, b);
        }
        let mut root = actual.find(0);
        for (right, expected_root) in [(1, 1), (5, 1), (2, 1), (6, 6), (14, 6), (15, 6)] {
            root = actual.union_from_root(root, right);
            reference.union(0, right);
            assert_eq!(root, expected_root);
            for unit in 0..16 {
                assert_eq!(actual.find(unit), reference.find(unit));
            }
            assert_eq!(actual.groups(16), reference.groups(16));
        }
        assert_eq!(actual.groups(16), vec![(0..16).collect::<Vec<_>>()]);
    }
}
