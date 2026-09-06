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
    pub(super) fn push(
        &mut self,
        edges: &mut [Option<SiteEdgeBuilder>],
        group: usize,
        left: u32,
        right: u32,
        score: f64,
    ) {
        let block = right / 64;
        if let Some(pending) = &mut self.0 {
            if pending.group == group
                && pending.left == left
                && pending.block == block
                && pending.score.to_bits() == score.to_bits()
            {
                pending.mask |= 1 << (right % 64);
                return;
            }
        }
        self.flush(edges);
        self.0 = Some(Pending {
            group,
            left,
            block,
            mask: 1 << (right % 64),
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
