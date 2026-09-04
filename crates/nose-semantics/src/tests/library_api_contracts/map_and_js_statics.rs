use super::*;

#[test]
fn map_key_view_contracts_distinguish_collection_and_iterator_views() {
    assert_eq!(
        map_key_view_contract(Lang::Python, "keys", 0),
        Some(MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Collection,
        })
    );
    assert_eq!(
        map_key_view_contract(Lang::Java, "keySet", 0),
        Some(MapKeyViewContract {
            method: "keySet",
            kind: MapKeyViewKind::Collection,
        })
    );
    assert_eq!(
        map_key_view_contract(Lang::TypeScript, "keys", 0),
        Some(MapKeyViewContract {
            method: "keys",
            kind: MapKeyViewKind::Iterator,
        })
    );
    assert_eq!(map_key_view_contract(Lang::JavaScript, "keySet", 0), None);
    assert_eq!(map_key_view_contract(Lang::Python, "keys", 1), None);
    assert_eq!(
        map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1),
        Some(MapKeyViewWrapperContract {
            receiver: "Array",
            method: "from",
            qualified_path: "Array.from",
        })
    );
    assert_eq!(
        map_key_view_wrapper_contract(Lang::Python, "Array", "from", 1),
        None
    );
    assert_eq!(
        map_key_view_contract_by_hash(Lang::Java, stable_symbol_hash("keySet"), 0)
            .map(|contract| contract.kind),
        Some(MapKeyViewKind::Collection)
    );
    assert!(map_key_view_wrapper_contract_by_hash(
        Lang::TypeScript,
        "Array",
        stable_symbol_hash("from"),
        1,
    )
    .is_some());
}

#[test]
fn go_zero_map_contracts_are_go_surface_and_default_constrained() {
    assert_eq!(
        go_zero_map_lookup_contract(Lang::Go),
        Some(GoZeroMapLookupContract {
            map_literal_tag: "composite_literal",
            entry_tag: "keyed_element",
            canonical_value_tag: "go_literal_zero_map",
        })
    );
    assert_eq!(go_zero_map_lookup_contract(Lang::Python), None);
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::LitInt(1)),
        Some(GoZeroMapDefaultKind::Int)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::LitStr(stable_symbol_hash("x"))),
        Some(GoZeroMapDefaultKind::String)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::Go, Payload::Lit(LitClass::Null)),
        Some(GoZeroMapDefaultKind::Null)
    );
    assert_eq!(
        go_zero_map_default_kind(Lang::JavaScript, Payload::LitInt(1)),
        None
    );
    assert_eq!(go_zero_map_default_kind(Lang::Go, Payload::None), None);
}

#[test]
fn map_get_contracts_are_language_and_arity_constrained() {
    assert_eq!(
        map_get_contract(Lang::Rust, "get", 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(
        map_get_contract_by_hash(Lang::Java, stable_symbol_hash("get"), 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(
        map_get_contract(Lang::TypeScript, "get", 1),
        Some(MapGetContract {
            method: "get",
            receiver: MethodReceiverContract::ExactMap,
        })
    );
    assert_eq!(map_get_contract(Lang::Python, "get", 1), None);
    assert_eq!(map_get_contract(Lang::Rust, "get", 2), None);
    assert_eq!(map_get_contract(Lang::Java, "getOrDefault", 1), None);
}

#[test]
fn swift_dictionary_default_subscript_requires_live_stable_coordinates() {
    let interner = Interner::new();
    let marker = interner.intern("swift_subscript_default");
    let mut b = IlBuilder::new(FileId(0));
    let receiver_param = b.add(NodeKind::Param, Payload::Cid(0), sp(40), &[]);
    let key_param = b.add(NodeKind::Param, Payload::Cid(1), sp(41), &[]);
    let default_param = b.add(NodeKind::Param, Payload::Cid(2), sp(42), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(43), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(44), &[]);
    let default = b.add(NodeKind::Var, Payload::Cid(2), sp(45), &[]);
    let marker = b.add(
        NodeKind::Seq,
        Payload::Name(marker),
        sp(46),
        &[key, default],
    );
    let index = b.add(NodeKind::Index, Payload::None, sp(47), &[receiver, marker]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(39),
        &[receiver_param, key_param, default_param, index],
    );
    let mut il = finish_il(b, root, Lang::Swift);
    il.push_evidence(language_core_evidence(
        0,
        EvidenceAnchor::param(sp(40)),
        EvidenceKind::Type(TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter),
        EvidenceStatus::Asserted,
        Lang::Swift,
    ));

    let contract = swift_dictionary_default_subscript_contract_for_node(&il, &interner, index)
        .expect("live Dictionary source and direct coordinates should be admitted");
    assert_eq!(contract.receiver, receiver);
    assert_eq!(contract.key, key);
    assert_eq!(contract.default, default);

    il.evidence[0].status = EvidenceStatus::Ambiguous;
    assert!(
        swift_dictionary_default_subscript_contract_for_node(&il, &interner, index).is_none(),
        "shadow-tombstoned Dictionary evidence must close the contract"
    );
}

#[test]
fn swift_dictionary_default_subscript_rejects_lazy_expression_defaults() {
    let interner = Interner::new();
    let marker_name = interner.intern("swift_subscript_default");
    let observe_name = interner.intern("observe");
    let mut b = IlBuilder::new(FileId(0));
    let receiver_param = b.add(NodeKind::Param, Payload::Cid(0), sp(50), &[]);
    let key_param = b.add(NodeKind::Param, Payload::Cid(1), sp(51), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(52), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(53), &[]);
    let callee = b.add(NodeKind::Var, Payload::Name(observe_name), sp(54), &[]);
    let effectful_default = b.add(NodeKind::Call, Payload::None, sp(55), &[callee]);
    let marker = b.add(
        NodeKind::Seq,
        Payload::Name(marker_name),
        sp(56),
        &[key, effectful_default],
    );
    let index = b.add(NodeKind::Index, Payload::None, sp(57), &[receiver, marker]);
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(49),
        &[receiver_param, key_param, index],
    );
    let mut il = finish_il(b, root, Lang::Swift);
    il.push_evidence(language_core_evidence(
        0,
        EvidenceAnchor::param(sp(50)),
        EvidenceKind::Type(TypeEvidenceKind::SwiftBracketDictionaryParameter),
        EvidenceStatus::Asserted,
        Lang::Swift,
    ));

    assert!(
        swift_dictionary_default_subscript_contract_for_node(&il, &interner, index).is_none(),
        "a lazy call expression is not a stable fallback coordinate"
    );
}

#[test]
fn js_static_builtin_contracts_are_language_and_arity_constrained() {
    assert_eq!(
        static_global_symbol_contract(Lang::JavaScript, "Math"),
        Some(StaticGlobalSymbolContract {
            name: "Math",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        static_global_symbol_contract(Lang::TypeScript, "undefined"),
        Some(StaticGlobalSymbolContract {
            name: "undefined",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        static_global_symbol_contract(Lang::TypeScript, "Promise"),
        Some(StaticGlobalSymbolContract {
            name: "Promise",
            requires_unshadowed: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.resolve"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.resolve",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.reject"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.reject",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.all"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.all",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.allSettled"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.allSettled",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.race"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.race",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(
        qualified_global_symbol_contract(Lang::JavaScript, "Promise.any"),
        Some(QualifiedGlobalSymbolContract {
            path: "Promise.any",
            root: "Promise",
            requires_unshadowed_root: true,
        })
    );
    assert_eq!(static_global_symbol_contract(Lang::Python, "Math"), None);
    assert_eq!(
        static_global_symbol_contract(Lang::JavaScript, "WeakMap"),
        None
    );
    assert_eq!(
        typeof_operator_contract(Lang::TypeScript, "typeof", 1),
        Some(TypeofOperatorContract {
            name: "typeof",
            required_source_fact: SourceFactKind::Operator(SourceOperatorKind::Typeof),
        })
    );
    assert_eq!(typeof_operator_contract(Lang::Python, "typeof", 1), None);
    assert_eq!(
        typeof_operator_contract(Lang::JavaScript, "typeof", 2),
        None
    );
    assert_eq!(
        js_array_is_array_contract(Lang::JavaScript, "Array", "isArray", 1),
        Some(StaticGlobalMethodContract {
            receiver: "Array",
            method: "isArray",
            qualified_path: "Array.isArray",
            requires_unshadowed_receiver: true,
        })
    );
    assert_eq!(
        js_array_is_array_contract(Lang::Python, "Array", "isArray", 1),
        None
    );
    assert_eq!(
        js_array_is_array_contract(Lang::TypeScript, "Array", "isArray", 2),
        None
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 1),
        Some(StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        })
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::TypeScript, "Boolean", 1),
        Some(StaticGlobalFunctionContract {
            function: "Boolean",
            requires_unshadowed_function: true,
        })
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::Python, "Boolean", 1),
        None
    );
    assert_eq!(
        js_boolean_coercion_contract(Lang::JavaScript, "Boolean", 2),
        None
    );
    assert_eq!(
        regex_test_contract(Lang::JavaScript, "test", 1),
        Some(RegexTestContract {
            method: "test",
            required_receiver_fact: SourceFactKind::Literal(SourceLiteralKind::Regex),
        })
    );
    assert_eq!(regex_test_contract(Lang::Ruby, "test", 1), None);
}
