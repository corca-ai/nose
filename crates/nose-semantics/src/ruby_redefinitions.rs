//! Ruby same-file method redefinition checks used by controlled stdlib admission.

use super::*;
use nose_il::UnitKind;

pub type RubyNodeNameResolver = for<'a> fn(&'a Il, &'a Interner, NodeId) -> Option<&'a str>;

pub fn ruby_class_instance_method_redefined_in_file(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
    node_name: RubyNodeNameResolver,
) -> bool {
    ruby_class_unit_redefines_method(il, interner, class_names, expected_method)
        || ruby_class_eval_redefines_method(il, interner, class_names, expected_method, node_name)
        || ruby_direct_define_method_redefines_method(
            il,
            interner,
            class_names,
            expected_method,
            node_name,
        )
}

fn ruby_class_unit_redefines_method(
    il: &Il,
    interner: &Interner,
    class_names: &[&str],
    expected_method: &str,
) -> bool {
    il.units.iter().any(|class_unit| {
        class_unit.kind == UnitKind::Class
            && class_unit
                .name
                .is_some_and(|name| ruby_class_name_matches(interner.resolve(name), class_names))
            && {
                let class_span = il.node(class_unit.root).span;
                il.units.iter().any(|method_unit| {
                    let method_span = il.node(method_unit.root).span;
                    method_unit.kind == UnitKind::Method
                        && method_unit
                            .name
                            .is_some_and(|name| interner.resolve(name) == expected_method)
                        && span_contains(class_span, method_span)
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
                        && span_contains(class_span, node.span)
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
) -> bool {
    let method_spans: Vec<_> = il
        .units
        .iter()
        .filter(|unit| {
            unit.kind == UnitKind::Method
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
            node.kind == NodeKind::Call
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
        node.kind == NodeKind::Call
            && ruby_class_eval_call(il, interner, call, class_names, node_name)
            && (method_spans
                .iter()
                .any(|&method_span| span_contains(node.span, method_span))
                || define_method_spans
                    .iter()
                    .any(|&define_method_span| span_contains(node.span, define_method_span)))
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
) -> bool {
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let call = NodeId(idx as u32);
        node.kind == NodeKind::Call
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
        && string_literal(il, *method, expected_method)
}

fn default_node_name<'a>(il: &'a Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    il.var_binding_name(node)
        .map(|symbol| interner.resolve(symbol))
}

fn ruby_class_name_matches(name: &str, class_names: &[&str]) -> bool {
    class_names.contains(&name)
}

fn string_literal(il: &Il, node: NodeId, expected: &str) -> bool {
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
