use super::*;

#[test]
fn post_lowering_emits_property_and_rust_option_occurrences() {
    let interner = Interner::new();
    assert_ts_length_property_occurrences(&interner);
    assert_ts_promise_then_occurrences(&interner);
    assert_rust_option_occurrences(&interner);
    assert_rust_result_occurrences(&interner);
}

fn assert_ts_length_property_occurrences(interner: &Interner) {
    let ts = lower_fixture(
        "t.ts",
        b"function f(xs: number[]) { return xs.length; }\n",
        Lang::TypeScript,
        interner,
    );
    let property_contract = library_property_builtin_contract(Lang::TypeScript, "length").unwrap();
    let length_field = named_node_span(&ts, interner, NodeKind::Field, "length")
        .expect("length field should be lowered");
    let property_api = library_api_evidence_ids_at_node(
        &ts.evidence,
        length_field,
        NodeKind::Field,
        library_api_contract_id_hash(property_contract.id),
        library_api_callee_contract_hash(property_contract.callee),
        0,
    );
    assert_eq!(
        property_api.len(),
        1,
        "typed exact-collection property access should carry LibraryApi occurrence evidence"
    );
    let property_records =
        contract_api_records(&ts.evidence, property_contract.id, property_contract.callee);
    assert_property_builtin_record_provenance(property_records[0]);
    let ts_filter_length = lower_fixture(
            "t.ts",
            b"function f(value: string) { return [\"red\", \"blue\"].filter((item: string) => item === value).length >= 1; }\n",
            Lang::TypeScript,
            interner,
        );
    let filter_length_field =
        named_node_span(&ts_filter_length, interner, NodeKind::Field, "length")
            .expect("filter length field should be lowered");
    let filter_length_api = library_api_evidence_ids_at_node(
        &ts_filter_length.evidence,
        filter_length_field,
        NodeKind::Field,
        library_api_contract_id_hash(property_contract.id),
        library_api_callee_contract_hash(property_contract.callee),
        0,
    );
    assert_eq!(
        filter_length_api.len(),
        1,
        "HOF result property access should carry LibraryApi occurrence evidence"
    );
    let filter_length_records = contract_api_records(
        &ts_filter_length.evidence,
        property_contract.id,
        property_contract.callee,
    );
    assert_property_builtin_record_provenance(filter_length_records[0]);
}

fn assert_ts_promise_then_occurrences(interner: &Interner) {
    let ts = lower_fixture(
        "t.ts",
        b"function f(p: Promise<number>) { return p.then((x: number) => x); }\n",
        Lang::TypeScript,
        interner,
    );
    let contract = nose_semantics::library_promise_then_contract(Lang::TypeScript, "then", 1)
        .expect("Promise.then contract");
    let then_records = contract_api_records(&ts.evidence, contract.id, contract.callee);
    assert_eq!(
        then_records.len(),
        1,
        "Promise-like receiver .then should carry LibraryApi occurrence evidence"
    );
    assert_js_like_promise_record_provenance(then_records[0]);
    let then_api = contract_api_ids(&ts.evidence, contract.id, contract.callee);
    assert!(result_domain_depends_on_any_api(
        &ts.evidence,
        DomainEvidence::PromiseLike,
        &then_api,
    ));
}

fn assert_rust_option_occurrences(interner: &Interner) {
    let rust_some = lower_fixture(
        "t.rs",
        b"fn f(x: i32) -> Option<i32> { Some(x) }\n",
        Lang::Rust,
        interner,
    );
    let some_contract =
        library_rust_option_some_constructor_contract(Lang::Rust, "Some", 1).unwrap();
    let some_call = call_span_with_callee_named(&rust_some, interner, "Some")
        .expect("Some call should be lowered");
    let some_api = library_api_evidence_ids_at(
        &rust_some.evidence,
        some_call,
        library_api_contract_id_hash(some_contract.id),
        library_api_callee_contract_hash(some_contract.callee),
        1,
    );
    assert_eq!(some_api.len(), 1);
    let some_records =
        contract_api_records(&rust_some.evidence, some_contract.id, some_contract.callee);
    assert_rust_option_record_provenance(some_records[0]);
    assert!(result_domain_depends_on_api(
        &rust_some.evidence,
        some_call,
        DomainEvidence::Option,
        &some_api,
    ));

    let rust_some_pattern = lower_fixture(
            "t.rs",
            b"pub fn f(value: Option<i32>) -> bool { if let Some(_) = value { true } else { false } }\n",
            Lang::Rust,
            interner,
        );
    let some_pattern_var = named_node_span(&rust_some_pattern, interner, NodeKind::Var, "Some")
        .expect("Some pattern var should be preserved");
    let some_pattern_api = library_api_evidence_ids_at_node(
        &rust_some_pattern.evidence,
        some_pattern_var,
        NodeKind::Var,
        library_api_contract_id_hash(some_contract.id),
        library_api_callee_contract_hash(some_contract.callee),
        1,
    );
    assert_eq!(some_pattern_api.len(), 1);
    let some_pattern_records = contract_api_records(
        &rust_some_pattern.evidence,
        some_contract.id,
        some_contract.callee,
    );
    assert_rust_option_record_provenance(some_pattern_records[0]);
    assert!(
        !result_domain_depends_on_api_at_node(
            &rust_some_pattern.evidence,
            some_pattern_var,
            NodeKind::Var,
            DomainEvidence::Option,
            &some_pattern_api,
        ),
        "pattern occurrence identity must not become a constructor result domain"
    );

    let rust_none = lower_fixture(
        "t.rs",
        b"fn f() -> Option<i32> { None }\n",
        Lang::Rust,
        interner,
    );
    let none_contract = library_rust_option_none_sentinel_contract(Lang::Rust, "None").unwrap();
    let none_var = named_node_span(&rust_none, interner, NodeKind::Var, "None")
        .expect("None var should be lowered");
    let none_api = library_api_evidence_ids_at_node(
        &rust_none.evidence,
        none_var,
        NodeKind::Var,
        library_api_contract_id_hash(none_contract.id),
        library_api_callee_contract_hash(none_contract.callee),
        0,
    );
    assert_eq!(none_api.len(), 1);
    let none_records =
        contract_api_records(&rust_none.evidence, none_contract.id, none_contract.callee);
    assert_rust_option_record_provenance(none_records[0]);
    assert!(result_domain_depends_on_api_at_node(
        &rust_none.evidence,
        none_var,
        NodeKind::Var,
        DomainEvidence::Option,
        &none_api,
    ));

    let shadowed_some = lower_fixture(
        "t.rs",
        b"fn Some(x: i32) -> Option<i32> { None }\nfn f(x: i32) -> Option<i32> { Some(x) }\n",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        contract_api_count(
            &shadowed_some.evidence,
            some_contract.id,
            some_contract.callee
        ),
        0,
        "local Rust Some item must close the std Option constructor occurrence"
    );
}

fn assert_rust_option_record_provenance(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::RUST_STDLIB_OPTION_PACK_ID
        ))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(RUST_STDLIB_OPTION_PRODUCER_ID))
    );
}

fn assert_rust_result_occurrences(interner: &Interner) {
    let rust_ok = lower_fixture(
        "t.rs",
        b"fn f(x: i32) -> Result<i32, i32> { Ok(x) }\n",
        Lang::Rust,
        interner,
    );
    let ok_contract = library_rust_result_ok_constructor_contract(Lang::Rust, "Ok", 1).unwrap();
    let ok_call =
        call_span_with_callee_named(&rust_ok, interner, "Ok").expect("Ok call should lower");
    let ok_api = library_api_evidence_ids_at(
        &rust_ok.evidence,
        ok_call,
        library_api_contract_id_hash(ok_contract.id),
        library_api_callee_contract_hash(ok_contract.callee),
        1,
    );
    assert_eq!(ok_api.len(), 1);
    let ok_records = contract_api_records(&rust_ok.evidence, ok_contract.id, ok_contract.callee);
    assert_rust_result_record_provenance(ok_records[0]);
    assert!(result_domain_depends_on_api(
        &rust_ok.evidence,
        ok_call,
        DomainEvidence::Result,
        &ok_api,
    ));

    let rust_err = lower_fixture(
        "t.rs",
        b"fn f(e: i32) -> Result<i32, i32> { Err(e) }\n",
        Lang::Rust,
        interner,
    );
    let err_contract = library_rust_result_err_constructor_contract(Lang::Rust, "Err", 1).unwrap();
    let err_call =
        call_span_with_callee_named(&rust_err, interner, "Err").expect("Err call should lower");
    let err_api = library_api_evidence_ids_at(
        &rust_err.evidence,
        err_call,
        library_api_contract_id_hash(err_contract.id),
        library_api_callee_contract_hash(err_contract.callee),
        1,
    );
    assert_eq!(err_api.len(), 1);
    let err_records =
        contract_api_records(&rust_err.evidence, err_contract.id, err_contract.callee);
    assert_rust_result_record_provenance(err_records[0]);
    assert!(result_domain_depends_on_api(
        &rust_err.evidence,
        err_call,
        DomainEvidence::Result,
        &err_api,
    ));

    let rust_is_ok = lower_fixture(
        "t.rs",
        b"pub fn f(value: Result<i32, i32>) -> bool { value.is_ok() }\n",
        Lang::Rust,
        interner,
    );
    let is_ok_contract =
        nose_semantics::library_rust_result_predicate_contract(Lang::Rust, "is_ok", 0).unwrap();
    let is_ok_call = call_span_with_field_callee_named(&rust_is_ok, interner, "is_ok")
        .expect("is_ok call should lower");
    let is_ok_api = library_api_evidence_ids_at(
        &rust_is_ok.evidence,
        is_ok_call,
        library_api_contract_id_hash(is_ok_contract.id),
        library_api_callee_contract_hash(is_ok_contract.callee),
        0,
    );
    assert_eq!(is_ok_api.len(), 1);
    assert!(result_domain_depends_on_api(
        &rust_is_ok.evidence,
        is_ok_call,
        DomainEvidence::Boolean,
        &is_ok_api,
    ));

    let shadowed_result_is_ok = lower_fixture(
        "t.rs",
        b"struct Result<T, E> { value: T, err: E }\nimpl<T, E> Result<T, E> { fn is_ok(&self) -> bool { false } }\npub fn f(value: Result<i32, i32>) -> bool { value.is_ok() }\n",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        contract_api_count(
            &shadowed_result_is_ok.evidence,
            is_ok_contract.id,
            is_ok_contract.callee
        ),
        0,
        "a local Rust Result type must close std Result predicate occurrence evidence"
    );

    let rust_is_err = lower_fixture(
        "t.rs",
        b"pub fn f(value: Result<i32, i32>) -> bool { value.is_err() }\n",
        Lang::Rust,
        interner,
    );
    let is_err_contract =
        nose_semantics::library_rust_result_predicate_contract(Lang::Rust, "is_err", 0).unwrap();
    let is_err_call = call_span_with_field_callee_named(&rust_is_err, interner, "is_err")
        .expect("is_err call should lower");
    let is_err_api = library_api_evidence_ids_at(
        &rust_is_err.evidence,
        is_err_call,
        library_api_contract_id_hash(is_err_contract.id),
        library_api_callee_contract_hash(is_err_contract.callee),
        0,
    );
    assert_eq!(is_err_api.len(), 1);

    let rust_ok_pattern = lower_fixture(
        "t.rs",
        b"pub fn f(value: Result<i32, i32>) -> bool { if let Ok(_) = value { true } else { false } }\n",
        Lang::Rust,
        interner,
    );
    let ok_pattern_var = named_node_span(&rust_ok_pattern, interner, NodeKind::Var, "Ok")
        .expect("Ok pattern var should be preserved");
    let ok_pattern_api = library_api_evidence_ids_at_node(
        &rust_ok_pattern.evidence,
        ok_pattern_var,
        NodeKind::Var,
        library_api_contract_id_hash(ok_contract.id),
        library_api_callee_contract_hash(ok_contract.callee),
        1,
    );
    assert_eq!(ok_pattern_api.len(), 1);

    let shadowed_ok = lower_fixture(
        "t.rs",
        b"struct Ok<T>(T);\nfn f(x: Ok<i32>) -> bool { if let Ok(_) = x { true } else { false } }\n",
        Lang::Rust,
        interner,
    );
    assert_eq!(
        contract_api_count(&shadowed_ok.evidence, ok_contract.id, ok_contract.callee),
        0,
        "local Rust Ok item must close the std Result constructor occurrence"
    );

    let result_unwrap_else = lower_fixture(
        "t.rs",
        b"pub fn f(value: Result<i32, i32>, fallback: i32) -> i32 { value.unwrap_or_else(|_| fallback) }\n",
        Lang::Rust,
        interner,
    );
    let unwrap_contract =
        nose_semantics::library_method_call_contract(Lang::Rust, "unwrap_or_else", 1).unwrap();
    assert_eq!(
        contract_api_count(
            &result_unwrap_else.evidence,
            unwrap_contract.id,
            unwrap_contract.callee
        ),
        0,
        "Result lazy/defaulting APIs stay out of this narrow capability"
    );
}

fn assert_rust_result_record_provenance(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::RUST_STDLIB_RESULT_PACK_ID
        ))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(RUST_STDLIB_RESULT_PRODUCER_ID))
    );
}

fn assert_js_like_promise_record_provenance(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::JS_LIKE_BUILTIN_PROMISE_PACK_ID
        ))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID))
    );
}

fn assert_property_builtin_record_provenance(record: &EvidenceRecord) {
    assert_eq!(
        record.provenance.pack_hash,
        Some(stable_symbol_hash(
            nose_semantics::PROPERTY_BUILTIN_PROTOCOL_PACK_ID
        ))
    );
    assert_eq!(
        record.provenance.rule_hash,
        Some(stable_symbol_hash(PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID))
    );
}
