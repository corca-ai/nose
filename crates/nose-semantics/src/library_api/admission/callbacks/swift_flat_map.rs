use super::*;
use nose_il::UnitKind;

/// Prove the controlled Swift one-level `flatMap` perimeter. The outer source
/// must be a direct plain bracket-array function parameter, optionally guarded
/// by one admitted `filter`. The callback must expose exactly one inner
/// collection: either a direct plain bracket-array parameter or a standard
/// `map` over one, whose source may likewise carry one admitted `filter`. This
/// makes outer/inner traversal, guard placement, emitted value, and one-level
/// flatten depth explicit in the normalized HOF graph instead of trusting the
/// `flatMap` spelling alone.
pub(super) fn swift_flat_map_coordinates_proven(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    callback: NodeId,
) -> bool {
    swift_flat_map_has_proven_parameter_source(il, interner, node)
        && !swift_flat_map_dispatch_ambiguous_in_file(il, interner)
        && callback_single_output(il, callback)
            .is_some_and(|output| swift_flat_map_inner_collection_proven(il, interner, output))
}

fn swift_flat_map_has_proven_parameter_source(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let source = match il.kind(node) {
        NodeKind::Call => il
            .children(node)
            .first()
            .copied()
            .filter(|&callee| il.kind(callee) == NodeKind::Field)
            .and_then(|callee| il.children(callee).first().copied()),
        NodeKind::HoF => il.children(node).first().copied(),
        _ => None,
    };
    source.is_some_and(|source| swift_flat_map_outer_source_proven(il, interner, source))
}

fn swift_flat_map_inner_collection_proven(il: &Il, interner: &Interner, output: NodeId) -> bool {
    match il.kind(output) {
        NodeKind::Var => direct_bracket_array_parameter_source(il, output),
        NodeKind::Call => admitted_inner_map_call(il, interner, output),
        NodeKind::HoF => admitted_inner_map_hof(il, interner, output),
        _ => false,
    }
}

fn admitted_inner_map_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    let Some(occurrence) = admitted_library_method_call_at_call(il, interner, call) else {
        return false;
    };
    if occurrence.contract.id
        != LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Map))
    {
        return false;
    }
    let Some(source) = il
        .children(call)
        .first()
        .copied()
        .filter(|&callee| il.kind(callee) == NodeKind::Field)
        .and_then(|callee| il.children(callee).first().copied())
    else {
        return false;
    };
    swift_flat_map_inner_source_proven(il, interner, source)
}

fn admitted_inner_map_hof(il: &Il, interner: &Interner, hof: NodeId) -> bool {
    admitted_hof_demand_effect_profile_at_node_with_interner(il, Some(interner), hof, HoFKind::Map)
        .is_some()
        && il
            .children(hof)
            .first()
            .copied()
            .is_some_and(|source| swift_flat_map_inner_source_proven(il, interner, source))
}

fn direct_bracket_array_parameter_source(il: &Il, source: NodeId) -> bool {
    swift_direct_parameter_source(il, source)
        .is_some_and(|(_, param)| swift_bracket_array_parameter_proven(il, param))
}

fn swift_flat_map_outer_source_proven(il: &Il, interner: &Interner, source: NodeId) -> bool {
    direct_bracket_array_function_parameter_source(il, source)
        || admitted_filter_source(il, interner, source)
            .is_some_and(|source| direct_bracket_array_function_parameter_source(il, source))
}

fn swift_flat_map_inner_source_proven(il: &Il, interner: &Interner, source: NodeId) -> bool {
    direct_bracket_array_parameter_source(il, source)
        || admitted_filter_source(il, interner, source)
            .is_some_and(|source| direct_bracket_array_parameter_source(il, source))
}

fn direct_bracket_array_function_parameter_source(il: &Il, source: NodeId) -> bool {
    swift_compact_map_direct_function_param(il, source)
        .is_some_and(|param| swift_bracket_array_parameter_proven(il, param))
}

fn admitted_filter_source(il: &Il, interner: &Interner, node: NodeId) -> Option<NodeId> {
    match il.kind(node) {
        NodeKind::Call => {
            let occurrence = admitted_library_method_call_at_call(il, interner, node)?;
            (occurrence.contract.id
                == LibraryApiContractId::MethodCall(MethodSemanticContract::HoF(HoFKind::Filter)))
            .then_some(occurrence.receiver?)
        }
        NodeKind::HoF => {
            admitted_hof_demand_effect_profile_at_node_with_interner(
                il,
                Some(interner),
                node,
                HoFKind::Filter,
            )?;
            il.children(node).first().copied()
        }
        _ => None,
    }
}

fn swift_flat_map_dispatch_ambiguous_in_file(il: &Il, interner: &Interner) -> bool {
    il.units.iter().any(|unit| {
        matches!(unit.kind, UnitKind::Function | UnitKind::Method)
            && unit.name.is_some_and(|name| {
                ["flatMap", "filter", "map"]
                    .into_iter()
                    .any(|expected| swift_identifier_matches(interner.resolve(name), expected))
            })
    }) || il.nodes.iter().any(|node| {
        matches!(node.payload, Payload::Name(name) if interner.resolve(name)
            == SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER)
    }) || swift_has_unproven_collection_parameter(il)
}
