use super::*;
use std::collections::BTreeMap;

mod sinks;
use sinks::{sink_deltas, sink_signatures};

pub(super) struct SharedHashes {
    pub(super) hashes: BTreeMap<u64, usize>,
    pub(super) units_checked: usize,
}

impl SharedHashes {
    fn contains_key(&self, hash: &u64) -> bool {
        self.hashes.contains_key(hash)
    }
}

pub(super) struct SemanticAnalysis {
    change_kind: SemanticChangeKind,
    facets: Vec<SemanticChangeFacet>,
    sink_deltas: Vec<SemanticSinkDelta>,
    base_affected_nodes: usize,
    current_affected_nodes: usize,
    base_affected_hashes: Vec<u64>,
    has_insertions: bool,
    has_non_insertions: bool,
    same_semantics: bool,
    removed: usize,
    inserted: usize,
}

pub(super) struct PreparedChange {
    pub(super) analysis: SemanticAnalysis,
    pub(super) alignment: SemanticAlignment,
    pub(super) base_exact_safe: bool,
    pub(super) current_exact_safe: bool,
    pub(super) base_truncated: bool,
    pub(super) current_truncated: bool,
    pub(super) unresolved_referent: bool,
}

pub(super) fn finish_witness(
    prepared: &PreparedChange,
    sibling_hashes: &SharedHashes,
) -> SemanticChangeWitness {
    let analysis = &prepared.analysis;
    let mapped_shared_nodes = analysis
        .base_affected_hashes
        .iter()
        .filter(|hash| sibling_hashes.contains_key(hash))
        .count();
    let caveats = analysis_caveats(analysis, prepared, mapped_shared_nodes);
    SemanticChangeWitness {
        status: if caveats.is_empty() {
            SemanticWitnessStatus::Complete
        } else {
            SemanticWitnessStatus::Advisory
        },
        change_kind: analysis.change_kind,
        facets: analysis.facets.clone(),
        alignment: prepared.alignment,
        base_projection: SemanticProjectionStatus::Ok,
        current_projection: SemanticProjectionStatus::Ok,
        coverage: SemanticWitnessCoverage {
            base_affected_nodes: analysis.base_affected_nodes,
            current_affected_nodes: analysis.current_affected_nodes,
            mapped_shared_nodes,
            sibling_units_checked: sibling_hashes.units_checked,
        },
        sink_deltas: analysis.sink_deltas.clone(),
        caveats,
        caps: CAPS,
    }
}

pub(super) fn analyze_change(
    base: &UnitProjection,
    current: &UnitProjection,
    base_ranges: &[(u32, u32)],
    current_ranges: &[(u32, u32)],
) -> SemanticAnalysis {
    let base_ranges = ranges_inside_unit(base_ranges, base);
    let current_ranges = ranges_inside_unit(current_ranges, current);
    let has_insertions = base_ranges.iter().any(|(start, end)| start > end);
    let has_non_insertions = base_ranges.iter().any(|(start, end)| start <= end);
    let source_deletion = has_non_insertions
        && !has_insertions
        && !current_ranges.is_empty()
        && current_ranges.iter().all(|(start, end)| start > end);
    let base_affected = affected_hashes(&base.dag, &base_ranges);
    let current_affected = if source_deletion {
        Vec::new()
    } else {
        affected_hashes(&current.dag, &current_ranges)
    };
    let base_nodes = node_hashes(&base.dag);
    let current_nodes = node_hashes(&current.dag);
    let sinks_before = sink_signatures(&base.dag);
    let sinks_after = sink_signatures(&current.dag);
    let same_semantics = base_nodes == current_nodes && sinks_before == sinks_after;
    let removed = multiset_removed(&base_affected, &current_affected);
    let inserted = multiset_removed(&current_affected, &base_affected);
    let sink_deltas = sink_deltas(&sinks_before, &sinks_after);
    SemanticAnalysis {
        change_kind: classify_change(
            same_semantics,
            has_insertions,
            has_non_insertions,
            removed,
            inserted,
        ),
        facets: change_facets(base_nodes != current_nodes, &sink_deltas),
        sink_deltas,
        base_affected_nodes: base_affected.len(),
        current_affected_nodes: current_affected.len(),
        base_affected_hashes: base_affected,
        has_insertions,
        has_non_insertions,
        same_semantics,
        removed,
        inserted,
    }
}

fn classify_change(
    same_semantics: bool,
    has_insertions: bool,
    has_non_insertions: bool,
    removed: usize,
    inserted: usize,
) -> SemanticChangeKind {
    match (
        same_semantics,
        has_insertions,
        has_non_insertions,
        removed,
        inserted,
    ) {
        (true, _, _, _, _) => SemanticChangeKind::NoSemanticDelta,
        (_, true, true, _, _) => SemanticChangeKind::Mixed,
        (_, true, false, _, _) | (_, false, _, 0, 1..) => SemanticChangeKind::Insertion,
        (_, false, _, 1.., 1..) => SemanticChangeKind::Replacement,
        (_, false, _, 1.., 0) => SemanticChangeKind::Deletion,
        _ => SemanticChangeKind::Unknown,
    }
}

fn change_facets(
    value_changed: bool,
    sink_deltas: &[SemanticSinkDelta],
) -> Vec<SemanticChangeFacet> {
    let mut facets = Vec::new();
    if value_changed {
        facets.push(SemanticChangeFacet::Value);
    }
    for delta in sink_deltas {
        match delta.kind {
            SemanticSinkKind::Return => facets.push(SemanticChangeFacet::Return),
            SemanticSinkKind::Cond | SemanticSinkKind::Break => {
                facets.push(SemanticChangeFacet::Control);
            }
            SemanticSinkKind::Effect => facets.push(SemanticChangeFacet::Effect),
            SemanticSinkKind::Throw => {
                facets.push(SemanticChangeFacet::Control);
                facets.push(SemanticChangeFacet::Effect);
            }
        }
    }
    facets.sort();
    facets.dedup();
    facets
}

fn analysis_caveats(
    analysis: &SemanticAnalysis,
    prepared: &PreparedChange,
    mapped_shared_nodes: usize,
) -> Vec<SemanticWitnessCaveat> {
    let mut caveats = Vec::new();
    if analysis.has_insertions && analysis.has_non_insertions {
        caveats.push(SemanticWitnessCaveat::MixedChange);
    } else if analysis.has_insertions {
        caveats.push(SemanticWitnessCaveat::PureInsertion);
    }
    if !prepared.base_exact_safe {
        caveats.push(SemanticWitnessCaveat::LossyBaseLowering);
    }
    if !prepared.current_exact_safe {
        caveats.push(SemanticWitnessCaveat::LossyCurrentLowering);
    }
    if prepared.unresolved_referent {
        caveats.push(SemanticWitnessCaveat::UnresolvedReferent);
    }
    if prepared.base_truncated || prepared.current_truncated {
        caveats.push(SemanticWitnessCaveat::Truncated);
    }
    if prepared.alignment == SemanticAlignment::NearestSpan {
        caveats.push(SemanticWitnessCaveat::HeuristicAlignment);
    }
    if analysis.base_affected_nodes == 0 {
        caveats.push(SemanticWitnessCaveat::NoAffectedSemanticNode);
    } else if mapped_shared_nodes == 0 {
        caveats.push(SemanticWitnessCaveat::NoSharedSemanticNode);
    }
    if !analysis.same_semantics && analysis.removed == 0 && analysis.inserted == 0 {
        caveats.push(SemanticWitnessCaveat::ScopedDeltaUnmapped);
    }
    caveats.sort();
    caveats.dedup();
    caveats
}

pub(super) fn project_file(
    absolute_path: &Path,
    relative_path: &str,
    opts: &nose_detect::DetectOptions,
) -> LoadState {
    let Some(lang) = Lang::from_file_path(absolute_path) else {
        return LoadState::Failed(SemanticProjectionStatus::Unsupported);
    };
    // Container/declarative projections need region-aware source coordinates or a
    // non-imperative fingerprint; neither may be silently interpreted as a value DAG.
    if matches!(lang, Lang::Css | Lang::Vue | Lang::Svelte | Lang::Html) {
        return LoadState::Failed(SemanticProjectionStatus::Unsupported);
    }
    let source = match fs::read(absolute_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadState::Failed(SemanticProjectionStatus::Missing)
        }
        Err(_) => return LoadState::Failed(SemanticProjectionStatus::ReadFailed),
    };
    if source.len() > MAX_FILE_BYTES {
        return LoadState::Failed(SemanticProjectionStatus::CapExceeded);
    }
    let interner = Interner::new();
    let raw = match nose_frontend::lower_source(FileId(0), relative_path, &source, lang, &interner)
    {
        Ok(raw) => raw,
        Err(_) => return LoadState::Failed(SemanticProjectionStatus::LowerFailed),
    };
    let normalized = nose_normalize::normalize(
        &raw,
        &interner,
        &nose_normalize::NormalizeOptions {
            cfg_norm: opts.cfg_norm,
            dce: opts.dce,
            ..Default::default()
        },
    );
    finish_file_projection(interner, normalized, HashMap::new(), None)
}

pub(super) fn project_normalized_file(
    absolute_path: &Path,
    interner: Interner,
    normalized: nose_il::Il,
    known_exact_safety: HashMap<(UnitKind, u32, u32), bool>,
    value_context: Option<nose_normalize::ValueFingerprintContext>,
) -> LoadState {
    let Some(lang) = Lang::from_file_path(absolute_path) else {
        return LoadState::Failed(SemanticProjectionStatus::Unsupported);
    };
    if matches!(lang, Lang::Css | Lang::Vue | Lang::Svelte | Lang::Html) {
        return LoadState::Failed(SemanticProjectionStatus::Unsupported);
    }
    match fs::metadata(absolute_path) {
        Ok(metadata) if metadata.len() > MAX_FILE_BYTES as u64 => {
            return LoadState::Failed(SemanticProjectionStatus::CapExceeded)
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadState::Failed(SemanticProjectionStatus::Missing)
        }
        Err(_) => return LoadState::Failed(SemanticProjectionStatus::ReadFailed),
    }
    finish_file_projection(interner, normalized, known_exact_safety, value_context)
}

fn finish_file_projection(
    interner: Interner,
    normalized: nose_il::Il,
    known_exact_safety: HashMap<(UnitKind, u32, u32), bool>,
    value_context: Option<nose_normalize::ValueFingerprintContext>,
) -> LoadState {
    if normalized.units.len() > MAX_UNITS_PER_FILE {
        return LoadState::Failed(SemanticProjectionStatus::CapExceeded);
    }
    let mut units = Vec::with_capacity(normalized.units.len());
    for unit in &normalized.units {
        let span = normalized.node(unit.root).span;
        let name = unit.name.map(|symbol| interner.resolve(symbol).to_string());
        units.push(UnitSkeleton {
            root: unit.root,
            kind: unit.kind,
            origin: unit.origin,
            name,
            start_line: span.start_line,
            end_line: span.end_line,
        });
    }
    LoadState::Ready(Box::new(FileProjection {
        interner,
        normalized,
        units,
        known_exact_safety,
        value_context,
        exact_safety: OnceLock::new(),
        referents: OnceLock::new(),
        prepared_units: HashMap::new(),
    }))
}

pub(super) fn prepare_file_projection(state: &mut LoadState, changed_ranges: &[(u32, u32)]) {
    let LoadState::Ready(file) = state else {
        return;
    };
    let _ = file.exact_safety.get_or_init(|| {
        let roots = file.units.iter().map(|unit| unit.root).collect::<Vec<_>>();
        nose_detect::exact_safe_roots_by_span(&file.normalized, &file.interner, &roots)
    });
    let _ = file
        .referents
        .get_or_init(|| FileReferents::new(&file.normalized, &file.interner));
    let wanted = file
        .units
        .iter()
        .filter(|unit| ranges_touch_skeleton(changed_ranges, unit))
        .cloned()
        .collect::<Vec<_>>();
    let prepared = wanted
        .into_iter()
        .map(|unit| (unit.root, Arc::new(project_unit(file, &unit))))
        .collect::<Vec<_>>();
    file.prepared_units.extend(prepared);
}

pub(super) fn project_unit(file: &FileProjection, unit: &UnitSkeleton) -> UnitProjection {
    let span = file.normalized.node(unit.root).span;
    let exact_safe = file
        .known_exact_safety
        .get(&(unit.kind, unit.start_line, unit.end_line))
        .copied()
        .unwrap_or_else(|| {
            file.exact_safety
                .get_or_init(|| {
                    let roots = file.units.iter().map(|unit| unit.root).collect::<Vec<_>>();
                    nose_detect::exact_safe_roots_by_span(&file.normalized, &file.interner, &roots)
                })
                .get(&(span.start_byte, span.end_byte))
                .copied()
                .unwrap_or(false)
        });
    let referents = file
        .referents
        .get_or_init(|| FileReferents::new(&file.normalized, &file.interner));
    let dag = nose_normalize::value_dag(
        &file.normalized,
        unit.root,
        &file.interner,
        file.value_context.as_ref(),
        referents,
    );
    let truncated = dag.nodes.len() > MAX_NODES_PER_UNIT;
    let unresolved_referent = dag
        .referents
        .iter()
        .any(|referent| referent.referent.is_none());
    UnitProjection {
        kind: unit.kind,
        origin: unit.origin,
        name: unit.name.clone(),
        start_line: unit.start_line,
        end_line: unit.end_line,
        exact_safe,
        dag,
        truncated,
        unresolved_referent,
    }
}

pub(super) fn unit_matches_site(unit: &UnitSkeleton, site: &Site, require_span: bool) -> bool {
    unit.kind == site.kind
        && (!require_span || (unit.start_line == site.start_line && unit.end_line == site.end_line))
        && match site.name.as_deref() {
            Some(name) => unit.name.as_deref() == Some(name),
            None => true,
        }
}

pub(super) fn unit_matches_enclosing(unit: &UnitSkeleton, enclosing: &EnclosingUnit) -> bool {
    unit.kind == enclosing.kind
        && unit.start_line == enclosing.start_line
        && unit.end_line == enclosing.end_line
        && match enclosing.name.as_deref() {
            Some(name) => unit.name.as_deref() == Some(name),
            None => true,
        }
}

pub(super) fn projected(
    unit: Arc<UnitProjection>,
    alignment: SemanticAlignment,
) -> ProjectionAttempt {
    ProjectionAttempt {
        status: SemanticProjectionStatus::Ok,
        alignment,
        unit: Some(unit),
    }
}

pub(super) fn select_current_by_change_or_distance(
    same_kind: &[UnitSkeleton],
    base: &UnitProjection,
    changed_ranges: &[(u32, u32)],
) -> Result<(UnitSkeleton, SemanticAlignment), SemanticProjectionStatus> {
    let changed = same_kind
        .iter()
        .filter(|unit| ranges_touch_skeleton(changed_ranges, unit))
        .cloned()
        .collect::<Vec<_>>();
    if let [unit] = changed.as_slice() {
        return Ok((unit.clone(), SemanticAlignment::ChangedRange));
    }
    let Some(min_distance) = same_kind
        .iter()
        .map(|unit| unit.start_line.abs_diff(base.start_line))
        .min()
    else {
        return Err(SemanticProjectionStatus::UnitMissing);
    };
    let nearest = same_kind
        .iter()
        .filter(|unit| unit.start_line.abs_diff(base.start_line) == min_distance)
        .cloned()
        .collect::<Vec<_>>();
    match nearest.as_slice() {
        [unit] => Ok((unit.clone(), SemanticAlignment::NearestSpan)),
        _ => Err(SemanticProjectionStatus::AmbiguousUnit),
    }
}

fn ranges_inside_unit(ranges: &[(u32, u32)], unit: &UnitProjection) -> Vec<(u32, u32)> {
    ranges
        .iter()
        .copied()
        .filter(|&(start, end)| {
            if start <= end {
                start <= unit.end_line && unit.start_line <= end
            } else {
                unit.start_line < start && start <= unit.end_line
            }
        })
        .collect()
}

fn ranges_touch_skeleton(ranges: &[(u32, u32)], unit: &UnitSkeleton) -> bool {
    ranges.iter().any(|&(start, end)| {
        if start <= end {
            start <= unit.end_line && unit.start_line <= end
        } else {
            unit.start_line < start && start <= unit.end_line
        }
    })
}

fn affected_hashes(dag: &ValueDag, ranges: &[(u32, u32)]) -> Vec<u64> {
    let mut hashes = dag
        .nodes
        .iter()
        .take(MAX_NODES_PER_UNIT)
        .filter(|node| node.line_start != 0 && node.line_end != 0)
        .filter(|node| {
            ranges.iter().any(|&(start, end)| {
                if start <= end {
                    start <= node.line_end && node.line_start <= end
                } else {
                    node.line_start < start && start <= node.line_end
                }
            })
        })
        .map(|node| node.hash)
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    hashes
}

pub(super) fn node_hashes(dag: &ValueDag) -> Vec<u64> {
    let mut hashes = dag
        .nodes
        .iter()
        .take(MAX_NODES_PER_UNIT)
        .map(|node| node.hash)
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    hashes
}

fn multiset_removed<T: Ord>(before: &[T], after: &[T]) -> usize {
    let mut before_index = 0;
    let mut after_index = 0;
    let mut removed = 0;
    while before_index < before.len() {
        while after_index < after.len() && after[after_index] < before[before_index] {
            after_index += 1;
        }
        if after_index < after.len() && after[after_index] == before[before_index] {
            after_index += 1;
        } else {
            removed += 1;
        }
        before_index += 1;
    }
    removed
}

pub(super) fn caveat_for_projection(
    status: SemanticProjectionStatus,
    base: bool,
) -> Vec<SemanticWitnessCaveat> {
    match status {
        SemanticProjectionStatus::Unsupported => vec![SemanticWitnessCaveat::UnsupportedLanguage],
        SemanticProjectionStatus::AmbiguousUnit => {
            vec![SemanticWitnessCaveat::AmbiguousAlignment]
        }
        SemanticProjectionStatus::CapExceeded => vec![SemanticWitnessCaveat::Truncated],
        SemanticProjectionStatus::Missing | SemanticProjectionStatus::UnitMissing if !base => {
            vec![SemanticWitnessCaveat::MissingCurrentUnit]
        }
        _ => Vec::new(),
    }
}

pub(super) fn unavailable(
    base_projection: SemanticProjectionStatus,
    current_projection: SemanticProjectionStatus,
    mut caveats: Vec<SemanticWitnessCaveat>,
) -> SemanticChangeWitness {
    caveats.sort();
    caveats.dedup();
    SemanticChangeWitness {
        status: SemanticWitnessStatus::Unavailable,
        change_kind: SemanticChangeKind::Unknown,
        facets: Vec::new(),
        alignment: SemanticAlignment::None,
        base_projection,
        current_projection,
        coverage: SemanticWitnessCoverage {
            base_affected_nodes: 0,
            current_affected_nodes: 0,
            mapped_shared_nodes: 0,
            sibling_units_checked: 0,
        },
        sink_deltas: Vec::new(),
        caveats,
        caps: CAPS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiset_difference_preserves_duplicate_counts() {
        assert_eq!(multiset_removed(&[1, 1, 2, 4], &[1, 2, 3]), 2);
        assert_eq!(multiset_removed(&[1, 2, 3], &[1, 1, 2, 4]), 1);
    }

    #[test]
    fn insertion_context_requires_a_node_to_straddle_the_gap() {
        let dag = ValueDag {
            nodes: vec![nose_normalize::VgNode {
                op: nose_normalize::VgOp::Input,
                key: 0,
                args: Vec::new(),
                hash: 7,
                line_start: 2,
                line_end: 4,
            }],
            sinks: Vec::new(),
            referents: Vec::new(),
        };
        assert_eq!(affected_hashes(&dag, &[(3, 2)]), vec![7]);
        assert!(affected_hashes(&dag, &[(2, 1)]).is_empty());
    }
}
