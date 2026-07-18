use super::detect::{site_touched_loc, to_site, to_site_touch, touches_shared_lines};
use super::{DirectPairWitness, PropagationTarget, Site};
use crate::source_lines::FileLineCache;
use nose_detect::{FragmentKind, Loc, RefactorFamily};
use std::collections::HashMap;
use std::path::Path;

pub(super) fn direct_targets(
    family: &RefactorFamily,
    base_root: &Path,
    lines: &mut FileLineCache,
    changed: &HashMap<String, Vec<(u32, u32)>>,
) -> Vec<PropagationTarget> {
    let mut targets = Vec::new();
    append_direct_targets(
        &mut targets,
        &family.locations,
        &family.direct_edges,
        base_root,
        lines,
        changed,
    );
    for coverage in &family.accepted_coverage {
        append_direct_targets(
            &mut targets,
            &coverage.sites,
            &coverage.edges,
            base_root,
            lines,
            changed,
        );
    }
    // Ranking can attach the same accepted obligation through more than one
    // subsumed family. Keep one deterministic strongest witness per directed site pair.
    targets.sort_by(|a, b| {
        a.target_id
            .cmp(&b.target_id)
            .then_with(|| {
                b.direct_witness
                    .similarity
                    .total_cmp(&a.direct_witness.similarity)
            })
            .then(a.direct_witness.kind.cmp(b.direct_witness.kind))
    });
    targets.dedup_by(|a, b| a.target_id == b.target_id);
    targets
}

fn append_direct_targets(
    targets: &mut Vec<PropagationTarget>,
    sites: &[Loc],
    edges: &[nose_detect::AcceptedEdge],
    base_root: &Path,
    lines: &mut FileLineCache,
    changed: &HashMap<String, Vec<(u32, u32)>>,
) {
    for edge in edges {
        let (Some(left), Some(right)) = (
            sites.get(edge.left as usize),
            sites.get(edge.right as usize),
        ) else {
            continue;
        };
        let left_changed = site_touched_loc(left, changed);
        let right_changed = site_touched_loc(right, changed);
        let (changed_loc, skipped_loc) = match (left_changed, right_changed) {
            (true, false) => (left, right),
            (false, true) => (right, left),
            _ => continue,
        };
        let touch = touches_shared_lines(
            changed_loc,
            &[skipped_loc],
            Some(edge.witness_kind),
            base_root,
            lines,
            changed,
        );
        let changed_site = to_site_touch(changed_loc, touch);
        let skipped_site = to_site(skipped_loc);
        targets.push(PropagationTarget {
            target_id: propagation_target_id(&changed_site, &skipped_site),
            changed: changed_site,
            skipped: skipped_site,
            direct_witness: DirectPairWitness {
                kind: edge.witness_kind,
                similarity: edge.score,
            },
        });
    }
}

pub(super) fn same_loc(loc: &Loc, site: &Site) -> bool {
    loc.file == site.file
        && loc.start_line == site.start_line
        && loc.end_line == site.end_line
        && loc.lang == site.lang
        && loc.kind == site.kind
        && loc.name == site.name
        && loc.is_fragment == site.is_fragment
        && loc.fragment_kind == site.fragment_kind
}

fn propagation_target_id(changed: &Site, skipped: &Site) -> String {
    let mut hash = crate::fnv::OFFSET_BASIS;
    let mut mix = |bytes: &[u8]| {
        for &byte in bytes {
            hash = crate::fnv::mix(hash, byte as u64);
        }
        hash = crate::fnv::mix(hash, 0xff);
    };
    mix(b"divergence-target-v1");
    mix_site_identity(&mut mix, changed);
    mix(b"changed-to-skipped");
    mix_site_identity(&mut mix, skipped);
    crate::baseline::format_key(hash)
}

fn mix_site_identity(mix: &mut impl FnMut(&[u8]), site: &Site) {
    mix(site.file.as_bytes());
    mix(site.lang.as_bytes());
    mix(&site.start_line.to_le_bytes());
    mix(&site.end_line.to_le_bytes());
    mix(match site.kind {
        nose_il::UnitKind::Function => b"function",
        nose_il::UnitKind::Method => b"method",
        nose_il::UnitKind::Class => b"class",
        nose_il::UnitKind::Block => b"block",
    });
    mix(site.name.as_deref().unwrap_or_default().as_bytes());
    mix(&[u8::from(site.is_fragment)]);
    mix(site
        .fragment_kind
        .map(FragmentKind::reason_code)
        .unwrap_or_default()
        .as_bytes());
    mix(site.reason_code.unwrap_or_default().as_bytes());
}
