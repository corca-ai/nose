use nose_il::{
    stable_symbol_hash, EvidenceId, EvidenceKind, EvidenceStatus, Il, Interner, Lang, NodeId,
    NodeKind, Payload, Symbol, SymbolEvidenceKind,
};
use rustc_hash::FxHashSet;

const SWIFT_STDLIB_FACTORY_NAMES: &[&str] = &["Array", "Set", "Dictionary"];

pub(crate) fn close_shadowed_stdlib_factories(files: &mut [Il], interner: &Interner) {
    let shadowed = shadowed_swift_stdlib_factory_name_hashes(files, interner);
    if shadowed.is_empty() {
        return;
    }
    for il in files.iter_mut().filter(|il| il.meta.lang == Lang::Swift) {
        close_shadowed_unshadowed_globals(il, &shadowed);
    }
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
    if SWIFT_STDLIB_FACTORY_NAMES.contains(&name) {
        shadowed.insert(stable_symbol_hash(name));
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
