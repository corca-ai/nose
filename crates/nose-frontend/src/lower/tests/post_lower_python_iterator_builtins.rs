use super::*;

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
