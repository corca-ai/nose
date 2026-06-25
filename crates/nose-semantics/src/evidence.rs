//! Evidence resolution plus source/domain proof helpers for the semantic facade.
//!
//! This module owns fail-closed lookup rules over `EvidenceRecord`s and the
//! lightweight value/domain contracts consumed by operators, normalize, and detect.

use super::*;
use nose_il::EvidenceProvenance;
use rustc_hash::FxHashMap;

mod call_target;
mod domain;
mod value_laws;

pub use call_target::*;
pub use domain::*;
pub use value_laws::*;
pub struct SourceFactContract {
    pub kind: SourceFactKind,
    pub channel: ChannelEligibility,
}

pub fn source_fact_contract(kind: SourceFactKind) -> SourceFactContract {
    SourceFactContract {
        kind,
        channel: ChannelEligibility::ExactProven,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EvidenceResolution<T> {
    Missing,
    Found(T),
    Ambiguous,
}

/// Resolve the unique evidence value projected by `project` among the records
/// anchored exactly at `span` (anchors only ever match by exact span equality,
/// so the span-bucketed index replaces a full `evidence` pass per query).
pub(crate) fn unique_evidence_at<T: Copy + Eq>(
    il: &Il,
    span: Span,
    anchor_matches: impl Fn(EvidenceAnchor) -> bool,
    project: impl Fn(EvidenceKind) -> Option<T>,
) -> EvidenceResolution<T> {
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !anchor_matches(record.anchor) {
            continue;
        }
        let Some(value) = project(record.kind) else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(value),
            Some(existing) if existing == value => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

pub(crate) fn unique_asserted_evidence_at<T: Copy + Eq>(
    il: &Il,
    span: Span,
    anchor_matches: impl Fn(EvidenceAnchor) -> bool,
    project: impl Fn(EvidenceKind) -> Option<T>,
) -> EvidenceResolution<T> {
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !anchor_matches(record.anchor) {
            continue;
        }
        let Some(value) = project(record.kind) else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return EvidenceResolution::Ambiguous;
        }
        match found {
            None => found = Some(value),
            Some(existing) if existing == value => {}
            Some(_) => return EvidenceResolution::Ambiguous,
        }
    }
    found.map_or(EvidenceResolution::Missing, EvidenceResolution::Found)
}

fn evidence_at_span<T: Copy + Eq>(
    il: &Il,
    span: Span,
    project: impl Fn(EvidenceKind) -> Option<T>,
) -> EvidenceResolution<T> {
    unique_asserted_evidence_at(il, span, |anchor| anchor.matches_span(span), project)
}

pub fn source_fact_at_node(il: &Il, node: NodeId, kind: SourceFactKind) -> bool {
    match kind {
        SourceFactKind::Operator(operator) => source_operator_at_node(il, node) == Some(operator),
        SourceFactKind::Cast(cast) => source_cast_at_node(il, node) == Some(cast),
        SourceFactKind::Call(call) => source_call_at_node(il, node) == Some(call),
        SourceFactKind::Protocol(protocol) => source_protocol_at_node(il, node) == Some(protocol),
        SourceFactKind::Literal(literal) => source_literal_at_node(il, node) == Some(literal),
        SourceFactKind::Comprehension(comprehension) => {
            source_comprehension_at_node(il, node) == Some(comprehension)
        }
        SourceFactKind::Range(range) => source_range_at_node(il, node) == Some(range),
        SourceFactKind::Pattern(pattern) => source_pattern_at_node(il, node) == Some(pattern),
        SourceFactKind::Binding(binding) => source_binding_at_node(il, node) == Some(binding),
    }
}

pub fn source_binding_at_node(il: &Il, node: NodeId) -> Option<SourceBindingKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Binding(binding)) => Some(binding),
        _ => None,
    }) {
        EvidenceResolution::Found(binding) => Some(binding),
        _ => None,
    }
}

/// The definition rooted at `node` was DECORATED in source: its runtime binding is the
/// decorator's result, not the lowered body. Consumers that attribute the body to the
/// function's NAME (call-target evidence, content-keyed seeding, inlining) must treat
/// such a binding as unprovable and fail closed.
pub fn decorated_definition_at_node(il: &Il, node: NodeId) -> bool {
    source_binding_at_node(il, node) == Some(SourceBindingKind::DecoratedDefinition)
}

/// The module-scope names a `ModuleRebind` fact reports as reassigned from inside another
/// scope (a Python `global name; name = ...`). A top-level function with such a name is
/// not provably its `def` body — its callers must not inline it, and it must not be
/// content-keyed or admitted to the exact channel (#302). Precise where the series-6
/// reassigned-anywhere predicate over-fired: a local `name = x` (no `global`) carries no
/// fact, so the function stays a valid target.
pub fn module_rebound_symbols(il: &Il, interner: &Interner) -> rustc_hash::FxHashSet<Symbol> {
    let mut out = rustc_hash::FxHashSet::default();
    for idx in 0..il.nodes.len() {
        let node = NodeId(idx as u32);
        if il.kind(node) != NodeKind::Assign
            || source_binding_at_node(il, node) != Some(SourceBindingKind::ModuleRebind)
        {
            continue;
        }
        // The target is a `Var` (`helper = `, `helper += `, `(helper := )`) or a `Seq`
        // of targets from a tuple/list unpack (`helper, y = `). Collect every name
        // written — each global-declared one is a module rebind (coevo series 7, S2).
        if let Some(&lhs) = il.children(node).first() {
            collect_target_names(il, lhs, &mut out);
        }
    }
    collect_dynamic_module_rebinds(il, interner, &mut out);
    out
}

/// Dynamic module rebinds with NO `global` declaration to key off: `globals()['name'] = …`
/// and `setattr(<module>, 'name', …)`. The reassigned name is a STRING LITERAL, so detect
/// the shape structurally and resolve the key to the module function it names — a `LitStr`
/// holds `stable_symbol_hash(content)`, the same hash a `Symbol` has, so a function whose
/// `symbol_hash` matches the key is the rebound target. Closes #307: a caller of `helper()`
/// must not inline its `def` body once `globals()['helper']` reassigns it. Fail-open — a key
/// that names no module function adds nothing.
fn collect_dynamic_module_rebinds(
    il: &Il,
    interner: &Interner,
    out: &mut rustc_hash::FxHashSet<Symbol>,
) {
    let by_hash: FxHashMap<u64, Symbol> = il
        .units
        .iter()
        .filter_map(|u| u.name)
        .map(|s| (interner.symbol_hash(s), s))
        .collect();
    if by_hash.is_empty() {
        return;
    }
    let str_lit_hash = |n: NodeId| match il.node(n).payload {
        Payload::LitStr(h) => Some(h),
        _ => None,
    };
    // A `Call` whose callee is the free name `want` (`globals` / `setattr`).
    let callee_named = |call: NodeId, want: &str| {
        il.kind(call) == NodeKind::Call
            && il
                .children(call)
                .first()
                .and_then(|&c| il.var_name(c))
                .is_some_and(|s| interner.resolve(s) == want)
    };
    for idx in 0..il.nodes.len() {
        let node = NodeId(idx as u32);
        let key_hash = match il.kind(node) {
            // `globals()['name'] = …` → Assign( Index( Call(globals), LitStr ), value )
            NodeKind::Assign => il.children(node).first().and_then(|&lhs| {
                if il.kind(lhs) != NodeKind::Index {
                    return None;
                }
                let kids = il.children(lhs);
                let base = *kids.first()?;
                let key = *kids.get(1)?;
                callee_named(base, "globals")
                    .then(|| str_lit_hash(key))
                    .flatten()
            }),
            // `setattr(<module>, 'name', …)` → Call[callee, module, LitStr, value]
            NodeKind::Call if callee_named(node, "setattr") => {
                il.children(node).get(2).copied().and_then(str_lit_hash)
            }
            _ => None,
        };
        if let Some(sym) = key_hash.and_then(|h| by_hash.get(&h).copied()) {
            out.insert(sym);
        }
    }
}

fn collect_target_names(il: &Il, target: NodeId, out: &mut rustc_hash::FxHashSet<Symbol>) {
    match il.kind(target) {
        NodeKind::Var => {
            if let Some(name) = il.var_name(target) {
                out.insert(name);
            }
        }
        NodeKind::Seq => {
            for &c in il.children(target) {
                collect_target_names(il, c, out);
            }
        }
        _ => {}
    }
}

pub fn source_operator_at_node(il: &Il, node: NodeId) -> Option<SourceOperatorKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Operator(operator)) => Some(operator),
        _ => None,
    }) {
        EvidenceResolution::Found(operator) => Some(operator),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_cast_at_node(il: &Il, node: NodeId) -> Option<SourceCastKind> {
    let span = il.node(node).span;
    let mut found = None;
    for record in il.evidence_anchored_at(span) {
        if !record.anchor.matches_span(span) {
            continue;
        }
        let EvidenceKind::Source(SourceFactKind::Cast(cast)) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || !il.evidence_dependencies_asserted(record)
            || !source_cast_provenance_admitted(record, cast)
        {
            return None;
        }
        match found {
            None => found = Some(cast),
            Some(existing) if existing == cast => {}
            Some(_) => return None,
        }
    }
    found
}

fn source_cast_provenance_admitted(record: &EvidenceRecord, cast: SourceCastKind) -> bool {
    match cast {
        SourceCastKind::CUnsigned32 => {
            record.provenance.emitter == EvidenceEmitter::Builtin
                && record.provenance.pack_hash == Some(stable_symbol_hash(C_LANGUAGE_PACK_ID))
                && record.provenance.rule_hash
                    == Some(stable_symbol_hash(C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID))
        }
    }
}

pub fn source_call_at_node(il: &Il, node: NodeId) -> Option<SourceCallKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Call(call)) => Some(call),
        _ => None,
    }) {
        EvidenceResolution::Found(call) => Some(call),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_protocol_at_node(il: &Il, node: NodeId) -> Option<SourceProtocolKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Protocol(protocol)) => Some(protocol),
        _ => None,
    }) {
        EvidenceResolution::Found(protocol) => Some(protocol),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_literal_at_node(il: &Il, node: NodeId) -> Option<SourceLiteralKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Literal(literal)) => Some(literal),
        _ => None,
    }) {
        EvidenceResolution::Found(literal) => Some(literal),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_comprehension_at_node(il: &Il, node: NodeId) -> Option<SourceComprehensionKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Comprehension(comprehension)) => Some(comprehension),
        _ => None,
    }) {
        EvidenceResolution::Found(comprehension) => Some(comprehension),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_range_at_node(il: &Il, node: NodeId) -> Option<SourceRangeKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Range(range)) => Some(range),
        _ => None,
    }) {
        EvidenceResolution::Found(range) => Some(range),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn source_pattern_at_node(il: &Il, node: NodeId) -> Option<SourcePatternKind> {
    let span = il.node(node).span;
    match evidence_at_span(il, span, |evidence| match evidence {
        EvidenceKind::Source(SourceFactKind::Pattern(pattern)) => Some(pattern),
        _ => None,
    }) {
        EvidenceResolution::Found(pattern) => Some(pattern),
        EvidenceResolution::Ambiguous | EvidenceResolution::Missing => None,
    }
}

pub fn admitted_hof_api_at_node(il: &Il, node: NodeId, kind: HoFKind) -> bool {
    admitted_hof_api_at_node_with_interner(il, None, node, kind)
}

pub fn admitted_hof_api_at_node_with_interner(
    il: &Il,
    interner: Option<&Interner>,
    node: NodeId,
    kind: HoFKind,
) -> bool {
    if il.kind(node) != NodeKind::HoF || il.node(node).payload != Payload::HoF(kind) {
        return false;
    }
    library_api_dependency_id_for_normalized_hof(il, interner, node).is_some()
}

pub fn admitted_hof_demand_effect_profile_at_node(
    il: &Il,
    node: NodeId,
    kind: HoFKind,
) -> Option<DemandEffectProfile> {
    admitted_hof_demand_effect_profile_at_node_with_interner(il, None, node, kind)
}

pub fn admitted_hof_demand_effect_profile_at_node_with_interner(
    il: &Il,
    interner: Option<&Interner>,
    node: NodeId,
    kind: HoFKind,
) -> Option<DemandEffectProfile> {
    if il.kind(node) != NodeKind::HoF || il.node(node).payload != Payload::HoF(kind) {
        return None;
    }
    if let Some(source) = source_comprehension_at_node(il, node) {
        return source_comprehension_hof_demand_effect_profile(kind, source);
    }
    admitted_hof_api_at_node_with_interner(il, interner, node, kind)
        .then(|| library_hof_demand_effect_profile(il.meta.lang, kind))
        .flatten()
}

pub fn admitted_terminal_count_reduction_at_call(il: &Il, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Call || il.node(node).payload != Payload::Builtin(Builtin::Len) {
        return false;
    }
    let Some(contract) = library_method_call_contract(il.meta.lang, "count", 0) else {
        return false;
    };
    library_api_dependency_id_for_canonical_builtin_method_call(
        il,
        node,
        Builtin::Len,
        contract.callee,
        0,
    )
    .is_some()
}

pub fn admitted_builtin_semantics_at_call(il: &Il, node: NodeId, builtin: Builtin) -> bool {
    if il.kind(node) != NodeKind::Call || il.node(node).payload != Payload::Builtin(builtin) {
        return false;
    }
    language_core_builtin_at_call(il, node, builtin)
        || library_api_dependency_id_for_canonical_builtin_call(il, node, builtin).is_some()
}

pub fn admitted_builtin_semantics_at_call_with_interner(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    builtin: Builtin,
) -> bool {
    admitted_builtin_semantics_at_call(il, node, builtin)
        || library_api_dependency_id_for_canonical_builtin_call_with_interner(
            il, interner, node, builtin,
        )
        .is_some()
}

pub fn construct_syntax_proof(il: &Il, node: NodeId) -> bool {
    source_call_at_node(il, node) == Some(SourceCallKind::Construct)
}

pub fn regex_literal_proof(il: &Il, node: NodeId) -> bool {
    source_literal_at_node(il, node) == Some(SourceLiteralKind::Regex)
}

pub fn exact_static_membership_predicate_operator(
    lang: Lang,
    op: Op,
    source: SourceOperatorKind,
) -> bool {
    js_like_lang(lang)
        && matches!(
            (op, source),
            (Op::Eq, SourceOperatorKind::StrictEquality)
                | (Op::Ne, SourceOperatorKind::StrictInequality)
        )
}
