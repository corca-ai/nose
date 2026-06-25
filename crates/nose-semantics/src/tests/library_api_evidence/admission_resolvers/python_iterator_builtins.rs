use super::*;

fn python_free_call_il(
    name: &str,
    arg_count: usize,
) -> (Il, Interner, NodeId, NodeId, Vec<NodeId>) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern(name)),
        sp(700),
        &[],
    );
    let args = (0..arg_count)
        .map(|idx| {
            b.add(
                NodeKind::Var,
                Payload::Cid((idx + 1) as u32),
                sp(701 + idx as u32),
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(callee);
    children.extend(args.iter().copied());
    let call = b.add(NodeKind::Call, Payload::None, sp(710), &children);
    let root = b.add(NodeKind::Func, Payload::None, sp(711), &[call]);
    (
        finish_il(b, root, Lang::Python),
        interner,
        call,
        callee,
        args,
    )
}

fn push_unshadowed_builtin_symbol(il: &mut Il, callee: NodeId, id: u32, name: &str) {
    il.evidence.push(language_core_symbol_record(
        id,
        EvidenceAnchor::node(il.node(callee).span, NodeKind::Var),
        SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash(name),
        },
        EvidenceStatus::Asserted,
        &[],
        Lang::Python,
    ));
}

fn push_source_collection_domain(il: &mut Il, source: NodeId, id: u32) {
    il.evidence.push(language_core_evidence(
        id,
        EvidenceAnchor::node(il.node(source).span, il.kind(source)),
        EvidenceKind::Domain(DomainEvidence::Collection),
        EvidenceStatus::Asserted,
        Lang::Python,
    ));
}

#[test]
fn python_iterator_hof_requires_iterator_pack_and_source_proof() {
    let (il, interner, call, _callee, _args) = python_free_call_il("map", 2);
    assert!(
        admitted_free_function_hof_at_call(&il, &interner, call).is_none(),
        "raw Python map(...) shape must not admit lazy HOF semantics"
    );

    let contract =
        library_free_function_hof_contract(Lang::Python, "map", 2).expect("Python map HOF");

    let (mut missing_source, interner, call, callee, _args) = python_free_call_il("map", 2);
    push_unshadowed_builtin_symbol(&mut missing_source, callee, 0, "map");
    missing_source
        .evidence
        .push(python_iterator_builtin_protocol_record(
            1,
            missing_source.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[0],
        ));
    assert!(
        admitted_free_function_hof_at_call(&missing_source, &interner, call).is_none(),
        "Python map evidence without source iterable proof is rejected"
    );

    let (mut wrong_pack, interner, call, callee, args) = python_free_call_il("map", 2);
    push_unshadowed_builtin_symbol(&mut wrong_pack, callee, 0, "map");
    push_source_collection_domain(&mut wrong_pack, args[1], 1);
    wrong_pack
        .evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            wrong_pack.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[0, 1],
            FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID,
            FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID,
        ));
    assert!(
        admitted_free_function_hof_at_call(&wrong_pack, &interner, call).is_none(),
        "Python iterator HOF evidence under the generic free-function pack is rejected"
    );

    let (mut admitted, interner, call, callee, args) = python_free_call_il("map", 2);
    push_unshadowed_builtin_symbol(&mut admitted, callee, 0, "map");
    push_source_collection_domain(&mut admitted, args[1], 1);
    admitted
        .evidence
        .push(python_iterator_builtin_protocol_record(
            2,
            admitted.node(call).span,
            contract.id,
            contract.callee,
            2,
            EvidenceStatus::Asserted,
            &[0, 1],
        ));
    let occurrence = admitted_free_function_hof_at_call(&admitted, &interner, call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::FreeFunctionHof(HoFKind::Map)
    );
    assert_eq!(
        occurrence.contract.pack_id,
        PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID
    );
    assert_eq!(occurrence.contract.result.source_arg, 1);
    assert_eq!(occurrence.contract.result.callback_arg, 0);
}

#[test]
fn python_iterator_hof_rejects_nested_non_iterable_api_dependency() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let any_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("any")),
        sp(720),
        &[],
    );
    let values = b.add(NodeKind::Var, Payload::Cid(1), sp(721), &[]);
    let any_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(722),
        &[any_callee, values],
    );
    let inner_map_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("map")),
        sp(723),
        &[],
    );
    let inner_callback = b.add(NodeKind::Var, Payload::Cid(2), sp(724), &[]);
    let inner_map = b.add(
        NodeKind::Call,
        Payload::None,
        sp(725),
        &[inner_map_callee, inner_callback, any_call],
    );
    let outer_map_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("map")),
        sp(726),
        &[],
    );
    let outer_callback = b.add(NodeKind::Var, Payload::Cid(3), sp(727), &[]);
    let outer_map = b.add(
        NodeKind::Call,
        Payload::None,
        sp(728),
        &[outer_map_callee, outer_callback, inner_map],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(729), &[outer_map]);
    let mut il = finish_il(b, root, Lang::Python);

    let any_contract = library_free_function_builtin_contract(Lang::Python, "any", 1).unwrap();
    let map_contract = library_free_function_hof_contract(Lang::Python, "map", 2).unwrap();
    push_unshadowed_builtin_symbol(&mut il, any_callee, 0, "any");
    push_source_collection_domain(&mut il, values, 1);
    il.evidence.push(python_iterator_builtin_protocol_record(
        2,
        il.node(any_call).span,
        any_contract.id,
        any_contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));
    push_unshadowed_builtin_symbol(&mut il, inner_map_callee, 3, "map");
    il.evidence.push(python_iterator_builtin_protocol_record(
        4,
        il.node(inner_map).span,
        map_contract.id,
        map_contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[3, 2],
    ));
    push_unshadowed_builtin_symbol(&mut il, outer_map_callee, 5, "map");
    il.evidence.push(python_iterator_builtin_protocol_record(
        6,
        il.node(outer_map).span,
        map_contract.id,
        map_contract.callee,
        2,
        EvidenceStatus::Asserted,
        &[5, 4],
    ));

    assert!(
        admitted_free_function_hof_at_call(&il, &interner, inner_map).is_none(),
        "an any/all terminal API record must not prove an iterable source for map"
    );
    assert!(
        admitted_free_function_hof_at_call(&il, &interner, outer_map).is_none(),
        "nested map evidence must not become iterable proof when its own source obligation fails"
    );
}

#[test]
fn python_iterator_terminal_accepts_generator_source_fact() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let any_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("any")),
        sp(730),
        &[],
    );
    let generator = b.add(NodeKind::HoF, Payload::HoF(HoFKind::Map), sp(731), &[]);
    let any_call = b.add(
        NodeKind::Call,
        Payload::None,
        sp(732),
        &[any_callee, generator],
    );
    let root = b.add(NodeKind::Func, Payload::None, sp(733), &[any_call]);
    let mut il = finish_il(b, root, Lang::Python);
    let contract = library_free_function_builtin_contract(Lang::Python, "any", 1).unwrap();

    push_unshadowed_builtin_symbol(&mut il, any_callee, 0, "any");
    let (source_pack_id, source_producer_id) = language_source_fact_provenance(Lang::Python);
    il.evidence.push(EvidenceRecord {
        id: EvidenceId(1),
        anchor: EvidenceAnchor::source_span(il.node(generator).span),
        kind: EvidenceKind::Source(SourceFactKind::Comprehension(
            SourceComprehensionKind::PythonGeneratorExpression,
        )),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: Some(stable_symbol_hash(source_pack_id)),
            rule_hash: Some(stable_symbol_hash(source_producer_id)),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });
    il.evidence.push(python_iterator_builtin_protocol_record(
        2,
        il.node(any_call).span,
        contract.id,
        contract.callee,
        1,
        EvidenceStatus::Asserted,
        &[0, 1],
    ));

    let occurrence = admitted_free_function_builtin_at_call(&il, &interner, any_call).unwrap();
    assert_eq!(
        occurrence.contract.id,
        LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any)
    );
}

#[test]
fn python_iterator_builtin_pack_covers_filter_zip_enumerate_and_terminals() {
    for (name, arity, source_args, expected) in [
        (
            "filter",
            2,
            &[1usize][..],
            LibraryApiContractId::FreeFunctionHof(HoFKind::Filter),
        ),
        (
            "zip",
            2,
            &[0usize, 1][..],
            LibraryApiContractId::FreeFunctionBuiltin(Builtin::Zip),
        ),
        (
            "enumerate",
            1,
            &[0usize][..],
            LibraryApiContractId::FreeFunctionBuiltin(Builtin::Enumerate),
        ),
        (
            "any",
            1,
            &[0usize][..],
            LibraryApiContractId::FreeFunctionBuiltin(Builtin::Any),
        ),
        (
            "all",
            1,
            &[0usize][..],
            LibraryApiContractId::FreeFunctionBuiltin(Builtin::All),
        ),
    ] {
        let (mut il, interner, call, callee, args) = python_free_call_il(name, arity);
        push_unshadowed_builtin_symbol(&mut il, callee, 0, name);
        let mut deps = vec![0];
        for (offset, &arg_idx) in source_args.iter().enumerate() {
            let id = (offset + 1) as u32;
            push_source_collection_domain(&mut il, args[arg_idx], id);
            deps.push(id);
        }
        let callee_contract = match expected {
            LibraryApiContractId::FreeFunctionHof(_) => {
                library_free_function_hof_contract(Lang::Python, name, arity)
                    .expect("Python HOF contract")
                    .callee
            }
            LibraryApiContractId::FreeFunctionBuiltin(_) => {
                library_free_function_builtin_contract(Lang::Python, name, arity)
                    .expect("Python builtin contract")
                    .callee
            }
            _ => unreachable!(),
        };
        il.evidence.push(python_iterator_builtin_protocol_record(
            10,
            il.node(call).span,
            expected,
            callee_contract,
            arity as u16,
            EvidenceStatus::Asserted,
            &deps,
        ));

        match expected {
            LibraryApiContractId::FreeFunctionHof(kind) => {
                let occurrence = admitted_free_function_hof_at_call(&il, &interner, call).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::FreeFunctionHof(kind)
                );
            }
            LibraryApiContractId::FreeFunctionBuiltin(builtin) => {
                let occurrence =
                    admitted_free_function_builtin_at_call(&il, &interner, call).unwrap();
                assert_eq!(
                    occurrence.contract.id,
                    LibraryApiContractId::FreeFunctionBuiltin(builtin)
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn python_iterator_pack_does_not_open_deferred_or_unsupported_surfaces() {
    assert!(
        library_free_function_hof_contract(Lang::Python, "map", 3).is_none(),
        "multi-iterable Python map stays unsupported until callback arity is modeled"
    );
    assert!(
        library_free_function_hof_contract(Lang::Python, "sorted", 1).is_none(),
        "sorted needs ordering/key semantics and stays out of this iterator slice"
    );
    assert!(
        library_free_function_builtin_contract(Lang::Python, "reversed", 1).is_none(),
        "reversed stays unsupported until ordering semantics are explicit"
    );
}
