//! A complete homogeneous row proves every cross-file site pair directly.
//! Same-file exclusions remain occurrence-specific and use the ordinary visitor.
use super::{RowPairs, SiteEvidence};
use rustc_hash::FxHashMap;

type SiteKey = (usize, u32, usize);

pub(super) struct SiteClique {
    score: f64,
    sites: Vec<(u32, usize, usize)>,
    blocks: Vec<Block>,
}

struct Block {
    index: u32,
    mask: u64,
    files: FxHashMap<usize, u64>,
}

pub(super) fn prepare(
    rows: &RowPairs,
    keys: &[Option<SiteKey>],
    exact: &[Option<usize>],
) -> Vec<Option<SiteClique>> {
    let mut counts = vec![0; rows.targets.len()];
    let mut group_rows = FxHashMap::default();
    for (unit, &row) in rows.row_of.iter().enumerate() {
        counts[row] += 1;
        if let Some((group, _, _)) = keys[unit] {
            group_rows
                .entry(group)
                .and_modify(|previous| {
                    if *previous != Some(row) {
                        *previous = None;
                    }
                })
                .or_insert(Some(row));
        }
    }
    rows.targets
        .iter()
        .enumerate()
        .map(|(row, targets)| {
            // Membership equality, not connectivity, proves this clique. Targets are
            // unique by the accepted-row constructor's invariant.
            if targets.len() < 8
                || targets.len() != counts[row]
                || targets.iter().any(|&(unit, _)| rows.row_of[unit] != row)
            {
                return None;
            }
            let group = targets
                .iter()
                .find_map(|&(unit, _)| keys[unit].map(|key| key.0))?;
            // Other rows of this group may project onto the same site pair.
            // Preserve original first-winner order for signed-zero or NaN ties.
            if group_rows.get(&group) != Some(&Some(row)) {
                return None;
            }
            SiteClique::new(rows, targets, keys, exact)
        })
        .collect()
}

impl SiteClique {
    fn new(
        rows: &RowPairs,
        targets: &[(usize, f64)],
        keys: &[Option<SiteKey>],
        exact: &[Option<usize>],
    ) -> Option<Self> {
        let score = targets.first()?.1;
        if !crate::candidates::round3(score).is_finite()
            || targets
                .iter()
                .any(|&(_, value)| value.to_bits() != score.to_bits())
        {
            return None;
        }
        let mut identity = None;
        let mut sites = FxHashMap::default();
        for &(unit, _) in targets {
            let Some((group, site, _)) = keys[unit] else {
                continue;
            };
            let class = exact[unit]?;
            let first = identity.get_or_insert((group, class));
            if *first != (group, class) {
                return None;
            }
            let path = rows.locations[unit].0;
            let entry = sites.entry(site).or_insert((unit, path));
            // A site spanning files needs occurrence-specific evidence instead.
            if entry.1 != path {
                return None;
            }
        }
        if sites.len() < 8 {
            return None;
        }
        let mut sites = sites
            .into_iter()
            .map(|(site, (unit, path))| (site, unit, path))
            .collect::<Vec<_>>();
        sites.sort_unstable_by_key(|&(site, _, _)| site);
        let mut blocks: Vec<Block> = Vec::new();
        for &(site, _, path) in &sites {
            let index = site / 64;
            if blocks.last().is_none_or(|block| block.index != index) {
                blocks.push(Block {
                    index,
                    mask: 0,
                    files: FxHashMap::default(),
                });
            }
            let block = blocks.last_mut().unwrap();
            let bit = 1 << (site % 64);
            block.mask |= bit;
            *block.files.entry(path).or_default() |= bit;
        }
        Some(Self {
            score,
            sites,
            blocks,
        })
    }

    pub(super) fn visit(&self, visit: &mut impl FnMut(SiteEvidence)) {
        for &(site, left, path) in &self.sites {
            for block in self
                .blocks
                .iter()
                .skip_while(|block| block.index < site / 64)
            {
                let mut mask = block.mask & !block.files.get(&path).copied().unwrap_or(0);
                if block.index == site / 64 {
                    mask &= !(u64::MAX >> (63 - site % 64));
                }
                if mask != 0 {
                    visit(SiteEvidence::ExactMask {
                        left,
                        block: block.index,
                        mask,
                        score: self.score,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
