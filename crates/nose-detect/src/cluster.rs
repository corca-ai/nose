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
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
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
