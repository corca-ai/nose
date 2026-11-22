use super::support::{
    library_api_dependency_counts, library_api_dependency_counts_for, library_api_evidence_count,
    lower_js, qualified_global_evidence_count, qualified_global_evidence_records,
    record_has_source_unshadowed_dependency, source_operator_evidence_count,
    unshadowed_global_evidence_count,
};
use nose_il::{Builtin, FileId, Interner, Lang, NodeKind, Payload, SourceOperatorKind};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash,
    library_js_array_is_array_contract, library_js_like_map_constructor_contract,
    library_js_like_set_constructor_contract, library_map_key_view_wrapper_contract,
    library_promise_aggregate_contract, PromiseAggregateKind,
};

#[test]
fn js_static_global_value_occurrences_emit_symbol_evidence() {
    let il = lower_js(
        "function f(value) {
            console.log(Math.abs(value));
            const picked = new Map([[\"x\", 1]]).get(\"x\") ?? undefined;
            return Array.isArray(value) || new Set([value]).has(value) || picked;
        }",
    );

    for name in ["console", "Math", "Map", "undefined", "Array", "Set"] {
        assert!(
            unshadowed_global_evidence_count(&il, name) >= 1,
            "missing global evidence for {name}"
        );
    }
    let map = library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap();
    assert!(
        library_api_evidence_count(
            &il,
            library_api_contract_id_hash(map.id),
            library_api_callee_contract_hash(map.callee),
            1,
        ) >= 1
    );
    let set = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
    assert!(
        library_api_evidence_count(
            &il,
            library_api_contract_id_hash(set.id),
            library_api_callee_contract_hash(set.callee),
            1,
        ) >= 1
    );
    let is_array =
        library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1).unwrap();
    assert!(
        library_api_evidence_count(
            &il,
            library_api_contract_id_hash(is_array.id),
            library_api_callee_contract_hash(is_array.callee),
            1,
        ) >= 1
    );
    assert!(
        library_api_dependency_counts(&il)
            .into_iter()
            .all(|count| count >= 1),
        "selected JS API evidence should depend on explicit proof"
    );
    for (id, callee, arity) in [
        (map.id, map.callee, 1),
        (set.id, set.callee, 1),
        (is_array.id, is_array.callee, 1),
    ] {
        assert!(
            library_api_dependency_counts_for(
                &il,
                library_api_contract_id_hash(id),
                library_api_callee_contract_hash(callee),
                arity,
            )
            .into_iter()
            .all(|count| count >= 2),
            "selected JS static/global API evidence should depend on source/symbol proof"
        );
    }
    assert!(
        !il.nodes
            .iter()
            .any(|node| matches!(node.payload, Payload::Builtin(Builtin::Abs))),
        "Math.abs should stay as Field(Var(Math), abs) for semantic consumers"
    );
}

#[test]
fn js_static_global_evidence_respects_local_and_destructured_shadows() {
    let il = lower_js(
        "function f(Math, value) { return Math.abs(value); }
         function g(scope) { const { Map } = scope; return new Map([]); }
         function h(value, undefined) { return value === undefined; }
         function i(value) { const Array = { isArray() { return false; } }; return Array.isArray(value); }
         function j(Promise, values) { return Promise.allSettled(values); }",
    );

    for name in ["Math", "Map", "undefined", "Array", "Promise"] {
        assert_eq!(
            unshadowed_global_evidence_count(&il, name),
            0,
            "shadowed {name} should not get global evidence"
        );
    }
    assert_eq!(
        qualified_global_evidence_count(&il, "Promise.allSettled", NodeKind::Field),
        0,
        "shadowed Promise.allSettled should not get qualified global evidence"
    );
}

#[test]
fn js_static_global_shadowing_stays_positional_and_function_local() {
    let il = lower_js(
        "function beforeBinding(value) { return Math.abs(value); }
         const Math = { abs: (value) => value };",
    );
    assert_eq!(
        unshadowed_global_evidence_count(&il, "Math"),
        1,
        "a use before the file's later binding keeps the global's semantics"
    );

    let il = lower_js(
        "function readsGlobal(value) { return Math.abs(value); }
         function bindsElsewhere() { const Math = { abs: (value) => value }; return Math.abs(1); }",
    );
    assert_eq!(
        unshadowed_global_evidence_count(&il, "Math"),
        1,
        "a binding inside another function does not shadow this use"
    );

    let il = lower_js(
        "function bindsBefore(value) {
             const Math = { abs: (value) => value };
             return Math.abs(value);
         }",
    );
    assert_eq!(
        unshadowed_global_evidence_count(&il, "Math"),
        0,
        "a binding earlier in the enclosing function still shadows the use"
    );

    let il = lower_js("function headerBinds(Math, value) { return Math.abs(value); }");
    assert_eq!(
        unshadowed_global_evidence_count(&il, "Math"),
        0,
        "a parameter in the enclosing function's header still shadows the use"
    );
}

#[test]
fn js_local_global_shadowing_survives_an_unreadable_file_prefix() {
    let il = crate::lower_source(
        FileId(0),
        "example.js",
        b"// caf\xe9\nfunction f(value) {
            const Math = { abs: (value) => value };
            return Math.abs(value);
        }",
        Lang::JavaScript,
        &Interner::new(),
    )
    .expect("lower js after non-UTF-8 comment");

    assert_eq!(unshadowed_global_evidence_count(&il, "Math"), 0);
    assert_eq!(
        qualified_global_evidence_count(&il, "Math.abs", NodeKind::Field),
        0,
    );
}

#[test]
fn js_typeof_unary_emits_source_operator_evidence() {
    let il = lower_js("function real(value) { return typeof value === \"string\"; }");
    assert_eq!(
        source_operator_evidence_count(&il, SourceOperatorKind::Typeof),
        1
    );
}

#[test]
fn js_qualified_global_paths_emit_symbol_evidence() {
    let il = lower_js(
        "function hasOwn(value) { return Object.hasOwn(value, \"ready\"); }
         function hasOwnCall(value) { return Object.prototype.hasOwnProperty.call(value, \"ready\"); }
         function fromKeys(lookup) { return Array.from(lookup.keys()).includes(\"ready\"); }
         function isArray(value) { return Array.isArray(value); }
         function settle(values) { return Promise.allSettled(values); }",
    );

    assert_eq!(
        qualified_global_evidence_count(&il, "Object.hasOwn", NodeKind::Seq),
        1
    );
    assert_eq!(
        qualified_global_evidence_count(&il, "Object.prototype.hasOwnProperty.call", NodeKind::Seq),
        1
    );
    assert_eq!(
        qualified_global_evidence_count(&il, "Array.from", NodeKind::Field),
        1
    );
    assert_eq!(
        qualified_global_evidence_count(&il, "Array.isArray", NodeKind::Field),
        1
    );
    assert_eq!(
        qualified_global_evidence_count(&il, "Promise.allSettled", NodeKind::Field),
        1
    );
    for (path, kind, root) in [
        ("Object.hasOwn", NodeKind::Seq, "Object"),
        (
            "Object.prototype.hasOwnProperty.call",
            NodeKind::Seq,
            "Object",
        ),
        ("Array.from", NodeKind::Field, "Array"),
        ("Array.isArray", NodeKind::Field, "Array"),
        ("Promise.allSettled", NodeKind::Field, "Promise"),
    ] {
        for record in qualified_global_evidence_records(&il, path, kind) {
            assert!(
                record_has_source_unshadowed_dependency(&il, record, root),
                "{path} evidence should depend on an unshadowed {root} proof"
            );
        }
    }
    let from = library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1).unwrap();
    let is_array =
        library_js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1).unwrap();
    let all_settled =
        library_promise_aggregate_contract(Lang::JavaScript, "Promise", "allSettled", 1).unwrap();
    assert_eq!(
        library_api_evidence_count(
            &il,
            library_api_contract_id_hash(from.id),
            library_api_callee_contract_hash(from.callee),
            1,
        ),
        1
    );
    assert_eq!(
        library_api_evidence_count(
            &il,
            library_api_contract_id_hash(is_array.id),
            library_api_callee_contract_hash(is_array.callee),
            1,
        ),
        1
    );
    assert_eq!(
        library_api_evidence_count(
            &il,
            library_api_contract_id_hash(all_settled.id),
            library_api_callee_contract_hash(all_settled.callee),
            1,
        ),
        1
    );
}

#[test]
fn js_promise_first_observed_aggregates_emit_symbol_evidence() {
    let il = lower_js(
        "function first(values) { return Promise.race(values); }
         function firstFulfilled(values) { return Promise.any(values); }",
    );

    for path in ["Promise.race", "Promise.any"] {
        assert_eq!(
            qualified_global_evidence_count(&il, path, NodeKind::Field),
            1
        );
        for record in qualified_global_evidence_records(&il, path, NodeKind::Field) {
            assert!(
                record_has_source_unshadowed_dependency(&il, record, "Promise"),
                "{path} evidence should depend on an unshadowed Promise proof"
            );
        }
    }

    for (method, kind) in [
        ("race", PromiseAggregateKind::Race),
        ("any", PromiseAggregateKind::Any),
    ] {
        let contract =
            library_promise_aggregate_contract(Lang::JavaScript, "Promise", method, 1).unwrap();
        assert_eq!(contract.result.kind, kind);
        assert_eq!(
            library_api_evidence_count(
                &il,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
                1,
            ),
            1
        );
    }
}
