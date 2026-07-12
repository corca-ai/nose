use super::*;

#[derive(Default)]
pub struct ImportedOccurrenceValidationCache {
    function_spans: Option<Vec<Span>>,
    function_span_by_span: FxHashMap<Span, Option<Span>>,
    var_cid_by_span: FxHashMap<Span, Option<u32>>,
    local_shadows_by_function: Option<FxHashMap<Span, LocalShadowIndex>>,
}

#[derive(Default)]
struct LocalShadowIndex {
    by_cid: FxHashMap<u32, Vec<Span>>,
    raw_assign_by_hash: FxHashMap<u64, Vec<Span>>,
}

/// Validate that an occurrence-level imported symbol record is backed by a
/// still-visible import binding and not reopened through a rebound or shadowed
/// local alias.
pub fn imported_occurrence_symbol_dependencies_valid(
    il: &Il,
    interner: &Interner,
    symbol_record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
) -> bool {
    let mut cache = ImportedOccurrenceValidationCache::default();
    imported_occurrence_symbol_dependencies_valid_with_cache(
        il,
        interner,
        symbol_record,
        expected,
        &mut cache,
    )
}

pub fn imported_occurrence_symbol_dependencies_valid_with_cache(
    il: &Il,
    interner: &Interner,
    symbol_record: &EvidenceRecord,
    expected: SymbolEvidenceKind,
    cache: &mut ImportedOccurrenceValidationCache,
) -> bool {
    let EvidenceAnchor::Node {
        span: occurrence_span,
        kind: NodeKind::Var,
    } = symbol_record.anchor
    else {
        return false;
    };
    let Some(binding_record) = symbol_record.dependencies.iter().find_map(|&id| {
        let dependency = il.evidence_record_by_id(id)?;
        (dependency.status == EvidenceStatus::Asserted
            && dependency.kind == EvidenceKind::Symbol(expected)
            && matches!(dependency.anchor, EvidenceAnchor::Binding { .. }))
        .then_some(dependency)
    }) else {
        return false;
    };
    let EvidenceAnchor::Binding {
        span: binding_span,
        local_hash,
    } = binding_record.anchor
    else {
        return false;
    };
    if unit_defines_hash_visible_at(il, interner, local_hash, occurrence_span) {
        return false;
    }
    if !matches!(
        binding_identity_matches(il, local_hash, binding_span, expected),
        EvidenceResolution::Found(true)
    ) {
        return false;
    }
    if !binding_has_no_visible_conflicting_assignment(il, interner, local_hash, binding_span) {
        return false;
    }
    if !binding_has_no_visible_local_shadow_with_cache(
        il,
        interner,
        local_hash,
        binding_span,
        occurrence_span,
        cache,
    ) {
        return false;
    }
    binding_symbol_evidence_consistent_for_local(il, local_hash, expected)
}

pub(in crate::library_api) fn binding_has_no_visible_conflicting_assignment(
    il: &Il,
    interner: &Interner,
    local_hash: u64,
    binding_span: Span,
) -> bool {
    top_level_statements(il)
        .into_iter()
        .filter(|&stmt| assignment_alias_hash(il, interner, stmt) == Some(local_hash))
        .all(|stmt| il.node(stmt).span == binding_span)
}

pub(in crate::library_api) fn binding_has_no_visible_local_shadow_with_cache(
    il: &Il,
    interner: &Interner,
    local_hash: u64,
    binding_span: Span,
    occurrence_span: Span,
    cache: &mut ImportedOccurrenceValidationCache,
) -> bool {
    let Some(function_span) =
        innermost_enclosing_function_span_with_cache(il, occurrence_span, cache)
    else {
        return true;
    };
    let occurrence_cid = var_cid_at_span_with_cache(il, occurrence_span, cache);
    let indexes = local_shadow_indexes(il, interner, cache);
    let Some(index) = indexes.get(&function_span) else {
        return true;
    };
    let visible_before_occurrence =
        |span: &Span| *span != binding_span && span.start_byte <= occurrence_span.start_byte;
    if occurrence_cid.is_some_and(|cid| {
        index
            .by_cid
            .get(&cid)
            .is_some_and(|spans| spans.iter().any(visible_before_occurrence))
    }) {
        return false;
    }
    !index
        .raw_assign_by_hash
        .get(&local_hash)
        .is_some_and(|spans| spans.iter().any(visible_before_occurrence))
}

pub(in crate::library_api) fn innermost_enclosing_function_span_with_cache(
    il: &Il,
    span: Span,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<Span> {
    if let Some(cached) = cache.function_span_by_span.get(&span) {
        return *cached;
    }
    let function_spans = cache.function_spans.get_or_insert_with(|| {
        il.nodes
            .iter()
            .filter_map(|node| (node.kind == NodeKind::Func).then_some(node.span))
            .collect()
    });
    let result = function_spans
        .iter()
        .copied()
        .filter(|function_span| function_span.contains(span))
        .min_by_key(|span| span.end_byte.saturating_sub(span.start_byte));
    cache.function_span_by_span.insert(span, result);
    result
}

pub(in crate::library_api) fn var_cid_at_span_with_cache(
    il: &Il,
    span: Span,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<u32> {
    if let Some(cached) = cache.var_cid_by_span.get(&span) {
        return *cached;
    }
    let result = var_cid_at_span(il, span);
    cache.var_cid_by_span.insert(span, result);
    result
}

fn local_shadow_indexes<'a>(
    il: &Il,
    interner: &Interner,
    cache: &'a mut ImportedOccurrenceValidationCache,
) -> &'a FxHashMap<Span, LocalShadowIndex> {
    if cache.local_shadows_by_function.is_none() {
        let mut indexes: FxHashMap<Span, LocalShadowIndex> = FxHashMap::default();
        for (idx, node) in il.nodes.iter().enumerate() {
            if !matches!(node.kind, NodeKind::Param | NodeKind::Assign) {
                continue;
            }
            let node_id = NodeId(idx as u32);
            let Some(function_span) =
                innermost_enclosing_function_span_with_cache(il, node.span, cache)
            else {
                continue;
            };
            let index = indexes.entry(function_span).or_default();
            match node.kind {
                NodeKind::Param => {
                    if let Some(cid) = node_cid(il, node_id) {
                        index.by_cid.entry(cid).or_default().push(node.span);
                    }
                }
                NodeKind::Assign => {
                    if let Some(cid) = assignment_lhs_cid(il, node_id) {
                        index.by_cid.entry(cid).or_default().push(node.span);
                    }
                    if let Some(hash) = assignment_lhs_raw_name_hash(il, interner, node_id) {
                        index
                            .raw_assign_by_hash
                            .entry(hash)
                            .or_default()
                            .push(node.span);
                    }
                }
                _ => {}
            }
        }
        cache.local_shadows_by_function = Some(indexes);
    }
    cache
        .local_shadows_by_function
        .as_ref()
        .expect("local shadow indexes initialized")
}

pub(in crate::library_api) fn var_cid_at_span(il: &Il, span: Span) -> Option<u32> {
    il.nodes
        .iter()
        .enumerate()
        .find_map(|(idx, node)| {
            (node.kind == NodeKind::Var && node.span == span).then_some(NodeId(idx as u32))
        })
        .and_then(|node| node_cid(il, node))
}

pub(in crate::library_api) fn node_cid(il: &Il, node: NodeId) -> Option<u32> {
    match il.node(node).payload {
        Payload::Cid(cid) => Some(cid),
        _ => None,
    }
}

pub(in crate::library_api) fn assignment_lhs_cid(il: &Il, stmt: NodeId) -> Option<u32> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    (il.kind(lhs) == NodeKind::Var)
        .then(|| node_cid(il, lhs))
        .flatten()
}

pub(in crate::library_api) fn assignment_lhs_raw_name_hash(
    il: &Il,
    interner: &Interner,
    stmt: NodeId,
) -> Option<u64> {
    let (lhs, _) = assignment_parts(il, stmt)?;
    match il.node(lhs).payload {
        Payload::Name(symbol) => Some(stable_symbol_hash(interner.resolve(symbol))),
        _ => None,
    }
}

pub(in crate::library_api) fn binding_symbol_evidence_consistent_for_local(
    il: &Il,
    local_hash: u64,
    expected: SymbolEvidenceKind,
) -> bool {
    let mut saw_symbol = false;
    for record in il.evidence_binding_anchored(local_hash) {
        let EvidenceKind::Symbol(symbol) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted || symbol != expected {
            return false;
        }
        saw_symbol = true;
    }
    saw_symbol
}
