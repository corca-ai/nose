use super::bindings::{assignment_name, import_binding_proof};
use super::exports::{collect_literal_exports, LiteralExports};
use super::namespace_members::collect_namespace_member_analyses;
use super::snapshot::{snapshot_subtree, surface_fingerprint};
use super::{ExportedBinding, FileImportContext};
use nose_il::{Il, Interner, Lang};
use rustc_hash::FxHashMap;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ResolutionDependencyKind {
    ImportedBinding,
    ImportedNamespaceMember,
    UnknownBinding,
    UnknownNamespace,
    SwiftGlobalSentinel,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ResolutionDependency {
    pub kind: ResolutionDependencyKind,
    pub provider_file: Option<usize>,
    pub module_hash: Option<u64>,
    pub exported_hash: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct FileResolutionDependencySummary {
    pub export_digest: [u8; 32],
    pub resolution_digest: [u8; 32],
    pub dependencies: Vec<ResolutionDependency>,
    pub over_invalidated: bool,
    pub requires_resolution: bool,
}

#[derive(Clone, Debug)]
pub struct ResolutionDependencySummary {
    pub files: Vec<FileResolutionDependencySummary>,
    pub swift_global_digest: [u8; 32],
    pub swift_global_active: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BindingKey {
    file: usize,
    root: u32,
}

impl BindingKey {
    fn of(binding: &ExportedBinding) -> Self {
        Self {
            file: binding.file_idx,
            root: binding.rhs.0,
        }
    }
}

struct BindingNode {
    base: [u8; 32],
    edges: Vec<BindingEdge>,
}

#[derive(Clone, Copy)]
struct BindingEdge {
    module_hash: u64,
    exported_hash: u64,
    target: usize,
}

/// Describe exactly which corpus facts can change each resolved IL. Export
/// fingerprints contain only consumer-visible literal snapshots; dependency
/// cycles are collapsed before hashing, so traversal order cannot affect keys.
pub fn resolution_dependency_summary(
    files: &[Il],
    interner: &Interner,
) -> ResolutionDependencySummary {
    let contexts = files
        .iter()
        .map(|il| FileImportContext::new(il, interner))
        .collect::<Vec<_>>();
    let exports = collect_literal_exports(files, interner, &contexts);
    let (nodes, binding_indexes) = binding_graph(files, interner, &contexts, &exports);
    let binding_digests = binding_digests(&nodes);
    let (file_export_digests, file_has_exports) =
        file_export_digests(files.len(), &exports, &binding_indexes, &binding_digests);
    let language_catalogs = language_catalogs(files, &file_export_digests);
    let (swift_global_digest, swift_global_active) =
        crate::swift_cross_file_shadows::swift_global_dependency_state(files, interner);
    let namespace_analyses =
        collect_namespace_member_analyses(files, interner, &contexts, &exports);

    let summaries = files
        .iter()
        .enumerate()
        .map(|(file_idx, il)| {
            summarize_file(
                file_idx,
                il,
                files,
                interner,
                &contexts,
                &exports,
                &binding_indexes,
                &binding_digests,
                &file_export_digests,
                file_has_exports[file_idx],
                language_catalogs[&il.meta.lang],
                swift_global_digest,
                swift_global_active,
                &namespace_analyses[file_idx],
            )
        })
        .collect();
    ResolutionDependencySummary {
        files: summaries,
        swift_global_digest,
        swift_global_active,
    }
}

fn binding_graph(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: &LiteralExports,
) -> (Vec<BindingNode>, BTreeMap<BindingKey, usize>) {
    let mut bindings = BTreeMap::new();
    for record in exports.records() {
        bindings.insert(BindingKey::of(&record.binding), record.binding.clone());
    }
    let indexes = bindings
        .keys()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect::<BTreeMap<_, _>>();
    let nodes = bindings
        .into_values()
        .map(|binding| {
            let rhs = snapshot_subtree(&files[binding.file_idx], binding.rhs);
            let mut components = vec![surface_fingerprint(&rhs, interner).to_vec()];
            components.extend(
                binding
                    .deps
                    .iter()
                    .map(|snapshot| surface_fingerprint(snapshot, interner).to_vec()),
            );
            let refs = components.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let base = derive(b"nose.export-surface.base.v1", &refs);
            let mut edges = binding
                .dependency_keys
                .iter()
                .filter_map(|&(module_hash, exported_hash)| {
                    let target =
                        exports.get(contexts, binding.file_idx, module_hash, exported_hash)?;
                    let target = indexes.get(&BindingKey::of(target)).copied()?;
                    Some(BindingEdge {
                        module_hash,
                        exported_hash,
                        target,
                    })
                })
                .collect::<Vec<_>>();
            edges.sort_by_key(|edge| (edge.module_hash, edge.exported_hash, edge.target));
            BindingNode { base, edges }
        })
        .collect();
    (nodes, indexes)
}

fn binding_digests(nodes: &[BindingNode]) -> Vec<[u8; 32]> {
    let components = strongly_connected_components(nodes);
    let mut component_of = vec![0; nodes.len()];
    for (component, members) in components.iter().enumerate() {
        for &member in members {
            component_of[member] = component;
        }
    }
    let mut memo = vec![None; components.len()];
    fn component_digest(
        component: usize,
        components: &[Vec<usize>],
        component_of: &[usize],
        nodes: &[BindingNode],
        memo: &mut [Option<[u8; 32]>],
    ) -> [u8; 32] {
        if let Some(digest) = memo[component] {
            return digest;
        }
        let mut rows = Vec::new();
        for &member in &components[component] {
            rows.push(framed(&[nodes[member].base.as_slice()]));
            for edge in &nodes[member].edges {
                let target_component = component_of[edge.target];
                let target = if target_component == component {
                    nodes[edge.target].base
                } else {
                    component_digest(target_component, components, component_of, nodes, memo)
                };
                rows.push(framed(&[
                    nodes[member].base.as_slice(),
                    &edge.module_hash.to_be_bytes(),
                    &edge.exported_hash.to_be_bytes(),
                    target.as_slice(),
                ]));
            }
        }
        rows.sort();
        let refs = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let digest = derive(b"nose.export-surface.scc.v1", &refs);
        memo[component] = Some(digest);
        digest
    }

    let mut out = vec![[0; 32]; nodes.len()];
    for index in 0..nodes.len() {
        let component = component_of[index];
        let aggregate = component_digest(component, &components, &component_of, nodes, &mut memo);
        out[index] = derive(
            b"nose.export-surface.member.v1",
            &[nodes[index].base.as_slice(), aggregate.as_slice()],
        );
    }
    out
}

fn strongly_connected_components(nodes: &[BindingNode]) -> Vec<Vec<usize>> {
    struct Tarjan<'a> {
        nodes: &'a [BindingNode],
        next: usize,
        indexes: Vec<Option<usize>>,
        low: Vec<usize>,
        stack: Vec<usize>,
        on_stack: Vec<bool>,
        out: Vec<Vec<usize>>,
    }
    impl Tarjan<'_> {
        fn visit(&mut self, node: usize) {
            let index = self.next;
            self.next += 1;
            self.indexes[node] = Some(index);
            self.low[node] = index;
            self.stack.push(node);
            self.on_stack[node] = true;
            for edge in &self.nodes[node].edges {
                if self.indexes[edge.target].is_none() {
                    self.visit(edge.target);
                    self.low[node] = self.low[node].min(self.low[edge.target]);
                } else if self.on_stack[edge.target] {
                    self.low[node] = self.low[node].min(self.indexes[edge.target].unwrap());
                }
            }
            if self.low[node] != index {
                return;
            }
            let mut component = Vec::new();
            loop {
                let member = self.stack.pop().expect("SCC root must be on the stack");
                self.on_stack[member] = false;
                component.push(member);
                if member == node {
                    break;
                }
            }
            component.sort_unstable();
            self.out.push(component);
        }
    }
    let mut state = Tarjan {
        nodes,
        next: 0,
        indexes: vec![None; nodes.len()],
        low: vec![0; nodes.len()],
        stack: Vec::new(),
        on_stack: vec![false; nodes.len()],
        out: Vec::new(),
    };
    for node in 0..nodes.len() {
        if state.indexes[node].is_none() {
            state.visit(node);
        }
    }
    state.out
}

fn file_export_digests(
    file_count: usize,
    exports: &LiteralExports,
    indexes: &BTreeMap<BindingKey, usize>,
    binding_digests: &[[u8; 32]],
) -> (Vec<[u8; 32]>, Vec<bool>) {
    let mut rows = vec![Vec::new(); file_count];
    for (&(module_hash, exported_hash), binding) in exports.iter_keyed() {
        let digest = binding_digests[indexes[&BindingKey::of(binding)]];
        rows[binding.file_idx].push(framed(&[
            &module_hash.to_be_bytes(),
            &exported_hash.to_be_bytes(),
            digest.as_slice(),
        ]));
    }
    for reexport in exports.reexports() {
        rows[reexport.file_idx].push(framed(&[
            &reexport.local_exported_hash.to_be_bytes(),
            &reexport.target_module_hash.to_be_bytes(),
            &reexport.target_exported_hash.to_be_bytes(),
        ]));
    }
    let has_exports = rows.iter().map(|rows| !rows.is_empty()).collect();
    let digests = rows
        .into_iter()
        .map(|mut rows| {
            rows.sort();
            let refs = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
            derive(b"nose.file-export-surface.v1", &refs)
        })
        .collect();
    (digests, has_exports)
}

fn language_catalogs(files: &[Il], exports: &[[u8; 32]]) -> FxHashMap<Lang, [u8; 32]> {
    let mut rows: FxHashMap<Lang, Vec<Vec<u8>>> = FxHashMap::default();
    for (il, digest) in files.iter().zip(exports) {
        rows.entry(il.meta.lang).or_default().push(digest.to_vec());
    }
    rows.into_iter()
        .map(|(lang, mut rows)| {
            rows.sort();
            let refs = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
            (lang, derive(b"nose.language-export-catalog.v1", &refs))
        })
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn summarize_file(
    file_idx: usize,
    il: &Il,
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: &LiteralExports,
    indexes: &BTreeMap<BindingKey, usize>,
    binding_digests: &[[u8; 32]],
    file_exports: &[[u8; 32]],
    has_exports: bool,
    catalog: [u8; 32],
    swift_digest: [u8; 32],
    swift_active: bool,
    namespace_analysis: &super::namespace_members::NamespaceMemberAnalysis,
) -> FileResolutionDependencySummary {
    let top_level = contexts[file_idx].top_level.as_deref().unwrap_or_default();
    let mut dependencies = Vec::new();
    let mut hash_rows = Vec::new();
    for &stmt in top_level {
        if let Some(proof) = import_binding_proof(il, stmt) {
            let Some(local) = assignment_name(il, stmt) else {
                continue;
            };
            if contexts[file_idx]
                .binding_uses(il, interner)
                .binding_mutated(il, local, stmt)
            {
                continue;
            }
            let export = exports.get(contexts, file_idx, proof.module_hash, proof.exported_hash);
            if let Some(export) = export.filter(|export| {
                export.file_idx != file_idx && files[export.file_idx].meta.lang == il.meta.lang
            }) {
                let digest = binding_digests[indexes[&BindingKey::of(export)]];
                dependencies.push(ResolutionDependency {
                    kind: ResolutionDependencyKind::ImportedBinding,
                    provider_file: Some(export.file_idx),
                    module_hash: Some(proof.module_hash),
                    exported_hash: Some(proof.exported_hash),
                });
                hash_rows.push(framed(&[
                    b"binding",
                    &proof.module_hash.to_be_bytes(),
                    &proof.exported_hash.to_be_bytes(),
                    digest.as_slice(),
                ]));
            } else {
                dependencies.push(ResolutionDependency {
                    kind: ResolutionDependencyKind::UnknownBinding,
                    provider_file: None,
                    module_hash: Some(proof.module_hash),
                    exported_hash: Some(proof.exported_hash),
                });
                hash_rows.push(framed(&[
                    b"unknown-binding",
                    &proof.module_hash.to_be_bytes(),
                    &proof.exported_hash.to_be_bytes(),
                    catalog.as_slice(),
                ]));
            }
        }
    }
    for replacement in &namespace_analysis.replacements {
        let Some(export) = exports.get_exact(replacement.module_hash, replacement.exported_hash)
        else {
            continue;
        };
        let digest = binding_digests[indexes[&BindingKey::of(export)]];
        dependencies.push(ResolutionDependency {
            kind: ResolutionDependencyKind::ImportedNamespaceMember,
            provider_file: Some(replacement.provider_file_idx),
            module_hash: Some(replacement.module_hash),
            exported_hash: Some(replacement.exported_hash),
        });
        hash_rows.push(framed(&[
            b"namespace-member",
            &replacement.module_hash.to_be_bytes(),
            &replacement.exported_hash.to_be_bytes(),
            digest.as_slice(),
        ]));
    }
    for &(module_hash, exported_hash) in &namespace_analysis.unresolved {
        dependencies.push(ResolutionDependency {
            kind: ResolutionDependencyKind::UnknownNamespace,
            provider_file: None,
            module_hash: Some(module_hash),
            exported_hash: Some(exported_hash),
        });
        hash_rows.push(framed(&[
            b"unknown-namespace",
            &module_hash.to_be_bytes(),
            &exported_hash.to_be_bytes(),
            catalog.as_slice(),
        ]));
    }
    if il.meta.lang == Lang::Swift {
        dependencies.push(ResolutionDependency {
            kind: ResolutionDependencyKind::SwiftGlobalSentinel,
            provider_file: None,
            module_hash: None,
            exported_hash: None,
        });
        hash_rows.push(framed(&[b"swift-global", swift_digest.as_slice()]));
    }
    dependencies.sort();
    dependencies.dedup();
    hash_rows.push(file_exports[file_idx].to_vec());
    hash_rows.sort();
    let refs = hash_rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let requires_resolution = has_exports
        || dependencies.iter().any(|dependency| {
            matches!(
                dependency.kind,
                ResolutionDependencyKind::ImportedBinding
                    | ResolutionDependencyKind::ImportedNamespaceMember
            )
        })
        || il.meta.lang == Lang::Swift && swift_active;
    FileResolutionDependencySummary {
        export_digest: file_exports[file_idx],
        resolution_digest: derive(b"nose.file-resolution-context.v1", &refs),
        over_invalidated: dependencies.iter().any(|dependency| {
            matches!(
                dependency.kind,
                ResolutionDependencyKind::UnknownBinding
                    | ResolutionDependencyKind::UnknownNamespace
            )
        }),
        requires_resolution,
        dependencies,
    }
}

fn framed(components: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for component in components {
        out.extend_from_slice(&(component.len() as u64).to_be_bytes());
        out.extend_from_slice(component);
    }
    out
}

fn derive(domain: &[u8], components: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_be_bytes());
    hash.update(domain);
    hash.update(framed(components));
    hash.finalize().into()
}
