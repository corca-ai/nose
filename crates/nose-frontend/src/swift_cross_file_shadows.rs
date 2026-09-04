use nose_il::{
    stable_symbol_hash, EvidenceId, EvidenceKind, EvidenceStatus, Il, Interner, Lang,
    LibraryApiEvidenceKind, NodeId, NodeKind, Payload, Symbol, SymbolEvidenceKind,
    TypeEvidenceKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash, library_method_call_contract,
    swift_has_unproven_collection_parameter, swift_identifier_matches,
    SWIFT_ALL_SATISFY_DISPATCH_BARRIER_MARKER, SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER,
    SWIFT_DICTIONARY_DEFAULT_SUBSCRIPT_BARRIER_MARKER, SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER,
    SWIFT_NIL_LITERAL_CONFORMANCE_MARKER, SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER,
};
use rustc_hash::FxHashSet;
use serde::Serialize;
use sha2::{Digest as _, Sha256};

const SWIFT_STDLIB_SHADOW_NAMES: &[&str] = &["Array", "Set", "Dictionary", "String", "Swift"];

pub(crate) fn close_local_dictionary_default_subscript(il: &mut Il, interner: &Interner) {
    if il.meta.lang != Lang::Swift {
        return;
    }
    let close_all =
        swift_dictionary_default_subscript_barrier_declared(std::slice::from_ref(il), interner);
    let shadowed = shadowed_swift_stdlib_factory_name_hashes(std::slice::from_ref(il), interner);
    let dictionary_shadowed = shadowed.contains(&stable_symbol_hash("Dictionary"));
    let string_shadowed = shadowed.contains(&stable_symbol_hash("String"));
    let swift_shadowed = shadowed.contains(&stable_symbol_hash("Swift"));
    if close_all || dictionary_shadowed || swift_shadowed {
        close_shadowed_swift_dictionary_parameters(
            il,
            dictionary_shadowed,
            swift_shadowed,
            close_all,
        );
    }
    close_shadowed_swift_string_bindings(il, string_shadowed, swift_shadowed);
    close_shadowed_swift_string_parameters(il, string_shadowed, swift_shadowed);
}

#[derive(Serialize)]
struct SwiftGlobalFacts {
    shadowed: Vec<u64>,
    unproven_collection_parameter: bool,
    compact_map_dispatch_ambiguous: bool,
    flat_map_dispatch_ambiguous: bool,
    all_satisfy_dispatch_ambiguous: bool,
    nil_literal_conformance: bool,
    dictionary_default_subscript_ambiguous: bool,
}

impl SwiftGlobalFacts {
    fn is_active(&self) -> bool {
        !self.shadowed.is_empty()
            || self.compact_map_dispatch_ambiguous
            || self.flat_map_dispatch_ambiguous
            || self.all_satisfy_dispatch_ambiguous
            || self.nil_literal_conformance
            || self.dictionary_default_subscript_ambiguous
    }
}

fn swift_global_facts(files: &[Il], interner: &Interner) -> SwiftGlobalFacts {
    let shadowed = shadowed_swift_stdlib_factory_name_hashes(files, interner);
    let unproven_collection_parameter = swift_unproven_collection_parameter_declared(files);
    let compact_map_dispatch_ambiguous =
        swift_compact_map_dispatch_shadow_declared(files, interner)
            || unproven_collection_parameter;
    let flat_map_dispatch_ambiguous =
        swift_flat_map_dispatch_shadow_declared(files, interner) || unproven_collection_parameter;
    let all_satisfy_dispatch_ambiguous =
        swift_all_satisfy_dispatch_shadow_declared(files, interner);
    let nil_literal_conformance = swift_nil_literal_conformance_declared(files, interner);
    let dictionary_default_subscript_ambiguous =
        swift_dictionary_default_subscript_barrier_declared(files, interner);
    let mut shadowed = shadowed.into_iter().collect::<Vec<_>>();
    shadowed.sort_unstable();
    SwiftGlobalFacts {
        shadowed,
        unproven_collection_parameter,
        compact_map_dispatch_ambiguous,
        flat_map_dispatch_ambiguous,
        all_satisfy_dispatch_ambiguous,
        nil_literal_conformance,
        dictionary_default_subscript_ambiguous,
    }
}

pub(crate) fn swift_global_dependency_state(files: &[Il], interner: &Interner) -> ([u8; 32], bool) {
    let facts = swift_global_facts(files, interner);
    let payload = rmp_serde::to_vec(&facts).expect("Swift global facts are serializable");
    (Sha256::digest(payload).into(), facts.is_active())
}

pub(crate) fn close_shadowed_stdlib_apis_affected(
    files: &mut [Il],
    interner: &Interner,
    targets: &[bool],
) {
    debug_assert_eq!(files.len(), targets.len());
    let facts = swift_global_facts(files, interner);
    if !facts.is_active() {
        return;
    }
    let shadowed = facts.shadowed.into_iter().collect::<FxHashSet<_>>();
    let dictionary_name_shadowed = shadowed.contains(&stable_symbol_hash("Dictionary"));
    let string_name_shadowed = shadowed.contains(&stable_symbol_hash("String"));
    let swift_namespace_shadowed = shadowed.contains(&stable_symbol_hash("Swift"));
    for (index, il) in files.iter_mut().enumerate() {
        if !targets[index] || il.meta.lang != Lang::Swift {
            continue;
        }
        close_shadowed_unshadowed_globals(il, &shadowed);
        if dictionary_name_shadowed
            || swift_namespace_shadowed
            || facts.dictionary_default_subscript_ambiguous
        {
            close_shadowed_swift_dictionary_parameters(
                il,
                dictionary_name_shadowed,
                swift_namespace_shadowed,
                facts.dictionary_default_subscript_ambiguous,
            );
        }
        close_shadowed_swift_string_bindings(il, string_name_shadowed, swift_namespace_shadowed);
        close_shadowed_swift_string_parameters(il, string_name_shadowed, swift_namespace_shadowed);
        if facts.compact_map_dispatch_ambiguous || facts.nil_literal_conformance {
            close_shadowed_compact_map(il);
        }
        if facts.flat_map_dispatch_ambiguous {
            close_shadowed_flat_map(il);
        }
        if facts.all_satisfy_dispatch_ambiguous {
            close_shadowed_swift_method(il, "allSatisfy", 1);
        }
    }
}

fn close_shadowed_swift_string_parameters(
    il: &mut Il,
    close_unqualified: bool,
    close_qualified: bool,
) {
    let spans: FxHashSet<_> = il
        .evidence
        .iter()
        .filter(|record| {
            record.status == EvidenceStatus::Asserted
                && matches!(record.anchor, nose_il::EvidenceAnchor::Param { .. })
                && (close_unqualified
                    && record.kind
                        == EvidenceKind::Type(TypeEvidenceKind::SwiftUnqualifiedStringParameter)
                    || close_qualified
                        && record.kind
                            == EvidenceKind::Type(TypeEvidenceKind::SwiftQualifiedStringParameter))
        })
        .map(|record| record.anchor.span())
        .collect();
    let mut ambiguous = FxHashSet::default();
    for record in &mut (*il.evidence_mut()) {
        if record.status == EvidenceStatus::Asserted
            && spans.contains(&record.anchor.span())
            && (matches!(record.anchor, nose_il::EvidenceAnchor::Param { .. }))
            && (record.kind == EvidenceKind::Domain(nose_il::DomainEvidence::String)
                || matches!(
                    record.kind,
                    EvidenceKind::Type(
                        TypeEvidenceKind::SwiftUnqualifiedStringParameter
                            | TypeEvidenceKind::SwiftQualifiedStringParameter
                    )
                ))
        {
            record.status = EvidenceStatus::Ambiguous;
            ambiguous.insert(record.id);
        }
    }
    propagate_ambiguity(il, ambiguous);
}

fn close_shadowed_swift_string_bindings(
    il: &mut Il,
    close_unqualified: bool,
    close_qualified: bool,
) {
    let mut ambiguous = FxHashSet::default();
    for record in &mut (*il.evidence_mut()) {
        if record.status != EvidenceStatus::Asserted {
            continue;
        }
        let closes = close_unqualified
            && record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftUnqualifiedStringBinding)
            || close_qualified
                && record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftQualifiedStringBinding);
        if closes {
            record.status = EvidenceStatus::Ambiguous;
            ambiguous.insert(record.id);
        }
    }
    propagate_ambiguity(il, ambiguous);
}

fn swift_dictionary_default_subscript_barrier_declared(files: &[Il], interner: &Interner) -> bool {
    files
        .iter()
        .filter(|il| il.meta.lang == Lang::Swift)
        .flat_map(|il| &il.nodes)
        .any(|node| {
            matches!(node.payload, Payload::Name(name) if interner.resolve(name)
                == SWIFT_DICTIONARY_DEFAULT_SUBSCRIPT_BARRIER_MARKER)
        })
}

fn close_shadowed_swift_dictionary_parameters(
    il: &mut Il,
    close_unqualified: bool,
    close_qualified: bool,
    close_all: bool,
) {
    let mut ambiguous = FxHashSet::default();
    for record in &mut (*il.evidence_mut()) {
        if record.status != EvidenceStatus::Asserted {
            continue;
        }
        let is_dictionary_parameter = close_all
            && matches!(
                record.kind,
                EvidenceKind::Type(
                    TypeEvidenceKind::SwiftBracketDictionaryParameter
                        | TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter
                        | TypeEvidenceKind::SwiftQualifiedDictionaryParameter
                )
            )
            || close_unqualified
                && matches!(
                    record.kind,
                    EvidenceKind::Type(TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter)
                )
            || close_qualified
                && matches!(
                    record.kind,
                    EvidenceKind::Type(TypeEvidenceKind::SwiftQualifiedDictionaryParameter)
                );
        if is_dictionary_parameter {
            record.status = EvidenceStatus::Ambiguous;
            ambiguous.insert(record.id);
        }
    }
    propagate_ambiguity(il, ambiguous);
}

fn swift_all_satisfy_dispatch_shadow_declared(files: &[Il], interner: &Interner) -> bool {
    files
        .iter()
        .filter(|il| il.meta.lang == Lang::Swift)
        .flat_map(|il| &il.nodes)
        .any(|node| {
            matches!(node.payload, Payload::Name(name) if interner.resolve(name)
                == SWIFT_ALL_SATISFY_DISPATCH_BARRIER_MARKER)
        })
}

fn swift_flat_map_dispatch_shadow_declared(files: &[Il], interner: &Interner) -> bool {
    let method_declared = files
        .iter()
        .filter(|il| il.meta.lang == Lang::Swift)
        .flat_map(|il| &il.units)
        .any(|unit| {
            unit.name.is_some_and(|name| {
                ["flatMap", "filter", "map"]
                    .into_iter()
                    .any(|expected| swift_identifier_matches(interner.resolve(name), expected))
            })
        });
    method_declared
        || files
            .iter()
            .filter(|il| il.meta.lang == Lang::Swift)
            .flat_map(|il| &il.nodes)
            .any(|node| {
                matches!(node.payload, Payload::Name(name) if interner.resolve(name)
                    == SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER)
            })
}

fn swift_nil_literal_conformance_declared(files: &[Il], interner: &Interner) -> bool {
    files
        .iter()
        .filter(|il| il.meta.lang == Lang::Swift)
        .flat_map(|il| &il.nodes)
        .any(|node| {
            matches!(node.payload, Payload::Name(name) if matches!(
                interner.resolve(name),
                SWIFT_NIL_LITERAL_CONFORMANCE_MARKER | SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER
            ))
        })
}

fn swift_compact_map_dispatch_shadow_declared(files: &[Il], interner: &Interner) -> bool {
    let method_declared = files
        .iter()
        .filter(|il| il.meta.lang == Lang::Swift)
        .flat_map(|il| &il.units)
        .any(|unit| {
            unit.name.is_some_and(|name| {
                ["compactMap", "filter", "map"]
                    .into_iter()
                    .any(|expected| swift_identifier_matches(interner.resolve(name), expected))
            })
        });
    method_declared
        || files
            .iter()
            .filter(|il| il.meta.lang == Lang::Swift)
            .flat_map(|il| &il.nodes)
            .any(|node| {
                matches!(node.payload, Payload::Name(name) if interner.resolve(name)
                    == SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER)
            })
}

fn swift_unproven_collection_parameter_declared(files: &[Il]) -> bool {
    files
        .iter()
        .filter(|il| il.meta.lang == Lang::Swift)
        .any(swift_has_unproven_collection_parameter)
}

fn shadowed_swift_stdlib_factory_name_hashes(files: &[Il], interner: &Interner) -> FxHashSet<u64> {
    let mut shadowed = FxHashSet::default();
    for il in files.iter().filter(|il| il.meta.lang == Lang::Swift) {
        for unit in &il.units {
            if let Some(symbol) = unit.name {
                insert_stdlib_factory_name_hash(&mut shadowed, interner, symbol);
            }
        }
        for id in il
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, _)| NodeId(idx as u32))
        {
            let node = il.node(id);
            let Payload::Name(symbol) = node.payload else {
                continue;
            };
            if node.kind == NodeKind::Block && il.children(id).is_empty() {
                insert_stdlib_factory_name_hash(&mut shadowed, interner, symbol);
            }
        }
    }
    shadowed
}

fn insert_stdlib_factory_name_hash(
    shadowed: &mut FxHashSet<u64>,
    interner: &Interner,
    symbol: Symbol,
) {
    let name = interner.resolve(symbol);
    if let Some(canonical) = SWIFT_STDLIB_SHADOW_NAMES
        .iter()
        .copied()
        .find(|expected| swift_identifier_matches(name, expected))
    {
        shadowed.insert(stable_symbol_hash(canonical));
    }
}

fn close_shadowed_unshadowed_globals(il: &mut Il, shadowed: &FxHashSet<u64>) {
    let mut ambiguous = FxHashSet::default();
    for record in &mut (*il.evidence_mut()) {
        if record.status != EvidenceStatus::Asserted {
            continue;
        }
        if matches!(
            record.kind,
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal { name_hash })
                if shadowed.contains(&name_hash)
        ) {
            record.status = EvidenceStatus::Ambiguous;
            ambiguous.insert(record.id);
        }
    }
    propagate_ambiguity(il, ambiguous);
}

fn close_shadowed_compact_map(il: &mut Il) {
    close_shadowed_swift_hof(il, "compactMap");
}

fn close_shadowed_flat_map(il: &mut Il) {
    close_shadowed_swift_hof(il, "flatMap");
}

fn close_shadowed_swift_hof(il: &mut Il, method: &str) {
    close_shadowed_swift_method(il, method, 1);
}

fn close_shadowed_swift_method(il: &mut Il, method: &str, arity: usize) {
    let contract =
        library_method_call_contract(Lang::Swift, method, arity).expect("known Swift method");
    let expected = EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
        contract_hash: library_api_contract_id_hash(contract.id),
        callee_hash: library_api_callee_contract_hash(contract.callee),
        arity: arity as u16,
    });
    let mut ambiguous = FxHashSet::default();
    for record in &mut (*il.evidence_mut()) {
        if record.status == EvidenceStatus::Asserted && record.kind == expected {
            record.status = EvidenceStatus::Ambiguous;
            ambiguous.insert(record.id);
        }
    }
    propagate_ambiguity(il, ambiguous);
}

fn propagate_ambiguity(il: &mut Il, mut ambiguous: FxHashSet<EvidenceId>) {
    if ambiguous.is_empty() {
        return;
    }
    loop {
        let mut changed = false;
        for record in &mut (*il.evidence_mut()) {
            if record.status != EvidenceStatus::Asserted {
                continue;
            }
            if record
                .dependencies
                .iter()
                .any(|dependency| ambiguous.contains(dependency))
            {
                record.status = EvidenceStatus::Ambiguous;
                changed |= ambiguous.insert(record.id);
            }
        }
        if !changed {
            break;
        }
    }
}
