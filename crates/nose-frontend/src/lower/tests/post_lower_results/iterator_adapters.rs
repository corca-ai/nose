use super::*;

#[test]
fn post_lowering_emits_result_domains_for_supported_receiver_methods() {
    let interner = Interner::new();
    let rust_iter_chain = lower_fixture(
        "iter_chain.rs",
        b"fn f(xs: Vec<i32>) -> usize { xs.iter().filter(|x| **x > 0).count() }",
        Lang::Rust,
        &interner,
    );
    let iter_contract =
        nose_semantics::library_iterator_identity_adapter_contract(Lang::Rust, "iter", 0)
            .expect("Rust iter adapter contract");
    let iter_api = contract_api_ids(
        &rust_iter_chain.evidence,
        iter_contract.id,
        iter_contract.callee,
    );
    let iter_span = call_span_with_field_callee_named(&rust_iter_chain, &interner, "iter")
        .expect("iter call span");
    assert!(result_domain_depends_on_api(
        &rust_iter_chain.evidence,
        iter_span,
        DomainEvidence::Iterator,
        &iter_api,
    ));

    let filter_contract = nose_semantics::library_method_call_contract(Lang::Rust, "filter", 1)
        .expect("Rust filter contract");
    assert_eq!(
        contract_api_count(
            &rust_iter_chain.evidence,
            filter_contract.id,
            filter_contract.callee
        ),
        1,
        "follow-on iterator HOFs should consume the .iter() result-domain receiver proof"
    );

    let rust_collect = lower_fixture(
        "iter_collect.rs",
        b"fn f(xs: Vec<i32>) -> Vec<i32> { xs.iter().collect() }",
        Lang::Rust,
        &interner,
    );
    let collect_contract =
        nose_semantics::library_iterator_identity_adapter_contract(Lang::Rust, "collect", 0)
            .expect("Rust collect adapter contract");
    assert_eq!(
        contract_api_count(
            &rust_collect.evidence,
            collect_contract.id,
            collect_contract.callee
        ),
        1
    );
    let collect_span = call_span_with_field_callee_named(&rust_collect, &interner, "collect")
        .expect("collect call span");
    assert_eq!(
        result_domain_any_count_at(&rust_collect.evidence, collect_span),
        0,
        "collect result type is caller-selected and must not emit a fixed result domain"
    );
}
