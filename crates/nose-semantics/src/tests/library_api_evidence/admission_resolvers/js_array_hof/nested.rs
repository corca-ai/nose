use super::*;

#[test]
fn js_array_normalized_hof_allows_admitted_nested_hof_callback() {
    let (
        mut il,
        interner,
        outer_hof,
        inner_hof,
        outer_receiver,
        inner_receiver,
        outer_param,
        inner_param,
    ) = js_array_normalized_nested_hof_callback_il();
    push_receiver_domain_dependency_with_id(&mut il, 0, outer_receiver, DomainEvidence::Array);
    push_receiver_domain_dependency_with_id(&mut il, 1, inner_receiver, DomainEvidence::Array);
    push_receiver_domain_dependency_with_id(&mut il, 4, outer_param, DomainEvidence::Number);
    push_receiver_domain_dependency_with_id(&mut il, 5, inner_param, DomainEvidence::Number);
    let inner_contract =
        library_method_call_contract(Lang::JavaScript, "map", 1).expect("JS Array.map row");
    il.push_evidence(js_array_hof_record(2, &il, inner_hof, inner_contract, &[1]));
    let outer_contract =
        library_method_call_contract(Lang::JavaScript, "flatMap", 1).expect("JS Array.flatMap row");
    il.push_evidence(js_array_hof_record(3, &il, outer_hof, outer_contract, &[0]));

    assert!(
        admitted_hof_api_at_node_with_interner(&il, Some(&interner), inner_hof, HoFKind::Map),
        "inner JS Array.map evidence should admit the nested normalized HOF"
    );
    assert!(
        admitted_hof_api_at_node_with_interner(&il, Some(&interner), outer_hof, HoFKind::FlatMap),
        "outer JS Array.flatMap callback may contain an admitted nested JS Array HOF"
    );
}
