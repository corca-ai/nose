use super::*;
use rayon::prelude::*;

impl WitnessBuilder<'_> {
    /// Lower independent witness files concurrently. The lazy path below still owns
    /// cap behavior; preloading is used only when the complete unique set fits under
    /// the same cap, so result availability and cap ordering cannot change.
    pub(super) fn preload_files(&mut self, flagged: &[Divergence]) {
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
                let normalized = (tree == Tree::Base)
                    .then(|| self.retained_base_files.remove(&path))
                    .flatten();
                let known_exact_safety = (tree == Tree::Base)
                    .then(|| self.retained_base_exact_safety.remove(&path))
                    .flatten()
                    .unwrap_or_default();
                let value_context = (tree == Tree::Base)
                    .then(|| self.retained_base_value_contexts.remove(&path))
                    .flatten();
                let preprojected = (tree == Tree::Current)
                    .then(|| self.preprojected_current_files.remove(&path))
                    .flatten();
                (
                    tree,
                    path,
                    normalized,
                    known_exact_safety,
                    value_context,
                    preprojected,
                )
            })
            .collect::<Vec<_>>();
        let loaded = jobs
            .into_par_iter()
            .map(
                |(tree, path, normalized, known_exact_safety, value_context, preprojected)| {
                    let root = match tree {
                        Tree::Base => base_root,
                        Tree::Current => current_root,
                    };
                    let state = match preprojected {
                        Some(state) => state,
                        None => match (normalized, retained_base_interner.as_ref()) {
                            (Some(normalized), Some(interner)) => project_normalized_file(
                                &root.join(&path),
                                interner.clone(),
                                normalized,
                                known_exact_safety,
                                value_context,
                            ),
                            _ => project_file(&root.join(&path), &path, &opts),
                        },
                    };
                    ((tree, path), state)
                },
            )
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

    pub(super) fn preload_variant_sources(&mut self, flagged: &[Divergence]) {
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

    pub(super) fn load_file(
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
            let state = if tree == Tree::Base {
                match (
                    self.retained_base_files.remove(relative_path),
                    self.retained_base_interner.as_ref(),
                ) {
                    (Some(normalized), Some(interner)) => project_normalized_file(
                        &root.join(relative_path),
                        interner.clone(),
                        normalized,
                        self.retained_base_exact_safety
                            .remove(relative_path)
                            .unwrap_or_default(),
                        self.retained_base_value_contexts.remove(relative_path),
                    ),
                    _ => project_file(&root.join(relative_path), relative_path, &self.opts),
                }
            } else {
                self.preprojected_current_files
                    .remove(relative_path)
                    .unwrap_or_else(|| {
                        project_file(&root.join(relative_path), relative_path, &self.opts)
                    })
            };
            self.files.insert(key.clone(), state);
        }
        match self.files.get(&key).expect("file projection was inserted") {
            LoadState::Ready(file) => Ok(file.as_ref()),
            LoadState::Failed(status) => Err(*status),
        }
    }
}

fn projection_may_load(site: &Site) -> bool {
    !(site.is_fragment || site.kind == UnitKind::Block && site.enclosing_unit.is_none())
}

fn tree_order(tree: Tree) -> u8 {
    match tree {
        Tree::Base => 0,
        Tree::Current => 1,
    }
}
