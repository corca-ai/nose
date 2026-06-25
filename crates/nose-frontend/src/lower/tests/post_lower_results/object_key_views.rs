use super::*;

#[test]
fn post_lowering_emits_object_keys_key_view_only_with_object_argument_proof() {
    let interner = Interner::new();
    let contract = library_object_key_view_contract(Lang::TypeScript, "Object", "keys", 1)
        .expect("Object.keys key-view contract");
    let ts = lower_fixture(
        "object_keys.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    let api = contract_api_ids(&ts.evidence, contract.id, contract.callee);
    assert_eq!(api.len(), 1);
    let record = ts.evidence_record_by_id(api[0]).expect("Object.keys API");
    assert!(
        record.dependencies.len() >= 4,
        "Object.keys key-view evidence should depend on qualified Object.keys, Object root, binding write, and object surface proof"
    );
    assert!(result_domain_depends_on_api(
        &ts.evidence,
        record.anchor.span(),
        DomainEvidence::Array,
        &api,
    ));

    let shadowed = lower_fixture(
        "object_keys_shadowed.ts",
        b"function f(Object: any, key: string) { const values = { red: 1, blue: 2 }; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&shadowed.evidence, contract.id, contract.callee).is_empty(),
        "shadowed Object must not emit Object.keys key-view API evidence"
    );

    let proto_key = lower_fixture(
        "object_keys_proto.ts",
        b"function f(key: string) { const values = { __proto__: null, red: 1 }; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&proto_key.evidence, contract.id, contract.callee).is_empty(),
        "__proto__ object-literal prototype syntax must close key-view evidence"
    );

    let escaped_proto_key = lower_fixture(
        "object_keys_escaped_proto.ts",
        b"function f(key: string) { const values = { \\u005f\\u005fproto__: null, red: 1 }; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&escaped_proto_key.evidence, contract.id, contract.callee).is_empty(),
        "escaped __proto__ object-literal prototype syntax must close key-view evidence"
    );

    let numeric_key = lower_fixture(
        "object_keys_numeric_key.ts",
        b"function f(key: string) { const values = { 1.0: true, red: 1 }; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&numeric_key.evidence, contract.id, contract.callee).is_empty(),
        "numeric object-literal keys must close key-view evidence until JS key canonicalization is modeled"
    );

    let mutated = lower_fixture(
        "object_keys_mutated.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; values.green = 3; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&mutated.evidence, contract.id, contract.callee).is_empty(),
        "mutation between object construction and Object.keys must close key-view evidence"
    );

    let delete_mutation = lower_fixture(
        "object_keys_delete_mutation.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; delete values.red; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&delete_mutation.evidence, contract.id, contract.callee).is_empty(),
        "delete before Object.keys must close key-view evidence"
    );

    let aliased = lower_fixture(
        "object_keys_aliased.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; const alias = values; alias.green = 3; return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&aliased.evidence, contract.id, contract.callee).is_empty(),
        "aliasing the object before Object.keys must close key-view evidence"
    );

    let receiver_call = lower_fixture(
        "object_keys_receiver_call.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; values.clear(); return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&receiver_call.evidence, contract.id, contract.callee).is_empty(),
        "receiver calls on the object before Object.keys must close key-view evidence"
    );

    let hoisted_mutator = lower_fixture(
        "object_keys_hoisted_mutator.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; mutate(); return Object.keys(values).includes(key); function mutate() { values.green = 3; } }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&hoisted_mutator.evidence, contract.id, contract.callee).is_empty(),
        "nested function declarations can close over the object and must close key-view evidence"
    );

    let direct_eval = lower_fixture(
        "object_keys_eval.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; eval(\"values.green = 3\"); return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&direct_eval.evidence, contract.id, contract.callee).is_empty(),
        "direct eval before Object.keys must close key-view evidence"
    );

    let with_scope_delete = lower_fixture(
        "object_keys_with_scope_delete.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; with (values) { delete red; } return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&with_scope_delete.evidence, contract.id, contract.callee).is_empty(),
        "with scopes over the object can mutate unqualified properties and must close key-view evidence"
    );

    let enclosing_with_scope = lower_fixture(
        "object_keys_enclosing_with_scope.ts",
        b"function f(key: string) { const values = { values: { red: 1 }, blue: 2 }; with (values) { return Object.keys(values).includes(key); } }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&enclosing_with_scope.evidence, contract.id, contract.callee).is_empty(),
        "Object.keys inside with scopes must close key-view evidence because names are dynamically resolved"
    );

    let for_in_target_mutation = lower_fixture(
        "object_keys_for_in_target_mutation.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; for (values.green in { green: 1 }) {} return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(
            &for_in_target_mutation.evidence,
            contract.id,
            contract.callee
        )
        .is_empty(),
        "for-in target writes before Object.keys must close key-view evidence"
    );

    let for_of_target_mutation = lower_fixture(
        "object_keys_for_of_target_mutation.ts",
        b"function f(key: string) { const values = { red: 1, blue: 2 }; for (values.green of [\"green\"]) {} return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(
            &for_of_target_mutation.evidence,
            contract.id,
            contract.callee
        )
        .is_empty(),
        "for-of target writes before Object.keys must close key-view evidence"
    );

    let conditional_initializer = lower_fixture(
        "object_keys_conditional_initializer.ts",
        b"function f(flag: boolean, key: string) { if (flag) { var values = { red: 1, blue: 2 }; } return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(
            &conditional_initializer.evidence,
            contract.id,
            contract.callee
        )
        .is_empty(),
        "non-dominating conditional object initializer must close key-view evidence"
    );

    let parameter_shadow = lower_fixture(
        "object_keys_parameter_shadow.ts",
        b"const values = { red: 1, blue: 2 }; function f(values: Record<string, number>, key: string) { return Object.keys(values).includes(key); }\n",
        Lang::TypeScript,
        &interner,
    );
    assert!(
        contract_api_ids(&parameter_shadow.evidence, contract.id, contract.callee).is_empty(),
        "a parameter shadowing a module static object must close key-view evidence"
    );
}
