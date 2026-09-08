//! Consecutive identical exact evidence updates share one block lookup.
use crate::report::edges::SiteEdgeBuilder;

#[derive(Default)]
pub(super) struct ExactBlocks(Option<Pending>);

struct Pending {
    group: usize,
    left: u32,
    block: u32,
    mask: u64,
    score: f64,
}

impl ExactBlocks {
    pub(super) fn push_sites(
        &mut self,
        edges: &mut [Option<SiteEdgeBuilder>],
        group: usize,
        site: u32,
        block: u32,
        mask: u64,
        score: f64,
    ) {
        let offset = site % 64;
        let (mut lower, upper) = match block.cmp(&(site / 64)) {
            std::cmp::Ordering::Less => (mask, 0),
            std::cmp::Ordering::Greater => (0, mask),
            std::cmp::Ordering::Equal => (
                mask & ((1u64 << offset) - 1),
                mask & !(u64::MAX >> (63 - offset)),
            ),
        };
        while lower != 0 {
            let left = block * 64 + lower.trailing_zeros();
            self.push_mask(edges, group, left, site / 64, 1 << offset, score);
            lower &= lower - 1;
        }
        if upper != 0 {
            self.push_mask(edges, group, site, block, upper, score);
        }
    }

    pub(super) fn push(
        &mut self,
        edges: &mut [Option<SiteEdgeBuilder>],
        group: usize,
        left: u32,
        right: u32,
        score: f64,
    ) {
        self.push_mask(edges, group, left, right / 64, 1 << (right % 64), score);
    }

    pub(super) fn push_mask(
        &mut self,
        edges: &mut [Option<SiteEdgeBuilder>],
        group: usize,
        left: u32,
        block: u32,
        mask: u64,
        score: f64,
    ) {
        if let Some(pending) = &mut self.0 {
            if pending.group == group
                && pending.left == left
                && pending.block == block
                && pending.score.to_bits() == score.to_bits()
            {
                pending.mask |= mask;
                return;
            }
        }
        self.flush(edges);
        self.0 = Some(Pending {
            group,
            left,
            block,
            mask,
            score,
        });
    }

    pub(super) fn flush(&mut self, edges: &mut [Option<SiteEdgeBuilder>]) {
        if let Some(pending) = self.0.take() {
            edges[pending.group].as_mut().unwrap().insert_exact_mask(
                pending.left,
                pending.block,
                pending.mask,
                pending.score,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_match_scalar_edges_across_canonical_boundaries() {
        for scores in [[-0.0, 0.0], [0.0, -0.0], [0.125, 0.75]] {
            let mut scalar = [Some(SiteEdgeBuilder::new(130))];
            let mut packed = [Some(SiteEdgeBuilder::new(130))];
            let mut scalar_pending = ExactBlocks::default();
            let mut packed_pending = ExactBlocks::default();
            for score in scores {
                for site in [0, 31, 63, 64, 127, 129] {
                    for (block, mask) in [(0, u64::MAX), (1, u64::MAX), (2, 3)] {
                        packed_pending.push_sites(&mut packed, 0, site, block, mask, score);
                        for bit in 0..64 {
                            let other = block * 64 + bit;
                            if mask & (1 << bit) != 0 && other != site {
                                scalar_pending.push(
                                    &mut scalar,
                                    0,
                                    site.min(other),
                                    site.max(other),
                                    score,
                                );
                            }
                        }
                    }
                }
            }
            scalar_pending.flush(&mut scalar);
            packed_pending.flush(&mut packed);
            let collect = |builder: SiteEdgeBuilder| {
                builder
                    .into_edges()
                    .iter()
                    .map(|edge| {
                        (
                            edge.left,
                            edge.right,
                            edge.score.to_bits(),
                            edge.witness_kind,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                collect(packed[0].take().unwrap()),
                collect(scalar[0].take().unwrap())
            );
        }
    }
}
