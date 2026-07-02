//! First-party call-target evidence producer.
//!
//! Consumers must not resolve user calls from raw spelling. This pass is the
//! narrow boundary where the lowered file's binding shape is checked and a
//! direct in-file or imported callable target is materialized as evidence.

use nose_il::{
    stable_symbol_hash, CallTargetEvidenceKind, DomainEvidence, EvidenceAnchor, EvidenceEmitter,
    EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus, Il, Interner,
    Lang, LoopKind, NodeId, NodeKind, Payload, SourceFactKind, SourceProtocolKind, Span, Symbol,
    SymbolEvidenceKind, UnitKind,
};
use nose_semantics::{
    imported_occurrence_symbol_dependencies_valid_with_cache, language_core_evidence_provenance,
    ImportedOccurrenceValidationCache, BUILTIN_COMPAT_PACK_ID,
};
use rustc_hash::{FxHashMap, FxHashSet};

const DIRECT_FUNCTION_RULE: &str = "normalize.call_target.direct_function";
const DIRECT_ASYNC_FUNCTION_RETURN_DOMAIN_RULE: &str =
    "normalize.call_target.direct_async_function_return_domain";
const DIRECT_FUNCTION_RETURN_DOMAIN_RULE: &str =
    "normalize.call_target.direct_function_return_domain";
const DIRECT_METHOD_RETURN_DOMAIN_RULE: &str = "normalize.call_target.direct_method_return_domain";
const IMPORTED_FUNCTION_RULE: &str = "normalize.call_target.imported_function";
const IMPORTED_MEMBER_RULE: &str = "normalize.call_target.imported_member";
const IMPORTED_BINDING_OCCURRENCE_RULE: &str =
    "normalize.symbol_imported_binding_occurrence_for_call_target";
const IMPORTED_NAMESPACE_OCCURRENCE_RULE: &str =
    "normalize.symbol_imported_namespace_occurrence_for_call_target";

#[derive(Clone, Copy)]
struct DirectFunctionTarget {
    root: NodeId,
    name_hash: u64,
}

#[derive(Clone, Copy)]
struct ImportedFunctionTarget {
    module_hash: u64,
    exported_hash: u64,
    local_hash: u64,
    dependency: EvidenceId,
}

#[derive(Clone, Copy)]
struct ImportedMemberTarget {
    module_hash: u64,
    exported_hash: u64,
    member_hash: u64,
    dependency: EvidenceId,
}

#[derive(Clone, Copy)]
enum ImportedBindingUse {
    FunctionCallee,
    MemberReceiver,
}

#[derive(Clone, Copy)]
struct CallTargetEvidenceProvenance {
    current: EvidenceProvenance,
    legacy_pack_hash: u64,
}

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let provenance = call_target_evidence_provenance(il);
    let targets = unique_direct_function_targets(il, interner);
    let mut calls = Vec::new();
    collect_call_nodes(il, il.root, &mut calls);

    let function_names = function_unit_names(il);
    let scope_bound = scope_bound_symbols(il, &function_names);
    let mut proven = Vec::new();
    let mut scope_stack = Vec::new();
    if !targets.is_empty() {
        collect_call_targets(
            il,
            interner,
            il.root,
            &mut scope_stack,
            &targets,
            &scope_bound,
            &mut proven,
        );
    }
    for (call, target) in proven {
        let target_evidence = upsert(
            il,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span: il.node(target.root).span,
                name_hash: target.name_hash,
            }),
            DIRECT_FUNCTION_RULE,
            provenance,
            Vec::new(),
        );
        record_direct_async_function_result_domain(
            il,
            call,
            target.root,
            target_evidence,
            provenance,
        );
        record_direct_function_return_promise_like_domain(
            il,
            call,
            target.root,
            target_evidence,
            provenance,
        );
    }
    let imported_binding_index = ImportedBindingIndex::new(il);
    if !imported_binding_index.is_empty() {
        let mut imported_occurrence_cache = ImportedOccurrenceValidationCache::default();
        for call in calls {
            record_imported_call_target(
                il,
                interner,
                call,
                provenance,
                &imported_binding_index,
                &mut imported_occurrence_cache,
            );
        }
    }
    record_existing_direct_method_return_domains(il, interner, provenance);
}

fn record_direct_async_function_result_domain(
    il: &mut Il,
    call: NodeId,
    target_root: NodeId,
    target_evidence: EvidenceId,
    provenance: CallTargetEvidenceProvenance,
) -> Option<EvidenceId> {
    if !matches!(il.meta.lang, Lang::JavaScript | Lang::TypeScript) {
        return None;
    }
    let boundary = direct_async_function_protocol_boundary(il, target_root)?;
    let protocol =
        asserted_source_protocol_evidence_id(il, boundary, SourceProtocolKind::AsyncFunction)?;
    Some(upsert(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::PromiseLike),
        DIRECT_ASYNC_FUNCTION_RETURN_DOMAIN_RULE,
        provenance,
        vec![target_evidence, protocol],
    ))
}

fn direct_async_function_protocol_boundary(il: &Il, target_root: NodeId) -> Option<NodeId> {
    if il.kind(target_root) != NodeKind::Func {
        return None;
    }
    let &boundary = il.children(target_root).last()?;
    (nose_semantics::source_protocol_at_node(il, boundary)
        == Some(SourceProtocolKind::AsyncFunction))
    .then_some(boundary)
}

fn record_direct_function_return_promise_like_domain(
    il: &mut Il,
    call: NodeId,
    target_root: NodeId,
    target_evidence: EvidenceId,
    provenance: CallTargetEvidenceProvenance,
) -> Option<EvidenceId> {
    if direct_async_function_protocol_boundary(il, target_root).is_some() {
        return None;
    }
    let mut dependencies = vec![target_evidence];
    dependencies.extend(direct_function_return_domain_evidence_ids(
        il,
        target_root,
        DomainEvidence::PromiseLike,
    )?);
    Some(upsert(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::PromiseLike),
        DIRECT_FUNCTION_RETURN_DOMAIN_RULE,
        provenance,
        dependencies,
    ))
}

fn record_existing_direct_method_return_domains(
    il: &mut Il,
    interner: &Interner,
    provenance: CallTargetEvidenceProvenance,
) {
    let targets = existing_direct_method_call_targets(il, interner, provenance.current);
    for target in targets {
        let Some(target_root) = unique_method_unit_root_at_span(il, target.target_span) else {
            continue;
        };
        record_direct_method_return_promise_like_domain(
            il,
            target.call,
            target_root,
            target.evidence,
            provenance,
        );
    }
}

#[derive(Clone, Copy)]
struct ExistingDirectMethodCallTarget {
    call: NodeId,
    target_span: Span,
    evidence: EvidenceId,
}

fn existing_direct_method_call_targets(
    il: &Il,
    interner: &Interner,
    provenance: EvidenceProvenance,
) -> Vec<ExistingDirectMethodCallTarget> {
    let mut calls = Vec::new();
    collect_call_nodes(il, il.root, &mut calls);
    let mut out = Vec::new();
    for call in calls {
        let nose_semantics::CallTargetEvidenceStatus::Admitted(
            CallTargetEvidenceKind::DirectMethod { target_span, .. },
        ) = nose_semantics::call_target_evidence_status_at_call(il, interner, call)
        else {
            continue;
        };
        let Some(evidence) =
            direct_method_call_target_evidence_id(il, call, target_span, provenance)
        else {
            continue;
        };
        out.push(ExistingDirectMethodCallTarget {
            call,
            target_span,
            evidence,
        });
    }
    out
}

fn direct_method_call_target_evidence_id(
    il: &Il,
    call: NodeId,
    target_span: Span,
    provenance: EvidenceProvenance,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(call).span, NodeKind::Call);
    il.evidence_anchored_at(anchor.span()).find_map(|record| {
        let EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
            target_span: record_target_span,
            ..
        }) = record.kind
        else {
            return None;
        };
        (record.anchor == anchor
            && record_target_span == target_span
            && record.status == EvidenceStatus::Asserted
            && record.provenance == provenance
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn unique_method_unit_root_at_span(il: &Il, target_span: Span) -> Option<NodeId> {
    let mut found = None;
    for unit in &il.units {
        if !matches!(unit.kind, UnitKind::Method | UnitKind::Function)
            || il.kind(unit.root) != NodeKind::Func
            || il.node(unit.root).span != target_span
        {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(unit.root);
    }
    found
}

fn record_direct_method_return_promise_like_domain(
    il: &mut Il,
    call: NodeId,
    target_root: NodeId,
    target_evidence: EvidenceId,
    provenance: CallTargetEvidenceProvenance,
) -> Option<EvidenceId> {
    if direct_async_function_protocol_boundary(il, target_root).is_some() {
        return None;
    }
    let mut dependencies = vec![target_evidence];
    dependencies.extend(direct_function_return_domain_evidence_ids(
        il,
        target_root,
        DomainEvidence::PromiseLike,
    )?);
    Some(upsert(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::Domain(DomainEvidence::PromiseLike),
        DIRECT_METHOD_RETURN_DOMAIN_RULE,
        provenance,
        dependencies,
    ))
}

fn direct_function_return_domain_evidence_ids(
    il: &Il,
    target_root: NodeId,
    domain: DomainEvidence,
) -> Option<Vec<EvidenceId>> {
    if il.kind(target_root) != NodeKind::Func {
        return None;
    }
    let &body = il.children(target_root).last()?;
    if il.kind(body) == NodeKind::Raw {
        return None;
    }
    let mut returns = Vec::new();
    if !collect_return_exprs_on_all_paths(il, body, &mut returns) || returns.is_empty() {
        return None;
    }
    let mut evidence = Vec::with_capacity(returns.len());
    for return_expr in returns {
        evidence.push(asserted_domain_evidence_id_at_node(
            il,
            return_expr,
            domain,
        )?);
    }
    evidence.sort_unstable_by_key(|id| id.0);
    evidence.dedup();
    Some(evidence)
}

fn collect_return_exprs_on_all_paths(il: &Il, node: NodeId, out: &mut Vec<NodeId>) -> bool {
    match il.kind(node) {
        NodeKind::Return => {
            let Some(&expr) = il.children(node).first() else {
                return false;
            };
            out.push(expr);
            true
        }
        NodeKind::Block => collect_block_return_exprs(il, il.children(node), out),
        NodeKind::If => {
            let kids = il.children(node);
            if kids.len() < 3 {
                return false;
            }
            let base = out.len();
            if collect_return_exprs_on_all_paths(il, kids[1], out)
                && collect_return_exprs_on_all_paths(il, kids[2], out)
            {
                true
            } else {
                out.truncate(base);
                false
            }
        }
        _ => false,
    }
}

fn collect_block_return_exprs(il: &Il, stmts: &[NodeId], out: &mut Vec<NodeId>) -> bool {
    for &stmt in stmts {
        let base = out.len();
        if collect_return_exprs_on_all_paths(il, stmt, out) {
            return true;
        }
        out.truncate(base);
        collect_conditional_return_exprs(il, stmt, out);
    }
    false
}

fn collect_conditional_return_exprs(il: &Il, node: NodeId, out: &mut Vec<NodeId>) {
    match il.kind(node) {
        NodeKind::If => {
            let kids = il.children(node);
            for &branch in kids.iter().skip(1).take(2) {
                collect_any_return_exprs(il, branch, out);
            }
        }
        NodeKind::Block => {
            for &child in il.children(node) {
                collect_conditional_return_exprs(il, child, out);
            }
        }
        _ => {}
    }
}

fn collect_any_return_exprs(il: &Il, node: NodeId, out: &mut Vec<NodeId>) {
    match il.kind(node) {
        NodeKind::Return => {
            if let Some(&expr) = il.children(node).first() {
                out.push(expr);
            }
        }
        NodeKind::Block => {
            for &stmt in il.children(node) {
                collect_any_return_exprs(il, stmt, out);
                if collect_return_exprs_on_all_paths(il, stmt, &mut Vec::new()) {
                    break;
                }
            }
        }
        NodeKind::If => {
            let kids = il.children(node);
            for &branch in kids.iter().skip(1).take(2) {
                collect_any_return_exprs(il, branch, out);
            }
        }
        _ => {}
    }
}

fn asserted_domain_evidence_id_at_node(
    il: &Il,
    node: NodeId,
    domain: DomainEvidence,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence_anchored_at(anchor.span()).find_map(|record| {
        (record.anchor == anchor
            && record.kind == EvidenceKind::Domain(domain)
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn asserted_source_protocol_evidence_id(
    il: &Il,
    node: NodeId,
    protocol: SourceProtocolKind,
) -> Option<EvidenceId> {
    let span = il.node(node).span;
    il.evidence_anchored_at(span).find_map(|record| {
        (record.kind == EvidenceKind::Source(SourceFactKind::Protocol(protocol))
            && record.status == EvidenceStatus::Asserted
            && il.evidence_dependencies_asserted(record))
        .then_some(record.id)
    })
}

fn upsert(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    _rule: &'static str,
    provenance: CallTargetEvidenceProvenance,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    let existing = asserted_builtin_record_index_with_pack_hash(
        il,
        anchor,
        &kind,
        provenance.current.pack_hash,
    )
    .or_else(|| {
        asserted_builtin_record_index_with_pack_hash(
            il,
            anchor,
            &kind,
            Some(provenance.legacy_pack_hash),
        )
    });
    if let Some(idx) = existing {
        let record = &mut il.evidence[idx as usize];
        record.provenance.pack_hash = provenance.current.pack_hash;
        record.provenance.rule_hash = provenance.current.rule_hash;
        record.dependencies = dependencies;
        return record.id;
    }
    il.find_or_push_builtin_evidence_with_provenance(anchor, kind, provenance.current, dependencies)
}

fn asserted_builtin_record_index_with_pack_hash(
    il: &Il,
    anchor: EvidenceAnchor,
    kind: &EvidenceKind,
    pack_hash: Option<u64>,
) -> Option<u32> {
    il.evidence_indices_anchored_at(anchor.span())
        .into_iter()
        .find(|&idx| {
            let record = &il.evidence[idx as usize];
            record.anchor == anchor
                && &record.kind == kind
                && record.status == EvidenceStatus::Asserted
                && record.provenance.emitter == EvidenceEmitter::Builtin
                && record.provenance.pack_hash == pack_hash
        })
}

fn call_target_evidence_provenance(il: &Il) -> CallTargetEvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(il.meta.lang);
    CallTargetEvidenceProvenance {
        current: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(pack_id)),
            rule_hash: Some(stable_symbol_hash(producer_id)),
        },
        legacy_pack_hash: stable_symbol_hash(BUILTIN_COMPAT_PACK_ID),
    }
}

mod direct;
mod imported;
mod scope;

use direct::*;
use imported::*;
use scope::*;

#[cfg(test)]
mod tests;
