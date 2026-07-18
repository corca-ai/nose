//! Bounded base-to-current semantic change witnesses for already-flagged divergences.
//!
//! This module is intentionally downstream of candidate detection. It reads only the
//! changed candidate files and a capped set of their base siblings; it never performs a
//! second repository discovery. The evidence is advisory in divergent-edit v2.

use super::git::DiffEntry;
use super::*;
use nose_il::{FileId, Interner, Lang, NodeId, UnitKind};
use nose_normalize::{FileReferents, ValueDag, VgSinkKind};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CHANGED_SITES_PER_FAMILY: usize = 16;
const MAX_SIBLINGS_PER_FAMILY: usize = 16;
const MAX_TARGETS_PER_FAMILY: usize = 64;
const MAX_UNITS_PER_FILE: usize = 512;
const MAX_NODES_PER_UNIT: usize = 2_048;

const CAPS: SemanticWitnessCaps = SemanticWitnessCaps {
    max_files: MAX_FILES,
    max_file_bytes: MAX_FILE_BYTES,
    max_changed_sites_per_family: MAX_CHANGED_SITES_PER_FAMILY,
    max_siblings_per_family: MAX_SIBLINGS_PER_FAMILY,
    max_targets_per_family: MAX_TARGETS_PER_FAMILY,
    max_units_per_file: MAX_UNITS_PER_FILE,
    max_nodes_per_unit: MAX_NODES_PER_UNIT,
};

pub(super) fn enrich_semantic_change_witnesses(
    flagged: &mut [Divergence],
    base_root: &Path,
    current_root: &Path,
    base_changed: &HashMap<String, Vec<(u32, u32)>>,
    current_changed: &HashMap<String, Vec<(u32, u32)>>,
    diff_entries: &[DiffEntry],
    opts: &nose_detect::DetectOptions,
) {
    let timed = std::env::var_os("NOSE_TIME").is_some();
    let mut family_elapsed = Duration::ZERO;
    let mut target_elapsed = Duration::ZERO;
    let mut builder = WitnessBuilder::new(
        base_root,
        current_root,
        base_changed,
        current_changed,
        diff_entries,
        opts,
    );
    for divergence in flagged
        .iter_mut()
        .filter(|d| d.lane == DivergenceLane::BaseDivergence)
    {
        let family_started = Instant::now();
        let siblings = divergence
            .not_updated
            .iter()
            .take(MAX_SIBLINGS_PER_FAMILY)
            .cloned()
            .collect::<Vec<_>>();
        for (index, site) in divergence.changed.iter_mut().enumerate() {
            site.semantic_change = Some(if index < MAX_CHANGED_SITES_PER_FAMILY {
                builder.witness(site, &siblings)
            } else {
                unavailable(
                    SemanticProjectionStatus::CapExceeded,
                    SemanticProjectionStatus::NotAttempted,
                    vec![SemanticWitnessCaveat::Truncated],
                )
            });
        }
        family_elapsed += family_started.elapsed();
        let target_started = Instant::now();
        for (index, target) in divergence.targets.iter_mut().enumerate() {
            let reusable = match divergence.not_updated.as_slice() {
                [only_sibling] if same_semantic_site(only_sibling, &target.skipped) => divergence
                    .changed
                    .iter()
                    .find(|site| same_semantic_site(site, &target.changed))
                    .and_then(|site| site.semantic_change.clone()),
                _ => None,
            };
            target.changed.semantic_change = Some(if let Some(witness) = reusable {
                // With exactly one identical skipped sibling, the family-level
                // mapping and target-local mapping have the same proof domain.
                witness
            } else if index < MAX_TARGETS_PER_FAMILY {
                builder.witness(&target.changed, std::slice::from_ref(&target.skipped))
            } else {
                unavailable(
                    SemanticProjectionStatus::CapExceeded,
                    SemanticProjectionStatus::NotAttempted,
                    vec![SemanticWitnessCaveat::Truncated],
                )
            });
        }
        target_elapsed += target_started.elapsed();
    }
    if timed {
        eprintln!(
            "  [time] semantic-family {:>7.1}ms",
            family_elapsed.as_secs_f64() * 1e3
        );
        eprintln!(
            "  [time] semantic-target {:>7.1}ms",
            target_elapsed.as_secs_f64() * 1e3
        );
    }
}

fn same_semantic_site(left: &Site, right: &Site) -> bool {
    left.file == right.file
        && left.start_line == right.start_line
        && left.end_line == right.end_line
        && left.lang == right.lang
        && left.kind == right.kind
        && left.name == right.name
        && left.is_fragment == right.is_fragment
        && left.fragment_kind == right.fragment_kind
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Tree {
    Base,
    Current,
}

enum LoadState {
    Ready(Box<FileProjection>),
    Failed(SemanticProjectionStatus),
}

struct FileProjection {
    interner: Interner,
    normalized: nose_il::Il,
    units: Vec<UnitSkeleton>,
}

#[derive(Clone)]
struct UnitSkeleton {
    root: NodeId,
    kind: UnitKind,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
}

#[derive(Clone)]
struct UnitProjection {
    kind: UnitKind,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
    exact_safe: bool,
    dag: ValueDag,
    truncated: bool,
    unresolved_referent: bool,
}

#[derive(Clone)]
struct ProjectionAttempt {
    status: SemanticProjectionStatus,
    alignment: SemanticAlignment,
    unit: Option<UnitProjection>,
}

impl ProjectionAttempt {
    fn failed(status: SemanticProjectionStatus) -> Self {
        Self {
            status,
            alignment: SemanticAlignment::None,
            unit: None,
        }
    }
}

struct WitnessBuilder<'a> {
    base_root: &'a Path,
    current_root: &'a Path,
    base_changed: &'a HashMap<String, Vec<(u32, u32)>>,
    current_changed: &'a HashMap<String, Vec<(u32, u32)>>,
    diff_entries: &'a [DiffEntry],
    opts: nose_detect::DetectOptions,
    files: HashMap<(Tree, String), LoadState>,
    projections: HashMap<(Tree, String, NodeId), UnitProjection>,
    prepared: HashMap<String, PreparedChange>,
    sibling_nodes: HashMap<String, Vec<u64>>,
}

struct UnavailableChange {
    base_projection: SemanticProjectionStatus,
    current_projection: SemanticProjectionStatus,
    caveats: Vec<SemanticWitnessCaveat>,
}

impl UnavailableChange {
    fn into_witness(self) -> SemanticChangeWitness {
        unavailable(self.base_projection, self.current_projection, self.caveats)
    }
}

impl<'a> WitnessBuilder<'a> {
    fn new(
        base_root: &'a Path,
        current_root: &'a Path,
        base_changed: &'a HashMap<String, Vec<(u32, u32)>>,
        current_changed: &'a HashMap<String, Vec<(u32, u32)>>,
        diff_entries: &'a [DiffEntry],
        opts: &nose_detect::DetectOptions,
    ) -> Self {
        let mut opts = *opts;
        // A current unit may shrink below the query's candidate floor after an edit. The
        // witness still needs to align it, so extraction here has no size floor.
        opts.min_lines = 0;
        opts.min_tokens = 0;
        opts.contiguous_min_lines = 0;
        opts.contiguous_min_tokens = 0;
        Self {
            base_root,
            current_root,
            base_changed,
            current_changed,
            diff_entries,
            opts,
            files: HashMap::new(),
            projections: HashMap::new(),
            prepared: HashMap::new(),
            sibling_nodes: HashMap::new(),
        }
    }

    fn witness(&mut self, site: &Site, siblings: &[Site]) -> SemanticChangeWitness {
        let key = semantic_site_key(site);
        let prepared = if let Some(prepared) = self.prepared.get(&key).cloned() {
            prepared
        } else {
            let prepared = match self.prepare_change(site) {
                Ok(prepared) => prepared,
                Err(unavailable) => return unavailable.into_witness(),
            };
            self.prepared.insert(key, prepared.clone());
            prepared
        };
        let sibling_hashes = self.sibling_hashes(siblings);
        finish_witness(&prepared, &sibling_hashes)
    }

    fn prepare_change(&mut self, site: &Site) -> Result<PreparedChange, UnavailableChange> {
        if site.is_fragment {
            return Err(UnavailableChange {
                base_projection: SemanticProjectionStatus::Unsupported,
                current_projection: SemanticProjectionStatus::NotAttempted,
                caveats: vec![SemanticWitnessCaveat::FragmentUnsupported],
            });
        }
        let base = self.project_base(site);
        let Some(base_unit) = base.unit else {
            return Err(UnavailableChange {
                base_projection: base.status,
                current_projection: SemanticProjectionStatus::NotAttempted,
                caveats: caveat_for_projection(base.status, true),
            });
        };

        let Some(current_path) = self.current_path(&site.file) else {
            return Err(UnavailableChange {
                base_projection: base.status,
                current_projection: SemanticProjectionStatus::Missing,
                caveats: vec![SemanticWitnessCaveat::MissingCurrentUnit],
            });
        };
        let current_ranges = self
            .current_changed
            .get(&current_path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let current = self.project_current(&base_unit, &current_path, current_ranges);
        let Some(current_unit) = current.unit else {
            let mut caveats = caveat_for_projection(current.status, false);
            if caveats.is_empty() {
                caveats.push(SemanticWitnessCaveat::MissingCurrentUnit);
            }
            return Err(UnavailableChange {
                base_projection: base.status,
                current_projection: current.status,
                caveats,
            });
        };

        let base_ranges = self
            .base_changed
            .get(&site.file)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let analysis = analyze_change(&base_unit, &current_unit, base_ranges, current_ranges);
        Ok(PreparedChange {
            analysis,
            alignment: current.alignment,
            base_exact_safe: base_unit.exact_safe,
            current_exact_safe: current_unit.exact_safe,
            base_truncated: base_unit.truncated,
            current_truncated: current_unit.truncated,
            unresolved_referent: base_unit.unresolved_referent || current_unit.unresolved_referent,
        })
    }

    fn current_path(&self, base_path: &str) -> Option<String> {
        self.diff_entries
            .iter()
            .find(|entry| entry.old_path.as_deref() == Some(base_path))
            .map(|entry| entry.new_path.clone())
            .unwrap_or_else(|| Some(base_path.to_string()))
    }

    fn project_base(&mut self, site: &Site) -> ProjectionAttempt {
        // Detector-added block roots are not frontend units in the normalized IL. Without
        // an enclosing frontend unit there is nothing this projection can align, so avoid
        // normalizing a potentially large file only to return the same `unit-missing` result.
        if site.kind == UnitKind::Block && site.enclosing_unit.is_none() {
            return ProjectionAttempt::failed(SemanticProjectionStatus::UnitMissing);
        }
        let selected = {
            let file = match self.load_file(Tree::Base, &site.file) {
                Ok(file) => file,
                Err(status) => return ProjectionAttempt::failed(status),
            };
            let mut candidates = file
                .units
                .iter()
                .filter(|unit| unit_matches_site(unit, site, true))
                .cloned()
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                if let Some(enclosing) = &site.enclosing_unit {
                    candidates = file
                        .units
                        .iter()
                        .filter(|unit| unit_matches_enclosing(unit, enclosing))
                        .cloned()
                        .collect();
                }
            }
            match candidates.as_slice() {
                [unit] => Ok(unit.clone()),
                [] => Err(SemanticProjectionStatus::UnitMissing),
                _ => Err(SemanticProjectionStatus::AmbiguousUnit),
            }
        };
        match selected {
            Ok(unit) => {
                self.project_selected(Tree::Base, &site.file, unit, SemanticAlignment::ExactSpan)
            }
            Err(status) => ProjectionAttempt::failed(status),
        }
    }

    fn project_current(
        &mut self,
        base: &UnitProjection,
        current_path: &str,
        changed_ranges: &[(u32, u32)],
    ) -> ProjectionAttempt {
        let selected = {
            let file = match self.load_file(Tree::Current, current_path) {
                Ok(file) => file,
                Err(status) => return ProjectionAttempt::failed(status),
            };
            let same_kind = file
                .units
                .iter()
                .filter(|unit| unit.kind == base.kind)
                .cloned()
                .collect::<Vec<_>>();
            let exact = same_kind
                .iter()
                .filter(|unit| {
                    unit.name == base.name
                        && unit.start_line == base.start_line
                        && unit.end_line == base.end_line
                })
                .cloned()
                .collect::<Vec<_>>();
            if let [unit] = exact.as_slice() {
                Ok((unit.clone(), SemanticAlignment::ExactSpan))
            } else if let Some(name) = base.name.as_deref() {
                let named = same_kind
                    .iter()
                    .filter(|unit| unit.name.as_deref() == Some(name))
                    .cloned()
                    .collect::<Vec<_>>();
                if let [unit] = named.as_slice() {
                    Ok((unit.clone(), SemanticAlignment::StableName))
                } else {
                    select_current_by_change_or_distance(&same_kind, base, changed_ranges)
                }
            } else {
                select_current_by_change_or_distance(&same_kind, base, changed_ranges)
            }
        };
        match selected {
            Ok((unit, alignment)) => {
                self.project_selected(Tree::Current, current_path, unit, alignment)
            }
            Err(status) => ProjectionAttempt::failed(status),
        }
    }

    fn project_selected(
        &mut self,
        tree: Tree,
        relative_path: &str,
        unit: UnitSkeleton,
        alignment: SemanticAlignment,
    ) -> ProjectionAttempt {
        let key = (tree, relative_path.to_string(), unit.root);
        if let Some(projection) = self.projections.get(&key).cloned() {
            return projected(projection, alignment);
        }
        let projection = {
            let file = match self.load_file(tree, relative_path) {
                Ok(file) => file,
                Err(status) => return ProjectionAttempt::failed(status),
            };
            project_unit(file, &unit)
        };
        self.projections.insert(key, projection.clone());
        projected(projection, alignment)
    }

    fn sibling_hashes(&mut self, siblings: &[Site]) -> SharedHashes {
        let mut hashes = BTreeMap::new();
        let mut units_checked = 0;
        for sibling in siblings {
            if sibling.is_fragment {
                continue;
            }
            let key = semantic_site_key(sibling);
            let node_hashes = if let Some(node_hashes) = self.sibling_nodes.get(&key).cloned() {
                Some(node_hashes)
            } else {
                self.project_base(sibling).unit.map(|unit| {
                    let node_hashes = node_hashes(&unit.dag);
                    self.sibling_nodes.insert(key, node_hashes.clone());
                    node_hashes
                })
            };
            if let Some(node_hashes) = node_hashes {
                units_checked += 1;
                for hash in node_hashes {
                    *hashes.entry(hash).or_insert(0usize) += 1;
                }
            }
        }
        SharedHashes {
            hashes,
            units_checked,
        }
    }

    fn load_file(
        &mut self,
        tree: Tree,
        relative_path: &str,
    ) -> Result<&FileProjection, SemanticProjectionStatus> {
        let key = (tree, relative_path.to_string());
        if !self.files.contains_key(&key) {
            if self.files.len() >= MAX_FILES {
                return Err(SemanticProjectionStatus::CapExceeded);
            }
            let root = match tree {
                Tree::Base => self.base_root,
                Tree::Current => self.current_root,
            };
            let state = project_file(&root.join(relative_path), relative_path, &self.opts);
            self.files.insert(key.clone(), state);
        }
        match self.files.get(&key).expect("file projection was inserted") {
            LoadState::Ready(file) => Ok(file.as_ref()),
            LoadState::Failed(status) => Err(*status),
        }
    }
}

fn semantic_site_key(site: &Site) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{:?}\0{}\0{}\0{:?}",
        site.file,
        site.lang,
        site.start_line,
        site.end_line,
        site.kind,
        site.name.as_deref().unwrap_or_default(),
        site.is_fragment,
        site.fragment_kind,
    )
}

struct SharedHashes {
    hashes: BTreeMap<u64, usize>,
    units_checked: usize,
}

impl SharedHashes {
    fn contains_key(&self, hash: &u64) -> bool {
        self.hashes.contains_key(hash)
    }
}

#[derive(Clone)]
struct SemanticAnalysis {
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

#[derive(Clone)]
struct PreparedChange {
    analysis: SemanticAnalysis,
    alignment: SemanticAlignment,
    base_exact_safe: bool,
    current_exact_safe: bool,
    base_truncated: bool,
    current_truncated: bool,
    unresolved_referent: bool,
}

fn finish_witness(
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

fn analyze_change(
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

fn project_file(
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
            name,
            start_line: span.start_line,
            end_line: span.end_line,
        });
    }
    LoadState::Ready(Box::new(FileProjection {
        interner,
        normalized,
        units,
    }))
}

fn project_unit(file: &FileProjection, unit: &UnitSkeleton) -> UnitProjection {
    let span = file.normalized.node(unit.root).span;
    let exact_safe =
        nose_detect::exact_safe_roots_by_span(&file.normalized, &file.interner, &[unit.root])
            .get(&(span.start_byte, span.end_byte))
            .copied()
            .unwrap_or(false);
    let referents = FileReferents::new(&file.normalized, &file.interner);
    let dag = nose_normalize::value_dag(
        &file.normalized,
        unit.root,
        &file.interner,
        None,
        &referents,
    );
    let truncated = dag.nodes.len() > MAX_NODES_PER_UNIT;
    let unresolved_referent = dag
        .referents
        .iter()
        .any(|referent| referent.referent.is_none());
    UnitProjection {
        kind: unit.kind,
        name: unit.name.clone(),
        start_line: unit.start_line,
        end_line: unit.end_line,
        exact_safe,
        dag,
        truncated,
        unresolved_referent,
    }
}

fn unit_matches_site(unit: &UnitSkeleton, site: &Site, require_span: bool) -> bool {
    unit.kind == site.kind
        && (!require_span || (unit.start_line == site.start_line && unit.end_line == site.end_line))
        && match site.name.as_deref() {
            Some(name) => unit.name.as_deref() == Some(name),
            None => true,
        }
}

fn unit_matches_enclosing(unit: &UnitSkeleton, enclosing: &EnclosingUnit) -> bool {
    unit.kind == enclosing.kind
        && unit.start_line == enclosing.start_line
        && unit.end_line == enclosing.end_line
        && match enclosing.name.as_deref() {
            Some(name) => unit.name.as_deref() == Some(name),
            None => true,
        }
}

fn projected(unit: UnitProjection, alignment: SemanticAlignment) -> ProjectionAttempt {
    ProjectionAttempt {
        status: SemanticProjectionStatus::Ok,
        alignment,
        unit: Some(unit),
    }
}

fn select_current_by_change_or_distance(
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

fn node_hashes(dag: &ValueDag) -> Vec<u64> {
    let mut hashes = dag
        .nodes
        .iter()
        .take(MAX_NODES_PER_UNIT)
        .map(|node| node.hash)
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    hashes
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SinkSignature {
    kind: SemanticSinkKind,
    hash: u64,
    effect_ord: Option<u32>,
}

fn sink_signatures(dag: &ValueDag) -> Vec<SinkSignature> {
    let mut signatures = dag
        .sinks
        .iter()
        .take(MAX_NODES_PER_UNIT)
        .filter_map(|sink| {
            Some(SinkSignature {
                kind: semantic_sink_kind(sink.kind),
                hash: dag.nodes.get(sink.value as usize)?.hash,
                effect_ord: sink.effect_ord,
            })
        })
        .collect::<Vec<_>>();
    signatures.sort();
    signatures
}

fn semantic_sink_kind(kind: VgSinkKind) -> SemanticSinkKind {
    match kind {
        VgSinkKind::Return => SemanticSinkKind::Return,
        VgSinkKind::Cond => SemanticSinkKind::Cond,
        VgSinkKind::Effect => SemanticSinkKind::Effect,
        VgSinkKind::Break => SemanticSinkKind::Break,
        VgSinkKind::Throw => SemanticSinkKind::Throw,
    }
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

fn sink_deltas(before: &[SinkSignature], after: &[SinkSignature]) -> Vec<SemanticSinkDelta> {
    let mut kinds = BTreeMap::<SemanticSinkKind, (Vec<SinkSignature>, Vec<SinkSignature>)>::new();
    for &sink in before {
        kinds.entry(sink.kind).or_default().0.push(sink);
    }
    for &sink in after {
        kinds.entry(sink.kind).or_default().1.push(sink);
    }
    kinds
        .into_iter()
        .filter_map(|(kind, (before, after))| {
            let removed = multiset_removed(&before, &after);
            let inserted = multiset_removed(&after, &before);
            (removed > 0 || inserted > 0).then_some(SemanticSinkDelta {
                kind,
                removed,
                inserted,
            })
        })
        .collect()
}

fn caveat_for_projection(
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

fn unavailable(
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
