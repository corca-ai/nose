use nose_il::{Interner, NodeId};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

mod callee_identity;
mod hof;
mod runtime_boundary;
use callee_identity::callee_identity_missing_evidence;
use hof::hof_missing_evidence;
use runtime_boundary::runtime_boundary_missing_evidence_with_context;

#[derive(Clone, Default)]
pub(crate) struct AdmissionContext {
    python_local_modules: Vec<PythonLocalModule>,
    rust_local_runtime_roots_by_file: HashMap<String, HashSet<String>>,
    swift_visible_names: HashSet<String>,
}

#[derive(Clone)]
struct PythonLocalModule {
    name: String,
    import_root: String,
}

impl AdmissionContext {
    pub(crate) fn from_corpus(corpus: &nose_il::Corpus) -> Self {
        let mut context = AdmissionContext::default();
        for il in &corpus.files {
            match il.meta.lang {
                nose_il::Lang::Python => {
                    context
                        .python_local_modules
                        .extend(python_module_names_from_path(&il.meta.path));
                }
                nose_il::Lang::Rust => {
                    context.collect_rust_runtime_root_definitions(il, &corpus.interner);
                }
                nose_il::Lang::Swift => {
                    context.collect_swift_visible_names(il, &corpus.interner);
                }
                _ => {}
            }
        }
        context
    }

    pub(crate) fn python_module_is_local_for_file(&self, module: &str, file_path: &str) -> bool {
        let file_dir = parent_dir(file_path);
        self.python_local_modules.iter().any(|local| {
            local.name == module && path_is_same_or_ancestor(&local.import_root, &file_dir)
        })
    }

    pub(crate) fn rust_runtime_root_is_local_for_file(&self, root: &str, file_path: &str) -> bool {
        self.rust_local_runtime_roots_by_file
            .get(file_path)
            .is_some_and(|roots| roots.contains(root))
    }

    pub(crate) fn swift_name_is_visible(&self, name: &str) -> bool {
        self.swift_visible_names.contains(name)
    }

    fn collect_rust_runtime_root_definitions(&mut self, il: &nose_il::Il, interner: &Interner) {
        const RUNTIME_ROOTS: &[&str] = &[
            "tokio",
            "tokio_test",
            "async_std",
            "futures",
            "futures_util",
        ];
        for name in names_defined_in_il(il, interner) {
            let normalized_name = normalize_rust_raw_identifier_name(&name);
            if RUNTIME_ROOTS.contains(&normalized_name.as_str()) {
                self.rust_local_runtime_roots_by_file
                    .entry(il.meta.path.clone())
                    .or_default()
                    .insert(normalized_name);
            }
        }
    }

    fn collect_swift_visible_names(&mut self, il: &nose_il::Il, interner: &Interner) {
        self.swift_visible_names
            .extend(names_defined_in_il(il, interner));
    }
}

fn python_module_names_from_path(path: &str) -> Vec<PythonLocalModule> {
    let path = Path::new(path);
    if path.file_stem().and_then(|name| name.to_str()) == Some("__init__") {
        return path
            .parent()
            .and_then(|package| {
                Some(PythonLocalModule {
                    name: package.file_name()?.to_str()?.to_string(),
                    import_root: path_to_string(package.parent().unwrap_or_else(|| Path::new(""))),
                })
            })
            .map(|local| vec![local])
            .unwrap_or_default();
    }
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(|name| {
            vec![PythonLocalModule {
                name: name.to_string(),
                import_root: parent_dir(path),
            }]
        })
        .unwrap_or_default()
}

fn parent_dir(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .parent()
        .map(path_to_string)
        .unwrap_or_default()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn path_is_same_or_ancestor(ancestor: &str, path: &str) -> bool {
    ancestor.is_empty() || Path::new(path).starts_with(Path::new(ancestor))
}

fn names_defined_in_il(il: &nose_il::Il, interner: &Interner) -> HashSet<String> {
    let mut names = HashSet::new();
    for unit in &il.units {
        if let Some(name) = unit.name {
            names.insert(interner.resolve(name).to_string());
        }
    }
    for (idx, node) in il.nodes.iter().enumerate() {
        let node_id = NodeId(idx as u32);
        match node.kind {
            nose_il::NodeKind::Module | nose_il::NodeKind::Block | nose_il::NodeKind::Param => {
                if let Some(name) = node_exact_name(il, interner, node_id) {
                    names.insert(name.to_string());
                }
            }
            nose_il::NodeKind::Assign => {
                if let Some(lhs) = il.children(node_id).first().copied() {
                    if let Some(name) = node_exact_name(il, interner, lhs) {
                        names.insert(name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn normalize_rust_raw_identifier_name(name: &str) -> String {
    name.strip_prefix("r#").unwrap_or(name).to_string()
}

fn node_exact_name<'a>(il: &nose_il::Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    match il.node(node).payload {
        nose_il::Payload::Name(symbol) => Some(interner.resolve(symbol)),
        nose_il::Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|symbol| interner.resolve(*symbol)),
        _ => None,
    }
}

#[derive(Clone)]
pub(crate) struct ExactAdmissionRejectionDiagnostic {
    pub(crate) reason: &'static str,
    pub(crate) admission_gate: &'static str,
    pub(crate) capability_id: &'static str,
    pub(crate) pack_id: Option<&'static str>,
    pub(crate) missing_evidence: Vec<&'static str>,
}

pub(crate) fn exact_admission_rejection_with_context(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
    exact_safe: bool,
    value_len: usize,
    context: &AdmissionContext,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    if exact_safe {
        return (!nose_detect::exact_claim_eligible_parts(true, value_len)).then(|| {
            ExactAdmissionRejectionDiagnostic {
                reason: "value-fingerprint-too-small",
                admission_gate: "exact-claim-value-fingerprint-floor",
                capability_id: "non-degenerate-value-fingerprint",
                pack_id: None,
                missing_evidence: vec!["non-degenerate-value-fingerprint"],
            }
        });
    }

    Some(strict_exact_rejection_reason(il, interner, root, context))
}

fn strict_exact_rejection_reason(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
    context: &AdmissionContext,
) -> ExactAdmissionRejectionDiagnostic {
    if let Some(diagnostic) =
        runtime_boundary_rejection_diagnostic_with_context(il, interner, root, context)
    {
        return diagnostic;
    }

    if subtree_has(il, root, |il, node| il.kind(node) == nose_il::NodeKind::HoF) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "hof-demand-effect-proof-missing",
            admission_gate: "strict-exact-hof-demand-effect",
            capability_id: "hof-demand-effect-materialization",
            pack_id: None,
            missing_evidence: hof_missing_evidence(il, interner, root),
        };
    }

    if subtree_has(il, root, effect_boundary_node) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "mutation-effect-boundary",
            admission_gate: "strict-exact-effect-safety",
            capability_id: "effect-and-place-contract",
            pack_id: None,
            missing_evidence: vec!["effect-preserving-contract"],
        };
    }

    if subtree_has(il, root, builtin_call_node) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "library-api-occurrence-proof-missing",
            admission_gate: "strict-exact-library-api-occurrence",
            capability_id: "library-api-occurrence",
            pack_id: None,
            missing_evidence: vec!["library-api-occurrence-evidence"],
        };
    }

    if subtree_has(il, root, |il, node| {
        receiver_method_call(il, interner, node)
    }) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "receiver-domain-proof-missing",
            admission_gate: "strict-exact-receiver-domain",
            capability_id: "receiver-domain-evidence",
            pack_id: None,
            missing_evidence: vec!["receiver-domain-proof"],
        };
    }

    if subtree_has(il, root, rust_macro_invocation_call) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "source-surface-proof-missing",
            admission_gate: "strict-exact-source-surface",
            capability_id: "source-surface-evidence",
            pack_id: None,
            missing_evidence: vec!["rust-macro-expansion-contract"],
        };
    }

    if subtree_has(il, root, |il, node| {
        il.kind(node) == nose_il::NodeKind::Call
    }) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "import-symbol-callee-identity-proof-missing",
            admission_gate: "strict-exact-callee-identity",
            capability_id: "callee-identity-evidence",
            pack_id: None,
            missing_evidence: callee_identity_missing_evidence(il, interner, root),
        };
    }

    if subtree_has(il, root, source_surface_boundary_node) {
        return ExactAdmissionRejectionDiagnostic {
            reason: "source-surface-proof-missing",
            admission_gate: "strict-exact-source-surface",
            capability_id: "source-surface-evidence",
            pack_id: None,
            missing_evidence: vec!["source-surface-contract"],
        };
    }

    ExactAdmissionRejectionDiagnostic {
        reason: "unattributed-strict-exact-unsafe",
        admission_gate: "strict-exact-safety",
        capability_id: "exact-semantic-merge",
        pack_id: None,
        missing_evidence: vec!["strict-exact-safe-tree"],
    }
}

pub(crate) fn runtime_boundary_rejection_diagnostic_with_context(
    il: &nose_il::Il,
    interner: &Interner,
    root: NodeId,
    context: &AdmissionContext,
) -> Option<ExactAdmissionRejectionDiagnostic> {
    runtime_boundary_missing_evidence_with_context(il, interner, root, context).map(
        |missing_evidence| ExactAdmissionRejectionDiagnostic {
            reason: "unsupported-runtime-boundary",
            admission_gate: "strict-exact-safety",
            capability_id: "runtime-boundary-model",
            pack_id: None,
            missing_evidence,
        },
    )
}

fn subtree_has(
    il: &nose_il::Il,
    root: NodeId,
    pred: impl Fn(&nose_il::Il, NodeId) -> bool,
) -> bool {
    visit_subtree_until(il, root, |node| pred(il, node))
}

fn visit_subtree(il: &nose_il::Il, root: NodeId, mut visit: impl FnMut(NodeId)) {
    visit_subtree_until(il, root, |node| {
        visit(node);
        false
    });
}

fn visit_subtree_until(
    il: &nose_il::Il,
    root: NodeId,
    mut stop: impl FnMut(NodeId) -> bool,
) -> bool {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if stop(node) {
            return true;
        }
        stack.extend(il.children(node).iter().copied());
    }
    false
}

fn push_unique(labels: &mut Vec<&'static str>, label: &'static str) {
    if !labels.contains(&label) {
        labels.push(label);
    }
}

fn effect_boundary_node(il: &nose_il::Il, node: NodeId) -> bool {
    match il.node(node).payload {
        nose_il::Payload::Builtin(nose_il::Builtin::Append | nose_il::Builtin::Print) => true,
        _ => {
            il.kind(node) == nose_il::NodeKind::Assign
                && il.children(node).first().is_some_and(|&lhs| {
                    matches!(
                        il.kind(lhs),
                        nose_il::NodeKind::Field | nose_il::NodeKind::Index
                    )
                })
                || expression_statement_call(il, node)
        }
    }
}

fn builtin_call_node(il: &nose_il::Il, node: NodeId) -> bool {
    il.kind(node) == nose_il::NodeKind::Call
        && matches!(il.node(node).payload, nose_il::Payload::Builtin(_))
}

fn expression_statement_call(il: &nose_il::Il, node: NodeId) -> bool {
    il.kind(node) == nose_il::NodeKind::ExprStmt
        && il.children(node).first().is_some_and(|&expr| {
            subtree_has(il, expr, |il, node| {
                il.kind(node) == nose_il::NodeKind::Call
            })
        })
}

fn receiver_method_call(il: &nose_il::Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != nose_il::NodeKind::Call {
        return false;
    }
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    if il.kind(callee) != nose_il::NodeKind::Field {
        return false;
    }
    let nose_il::Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    matches!(
        interner.resolve(method),
        "and_then"
            | "any"
            | "all"
            | "collect"
            | "contains"
            | "end_with?"
            | "endsWith"
            | "filter"
            | "filter_map"
            | "flatMap"
            | "flat_map"
            | "get"
            | "getOrDefault"
            | "is_empty"
            | "isEmpty"
            | "map"
            | "max"
            | "min"
            | "reduce"
            | "reject"
            | "some"
            | "start_with?"
            | "startsWith"
            | "then"
    )
}

fn source_surface_boundary_node(il: &nose_il::Il, node: NodeId) -> bool {
    if rust_macro_invocation_call(il, node) {
        return true;
    }
    matches!(
        il.kind(node),
        nose_il::NodeKind::Seq
            | nose_il::NodeKind::Lambda
            | nose_il::NodeKind::Index
            | nose_il::NodeKind::BinOp
            | nose_il::NodeKind::UnOp
    )
}

fn rust_macro_invocation_call(il: &nose_il::Il, node: NodeId) -> bool {
    il.meta.lang == nose_il::Lang::Rust
        && il.kind(node) == nose_il::NodeKind::Call
        && il.evidence_anchored_at(il.node(node).span).any(|record| {
            matches!(
                record.kind,
                nose_il::EvidenceKind::Source(nose_il::SourceFactKind::Call(
                    nose_il::SourceCallKind::MacroInvocation
                ))
            )
        })
}
