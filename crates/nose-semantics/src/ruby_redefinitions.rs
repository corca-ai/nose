//! Ruby same-file method redefinition checks used by controlled stdlib admission.

mod dynamic_method_changes;

use super::*;
use nose_il::UnitKind;

pub type RubyNodeNameResolver = for<'a> fn(&'a Il, &'a Interner, NodeId) -> Option<&'a str>;

#[derive(Clone, PartialEq, Eq, Hash)]
struct RubyClassInstanceMethodQuery {
    class_names: Vec<u64>,
    expected_method: u64,
    node_name: usize,
}

/// Per-IL cache for Ruby same-file redefinition queries.
///
/// Callers build one cache for a completed IL arena and reuse it while recording
/// library-API evidence. The arena and its units are immutable during these
/// queries, so repeated receiver occurrences must not rescan the whole file.
#[derive(Default)]
pub struct RubyRedefinitionCache {
    class_instance_methods: FxHashMap<RubyClassInstanceMethodQuery, bool>,
    core_nil_predicate_by_file: FxHashMap<u32, bool>,
}

pub fn ruby_class_instance_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
) -> bool {
    let mut cache = RubyRedefinitionCache::default();
    ruby_class_instance_method_redefined_in_file_with_cache(
        il,
        interner,
        class_names,
        expected_method,
        node_name,
        &mut cache,
    )
}

pub fn ruby_class_instance_method_redefined_in_file_with_cache(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    cache: &mut RubyRedefinitionCache,
) -> bool {
    let mut class_name_hashes: Vec<_> = class_names
        .iter()
        .map(|name| stable_symbol_hash(name))
        .collect();
    class_name_hashes.sort_unstable();
    class_name_hashes.dedup();
    let query = RubyClassInstanceMethodQuery {
        class_names: class_name_hashes,
        expected_method: stable_symbol_hash(expected_method),
        node_name: node_name as usize,
    };
    if let Some(&redefined) = cache.class_instance_methods.get(&query) {
        return redefined;
    }
    let redefined = ruby_class_instance_method_redefined_visible_in_file(
        il,
        interner,
        class_names,
        expected_method,
        node_name,
        None,
    );
    cache.class_instance_methods.insert(query, redefined);
    redefined
}

fn ruby_class_instance_method_redefined_visible_in_file(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Option<Span>,
) -> bool {
    ruby_class_unit_redefines_method(il, interner, class_names, expected_method, occurrence_span)
        || ruby_class_eval_redefines_method(
            il,
            interner,
            class_names,
            expected_method,
            node_name,
            occurrence_span,
        )
        || ruby_direct_define_method_redefines_method(
            il,
            interner,
            class_names,
            expected_method,
            node_name,
            occurrence_span,
        )
}

pub fn ruby_default_node_name<'a>(
    il: &'a Il,
    interner: &'a Interner,
    node: NodeId,
) -> Option<&'a str> {
    default_node_name(il, interner, node)
}

pub fn ruby_core_nil_predicate_unmodified_in_file(
    il: &Il,
    interner: &Interner,
    occurrence_span: Span,
) -> bool {
    let mut cache = RubyRedefinitionCache::default();
    ruby_core_nil_predicate_unmodified_in_file_with_cache(il, interner, occurrence_span, &mut cache)
}

pub fn ruby_core_nil_predicate_unmodified_in_file_with_cache(
    il: &Il,
    interner: &Interner,
    occurrence_span: Span,
    cache: &mut RubyRedefinitionCache,
) -> bool {
    let file = occurrence_span.file.0;
    if let Some(&unmodified) = cache.core_nil_predicate_by_file.get(&file) {
        return unmodified;
    }
    let unmodified = il.meta.lang == Lang::Ruby
        && !ruby_class_instance_method_redefined_visible_in_file(
            il,
            interner,
            &["*"],
            "nil?",
            ruby_default_node_name,
            Some(occurrence_span),
        )
        && !ruby_method_unit_named_visible_in_file(il, interner, "nil?", occurrence_span)
        && !ruby_method_alias_or_removal_visible_in_file(il, interner, "nil?", occurrence_span)
        && !ruby_nil_predicate_alias_or_undef_marker_visible_in_file(il, interner, occurrence_span);
    cache.core_nil_predicate_by_file.insert(file, unmodified);
    unmodified
}

pub fn ruby_method_unit_named_in_file(il: &Il, interner: &Interner, expected_method: &str) -> bool {
    il.units.iter().any(|unit| {
        unit.kind == UnitKind::Method
            && unit
                .name
                .is_some_and(|name| interner.resolve(name) == expected_method)
    })
}

pub fn ruby_method_alias_or_removal_in_file(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
) -> bool {
    let index =
        dynamic_method_changes::RubyDynamicMethodChangeIndex::new(il, interner, default_node_name);
    il.nodes.iter().enumerate().any(|(idx, _)| {
        dynamic_method_changes::ruby_dynamic_method_change_operation(
            il,
            interner,
            NodeId(idx as u32),
            expected_method,
            default_node_name,
            &index,
        )
    })
}

fn ruby_class_unit_redefines_method(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    occurrence_span: Option<Span>,
) -> bool {
    il.units.iter().any(|class_unit| {
        let class_span = il.node(class_unit.root).span;
        same_file_as_occurrence(class_span, occurrence_span)
            && class_unit.kind == UnitKind::Class
            && class_unit
                .name
                .is_some_and(|name| ruby_class_name_matches(interner.resolve(name), class_names))
            && {
                il.units.iter().any(|method_unit| {
                    let method_span = il.node(method_unit.root).span;
                    method_unit.kind == UnitKind::Method
                        && method_unit
                            .name
                            .is_some_and(|name| interner.resolve(name) == expected_method)
                        && class_span.contains(method_span)
                }) || il.nodes.iter().enumerate().any(|(idx, node)| {
                    let call = NodeId(idx as u32);
                    node.kind == NodeKind::Call
                        && ruby_define_method_call_redefines_method(
                            il,
                            interner,
                            call,
                            expected_method,
                            default_node_name,
                        )
                        && class_span.contains(node.span)
                })
            }
    })
}

fn ruby_class_eval_redefines_method(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Option<Span>,
) -> bool {
    let method_spans: Vec<_> = il
        .units
        .iter()
        .filter(|unit| {
            same_file_as_occurrence(il.node(unit.root).span, occurrence_span)
                && unit.kind == UnitKind::Method
                && unit
                    .name
                    .is_some_and(|name| interner.resolve(name) == expected_method)
        })
        .map(|unit| il.node(unit.root).span)
        .collect();
    let define_method_spans: Vec<_> = il
        .nodes
        .iter()
        .enumerate()
        .filter(|(idx, node)| {
            same_file_as_occurrence(node.span, occurrence_span)
                && node.kind == NodeKind::Call
                && ruby_define_method_call_redefines_method(
                    il,
                    interner,
                    NodeId(*idx as u32),
                    expected_method,
                    node_name,
                )
        })
        .map(|(_, node)| node.span)
        .collect();
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let call = NodeId(idx as u32);
        same_file_as_occurrence(node.span, occurrence_span)
            && node.kind == NodeKind::Call
            && ruby_class_eval_call(il, interner, call, class_names, node_name)
            && (method_spans
                .iter()
                .any(|&method_span| node.span.contains(method_span))
                || define_method_spans
                    .iter()
                    .any(|&define_method_span| node.span.contains(define_method_span)))
    })
}

fn ruby_class_eval_call(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    class_names: &[&str],
    node_name: RubyNodeNameResolver,
) -> bool {
    let Some(&callee) = il.children(call).first() else {
        return false;
    };
    matches!(
        field_name(il, interner, callee),
        Some("class_eval" | "module_eval")
    ) && il
        .children(callee)
        .first()
        .copied()
        .and_then(|receiver| node_name(il, interner, receiver))
        .is_some_and(|name| ruby_class_name_matches(name, class_names))
}

fn ruby_direct_define_method_redefines_method(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
    occurrence_span: Option<Span>,
) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let call = NodeId(idx as u32);
        same_file_as_occurrence(node.span, occurrence_span)
            && node.kind == NodeKind::Call
            && ruby_define_method_call_redefines_method(
                il,
                interner,
                call,
                expected_method,
                node_name,
            )
            && il.children(call).first().copied().is_some_and(|callee| {
                field_name(il, interner, callee) == Some("define_method")
                    && il
                        .children(callee)
                        .first()
                        .copied()
                        .and_then(|receiver| node_name(il, interner, receiver))
                        .is_some_and(|name| ruby_class_name_matches(name, class_names))
            })
    })
}

fn ruby_define_method_call_redefines_method(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    expected_method: &str,
    node_name: RubyNodeNameResolver,
) -> bool {
    let [callee, method, ..] = il.children(call) else {
        return false;
    };
    (node_name(il, interner, *callee) == Some("define_method")
        || field_name(il, interner, *callee) == Some("define_method"))
        && method_name_argument_may_match(il, *method, expected_method)
}

fn ruby_method_unit_named_visible_in_file(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
    occurrence_span: Span,
) -> bool {
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit.kind == UnitKind::Method
            && unit
                .name
                .is_some_and(|name| interner.resolve(name) == expected_method)
    })
}

fn ruby_method_alias_or_removal_visible_in_file(
    il: &Il,
    interner: &Interner,
    expected_method: &str,
    occurrence_span: Span,
) -> bool {
    let index =
        dynamic_method_changes::RubyDynamicMethodChangeIndex::new(il, interner, default_node_name);
    il.nodes.iter().enumerate().any(|(idx, node)| {
        node.span.file == occurrence_span.file
            && dynamic_method_changes::ruby_dynamic_method_change_operation(
                il,
                interner,
                NodeId(idx as u32),
                expected_method,
                default_node_name,
                &index,
            )
    })
}

fn ruby_nil_predicate_alias_or_undef_marker_visible_in_file(
    il: &Il,
    interner: &Interner,
    occurrence_span: Span,
) -> bool {
    il.nodes.iter().any(|node| {
        node.span.file == occurrence_span.file
            && matches!(
                node.payload,
                Payload::Name(symbol)
                    if interner.resolve(symbol) == "ruby_nil_predicate_alias_or_undef"
            )
    })
}

fn same_file_as_occurrence(span: Span, occurrence_span: Option<Span>) -> bool {
    occurrence_span.is_none_or(|occurrence_span| span.file == occurrence_span.file)
}

fn default_node_name<'a>(il: &'a Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    il.var_binding_name(node)
        .map(|symbol| interner.resolve(symbol))
}

fn ruby_class_name_matches(name: &str, class_names: &[&str]) -> bool {
    class_names.contains(&"*") || class_names.contains(&name)
}

fn method_name_argument_may_match(il: &Il, node: NodeId, expected: &str) -> bool {
    match il.node(node).payload {
        Payload::LitStr(hash) => hash == stable_symbol_hash(expected),
        _ => true,
    }
}

fn method_name_argument_is_literal(il: &Il, node: NodeId, expected: &str) -> bool {
    matches!(il.node(node).payload, Payload::LitStr(hash) if hash == stable_symbol_hash(expected))
}

fn field_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(symbol) = il.node(node).payload else {
        return None;
    };
    Some(interner.resolve(symbol))
}
