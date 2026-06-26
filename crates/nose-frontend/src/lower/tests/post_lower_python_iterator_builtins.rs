use super::*;
use nose_il::HoFKind;

#[test]
fn post_lowering_emits_python_iterator_builtin_occurrences() {
    let interner = Interner::new();
    let py_iterators = lower_fixture(
        "iterator_builtins.py",
        br#"from typing import List

def m(values: List[int]):
    return map(lambda x: x, values)

def f(values: List[int]):
    return filter(lambda x: x, values)

def z(left: List[int], right: List[int]):
    return zip(left, right)

def e(values: List[int]):
    return enumerate(values)

def a(values: List[int]):
    return any(values)

def al(values: List[int]):
    return all(values)
"#,
        Lang::Python,
        &interner,
    );

    for (name, kind) in [("map", HoFKind::Map), ("filter", HoFKind::Filter)] {
        let contract = library_free_function_hof_contract(Lang::Python, name, 2).unwrap();
        let api_records =
            contract_api_records(&py_iterators.evidence, contract.id, contract.callee);
        assert_eq!(
            api_records.len(),
            1,
            "{name} should emit one iterator HOF API"
        );
        assert_eq!(contract.id, LibraryApiContractId::FreeFunctionHof(kind));
        assert_python_iterator_pack_record(api_records[0]);
        assert!(
            api_records[0].dependencies.len() >= 2,
            "{name} API must depend on both builtin symbol and iterable source proof"
        );
        assert!(result_domain_depends_on_any_api(
            &py_iterators.evidence,
            DomainEvidence::Iterator,
            &[api_records[0].id],
        ));
        let call = call_with_callee_named(&py_iterators, &interner, name).unwrap();
        assert!(
            nose_semantics::admitted_free_function_hof_at_call(&py_iterators, &interner, call)
                .is_some(),
            "{name} evidence emitted by lowering must admit through the semantic kernel"
        );
    }

    for (name, builtin, arity, min_deps, has_iterator_domain) in [
        ("zip", Builtin::Zip, 2, 3, true),
        ("enumerate", Builtin::Enumerate, 1, 2, true),
        ("any", Builtin::Any, 1, 2, false),
        ("all", Builtin::All, 1, 2, false),
    ] {
        let contract = library_free_function_builtin_contract(Lang::Python, name, arity).unwrap();
        let api_records =
            contract_api_records(&py_iterators.evidence, contract.id, contract.callee);
        assert_eq!(
            api_records.len(),
            1,
            "{name} should emit one iterator builtin API"
        );
        assert_eq!(
            contract.id,
            LibraryApiContractId::FreeFunctionBuiltin(builtin)
        );
        assert_python_iterator_pack_record(api_records[0]);
        assert!(
            api_records[0].dependencies.len() >= min_deps,
            "{name} API must keep its symbol/source proof dependencies"
        );
        let span = call_span_with_callee_named(&py_iterators, &interner, name).unwrap();
        let has_domain = result_domain_depends_on_api_at_node(
            &py_iterators.evidence,
            span,
            NodeKind::Call,
            DomainEvidence::Iterator,
            &[api_records[0].id],
        );
        assert_eq!(
            has_domain, has_iterator_domain,
            "{name} iterator result-domain boundary changed"
        );
        let call = call_with_callee_named(&py_iterators, &interner, name).unwrap();
        assert!(
            nose_semantics::admitted_free_function_builtin_at_call(&py_iterators, &interner, call)
                .is_some(),
            "{name} evidence emitted by lowering must admit through the semantic kernel"
        );
    }

    let py_len_contract = library_free_function_builtin_contract(Lang::Python, "len", 1).unwrap();
    let py_len = lower_fixture(
        "len_pack.py",
        b"def f(values):\n    return len(values)\n",
        Lang::Python,
        &interner,
    );
    let len_records =
        contract_api_records(&py_len.evidence, py_len_contract.id, py_len_contract.callee);
    assert_eq!(len_records.len(), 1);
    assert_eq!(
        len_records[0].provenance.pack_hash,
        Some(stable_symbol_hash(FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID)),
        "non-iterator free-name builtins stay in the generic free-function pack"
    );

    let any_generator = lower_fixture(
        "iterator_any_generator.py",
        b"def f(values):\n    return any(x > 0 for x in values)\n",
        Lang::Python,
        &interner,
    );
    let any_contract = library_free_function_builtin_contract(Lang::Python, "any", 1).unwrap();
    let any_records = contract_api_records(
        &any_generator.evidence,
        any_contract.id,
        any_contract.callee,
    );
    assert_eq!(any_records.len(), 1);
    assert_python_iterator_pack_record(any_records[0]);
    let any_call = call_with_callee_named(&any_generator, &interner, "any").unwrap();
    assert!(
        nose_semantics::admitted_free_function_builtin_at_call(
            &any_generator,
            &interner,
            any_call
        )
        .is_some(),
        "Python any(...) over a generator expression keeps the existing terminal-reduction source proof"
    );

    let no_source = lower_fixture(
        "iterator_no_source.py",
        b"def f(values):\n    return map(lambda x: x, values)\n",
        Lang::Python,
        &interner,
    );
    let map_contract = library_free_function_hof_contract(Lang::Python, "map", 2).unwrap();
    assert_eq!(
        contract_api_count(&no_source.evidence, map_contract.id, map_contract.callee),
        0,
        "Python iterator HOFs require source iterable proof before post-lower admission"
    );

    let wildcard = lower_fixture(
        "iterator_wildcard.py",
        b"from custom import *\nfrom typing import List\n\ndef f(values: List[int]):\n    return map(lambda x: x, values)\n",
        Lang::Python,
        &interner,
    );
    assert_eq!(
        contract_api_count(&wildcard.evidence, map_contract.id, map_contract.callee),
        0,
        "wildcard imports keep Python iterator builtins closed"
    );

    let nested_terminal_source = lower_fixture(
        "iterator_nested_terminal.py",
        b"from typing import List\n\ndef f(values: List[int]):\n    return map(lambda y: y, map(lambda x: x, any(values)))\n",
        Lang::Python,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &nested_terminal_source.evidence,
            map_contract.id,
            map_contract.callee
        ),
        0,
        "an any/all terminal API record must not become iterable source proof for nested map"
    );

    let reassigned_param = lower_fixture(
        "iterator_reassigned_param.py",
        b"from typing import List\n\ndef f(values: List[int]):\n    values = False\n    return map(lambda x: x, values)\n",
        Lang::Python,
        &interner,
    );
    assert_eq!(
        contract_api_count(
            &reassigned_param.evidence,
            map_contract.id,
            map_contract.callee
        ),
        0,
        "param-domain evidence must not prove a source after the binding is reassigned"
    );
}

#[test]
fn post_lowering_rechecks_prior_python_iterator_source_api_dependencies() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let inner_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("map")),
        sp_at(10),
        &[],
    );
    let inner_lambda = b.add(NodeKind::Lambda, Payload::None, sp_at(11), &[]);
    let values = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("values")),
        sp_at(12),
        &[],
    );
    let inner_map = b.add(
        NodeKind::Call,
        Payload::None,
        sp_at(13),
        &[inner_callee, inner_lambda, values],
    );
    let outer_callee = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("map")),
        sp_at(14),
        &[],
    );
    let outer_lambda = b.add(NodeKind::Lambda, Payload::None, sp_at(15), &[]);
    let outer_map = b.add(
        NodeKind::Call,
        Payload::None,
        sp_at(16),
        &[outer_callee, outer_lambda, inner_map],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp_at(17), &[outer_map]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "post-lower-invalid-prior-map.py".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::Python);
    let inner_symbol = il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(sp_at(10), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("map"),
        }),
        pack_id,
        producer_id,
        Vec::new(),
    );
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(sp_at(14), NodeKind::Var),
        EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
            name_hash: stable_symbol_hash("map"),
        }),
        pack_id,
        producer_id,
        Vec::new(),
    );
    let contract = library_free_function_hof_contract(Lang::Python, "map", 2).unwrap();
    let legacy_inner_api = il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(sp_at(13), NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(contract.id),
            callee_hash: library_api_callee_contract_hash(contract.callee),
            arity: 2,
        }),
        PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID,
        PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID,
        vec![inner_symbol],
    );

    library_api_post_lower::record_post_lower_library_api_evidence(&mut il, &interner);

    let outer_records = il
        .evidence
        .iter()
        .filter(|record| {
            record.anchor == EvidenceAnchor::node(sp_at(16), NodeKind::Call)
                && matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash,
                        callee_hash,
                        arity: 2,
                    }) if contract_hash == library_api_contract_id_hash(contract.id)
                        && callee_hash == library_api_callee_contract_hash(contract.callee)
                )
        })
        .collect::<Vec<_>>();
    assert!(
        outer_records.is_empty(),
        "post-lower source discovery must not treat a prior source API record as iterable unless that record still passes full dependency-closed admission"
    );
    assert!(
        il.evidence_record_by_id(legacy_inner_api).is_some(),
        "the fixture's invalid prior LibraryApi record should remain present so the regression exercises source discovery"
    );
}

fn assert_python_iterator_pack_record(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACK_ID))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(
            PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID
        ))
    );
}
