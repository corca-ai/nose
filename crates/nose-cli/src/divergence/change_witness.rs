//! Bounded base-to-current semantic change witnesses for already-flagged divergences.
//!
//! This module is intentionally downstream of candidate detection. It reads only the
//! changed candidate files and a capped set of their base siblings; it never performs a
//! second repository discovery. The evidence is advisory in divergent-edit v2.

mod analysis;
mod variant_projection;

use self::analysis::{
    analyze_change, caveat_for_projection, finish_witness, node_hashes, prepare_file_projection,
    project_file, project_resolved_file, project_unit, projected,
    select_current_by_change_or_distance, unavailable, unit_matches_enclosing, unit_matches_site,
    PreparedChange, SharedHashes,
};
use super::git::DiffEntry;
use super::*;
use crate::query_dataset::RetainedResolvedCorpus;
use crate::source_lines::FileLineCache;
use nose_il::{FileId, Interner, Lang, NodeId, UnitKind, UnitOrigin};
use nose_normalize::{FileReferents, ValueDag};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
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

pub(super) struct PreprojectedCurrentFiles(HashMap<String, LoadState>);

pub(super) struct SemanticWitnessInputs<'a> {
    pub(super) base_root: &'a Path,
    pub(super) current_root: &'a Path,
    pub(super) base_changed: &'a HashMap<String, Vec<(u32, u32)>>,
    pub(super) current_changed: &'a HashMap<String, Vec<(u32, u32)>>,
    pub(super) diff_entries: &'a [DiffEntry],
    pub(super) opts: &'a nose_detect::DetectOptions,
    pub(super) retained_base: Option<RetainedResolvedCorpus>,
    pub(super) preprojected_current: PreprojectedCurrentFiles,
}

pub(super) fn preproject_current_files(
    current_root: &Path,
    changed: &HashMap<String, Vec<(u32, u32)>>,
    opts: &nose_detect::DetectOptions,
) -> PreprojectedCurrentFiles {
    let mut paths = changed.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    let files = paths
        .into_iter()
        .take(MAX_FILES)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|path| {
            let mut state = project_file(&current_root.join(&path), &path, opts);
            prepare_file_projection(
                &mut state,
                changed.get(&path).map(Vec::as_slice).unwrap_or(&[]),
            );
            (path, state)
        })
        .collect();
    PreprojectedCurrentFiles(files)
}

pub(super) fn enrich_semantic_change_witnesses(
    flagged: &mut [Divergence],
    inputs: SemanticWitnessInputs<'_>,
) {
    let timed = std::env::var_os("NOSE_TIME").is_some();
    let mut family_elapsed = Duration::ZERO;
    let mut target_elapsed = Duration::ZERO;
    let mut builder = WitnessBuilder::new(inputs);
    builder.preload_files(flagged);
    builder.preload_variant_sources(flagged);
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
            builder.variant_evidence(
                &target.changed,
                &target.skipped,
                &mut target.variant_evidence,
            );
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
    known_exact_safety: HashMap<(UnitKind, u32, u32), bool>,
    value_context: Option<nose_normalize::ValueFingerprintContext>,
    exact_safety: OnceLock<HashMap<(u32, u32), bool>>,
    referents: OnceLock<FileReferents>,
    prepared_units: HashMap<NodeId, Arc<UnitProjection>>,
}

#[derive(Clone)]
struct UnitSkeleton {
    root: NodeId,
    kind: UnitKind,
    origin: UnitOrigin,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
}

struct UnitProjection {
    kind: UnitKind,
    origin: UnitOrigin,
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
    unit: Option<Arc<UnitProjection>>,
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
    retained_base_interner: Option<Interner>,
    retained_base_files: Vec<nose_il::Il>,
    preprojected_current_files: HashMap<String, LoadState>,
    files: HashMap<(Tree, String), LoadState>,
    projections: HashMap<(Tree, String, NodeId), Arc<UnitProjection>>,
    prepared: HashMap<String, PreparedChange>,
    sibling_nodes: HashMap<String, Vec<u64>>,
    source_lines: FileLineCache,
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
    fn new(inputs: SemanticWitnessInputs<'a>) -> Self {
        let SemanticWitnessInputs {
            base_root,
            current_root,
            base_changed,
            current_changed,
            diff_entries,
            opts,
            retained_base,
            preprojected_current,
        } = inputs;
        let mut opts = *opts;
        // A current unit may shrink below the query's candidate floor after an edit. The
        // witness still needs to align it, so extraction here has no size floor.
        opts.min_lines = 0;
        opts.min_tokens = 0;
        opts.contiguous_min_lines = 0;
        opts.contiguous_min_tokens = 0;
        let (retained_base_interner, retained_base_files) = retained_base
            .map(|retained| {
                let nose_il::Corpus { interner, files } = retained.corpus;
                (Some(interner), files)
            })
            .unwrap_or_default();
        Self {
            base_root,
            current_root,
            base_changed,
            current_changed,
            diff_entries,
            opts,
            retained_base_interner,
            retained_base_files,
            preprojected_current_files: preprojected_current.0,
            files: HashMap::new(),
            projections: HashMap::new(),
            prepared: HashMap::new(),
            sibling_nodes: HashMap::new(),
            source_lines: FileLineCache::default(),
        }
    }

    fn witness(&mut self, site: &Site, siblings: &[Site]) -> SemanticChangeWitness {
        let key = semantic_site_key(site);
        if !self.prepared.contains_key(&key) {
            let prepared = match self.prepare_change(site) {
                Ok(prepared) => prepared,
                Err(unavailable) => return unavailable.into_witness(),
            };
            self.prepared.insert(key.clone(), prepared);
        }
        let sibling_hashes = self.sibling_hashes(siblings);
        finish_witness(
            self.prepared
                .get(&key)
                .expect("prepared change was inserted"),
            &sibling_hashes,
        )
    }

    /// Lower independent witness files concurrently. The lazy path below still owns
    /// cap behavior; preloading is used only when the complete unique set fits under
    /// the same cap, so result availability and cap ordering cannot change.
    fn preload_files(&mut self, flagged: &[Divergence]) {
        let mut keys = Vec::new();
        for divergence in flagged
            .iter()
            .filter(|divergence| divergence.lane == DivergenceLane::BaseDivergence)
        {
            let base_sites = divergence
                .changed
                .iter()
                .chain(&divergence.not_updated)
                .chain(
                    divergence
                        .targets
                        .iter()
                        .flat_map(|target| [&target.changed, &target.skipped]),
                );
            for site in base_sites {
                if projection_may_load(site) {
                    keys.push((Tree::Base, site.file.clone()));
                }
            }
            for site in divergence
                .changed
                .iter()
                .chain(divergence.targets.iter().map(|target| &target.changed))
            {
                if projection_may_load(site) {
                    if let Some(path) = self.current_path(&site.file) {
                        keys.push((Tree::Current, path));
                    }
                }
            }
        }
        keys.sort_by(|left, right| {
            tree_order(left.0)
                .cmp(&tree_order(right.0))
                .then_with(|| left.1.cmp(&right.1))
        });
        keys.dedup();
        if keys.len() > MAX_FILES {
            return;
        }
        let base_root = self.base_root;
        let current_root = self.current_root;
        let opts = self.opts;
        let retained_base_interner = self.retained_base_interner.clone();
        let jobs = keys
            .into_iter()
            .map(|(tree, path)| {
                let resolved = (tree == Tree::Base)
                    .then(|| self.take_retained_base_file(&path))
                    .flatten();
                let preprojected = (tree == Tree::Current)
                    .then(|| self.preprojected_current_files.remove(&path))
                    .flatten();
                (tree, path, resolved, preprojected)
            })
            .collect::<Vec<_>>();
        let loaded = jobs
            .into_par_iter()
            .map(|(tree, path, resolved, preprojected)| {
                let root = match tree {
                    Tree::Base => base_root,
                    Tree::Current => current_root,
                };
                let state = match (preprojected, resolved, retained_base_interner.as_ref()) {
                    (Some(state), _, _) => state,
                    (None, Some(raw), Some(interner)) => {
                        project_resolved_file(&root.join(&path), interner.clone(), raw, &opts)
                    }
                    _ => project_file(&root.join(&path), &path, &opts),
                };
                ((tree, path), state)
            })
            .collect::<Vec<_>>();
        self.files.extend(loaded);
        self.preload_base_unit_projections(flagged);
    }

    fn preload_base_unit_projections(&mut self, flagged: &[Divergence]) {
        let mut units_by_path = BTreeMap::<String, Vec<UnitSkeleton>>::new();
        for divergence in flagged
            .iter()
            .filter(|divergence| divergence.lane == DivergenceLane::BaseDivergence)
        {
            for site in divergence
                .changed
                .iter()
                .chain(&divergence.not_updated)
                .chain(
                    divergence
                        .targets
                        .iter()
                        .flat_map(|target| [&target.changed, &target.skipped]),
                )
            {
                if !projection_may_load(site) {
                    continue;
                }
                let key = (Tree::Base, site.file.clone());
                let Some(LoadState::Ready(file)) = self.files.get(&key) else {
                    continue;
                };
                if let Ok(unit) = select_base_unit(file, site) {
                    units_by_path
                        .entry(site.file.clone())
                        .or_default()
                        .push(unit);
                }
            }
        }
        for units in units_by_path.values_mut() {
            units.sort_by_key(|unit| unit.root.0);
            units.dedup_by_key(|unit| unit.root);
        }
        let jobs = units_by_path
            .into_iter()
            .filter_map(|(path, units)| {
                let key = (Tree::Base, path);
                self.files.remove(&key).map(|state| (key, state, units))
            })
            .collect::<Vec<_>>();
        let loaded = jobs
            .into_par_iter()
            .map(|(key, state, units)| {
                let projections = match &state {
                    LoadState::Ready(file) => units
                        .into_iter()
                        .map(|unit| {
                            (
                                (key.0, key.1.clone(), unit.root),
                                Arc::new(project_unit(file, &unit)),
                            )
                        })
                        .collect(),
                    LoadState::Failed(_) => Vec::new(),
                };
                (key, state, projections)
            })
            .collect::<Vec<_>>();
        for (key, state, projections) in loaded {
            self.files.insert(key, state);
            self.projections.extend(projections);
        }
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

    fn preload_variant_sources(&mut self, flagged: &[Divergence]) {
        let mut paths = Vec::new();
        for target in flagged
            .iter()
            .filter(|divergence| divergence.lane == DivergenceLane::BaseDivergence)
            .flat_map(|divergence| &divergence.targets)
        {
            if let Some(current_path) = self.current_path(&target.changed.file) {
                paths.push(
                    self.current_root
                        .join(current_path)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            paths.push(
                self.base_root
                    .join(&target.skipped.file)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        self.source_lines.preload(paths);
    }

    fn current_path(&self, base_path: &str) -> Option<String> {
        self.diff_entries
            .iter()
            .find(|entry| entry.old_path.as_deref() == Some(base_path))
            .map(|entry| entry.new_path.clone())
            .unwrap_or_else(|| Some(base_path.to_string()))
    }

    fn project_base(&mut self, site: &Site) -> ProjectionAttempt {
        let selected = self.base_unit(site);
        match selected {
            Ok(unit) => {
                self.project_selected(Tree::Base, &site.file, unit, SemanticAlignment::ExactSpan)
            }
            Err(status) => ProjectionAttempt::failed(status),
        }
    }

    fn base_unit(&mut self, site: &Site) -> Result<UnitSkeleton, SemanticProjectionStatus> {
        if site.kind == UnitKind::Block && site.enclosing_unit.is_none() {
            return Err(SemanticProjectionStatus::UnitMissing);
        }
        let file = self.load_file(Tree::Base, &site.file)?;
        select_base_unit(file, site)
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
        if let Some(projection) = self.projections.get(&key) {
            return projected(Arc::clone(projection), alignment);
        }
        let projection = {
            let file = match self.load_file(tree, relative_path) {
                Ok(file) => file,
                Err(status) => return ProjectionAttempt::failed(status),
            };
            file.prepared_units
                .get(&unit.root)
                .cloned()
                .unwrap_or_else(|| Arc::new(project_unit(file, &unit)))
        };
        self.projections.insert(key, Arc::clone(&projection));
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
            if !self.sibling_nodes.contains_key(&key) {
                if let Some(unit) = self.project_base(sibling).unit {
                    let node_hashes = node_hashes(&unit.dag);
                    self.sibling_nodes.insert(key.clone(), node_hashes);
                }
            }
            if let Some(node_hashes) = self.sibling_nodes.get(&key) {
                units_checked += 1;
                for hash in node_hashes {
                    *hashes.entry(*hash).or_insert(0usize) += 1;
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
            let state = if tree == Tree::Current {
                self.preprojected_current_files
                    .remove(relative_path)
                    .unwrap_or_else(|| {
                        project_file(&root.join(relative_path), relative_path, &self.opts)
                    })
            } else {
                match (
                    self.take_retained_base_file(relative_path),
                    self.retained_base_interner.as_ref(),
                ) {
                    (Some(raw), Some(interner)) => project_resolved_file(
                        &root.join(relative_path),
                        interner.clone(),
                        raw,
                        &self.opts,
                    ),
                    _ => project_file(&root.join(relative_path), relative_path, &self.opts),
                }
            };
            self.files.insert(key.clone(), state);
        }
        match self.files.get(&key).expect("file projection was inserted") {
            LoadState::Ready(file) => Ok(file.as_ref()),
            LoadState::Failed(status) => Err(*status),
        }
    }

    fn take_retained_base_file(&mut self, relative_path: &str) -> Option<nose_il::Il> {
        let expected = self.base_root.join(relative_path);
        let index = self.retained_base_files.iter().position(|file| {
            let path = Path::new(&file.meta.path);
            path == expected || path == Path::new(relative_path)
        })?;
        Some(self.retained_base_files.swap_remove(index))
    }
}

fn projection_may_load(site: &Site) -> bool {
    !(site.is_fragment || site.kind == UnitKind::Block && site.enclosing_unit.is_none())
}

fn select_base_unit(
    file: &FileProjection,
    site: &Site,
) -> Result<UnitSkeleton, SemanticProjectionStatus> {
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
}

fn tree_order(tree: Tree) -> u8 {
    match tree {
        Tree::Base => 0,
        Tree::Current => 1,
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
