//! Strict exact-safety proof gates for semantic unit extraction.
//!
//! This module owns the fail-closed checks that decide whether a normalized IL
//! subtree can participate in the exact semantic channel. Unit extraction keeps
//! orchestration in `units.rs`; proof policy lives here.

use nose_il::{
    Builtin, CallTargetEvidenceKind, HoFKind, Il, Interner, LitClass, NodeId, NodeKind, Op,
    Payload, SourceComprehensionKind, Symbol,
};
use nose_normalize::module_facts::collect_module_mutations;
use nose_semantics::{
    admitted_builtin_semantics_at_call, admitted_free_name_collection_factory_at_call,
    admitted_free_name_map_factory_at_call, admitted_hof_demand_effect_profile_at_node,
    admitted_imported_collection_factory_at_call, admitted_iterator_identity_adapter_at_call,
    admitted_java_collection_constructor_at_call, admitted_java_collection_factory_at_call,
    admitted_java_map_entry_at_call, admitted_java_map_factory_at_call,
    admitted_js_array_is_array_at_call, admitted_js_like_map_constructor_at_call,
    admitted_js_like_set_constructor_at_call, admitted_library_method_call_at_call,
    admitted_map_get_at_call, admitted_map_key_view_at_call, admitted_map_key_view_wrapper_at_call,
    admitted_regex_test_at_call, admitted_ruby_set_factory_at_call,
    admitted_rust_option_none_sentinel_at_node, admitted_rust_vec_macro_factory_at_call,
    admitted_rust_vec_new_factory_at_call, admitted_static_index_membership_at_call,
    admitted_terminal_count_reduction_at_call, asserted_unshadowed_global_symbol,
    call_target_evidence_status_at_call, construct_syntax_proof,
    direct_function_call_target_at_call, direct_method_call_target_at_call,
    exact_static_membership_predicate_operator, go_zero_map_default_kind,
    go_zero_map_entry_contract_for_node, go_zero_map_literal_contract_for_node,
    go_zero_map_lookup_contract, nullish_global_contract, own_property_guard_for_node,
    record_shape_guard_for_node, semantics, seq_surface_contract_for_node,
    source_comprehension_at_node, source_fact_at_node, source_operator_at_node,
    typeof_operator_contract, CallTargetEvidenceStatus, DomainRequirement,
    IndexMembershipThreshold, JavaMapFactoryKind, LibraryCollectionFactoryResult,
    LibraryMapFactoryResult, LibraryMethodCallContract, MapKeyViewKind, MethodBuiltinArgs,
    MethodReceiverContract, MethodSemanticContract, ReceiverDomainEvidenceIndex,
    StaticIndexMembershipKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) struct StrictFacts<'a> {
    immutable_names: FxHashSet<Symbol>,
    function_roots: FxHashSet<NodeId>,
    receiver_domains: ReceiverDomainEvidenceIndex<'a>,
}

impl<'a> StrictFacts<'a> {
    pub(crate) fn collect(il: &'a Il, interner: &'a Interner) -> Self {
        let mut facts = StrictFacts {
            immutable_names: FxHashSet::default(),
            function_roots: FxHashSet::default(),
            receiver_domains: ReceiverDomainEvidenceIndex::new(il, interner),
        };
        facts.collect_immutable_bindings(il, interner);
        facts.collect_function_bindings(il, interner);
        facts
    }

    fn exact_value_name(&self, name: Symbol) -> bool {
        self.immutable_names.contains(&name)
    }

    fn direct_function_target_at_call(&self, il: &Il, call: NodeId) -> bool {
        self.function_roots
            .iter()
            .any(|&root| direct_function_call_target_at_call(il, call, root))
    }

    fn direct_method_target_at_call(&self, il: &Il, interner: &Interner, call: NodeId) -> bool {
        self.function_roots
            .iter()
            .any(|&root| direct_method_call_target_at_call(il, interner, call, root))
    }

    fn receiver_satisfies_domain(&self, receiver: NodeId, requirement: DomainRequirement) -> bool {
        self.receiver_domains
            .receiver_satisfies_domain(receiver, requirement)
    }

    fn collect_immutable_bindings(&mut self, il: &Il, interner: &Interner) {
        let top_level = top_level_statements(il);
        let mut is_top_level = vec![false; il.nodes.len()];
        for &stmt in &top_level {
            if let Some(slot) = is_top_level.get_mut(stmt.0 as usize) {
                *slot = true;
            }
        }

        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for &stmt in &top_level {
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }
        let candidate_names: FxHashSet<Symbol> = counts
            .iter()
            .filter_map(|(&name, &count)| (count == 1).then_some(name))
            .collect();
        let mutated_bindings =
            collect_module_mutations(il, interner, &candidate_names, &is_top_level);

        let mut env: FxHashSet<u32> = FxHashSet::default();
        for &stmt in &top_level {
            let kids = il.children(stmt);
            if kids.len() != 2 {
                continue;
            }
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            if mutated_bindings.contains(&name) {
                continue;
            }
            let safe_literal = immutable_binding_safe(il, &env, &self.immutable_names, kids[1]);
            if safe_literal {
                self.immutable_names.insert(name);
                if let Payload::Cid(cid) = il.node(kids[0]).payload {
                    env.insert(cid);
                }
            }
        }
    }

    fn collect_function_bindings(&mut self, il: &Il, interner: &Interner) {
        for unit in &il.units {
            if il.kind(unit.root) != NodeKind::Func {
                continue;
            }
            if function_binding_safe(il, interner, self, unit.root, unit.root) {
                self.function_roots.insert(unit.root);
            }
        }
    }
}

fn top_level_statements(il: &Il) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &stmt in il.children(il.root) {
        if il.kind(stmt) == NodeKind::Block {
            out.extend(il.children(stmt).iter().copied());
        } else {
            out.push(stmt);
        }
    }
    out
}

fn assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    let (lhs, _) = il.assignment_var_parts(stmt)?;
    let cid = il.var_cid(lhs)?;
    il.cid_names.get(cid as usize).copied()
}

fn immutable_binding_safe(
    il: &Il,
    env: &FxHashSet<u32>,
    immutable_names: &FxHashSet<Symbol>,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::Call
        | NodeKind::HoF
        | NodeKind::Func
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw
        | NodeKind::Assign => false,
        NodeKind::Var => match il.node(node).payload {
            Payload::Cid(c) => env.contains(&c),
            Payload::Name(s) => immutable_names.contains(&s),
            _ => false,
        },
        NodeKind::Lit => exact_literal_safe(il, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| immutable_binding_safe(il, env, immutable_names, c)),
    }
}

pub(crate) fn function_binding_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    root: NodeId,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::HoF
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw => false,
        NodeKind::Func if node != root => false,
        NodeKind::Call => match il.node(node).payload {
            Payload::Builtin(builtin) => admitted_builtin_semantics_at_call(il, node, builtin),
            _ => false,
        },
        NodeKind::Seq => strict_exact_safe_seq(il, interner, node),
        NodeKind::Lit => exact_literal_safe(il, node),
        NodeKind::Var => {
            strict_exact_safe_var(il, facts, node)
                || strict_exact_nullish_global_safe(il, interner, node)
                || strict_exact_rust_option_none_safe(il, interner, node)
        }
        _ => il
            .children(node)
            .iter()
            .all(|&c| function_binding_safe(il, interner, facts, root, c)),
    }
}

pub(crate) fn strict_exact_safe_tree(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw => false,
        NodeKind::Seq => {
            strict_exact_safe_seq(il, interner, node)
                && il
                    .children(node)
                    .iter()
                    .all(|&c| strict_exact_safe_tree(il, interner, facts, c))
        }
        NodeKind::Call => strict_exact_safe_call(il, interner, facts, node),
        NodeKind::HoF => strict_exact_safe_hof(il, interner, facts, node),
        NodeKind::Index
            if strict_exact_go_literal_zero_map_index_safe(il, interner, facts, node) =>
        {
            true
        }
        NodeKind::BinOp if strict_exact_static_index_membership_safe(il, interner, facts, node) => {
            true
        }
        NodeKind::BinOp if matches!(il.node(node).payload, Payload::Op(Op::In)) => {
            strict_exact_in_membership_safe(il, interner, facts, node)
        }
        NodeKind::Lit => exact_literal_safe(il, node),
        NodeKind::Var => {
            strict_exact_safe_var(il, facts, node)
                || strict_exact_nullish_global_safe(il, interner, node)
                || strict_exact_rust_option_none_safe(il, interner, node)
        }
        _ => il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c)),
    }
}

fn strict_exact_safe_hof(il: &Il, interner: &Interner, facts: &StrictFacts, node: NodeId) -> bool {
    match source_comprehension_at_node(il, node) {
        Some(SourceComprehensionKind::PythonListComprehension)
        | Some(SourceComprehensionKind::PythonDictComprehension) => {
            strict_exact_hof_children_safe(il, interner, facts, node)
        }
        Some(
            SourceComprehensionKind::PythonGeneratorExpression
            | SourceComprehensionKind::PythonSetComprehension,
        ) => false,
        None => match il.node(node).payload {
            Payload::HoF(kind)
                if admitted_hof_demand_effect_profile_at_node(il, node, kind).is_some() =>
            {
                strict_exact_hof_children_safe(il, interner, facts, node)
            }
            _ => false,
        },
    }
}

fn strict_exact_terminal_reduction_arg_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::HoF {
        return strict_exact_safe_tree(il, interner, facts, node);
    }
    match source_comprehension_at_node(il, node) {
        Some(
            SourceComprehensionKind::PythonGeneratorExpression
            | SourceComprehensionKind::PythonListComprehension,
        ) => strict_exact_hof_children_safe(il, interner, facts, node),
        Some(
            SourceComprehensionKind::PythonDictComprehension
            | SourceComprehensionKind::PythonSetComprehension,
        ) => false,
        None => match il.node(node).payload {
            Payload::HoF(kind)
                if admitted_hof_demand_effect_profile_at_node(il, node, kind).is_some() =>
            {
                strict_exact_hof_children_safe(il, interner, facts, node)
            }
            _ => false,
        },
    }
}

fn strict_exact_len_arg_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::HoF {
        return strict_exact_safe_tree(il, interner, facts, node);
    }
    match source_comprehension_at_node(il, node) {
        Some(SourceComprehensionKind::PythonListComprehension) => {
            strict_exact_hof_children_safe(il, interner, facts, node)
        }
        Some(
            SourceComprehensionKind::PythonDictComprehension
            | SourceComprehensionKind::PythonGeneratorExpression
            | SourceComprehensionKind::PythonSetComprehension,
        ) => false,
        None => match il.node(node).payload {
            Payload::HoF(kind)
                if admitted_hof_demand_effect_profile_at_node(il, node, kind)
                    .is_some_and(|profile| profile.proves_eager_per_element_callback_demand()) =>
            {
                strict_exact_hof_children_safe(il, interner, facts, node)
            }
            _ => false,
        },
    }
}

fn strict_exact_hof_children_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.children(node).iter().all(|&child| {
        if il.kind(child) == NodeKind::HoF {
            strict_exact_hof_internal_safe(il, interner, facts, child)
        } else {
            strict_exact_safe_tree(il, interner, facts, child)
        }
    })
}

fn strict_exact_hof_internal_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    matches!(
        il.node(node).payload,
        Payload::HoF(HoFKind::Map | HoFKind::FlatMap | HoFKind::Filter | HoFKind::FilterMap)
    ) && strict_exact_hof_children_safe(il, interner, facts, node)
}

fn strict_exact_in_membership_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    let Payload::Op(Op::In) = il.node(node).payload else {
        return false;
    };
    if semantics(il.meta.lang)
        .operators()
        .membership_operator(Op::In)
        .is_none()
    {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && strict_exact_safe_tree(il, interner, facts, kids[0])
        && strict_exact_in_membership_collection_safe(il, interner, facts, kids[1])
}

fn strict_exact_in_membership_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if strict_exact_proven_collection_receiver_safe(il, interner, facts, node)
        || strict_exact_proven_map_receiver_safe(il, interner, facts, node)
    {
        return true;
    }
    match il.kind(node) {
        NodeKind::Seq => strict_exact_membership_collection_safe(il, interner, facts, node),
        NodeKind::Call => {
            strict_exact_set_constructor_collection_safe(il, interner, facts, node)
                || strict_exact_python_collection_factory_safe(il, interner, facts, node)
                || strict_exact_ruby_set_factory_safe(il, interner, facts, node)
                || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node)
                || strict_exact_rust_std_collection_factory_safe(il, interner, facts, node)
                || strict_exact_java_collection_factory_safe(il, interner, facts, node)
                || strict_exact_map_key_view_collection_safe(il, interner, facts, node)
        }
        NodeKind::Var => {
            matches!(il.node(node).payload, Payload::Name(name) if facts.exact_value_name(name))
        }
        _ => false,
    }
}

fn exact_literal_safe(il: &Il, node: NodeId) -> bool {
    matches!(
        il.node(node).payload,
        Payload::LitInt(_)
            | Payload::LitBool(_)
            | Payload::LitStr(_)
            | Payload::LitFloat(_)
            | Payload::Lit(LitClass::Null)
    )
}

fn strict_exact_static_index_membership_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    let Payload::Op(op) = il.node(node).payload else {
        return false;
    };
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    if strict_exact_index_membership_threshold(il, op, false, kids[1]) {
        if let Some((element, collection)) =
            strict_exact_static_index_membership_parts(il, interner, facts, kids[0])
        {
            return strict_exact_safe_tree(il, interner, facts, element)
                && strict_exact_static_non_float_collection(il, interner, collection);
        }
    }
    if strict_exact_index_membership_threshold(il, op, true, kids[0]) {
        if let Some((element, collection)) =
            strict_exact_static_index_membership_parts(il, interner, facts, kids[1])
        {
            return strict_exact_safe_tree(il, interner, facts, element)
                && strict_exact_static_non_float_collection(il, interner, collection);
        }
    }
    false
}

fn strict_exact_static_index_membership_parts(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let admitted = admitted_static_index_membership_at_call(il, interner, node)?;
    let receiver = admitted.receiver?;
    if !strict_exact_static_non_float_collection(il, interner, receiver) {
        return None;
    }
    match admitted.contract.result.kind {
        StaticIndexMembershipKind::IndexOf => Some((kids[1], receiver)),
        StaticIndexMembershipKind::FindIndex => {
            let element = strict_exact_lambda_eq_param_element(il, interner, facts, kids[1])?;
            Some((element, receiver))
        }
    }
}

fn strict_exact_index_membership_threshold(
    il: &Il,
    op: Op,
    index_call_on_right: bool,
    threshold: NodeId,
) -> bool {
    if strict_exact_minus_one_literal(il, threshold) {
        return semantics(il.meta.lang)
            .operators()
            .static_index_membership_threshold(
                op,
                index_call_on_right,
                IndexMembershipThreshold::MinusOne,
            )
            .is_some();
    }
    if matches!(il.node(threshold).payload, Payload::LitInt(0)) {
        return semantics(il.meta.lang)
            .operators()
            .static_index_membership_threshold(
                op,
                index_call_on_right,
                IndexMembershipThreshold::Zero,
            )
            .is_some();
    }
    false
}

fn strict_exact_lambda_eq_param_element(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    lambda: NodeId,
) -> Option<NodeId> {
    if il.kind(lambda) != NodeKind::Lambda {
        return None;
    }
    let kids = il.children(lambda);
    let param = kids.iter().find_map(|&kid| {
        if il.kind(kid) != NodeKind::Param {
            return None;
        }
        if let Payload::Cid(cid) = il.node(kid).payload {
            Some(cid)
        } else {
            None
        }
    })?;
    let ret = strict_exact_first_return_expr(il, *kids.last()?)?;
    if il.kind(ret) != NodeKind::BinOp || !matches!(il.node(ret).payload, Payload::Op(Op::Eq)) {
        return None;
    }
    let source_operator = source_operator_at_node(il, ret)?;
    if !exact_static_membership_predicate_operator(il.meta.lang, Op::Eq, source_operator) {
        return None;
    }
    let ret_kids = il.children(ret);
    if ret_kids.len() != 2 {
        return None;
    }
    if strict_exact_lambda_param_var(il, ret_kids[0], param) {
        return strict_exact_safe_tree(il, interner, facts, ret_kids[1]).then_some(ret_kids[1]);
    }
    if strict_exact_lambda_param_var(il, ret_kids[1], param) {
        return strict_exact_safe_tree(il, interner, facts, ret_kids[0]).then_some(ret_kids[0]);
    }
    None
}

fn strict_exact_first_return_expr(il: &Il, node: NodeId) -> Option<NodeId> {
    if il.kind(node) == NodeKind::Return {
        return il.children(node).first().copied();
    }
    if il.kind(node) == NodeKind::Block {
        return il
            .children(node)
            .iter()
            .find_map(|&child| strict_exact_first_return_expr(il, child));
    }
    None
}

fn strict_exact_lambda_param_var(il: &Il, node: NodeId, param: u32) -> bool {
    il.kind(node) == NodeKind::Var
        && matches!(il.node(node).payload, Payload::Cid(cid) if cid == param)
}

fn strict_exact_minus_one_literal(il: &Il, node: NodeId) -> bool {
    if matches!(il.node(node).payload, Payload::LitInt(-1)) {
        return true;
    }
    if il.kind(node) != NodeKind::UnOp || !matches!(il.node(node).payload, Payload::Op(Op::Neg)) {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 1 && matches!(il.node(kids[0]).payload, Payload::LitInt(1))
}

fn strict_exact_static_non_float_collection(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    if !seq_surface_contract_for_node(il, interner, node)
        .is_some_and(|contract| contract.membership_collection)
    {
        return false;
    }
    let kids = il.children(node);
    !kids.is_empty()
        && kids.iter().all(|&kid| {
            matches!(
                il.node(kid).payload,
                Payload::LitInt(_)
                    | Payload::LitBool(_)
                    | Payload::LitStr(_)
                    | Payload::Lit(LitClass::Null)
            )
        })
}

fn strict_exact_safe_var(il: &Il, facts: &StrictFacts, node: NodeId) -> bool {
    match il.node(node).payload {
        Payload::Cid(_) => true,
        Payload::Name(name) => facts.exact_value_name(name),
        _ => false,
    }
}

fn strict_exact_nullish_global_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let (NodeKind::Var, Payload::Name(name)) = (il.kind(node), il.node(node).payload) else {
        return false;
    };
    let name = interner.resolve(name);
    let Some(contract) = nullish_global_contract(il.meta.lang, name) else {
        return false;
    };
    !contract.requires_unshadowed || asserted_unshadowed_global_symbol(il, node, contract.name)
}

fn strict_exact_rust_option_none_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    admitted_rust_option_none_sentinel_at_node(il, interner, node).is_some()
}

fn strict_exact_safe_seq(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if let Payload::Name(tag) = il.node(node).payload {
        match interner.resolve(tag) {
            "own_property_guard" => {
                return strict_exact_own_property_guard_seq_safe(il, interner, node);
            }
            "record_guard" => return record_shape_guard_for_node(il, interner, node),
            _ => {}
        }
    }
    seq_surface_contract_for_node(il, interner, node)
        .is_some_and(|contract| contract.exact_tree_safe)
}

fn strict_exact_own_property_guard_seq_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    own_property_guard_for_node(il, interner, node)
}

fn strict_exact_safe_call(il: &Il, interner: &Interner, facts: &StrictFacts, node: NodeId) -> bool {
    if let Payload::Builtin(builtin) = il.node(node).payload {
        if !admitted_builtin_semantics_at_call(il, node, builtin) {
            return false;
        }
        let kids = il.children(node);
        return match builtin {
            Builtin::Contains if kids.len() == 2 => {
                strict_exact_safe_tree(il, interner, facts, kids[0])
                    && strict_exact_membership_collection_safe(il, interner, facts, kids[1])
            }
            Builtin::Len if kids.len() == 1 => {
                if admitted_terminal_count_reduction_at_call(il, node) {
                    strict_exact_terminal_reduction_arg_safe(il, interner, facts, kids[0])
                } else {
                    strict_exact_len_arg_safe(il, interner, facts, kids[0])
                }
            }
            Builtin::Sum | Builtin::Any | Builtin::All if kids.len() == 1 => {
                strict_exact_terminal_reduction_arg_safe(il, interner, facts, kids[0])
            }
            Builtin::Min | Builtin::Max if kids.len() == 1 => {
                strict_exact_terminal_reduction_arg_safe(il, interner, facts, kids[0])
            }
            _ => kids
                .iter()
                .all(|&c| strict_exact_safe_tree(il, interner, facts, c)),
        };
    }
    if strict_exact_set_constructor_collection_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_python_collection_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_ruby_set_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_std_collection_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_vec_new_safe(il, interner, node) {
        return true;
    }
    if strict_exact_java_collection_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_java_collection_constructor_safe(il, interner, node) {
        return true;
    }
    if strict_exact_java_map_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_std_map_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_map_constructor_entries_safe(il, interner, facts, node) {
        return true;
    }
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    if strict_exact_typeof_operator_safe(il, interner, facts, node, callee) {
        return true;
    }
    if il.kind(callee) != NodeKind::Field {
        return strict_exact_callee_identity(il, interner, facts, node, callee)
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(name);
    if let Some(regex_safe) =
        strict_exact_regex_test_safe(il, interner, facts, node, callee, method)
    {
        return regex_safe;
    }
    if strict_exact_js_array_is_array_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_collection_contains_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_map_contains_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_map_get_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_map_get_default_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    if strict_exact_iterator_identity_adapter_call_safe(il, interner, facts, node, callee, method) {
        return true;
    }
    // Opaque exact method identity: this keeps same-callee calls eligible as exact clones
    // without assigning semantic meaning to the method name. Cross-language/builtin
    // convergence still has to pass the proof-backed contracts above or in normalization.
    strict_exact_callee_identity(il, interner, facts, node, callee)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_typeof_operator_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
) -> bool {
    let (NodeKind::Var, Payload::Name(name)) = (il.kind(callee), il.node(callee).payload) else {
        return false;
    };
    let Some(contract) = typeof_operator_contract(
        il.meta.lang,
        interner.resolve(name),
        il.children(node).len().saturating_sub(1),
    ) else {
        return false;
    };
    source_fact_at_node(il, node, contract.required_source_fact)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn admitted_method_call_contract(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<(LibraryMethodCallContract, usize)> {
    let admitted = admitted_library_method_call_at_call(il, interner, node)?;
    Some((admitted.contract, admitted.arg_count))
}

fn field_receiver(il: &Il, callee: NodeId) -> Option<NodeId> {
    il.children(callee).first().copied()
}

fn strict_exact_regex_test_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    _callee: NodeId,
    method: &str,
) -> Option<bool> {
    if method != "test" {
        return None;
    }
    if admitted_regex_test_at_call(il, interner, node).is_none() {
        return Some(false);
    }
    Some(strict_exact_call_args_safe(il, interner, facts, node))
}

fn strict_exact_js_array_is_array_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    _callee: NodeId,
    method: &str,
) -> bool {
    if method != "isArray" || admitted_js_array_is_array_at_call(il, interner, node).is_none() {
        return false;
    }
    strict_exact_call_args_safe(il, interner, facts, node)
}

pub(crate) fn strict_exact_collection_contains_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    _method: &str,
) -> bool {
    let Some((contract, _arg_count)) = admitted_method_call_contract(il, interner, node) else {
        return false;
    };
    let result = contract.result;
    if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains)
        || result.args != MethodBuiltinArgs::FirstThenReceiver
    {
        return false;
    }
    let receiver_safe = match result.receiver {
        MethodReceiverContract::ExactCollection
        | MethodReceiverContract::ExactCollectionOrMap
        | MethodReceiverContract::ExactCollectionOrJavaKeySet => {
            let Some(receiver) = field_receiver(il, callee) else {
                return false;
            };
            strict_exact_literal_collection_receiver_safe(il, interner, facts, receiver)
                || strict_exact_proven_collection_receiver_safe(il, interner, facts, receiver)
                || strict_exact_python_collection_factory_safe(il, interner, facts, receiver)
                || strict_exact_ruby_set_factory_safe(il, interner, facts, receiver)
                || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, receiver)
                || strict_exact_rust_std_collection_factory_safe(il, interner, facts, receiver)
                || strict_exact_java_collection_factory_safe(il, interner, facts, receiver)
                || strict_exact_map_key_view_collection_safe(il, interner, facts, receiver)
        }
        MethodReceiverContract::ExactSetOrMap => {
            let Some(receiver) = field_receiver(il, callee) else {
                return false;
            };
            strict_exact_typed_set_param_receiver_safe(il, interner, facts, receiver)
                || strict_exact_set_constructor_collection_safe(il, interner, facts, receiver)
        }
        _ => false,
    };
    receiver_safe && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_map_contains_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    _method: &str,
) -> bool {
    let Some((contract, _arg_count)) = admitted_method_call_contract(il, interner, node) else {
        return false;
    };
    let result = contract.result;
    if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains)
        || result.args != MethodBuiltinArgs::FirstThenReceiver
        || !matches!(
            result.receiver,
            MethodReceiverContract::ExactMap
                | MethodReceiverContract::ExactCollectionOrMap
                | MethodReceiverContract::ExactSetOrMap
        )
    {
        return false;
    }
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    strict_exact_map_receiver_or_factory_safe(il, interner, facts, receiver, true)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_map_get_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    method: &str,
) -> bool {
    if method != "get" || admitted_map_get_at_call(il, interner, node).is_none() {
        return false;
    }
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    strict_exact_map_receiver_or_factory_safe(il, interner, facts, receiver, false)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_map_get_default_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    _method: &str,
) -> bool {
    let Some((contract, _arg_count)) = admitted_method_call_contract(il, interner, node) else {
        return false;
    };
    let result = contract.result;
    if result.semantic != MethodSemanticContract::Builtin(Builtin::GetOrDefault)
        || result.receiver != MethodReceiverContract::ExactMap
        || !matches!(
            result.args,
            MethodBuiltinArgs::MapGetDefault | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda
        )
    {
        return false;
    }
    let Some(receiver) = field_receiver(il, callee) else {
        return false;
    };
    strict_exact_map_receiver_or_factory_safe(il, interner, facts, receiver, false)
        && strict_exact_map_get_default_args_safe(il, interner, facts, node, result.args)
}

fn strict_exact_map_receiver_or_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
    allow_rust_std_factory: bool,
) -> bool {
    strict_exact_proven_map_receiver_safe(il, interner, facts, receiver)
        || strict_exact_java_map_factory_safe(il, interner, facts, receiver)
        || strict_exact_map_constructor_entries_safe(il, interner, facts, receiver)
        || (allow_rust_std_factory
            && strict_exact_rust_std_map_factory_safe(il, interner, facts, receiver))
}

fn strict_exact_map_get_default_args_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    contract: MethodBuiltinArgs,
) -> bool {
    let kids = il.children(node);
    let [_, key, default] = kids else {
        return false;
    };
    strict_exact_safe_tree(il, interner, facts, *key)
        && match contract {
            MethodBuiltinArgs::MapGetDefault => {
                strict_exact_safe_tree(il, interner, facts, *default)
            }
            MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda => {
                strict_exact_map_default_value_arg_safe(il, interner, facts, *default)
            }
            _ => false,
        }
}

fn strict_exact_map_default_value_arg_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    default: NodeId,
) -> bool {
    if il.kind(default) != NodeKind::Lambda {
        return strict_exact_safe_tree(il, interner, facts, default);
    }
    let kids = il.children(default);
    let [body] = kids else {
        return false;
    };
    let value = implicit_single_value_body(il, *body).unwrap_or(*body);
    strict_exact_safe_tree(il, interner, facts, value)
}

fn implicit_single_value_body(il: &Il, body: NodeId) -> Option<NodeId> {
    if il.kind(body) != NodeKind::Block {
        return None;
    }
    let [stmt] = il.children(body) else {
        return None;
    };
    match il.kind(*stmt) {
        NodeKind::ExprStmt | NodeKind::Return => il.children(*stmt).first().copied(),
        _ => None,
    }
}

fn strict_exact_iterator_identity_adapter_call_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    callee: NodeId,
    method: &str,
) -> bool {
    if method.is_empty() {
        return false;
    }
    let Some(admitted) = admitted_iterator_identity_adapter_at_call(il, interner, node) else {
        return false;
    };
    let Some(receiver) = admitted.receiver else {
        return false;
    };
    if admitted.callee != callee {
        return false;
    }
    strict_exact_iterator_receiver_safe(il, interner, facts, receiver)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_iterator_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_proven_collection_receiver_safe(il, interner, facts, receiver)
        || strict_exact_literal_collection_receiver_safe(il, interner, facts, receiver)
        || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, receiver)
        || strict_exact_rust_std_collection_factory_safe(il, interner, facts, receiver)
        || strict_exact_rust_vec_new_safe(il, interner, receiver)
        || strict_exact_iterator_identity_adapter_node_safe(il, interner, facts, receiver)
}

fn strict_exact_iterator_identity_adapter_node_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    strict_exact_iterator_identity_adapter_call_safe(
        il,
        interner,
        facts,
        node,
        callee,
        interner.resolve(method),
    )
}

fn strict_exact_typed_set_param_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_receiver_safe(il, interner, facts, receiver, DomainRequirement::Set)
}

fn strict_exact_typed_collection_param_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_receiver_safe(
        il,
        interner,
        facts,
        receiver,
        DomainRequirement::ArrayCollectionOrSet,
    )
}

fn strict_exact_typed_receiver_safe(
    il: &Il,
    _interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
    requirement: DomainRequirement,
) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    facts.receiver_satisfies_domain(receiver, requirement)
}

fn strict_exact_proven_collection_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_collection_param_receiver_safe(il, interner, facts, receiver)
}

fn strict_exact_typed_map_param_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_receiver_safe(il, interner, facts, receiver, DomainRequirement::Map)
}

fn strict_exact_proven_map_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    receiver: NodeId,
) -> bool {
    strict_exact_typed_map_param_receiver_safe(il, interner, facts, receiver)
}

fn strict_exact_map_key_view_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    strict_exact_map_key_view_safe_matching(il, interner, facts, node, |kind| {
        kind == MapKeyViewKind::Collection
    })
}

fn strict_exact_map_key_view_safe_matching(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    accepts: impl Fn(MapKeyViewKind) -> bool + Copy,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 1 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Some(admitted) = admitted_map_key_view_at_call(il, interner, node) else {
        return false;
    };
    let result = admitted.contract.result;
    if !accepts(result.kind) {
        return false;
    }
    let Some(receiver) = admitted.receiver else {
        return false;
    };
    strict_exact_proven_map_receiver_safe(il, interner, facts, receiver)
        || strict_exact_map_constructor_entries_safe(il, interner, facts, receiver)
        || strict_exact_java_map_factory_safe(il, interner, facts, receiver)
        || strict_exact_rust_std_map_factory_safe(il, interner, facts, receiver)
}

fn strict_exact_map_key_view_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if strict_exact_map_key_view_safe(il, interner, facts, node) {
        return true;
    }
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Some(_admitted) = admitted_map_key_view_wrapper_at_call(il, interner, node) else {
        return false;
    };
    strict_exact_map_key_view_safe_matching(il, interner, facts, kids[1], |kind| {
        kind == MapKeyViewKind::Iterator
    })
}

fn strict_exact_literal_collection_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.kind(node) == NodeKind::Seq
        && strict_exact_membership_collection_safe(il, interner, facts, node)
}

pub(crate) fn strict_exact_membership_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        if il.kind(node) == NodeKind::Call {
            return strict_exact_set_constructor_collection_safe(il, interner, facts, node)
                || strict_exact_python_collection_factory_safe(il, interner, facts, node)
                || strict_exact_ruby_set_factory_safe(il, interner, facts, node)
                || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node)
                || strict_exact_rust_std_collection_factory_safe(il, interner, facts, node)
                || strict_exact_java_collection_factory_safe(il, interner, facts, node)
                || strict_exact_map_key_view_collection_safe(il, interner, facts, node);
        }
        if strict_exact_proven_collection_receiver_safe(il, interner, facts, node)
            || strict_exact_proven_map_receiver_safe(il, interner, facts, node)
        {
            return true;
        }
        return false;
    }
    let tag_safe = seq_surface_contract_for_node(il, interner, node)
        .is_some_and(|contract| contract.membership_collection);
    tag_safe
        && il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c))
}

pub(crate) fn strict_exact_set_constructor_collection_safe(
    il: &Il,
    interner: &Interner,
    _facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !construct_syntax_proof(il, node) {
        return false;
    };
    let Some(occurrence) = admitted_js_like_set_constructor_at_call(il, interner, node) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let [_, collection] = il.children(node) else {
        return false;
    };
    strict_exact_static_non_float_collection(il, interner, *collection)
}

pub(crate) fn strict_exact_python_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang)
        .stdlib()
        .python_collection_factories()
        || il.kind(node) != NodeKind::Call
    {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let Some(occurrence) = admitted_free_name_collection_factory_at_call(il, interner, node)
        .or_else(|| admitted_imported_collection_factory_at_call(il, interner, node))
    else {
        return false;
    };
    occurrence.arg_count == 1
        && strict_exact_membership_collection_safe(il, interner, facts, kids[1])
}

fn strict_exact_ruby_set_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let Some(occurrence) = admitted_ruby_set_factory_at_call(il, interner, node) else {
        return false;
    };
    occurrence.arg_count == 1
        && strict_exact_membership_collection_safe(il, interner, facts, kids[1])
}

fn strict_exact_rust_vec_macro_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang).stdlib().rust_vec_macro_factory() || il.kind(node) != NodeKind::Call
    {
        return false;
    }
    let kids = il.children(node);
    if admitted_rust_vec_macro_factory_at_call(il, interner, node).is_none() {
        return false;
    }
    kids.iter()
        .skip(1)
        .all(|&kid| strict_exact_safe_tree(il, interner, facts, kid))
}

/// `Vec::new()` (no args) is always the empty vector — the value graph already models it as
/// an empty `Seq`, identical to a `[]` literal (`value_graph::is_rust_vec_new_call`). Mirror
/// that in the exact-safe gate so a Rust builder loop seeded with `out = Vec::new()` enters
/// the exact channel like the `out = []` builder loops in Python/JS. Sound: it is a constant
/// empty collection, no inputs or effects.
fn strict_exact_rust_vec_new_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang).stdlib().rust_vec_new_factory() || il.kind(node) != NodeKind::Call {
        return false;
    }
    admitted_rust_vec_new_factory_at_call(il, interner, node).is_some()
}

fn strict_exact_rust_std_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang)
        .stdlib()
        .rust_std_collection_factories()
    {
        return false;
    }
    let Some(occurrence) = admitted_free_name_collection_factory_at_call(il, interner, node) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let [_, collection] = il.children(node) else {
        return false;
    };
    strict_exact_membership_collection_safe(il, interner, facts, *collection)
}

pub(crate) fn strict_exact_java_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang).stdlib().java_collection_factories()
        || il.kind(node) != NodeKind::Call
    {
        return false;
    }
    let Some(occurrence) = admitted_java_collection_factory_at_call(il, interner, node) else {
        return false;
    };
    if !matches!(
        occurrence.contract.result,
        LibraryCollectionFactoryResult::VariadicElements { .. }
    ) {
        return false;
    }
    il.children(node)
        .iter()
        .skip(1)
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

/// An empty `java.util` collection constructor (`new ArrayList<>()`, `new LinkedList<>()`)
/// canonicalizes to an empty collection in the value graph
/// (`eval_java_collection_constructor_expr`) whenever its `JavaUtilConstructor` LibraryApi
/// occurrence evidence is admitted — including when the type is authorized only by a
/// wildcard `import java.util.*;`. The exact-safe gate must agree, mirroring the same
/// admission check, so the constructor node is not left unproven (which would only pass
/// incidentally when an explicit import made the callee name a proven top-level binding).
fn strict_exact_java_collection_constructor_safe(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let Some(occurrence) = admitted_java_collection_constructor_at_call(il, interner, node) else {
        return false;
    };
    occurrence.arg_count == 0
        && matches!(
            occurrence.contract.result,
            LibraryCollectionFactoryResult::EmptySequence
        )
}

pub(crate) fn strict_exact_java_map_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang).stdlib().java_map_factories() {
        return false;
    }
    let Some(occurrence) = admitted_java_map_factory_at_call(il, interner, node) else {
        return false;
    };
    let LibraryMapFactoryResult::JavaFactory { kind } = occurrence.contract.result else {
        return false;
    };
    let args = &il.children(node)[1..];
    match kind {
        JavaMapFactoryKind::Of => {
            args.len() % 2 == 0
                && args
                    .iter()
                    .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
        }
        JavaMapFactoryKind::OfEntries => args
            .iter()
            .all(|&entry| strict_exact_java_map_entry_safe(il, interner, facts, entry)),
    }
}

fn strict_exact_java_map_entry_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    let Some(occurrence) = admitted_java_map_entry_at_call(il, interner, node) else {
        return false;
    };
    let args = &il.children(node)[1..];
    if args.len() != 2 {
        return false;
    }
    if occurrence.arg_count != 2 {
        return false;
    }
    args.iter()
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

fn strict_exact_map_constructor_entries_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !construct_syntax_proof(il, node) {
        return false;
    }
    let Some(occurrence) = admitted_js_like_map_constructor_at_call(il, interner, node) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let [_, entries] = il.children(node) else {
        return false;
    };
    matches!(
        occurrence.contract.result,
        LibraryMapFactoryResult::EntrySequence { .. }
    ) && strict_exact_map_entries_safe(il, interner, facts, *entries)
}

fn strict_exact_rust_std_map_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang).stdlib().rust_std_map_factories() {
        return false;
    }
    let Some(occurrence) = admitted_free_name_map_factory_at_call(il, interner, node) else {
        return false;
    };
    if !matches!(
        occurrence.contract.result,
        LibraryMapFactoryResult::EntrySequence { .. }
    ) {
        return false;
    }
    if occurrence.arg_count != 1 {
        return false;
    }
    let [_, entries] = il.children(node) else {
        return false;
    };
    strict_exact_map_entries_safe(il, interner, facts, *entries)
}

fn strict_exact_map_entries_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    if !seq_surface_contract_for_node(il, interner, node)
        .is_some_and(|contract| contract.map_entry_list)
    {
        return false;
    }
    il.children(node).iter().all(|&entry| {
        il.kind(entry) == NodeKind::Seq
            && il.children(entry).len() == 2
            && strict_exact_safe_tree(il, interner, facts, entry)
    })
}

fn strict_exact_go_literal_zero_map_index_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if go_zero_map_lookup_contract(il.meta.lang).is_none() || il.kind(node) != NodeKind::Index {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && strict_exact_go_literal_zero_map_safe(il, interner, facts, kids[0])
        && strict_exact_safe_tree(il, interner, facts, kids[1])
}

fn strict_exact_go_literal_zero_map_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if go_zero_map_literal_contract_for_node(il, interner, node).is_none()
        || il.children(node).is_empty()
    {
        return false;
    }
    let mut value_kind = None;
    il.children(node).iter().all(|&entry| {
        if go_zero_map_entry_contract_for_node(il, interner, entry).is_none() {
            return false;
        }
        let kv = il.children(entry);
        if kv.len() != 2
            || !matches!(il.node(kv[0]).payload, Payload::LitStr(_))
            || !strict_exact_safe_tree(il, interner, facts, kv[0])
        {
            return false;
        }
        let Some(kind) = go_zero_map_default_kind(il.meta.lang, il.node(kv[1]).payload) else {
            return false;
        };
        match value_kind {
            Some(current) if current != kind => false,
            Some(_) => true,
            None => {
                value_kind = Some(kind);
                true
            }
        }
    })
}

fn strict_exact_call_args_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.children(node)
        .iter()
        .skip(1)
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

fn strict_exact_callee_identity(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    call: NodeId,
    callee: NodeId,
) -> bool {
    let target_status = call_target_evidence_status_at_call(il, interner, call);
    match il.kind(callee) {
        NodeKind::Var => match target_status {
            CallTargetEvidenceStatus::Rejected => false,
            CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::ImportedFunction {
                ..
            }) => true,
            CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DirectFunction {
                ..
            }) => facts.direct_function_target_at_call(il, call),
            CallTargetEvidenceStatus::Admitted(
                CallTargetEvidenceKind::DirectMethod { .. }
                | CallTargetEvidenceKind::ImportedMember { .. }
                | CallTargetEvidenceKind::DynamicDispatch { .. },
            ) => false,
            CallTargetEvidenceStatus::Missing => {
                strict_exact_safe_var(il, facts, callee)
                    || facts.direct_function_target_at_call(il, call)
            }
        },
        NodeKind::Field => {
            let exact_receiver = il.children(callee).first().is_some_and(|&receiver| {
                strict_exact_callee_receiver_identity(il, facts, receiver)
            });
            if !matches!(il.node(callee).payload, Payload::Name(_)) {
                return false;
            }
            match target_status {
                CallTargetEvidenceStatus::Rejected => false,
                CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::ImportedMember {
                    ..
                }) => true,
                CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DirectMethod {
                    ..
                }) => exact_receiver && facts.direct_method_target_at_call(il, interner, call),
                CallTargetEvidenceStatus::Admitted(CallTargetEvidenceKind::DynamicDispatch {
                    ..
                }) => exact_receiver,
                CallTargetEvidenceStatus::Admitted(
                    CallTargetEvidenceKind::DirectFunction { .. }
                    | CallTargetEvidenceKind::ImportedFunction { .. },
                ) => false,
                CallTargetEvidenceStatus::Missing => exact_receiver,
            }
        }
        _ => false,
    }
}

fn strict_exact_callee_receiver_identity(il: &Il, facts: &StrictFacts, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Var => strict_exact_safe_var(il, facts, node),
        NodeKind::Field => {
            matches!(il.node(node).payload, Payload::Name(_))
                && il.children(node).first().is_some_and(|&receiver| {
                    strict_exact_callee_receiver_identity(il, facts, receiver)
                })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        stable_symbol_hash, CallTargetEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId,
        EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta,
        IlBuilder, Lang, LibraryApiEvidenceKind, Span, Unit, UnitKind,
    };
    use nose_normalize::{normalize, NormalizeOptions};
    use nose_semantics::{
        library_api_callee_contract_hash, library_api_contract_id_hash, library_map_get_contract,
        library_method_call_contract, FIRST_PARTY_PACK_ID,
    };

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, line, line)
    }

    fn normalized_python(src: &str, interner: &Interner) -> Il {
        let raw =
            nose_frontend::lower_source(FileId(0), "t.py", src.as_bytes(), Lang::Python, interner)
                .expect("lower python source");
        normalize(&raw, interner, &NormalizeOptions::default())
    }

    fn first_call_with_target(
        il: &Il,
        interner: &Interner,
        target_matches: impl Fn(CallTargetEvidenceKind) -> bool,
    ) -> NodeId {
        il.nodes
            .iter()
            .enumerate()
            .find_map(|(idx, node)| {
                if node.kind != NodeKind::Call {
                    return None;
                }
                let call = NodeId(idx as u32);
                matches!(
                    call_target_evidence_status_at_call(il, interner, call),
                    CallTargetEvidenceStatus::Admitted(target) if target_matches(target)
                )
                .then_some(call)
            })
            .expect("admitted call-target call")
    }

    fn evidence(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor,
            kind,
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("strict-exact-test")),
            },
            dependencies,
            status: EvidenceStatus::Asserted,
        }
    }

    fn method_call_library_api_evidence(
        id: u32,
        lang: Lang,
        method: &str,
        call_span: Span,
        arity: usize,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        let contract =
            library_method_call_contract(lang, method, arity).expect("method call contract");
        evidence(
            id,
            EvidenceAnchor::node(call_span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract.id),
                callee_hash: library_api_callee_contract_hash(contract.callee),
                arity: arity as u16,
            }),
            dependencies,
        )
    }

    fn map_get_library_api_evidence(
        id: u32,
        lang: Lang,
        method: &str,
        call_span: Span,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        let contract = library_map_get_contract(lang, method, 1).expect("map get contract");
        evidence(
            id,
            EvidenceAnchor::node(call_span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract.id),
                callee_hash: library_api_callee_contract_hash(contract.callee),
                arity: 1,
            }),
            dependencies,
        )
    }

    fn call_target_evidence(
        id: u32,
        call_span: Span,
        target: CallTargetEvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        evidence(
            id,
            EvidenceAnchor::node(call_span, NodeKind::Call),
            EvidenceKind::CallTarget(target),
            dependencies,
        )
    }

    #[test]
    fn strict_exact_len_rejects_pull_lazy_library_hof_arg() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(1), &[]);
        let coll = b.add(NodeKind::Seq, Payload::None, sp(1), &[item]);
        let param = b.add(NodeKind::Param, Payload::Cid(0), sp(2), &[]);
        let body_value = b.add(NodeKind::Var, Payload::Cid(0), sp(2), &[]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(2), &[body_value]);
        let body = b.add(NodeKind::Block, Payload::None, sp(2), &[ret]);
        let lambda = b.add(NodeKind::Lambda, Payload::None, sp(2), &[param, body]);
        let hof = b.add(
            NodeKind::HoF,
            Payload::HoF(HoFKind::Map),
            sp(3),
            &[coll, lambda],
        );
        let len = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Len),
            sp(4),
            &[hof],
        );
        let mut il = b.finish(
            len,
            FileMeta {
                path: "t.rs".into(),
                lang: Lang::Rust,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(method_call_library_api_evidence(
            0,
            Lang::Rust,
            "map",
            il.node(hof).span,
            1,
            Vec::new(),
        ));
        il.evidence.push(method_call_library_api_evidence(
            1,
            Lang::Rust,
            "len",
            il.node(len).span,
            0,
            Vec::new(),
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, len),
            "len must not treat an admitted pull-lazy iterator HOF as an exact materialized collection"
        );
    }

    #[test]
    fn binding_domain_does_not_make_opaque_binding_exact_value() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let lhs = b.add(NodeKind::Var, Payload::Cid(0), sp(10), &[]);
        let opaque = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("opaque")),
            sp(11),
            &[],
        );
        let rhs = b.add(NodeKind::Call, Payload::None, sp(12), &[opaque]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(10), &[lhs, rhs]);
        let use_name = b.add(NodeKind::Var, Payload::Name(xs), sp(13), &[]);
        let root = b.add(NodeKind::Block, Payload::None, sp(9), &[assign, use_name]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            vec![xs],
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(10), stable_symbol_hash("xs")),
            EvidenceKind::Domain(nose_il::DomainEvidence::Collection),
            Vec::new(),
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, use_name),
            "binding-domain evidence proves receiver capability, not exact value safety"
        );
    }

    #[test]
    fn binding_domain_after_receiver_use_does_not_prove_receiver() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(20), &[]);
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("includes")),
            sp(21),
            &[receiver],
        );
        let item = b.add(NodeKind::Lit, Payload::LitInt(7), sp(22), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(23), &[callee, item]);
        let lhs = b.add(NodeKind::Var, Payload::Cid(0), sp(30), &[]);
        let seq = b.add(NodeKind::Seq, Payload::None, sp(31), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, seq]);
        let root = b.add(NodeKind::Block, Payload::None, sp(19), &[call, assign]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            vec![xs],
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(30), stable_symbol_hash("xs")),
            EvidenceKind::Domain(nose_il::DomainEvidence::Collection),
            Vec::new(),
        ));
        il.evidence.push(method_call_library_api_evidence(
            1,
            Lang::TypeScript,
            "includes",
            sp(23),
            1,
            vec![EvidenceId(0)],
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_collection_contains_call_safe(
                &il, &interner, &facts, call, callee, "includes"
            ),
            "binding-domain evidence must be visible at the receiver use site"
        );
    }

    #[test]
    fn map_get_method_requires_library_api_occurrence_evidence() {
        let interner = Interner::new();
        let map = interner.intern("m");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(40), &[]);
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("get")),
            sp(41),
            &[receiver],
        );
        let key = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("ready")),
            sp(42),
            &[],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(43), &[callee, key]);
        let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            vec![map],
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(40), NodeKind::Var),
            EvidenceKind::Domain(nose_il::DomainEvidence::Map),
            Vec::new(),
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_map_get_call_safe(&il, &interner, &facts, call, callee, "get"),
            "receiver domain plus method spelling must not admit map-get semantics"
        );

        il.evidence.push(map_get_library_api_evidence(
            1,
            Lang::TypeScript,
            "get",
            sp(43),
            vec![EvidenceId(0)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            strict_exact_map_get_call_safe(&il, &interner, &facts, call, callee, "get"),
            "admitted map-get occurrence evidence should open the exact-safe API path"
        );
    }

    #[test]
    fn same_spelled_function_call_requires_direct_call_target_evidence() {
        let interner = Interner::new();
        let helper = interner.intern("helper");
        let mut b = IlBuilder::new(FileId(0));
        let body = b.add(NodeKind::Lit, Payload::LitInt(1), sp(40), &[]);
        let function = b.add(NodeKind::Func, Payload::None, sp(40), &[body]);
        let callee = b.add(NodeKind::Var, Payload::Name(helper), sp(50), &[]);
        let arg = b.add(NodeKind::Lit, Payload::LitInt(2), sp(51), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(52), &[callee, arg]);
        let root = b.add(NodeKind::Block, Payload::None, sp(39), &[function, call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            vec![Unit {
                root: function,
                kind: UnitKind::Function,
                name: Some(helper),
            }],
            Vec::new(),
        );

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "same spelling alone must not prove a direct function callee"
        );

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(52), NodeKind::Call),
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span: sp(40),
                name_hash: stable_symbol_hash("helper"),
            }),
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn imported_function_call_target_opens_opaque_exact_identity() {
        let interner = Interner::new();
        let prod = interner.intern("prod");
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(prod), sp(80), &[]);
        let arg = b.add(NodeKind::Lit, Payload::LitInt(2), sp(81), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(82), &[callee, arg]);
        let root = b.add(NodeKind::Block, Payload::None, sp(79), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "same local function spelling must not prove imported call identity"
        );

        il.evidence.push(call_target_evidence(
            0,
            sp(82),
            CallTargetEvidenceKind::ImportedFunction {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
                local_hash: interner.symbol_hash(prod),
            },
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn ambiguous_call_target_evidence_blocks_parameter_callee_fallback() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let callee_param = b.add(NodeKind::Param, Payload::Cid(0), sp(90), &[]);
        let value_param = b.add(NodeKind::Param, Payload::Cid(1), sp(91), &[]);
        let callee = b.add(NodeKind::Var, Payload::Cid(0), sp(92), &[]);
        let value = b.add(NodeKind::Var, Payload::Cid(1), sp(93), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(94), &[callee, value]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            sp(89),
            &[callee_param, value_param, call],
        );
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );

        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));

        il.evidence.push(call_target_evidence(
            0,
            sp(94),
            CallTargetEvidenceKind::ImportedFunction {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
                local_hash: stable_symbol_hash("prod"),
            },
            Vec::new(),
        ));
        il.evidence.push(call_target_evidence(
            1,
            sp(94),
            CallTargetEvidenceKind::ImportedFunction {
                module_hash: stable_symbol_hash("statistics"),
                exported_hash: stable_symbol_hash("prod"),
                local_hash: stable_symbol_hash("prod"),
            },
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "conflicting call-target evidence must not reopen opaque callee identity"
        );
    }

    #[test]
    fn imported_member_call_target_opens_static_member_identity() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("math")),
            sp(100),
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("prod")),
            sp(101),
            &[receiver],
        );
        let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(102), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(103), &[callee, arg]);
        let root = b.add(NodeKind::Block, Payload::None, sp(99), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "namespace/member spelling without proof is not exact call identity"
        );

        il.evidence.push(call_target_evidence(
            0,
            sp(103),
            CallTargetEvidenceKind::ImportedMember {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("math"),
                member_hash: interner.symbol_hash(interner.intern("prod")),
            },
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn normalized_imported_function_call_target_opens_opaque_exact_identity() {
        let interner = Interner::new();
        let il = normalized_python(
            "from acme.ops import transform as tx\n\ndef f(x):\n    return tx(x)\n",
            &interner,
        );
        let call = first_call_with_target(&il, &interner, |target| {
            matches!(
                target,
                CallTargetEvidenceKind::ImportedFunction {
                    module_hash,
                    exported_hash,
                    ..
                } if module_hash == stable_symbol_hash("acme.ops")
                    && exported_hash == stable_symbol_hash("transform")
            )
        });

        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn normalized_imported_namespace_member_target_opens_static_member_identity() {
        let interner = Interner::new();
        let il = normalized_python(
            "import acme.ops as ops\n\ndef f(x):\n    return ops.transform(x)\n",
            &interner,
        );
        let call = first_call_with_target(&il, &interner, |target| {
            matches!(
                target,
                CallTargetEvidenceKind::ImportedMember {
                    module_hash,
                    exported_hash,
                    member_hash,
                } if module_hash == stable_symbol_hash("acme.ops")
                    && exported_hash == stable_symbol_hash("transform")
                    && member_hash == stable_symbol_hash("transform")
            )
        });

        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn direct_method_call_target_does_not_skip_receiver_identity() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let method_body = b.add(NodeKind::Block, Payload::None, sp(110), &[]);
        let method = b.add(NodeKind::Func, Payload::None, sp(111), &[method_body]);
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("worker")),
            sp(112),
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("run")),
            sp(113),
            &[receiver],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(114), &[callee]);
        let root = b.add(NodeKind::Module, Payload::None, sp(109), &[method, call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(call_target_evidence(
            0,
            sp(114),
            CallTargetEvidenceKind::DirectMethod {
                target_span: il.node(method).span,
                receiver_type_hash: stable_symbol_hash("Worker"),
                method_hash: interner.symbol_hash(interner.intern("run")),
            },
            Vec::new(),
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "direct method target proof does not prove the receiver value identity"
        );
    }

    #[test]
    fn parameter_callee_identity_is_exact_safe_without_library_semantics() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let callee_param = b.add(NodeKind::Param, Payload::Cid(0), sp(10), &[]);
        let value_param = b.add(NodeKind::Param, Payload::Cid(1), sp(11), &[]);
        let callee = b.add(NodeKind::Var, Payload::Cid(0), sp(12), &[]);
        let value = b.add(NodeKind::Var, Payload::Cid(1), sp(13), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(14), &[callee, value]);
        let root = b.add(
            NodeKind::Func,
            Payload::None,
            sp(9),
            &[callee_param, value_param, call],
        );
        let il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root,
                kind: UnitKind::Function,
                name: None,
            }],
            Vec::new(),
        );

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            strict_exact_safe_tree(&il, &interner, &facts, call),
            "a parameter callee is opaque value identity, not library/API semantics"
        );
        assert!(strict_exact_safe_tree(&il, &interner, &facts, root));
    }

    #[test]
    fn function_name_is_not_a_membership_collection_proof() {
        let interner = Interner::new();
        let helper = interner.intern("helper");
        let mut b = IlBuilder::new(FileId(0));
        let body = b.add(NodeKind::Lit, Payload::LitInt(1), sp(60), &[]);
        let function = b.add(NodeKind::Func, Payload::None, sp(60), &[body]);
        let element = b.add(NodeKind::Lit, Payload::LitInt(2), sp(70), &[]);
        let collection = b.add(NodeKind::Var, Payload::Name(helper), sp(71), &[]);
        let membership = b.add(
            NodeKind::BinOp,
            Payload::Op(Op::In),
            sp(72),
            &[element, collection],
        );
        let root = b.add(
            NodeKind::Block,
            Payload::None,
            sp(59),
            &[function, membership],
        );
        let il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            vec![Unit {
                root: function,
                kind: UnitKind::Function,
                name: Some(helper),
            }],
            Vec::new(),
        );

        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, membership),
            "function identity must not be reused as collection receiver evidence"
        );
    }
}
