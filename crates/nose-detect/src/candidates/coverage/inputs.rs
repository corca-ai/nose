//! Reuse whole-group exact evidence; retain full witness inputs for mixed groups.
use crate::{exact_policy::exact_claim_eligible, Group, UnitFeat, WitnessEvidence};
use nose_normalize::Anchor;
use rustc_hash::FxHashMap;

type SiteKey = (usize, u32, usize);

pub(super) struct WitnessInputs {
    pub(super) keys: Vec<Option<SiteKey>>,
    pub(super) exact: Vec<Option<usize>>,
    pub(super) anchors: Vec<Vec<Anchor>>,
}

impl WitnessInputs {
    pub(super) fn new(
        units: &[UnitFeat],
        positions: &[Option<(usize, u32)>],
        groups: &[Group],
    ) -> Self {
        let mut anchors = Vec::new();
        let exact_groups = groups
            .iter()
            .map(|group| {
                group
                    .witness
                    .as_ref()
                    .filter(|witness| {
                        matches!(&witness.evidence, WitnessEvidence::ExactValueGraph { .. })
                    })
                    .map(|_| {
                        let class = anchors.len();
                        // Every pair inside this group is exact. No cross-group pair is
                        // admitted, so anchors cannot affect any of its pair witnesses.
                        anchors.push(Vec::new());
                        class
                    })
            })
            .collect::<Vec<_>>();
        let exact_prefix = anchors.len();
        let mut witnesses = FxHashMap::default();
        let mut values = FxHashMap::default();
        let mut keys = vec![None; units.len()];
        let mut exact = vec![None; units.len()];
        for (index, (unit, &position)) in units.iter().zip(positions).enumerate() {
            let Some((group, site)) = position else {
                continue;
            };
            if let Some(class) = exact_groups[group] {
                keys[index] = Some((group, site, class));
                exact[index] = Some(class);
                continue;
            }
            if exact_claim_eligible(unit) {
                let next = exact_prefix + values.len();
                exact[index] = Some(*values.entry(&unit.value).or_insert(next));
            }
            let next = anchors.len();
            let class = *witnesses
                .entry((
                    &unit.value,
                    unit.exact_safe,
                    unit.anchors
                        .iter()
                        .map(|anchor| (anchor.hash, anchor.weight))
                        .collect::<Vec<_>>(),
                ))
                .or_insert(next);
            if class == next {
                anchors.push(unit.anchors.clone());
            }
            keys[index] = Some((group, site, class));
        }
        Self {
            keys,
            exact,
            anchors,
        }
    }
}
