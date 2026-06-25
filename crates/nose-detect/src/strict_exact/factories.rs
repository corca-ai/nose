use super::*;

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

pub(super) fn strict_exact_ruby_set_factory_safe(
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

pub(super) fn strict_exact_rust_vec_macro_collection_safe(
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
pub(super) fn strict_exact_rust_vec_new_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !semantics(il.meta.lang).stdlib().rust_vec_new_factory() || il.kind(node) != NodeKind::Call {
        return false;
    }
    admitted_rust_vec_new_factory_at_call(il, interner, node).is_some()
}

pub(super) fn strict_exact_rust_std_collection_factory_safe(
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

pub(crate) fn strict_exact_swift_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    strict_exact_swift_collection_factory_safe_matching(il, interner, facts, node, |kind| {
        kind == SwiftCollectionFactoryKind::Set
    })
}

pub(super) fn strict_exact_swift_membership_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    strict_exact_swift_collection_factory_safe_matching(il, interner, facts, node, |_| true)
}

fn strict_exact_swift_collection_factory_safe_matching(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
    accepts: impl Fn(SwiftCollectionFactoryKind) -> bool,
) -> bool {
    if !semantics(il.meta.lang)
        .stdlib()
        .swift_collection_factories()
        || il.kind(node) != NodeKind::Call
    {
        return false;
    }
    let Some(occurrence) = admitted_swift_collection_factory_at_call(il, interner, node) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let LibraryApiContractId::SwiftCollectionFactory(kind) = occurrence.contract.id else {
        return false;
    };
    if !accepts(kind) {
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
    let args = &il.children(node)[1..];
    match occurrence.contract.result {
        LibraryCollectionFactoryResult::EmptySequence if !args.is_empty() => return false,
        LibraryCollectionFactoryResult::EmptySequence => return true,
        LibraryCollectionFactoryResult::ElementArguments if args.len() != 1 => return false,
        LibraryCollectionFactoryResult::ElementArguments
        | LibraryCollectionFactoryResult::VariadicElements { .. } => {}
        _ => return false,
    }
    if matches!(
        occurrence.contract.id,
        LibraryApiContractId::JavaCollectionFactory(kind)
            if java_collection_factory_rejects_null_literal(kind)
    ) && nodes_contain_static_null_literal(il, args.iter().copied())
    {
        return false;
    }
    args.iter()
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

/// An empty `java.util` collection constructor (`new ArrayList<>()`, `new LinkedList<>()`)
/// canonicalizes to an empty collection in the value graph
/// (`eval_java_collection_constructor_expr`) whenever its `JavaUtilConstructor` LibraryApi
/// occurrence evidence is admitted — including when the type is authorized only by a
/// wildcard `import java.util.*;`. The exact-safe gate must agree, mirroring the same
/// admission check, so the constructor node is not left unproven (which would only pass
/// incidentally when an explicit import made the callee name a proven top-level binding).
pub(super) fn strict_exact_java_collection_constructor_safe(
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
        kind if java_map_factory_uses_positional_entries(kind) => {
            java_map_factory_positional_arg_count_supported(kind, args.len())
                && strict_exact_java_map_positional_args_safe(il, kind, args)
                && args
                    .iter()
                    .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
        }
        JavaMapFactoryKind::OfEntries => args
            .iter()
            .all(|&entry| strict_exact_java_map_entry_safe(il, interner, facts, entry)),
        _ => false,
    }
}

fn strict_exact_java_map_positional_args_safe(
    il: &Il,
    kind: JavaMapFactoryKind,
    args: &[NodeId],
) -> bool {
    if kind != JavaMapFactoryKind::GuavaImmutableMapOf {
        return true;
    }
    !nodes_contain_static_null_literal(il, args.iter().copied())
        && !nodes_contain_duplicate_static_literal_keys(il, args.iter().step_by(2).copied())
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

pub(super) fn strict_exact_map_constructor_entries_safe(
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

pub(super) fn strict_exact_rust_std_map_factory_safe(
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

pub(crate) fn strict_exact_swift_map_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if !semantics(il.meta.lang)
        .stdlib()
        .swift_collection_factories()
        || il.kind(node) != NodeKind::Call
    {
        return false;
    }
    let Some(occurrence) = admitted_swift_map_factory_at_call(il, interner, node) else {
        return false;
    };
    if occurrence.arg_count != 1 {
        return false;
    }
    let [_, kwarg] = il.children(node) else {
        return false;
    };
    if il.kind(*kwarg) != NodeKind::KwArg {
        return false;
    }
    let [entries] = il.children(*kwarg) else {
        return false;
    };
    strict_exact_swift_dictionary_entries_safe(il, interner, facts, *entries)
}

fn strict_exact_swift_dictionary_entries_safe(
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
    let mut keys = Vec::new();
    for &entry in il.children(node) {
        if seq_surface_contract_for_node(il, interner, entry)
            .is_none_or(|contract| contract.value_tag != SEQ_VALUE_TUPLE)
        {
            return false;
        }
        let [key, value] = il.children(entry) else {
            return false;
        };
        keys.push(*key);
        if !strict_exact_safe_tree(il, interner, facts, *key)
            || !strict_exact_safe_tree(il, interner, facts, *value)
        {
            return false;
        }
    }
    !nodes_contain_duplicate_static_literal_keys(il, keys)
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

pub(super) fn strict_exact_go_literal_zero_map_index_safe(
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

pub(super) fn strict_exact_swift_default_subscript_index_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Swift || il.kind(node) != NodeKind::Index {
        return false;
    }
    let [receiver, index] = il.children(node) else {
        return false;
    };
    if !strict_exact_proven_map_receiver_safe(il, interner, facts, *receiver) {
        return false;
    }
    if il.kind(*index) != NodeKind::Seq {
        return false;
    }
    if !matches!(
        il.node(*index).payload,
        Payload::Name(tag) if stable_symbol_hash(interner.resolve(tag))
            == stable_symbol_hash("swift_subscript_default")
    ) {
        return false;
    }
    let [key, default] = il.children(*index) else {
        return false;
    };
    strict_exact_safe_tree(il, interner, facts, *key)
        && strict_exact_safe_tree(il, interner, facts, *default)
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
