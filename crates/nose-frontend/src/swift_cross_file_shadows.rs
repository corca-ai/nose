use nose_il::{
    stable_symbol_hash, EvidenceId, EvidenceKind, EvidenceStatus, Il, Interner, Lang,
    LibraryApiEvidenceKind, NodeId, NodeKind, Payload, Symbol, SymbolEvidenceKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash, library_method_call_contract,
    swift_has_unproven_collection_parameter, swift_identifier_matches,
    SWIFT_ALL_SATISFY_DISPATCH_BARRIER_MARKER, SWIFT_COMPACT_MAP_DISPATCH_BARRIER_MARKER,
    SWIFT_FLAT_MAP_DISPATCH_BARRIER_MARKER, SWIFT_NIL_LITERAL_CONFORMANCE_MARKER,
    SWIFT_NIL_LITERAL_PROOF_BARRIER_MARKER,
};
use rustc_hash::FxHashSet;

const SWIFT_STDLIB_FACTORY_NAMES: &[&str] = &["Array", "Set", "Dictionary"];

pub(crate) fn close_shadowed_stdlib_apis(files: &mut [Il], interner: &Interner) {
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
    if shadowed.is_empty()
        && !compact_map_dispatch_ambiguous
        && !flat_map_dispatch_ambiguous
        && !all_satisfy_dispatch_ambiguous
        && !nil_literal_conformance
    {
        return;
    }
    for il in files.iter_mut().filter(|il| il.meta.lang == Lang::Swift) {
        close_shadowed_unshadowed_globals(il, &shadowed);
        if compact_map_dispatch_ambiguous || nil_literal_conformance {
            close_shadowed_compact_map(il);
        }
        if flat_map_dispatch_ambiguous {
            close_shadowed_flat_map(il);
        }
        if all_satisfy_dispatch_ambiguous {
            close_shadowed_swift_method(il, "allSatisfy", 1);
        }
    }
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
    if let Some(canonical) = SWIFT_STDLIB_FACTORY_NAMES
        .iter()
        .copied()
        .find(|expected| swift_identifier_matches(name, expected))
    {
        shadowed.insert(stable_symbol_hash(canonical));
    }
}

fn close_shadowed_unshadowed_globals(il: &mut Il, shadowed: &FxHashSet<u64>) {
    let mut ambiguous = FxHashSet::default();
    for record in &mut il.evidence {
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
    for record in &mut il.evidence {
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
        for record in &mut il.evidence {
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
