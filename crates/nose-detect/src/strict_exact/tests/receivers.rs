use super::super::*;
use super::support::*;

#[test]
fn strict_exact_len_rejects_pull_lazy_library_hof_arg() {
    let interner = Interner::new();
    let (il, _hof, len) = nose_semantics::test_support::rust_pull_lazy_map_len_test_il();

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, len),
        "len must not treat an admitted pull-lazy iterator HOF as an exact materialized collection"
    );
}

#[test]
fn binding_domain_does_not_make_opaque_binding_exact_value() {
    let interner = Interner::new();
    let xs = interner.intern("xs");
    let mut b = IlBuilder::new(FileId(0));
    let lhs = b.add(NodeKind::Var, Payload::Cid(0), sp(10), &[]);
    let opaque = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("opaque")),
        sp(11),
        &[],
    );
    let rhs = b.add(NodeKind::Call, Payload::None, sp(12), &[opaque]);
    let assign = b.add(NodeKind::Assign, Payload::None, sp(10), &[lhs, rhs]);
    let use_name = b.add(NodeKind::Var, Payload::Name(xs), sp(13), &[]);
    let root = b.add(NodeKind::Block, Payload::None, sp(9), &[assign, use_name]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        vec![xs],
    );
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::binding(sp(10), stable_symbol_hash("xs")),
        EvidenceKind::Domain(nose_il::DomainEvidence::Collection),
        Vec::new(),
    ));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, use_name),
        "binding-domain evidence proves receiver capability, not exact value safety"
    );
}

#[test]
fn binding_domain_after_receiver_use_does_not_prove_receiver() {
    let fixture = crate::test_support::BindingDomainContainsFixture::after_receiver_use();
    assert!(
        !fixture.is_safe(),
        "binding-domain evidence must be visible at the receiver use site"
    );
}

#[test]
fn map_get_method_requires_library_api_occurrence_evidence() {
    let interner = Interner::new();
    let map = interner.intern("m");
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(40), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern("get")),
        sp(41),
        &[receiver],
    );
    let key = b.add(
        NodeKind::Lit,
        Payload::LitStr(stable_symbol_hash("ready")),
        sp(42),
        &[],
    );
    let call = b.add(NodeKind::Call, Payload::None, sp(43), &[callee, key]);
    let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.ts".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        vec![map],
    );
    il.push_evidence(evidence(
        0,
        EvidenceAnchor::node(sp(40), NodeKind::Var),
        EvidenceKind::Domain(nose_il::DomainEvidence::Map),
        Vec::new(),
    ));

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_map_get_call_safe(&il, &interner, &facts, call, callee, "get"),
        "receiver domain plus method spelling must not admit map-get semantics"
    );

    il.push_evidence(map_get_library_api_evidence(
        1,
        Lang::TypeScript,
        "get",
        sp(43),
        vec![EvidenceId(0)],
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        strict_exact_map_get_call_safe(&il, &interner, &facts, call, callee, "get"),
        "admitted map-get occurrence evidence should open the exact-safe API path"
    );
}

#[test]
fn swift_default_subscript_requires_language_core_dictionary_receiver() {
    let interner = Interner::new();
    let dict = interner.intern("dict");
    let key_name = interner.intern("key");
    let default_name = interner.intern("fallback");
    let marker = interner.intern("swift_subscript_default");
    let mut b = IlBuilder::new(FileId(0));
    let receiver_param = b.add(NodeKind::Param, Payload::Cid(0), sp(50), &[]);
    let key_param = b.add(NodeKind::Param, Payload::Cid(1), sp(51), &[]);
    let default_param = b.add(NodeKind::Param, Payload::Cid(2), sp(52), &[]);
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(53), &[]);
    let key = b.add(NodeKind::Var, Payload::Cid(1), sp(54), &[]);
    let default = b.add(NodeKind::Var, Payload::Cid(2), sp(55), &[]);
    let index_marker = b.add(
        NodeKind::Seq,
        Payload::Name(marker),
        sp(56),
        &[key, default],
    );
    let index = b.add(
        NodeKind::Index,
        Payload::None,
        sp(57),
        &[receiver, index_marker],
    );
    let root = b.add(
        NodeKind::Func,
        Payload::None,
        sp(49),
        &[receiver_param, key_param, default_param, index],
    );
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t.swift".into(),
            lang: Lang::Swift,
        },
        Vec::new(),
        vec![dict, key_name, default_name],
    );

    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, index),
        "marker spelling alone must not prove Swift Dictionary default-subscript semantics"
    );

    il.push_evidence(evidence(
        0,
        EvidenceAnchor::param(sp(50)),
        EvidenceKind::Domain(nose_il::DomainEvidence::Map),
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, index),
        "broad Map-domain evidence alone must not open the Swift subscript path"
    );

    il.push_evidence(language_core_evidence(
        1,
        Lang::Swift,
        EvidenceAnchor::param(sp(50)),
        EvidenceKind::Type(nose_il::TypeEvidenceKind::SwiftUnqualifiedDictionaryParameter),
        Vec::new(),
    ));
    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        strict_exact_safe_tree(&il, &interner, &facts, index),
        "live language-core Dictionary evidence should open stable direct coordinates"
    );
}

#[test]
fn swift_compact_map_with_proven_option_emission_is_exact_safe() {
    let interner = Interner::new();
    let il = normalized_swift(
        "func f(_ xs: [Bool]) -> [Bool] {\n    return xs.compactMap { x in x ? x : nil }\n}\n",
        &interner,
    );
    let root = il
        .units
        .iter()
        .find(|unit| unit.name.is_some_and(|name| interner.resolve(name) == "f"))
        .expect("function f")
        .root;
    let facts = StrictFacts::collect(&il, &interner);
    let hof = il
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            (node.payload == Payload::HoF(HoFKind::FilterMap)).then_some(NodeId(index as u32))
        })
        .expect("normalized compactMap HOF");
    assert!(
        admitted_hof_demand_effect_profile_at_node_with_interner(
            &il,
            Some(&interner),
            hof,
            HoFKind::FilterMap,
        )
        .is_some(),
        "the admitted compactMap HOF should retain its demand/effect evidence"
    );
    for &child in il.children(hof) {
        assert!(
            strict_exact_safe_tree(&il, &interner, &facts, child),
            "compactMap HOF child {:?} must be exact-safe",
            il.kind(child)
        );
    }
    assert!(
        strict_exact_safe_tree(&il, &interner, &facts, root),
        "an admitted Swift compactMap callback should participate in exact semantic matching"
    );
}

#[test]
fn unmodeled_swift_compact_map_selector_is_not_opaque_exact_identity() {
    let interner = Interner::new();
    let il = normalized_swift(
        "func f(_ xs: [Bool], _ other: [Bool], _ flag: Bool) -> [Bool] {\n    return xs.map { _ in flag }.compactMap { x in x ? x : nil }\n}\n",
        &interner,
    );
    let compact_map = il
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            if node.kind != NodeKind::Call {
                return None;
            }
            let call = NodeId(index as u32);
            let callee = il.children(call).first().copied()?;
            matches!(
                (il.kind(callee), il.node(callee).payload),
                (NodeKind::Field, Payload::Name(name)) if interner.resolve(name) == "compactMap"
            )
            .then_some(call)
        })
        .expect("unmodeled compactMap call remains raw");
    let facts = StrictFacts::collect(&il, &interner);
    assert!(
        !strict_exact_safe_tree(&il, &interner, &facts, compact_map),
        "a surviving compactMap selector must not borrow opaque exact method identity"
    );
}

#[test]
fn unmodeled_swift_flat_map_selector_is_not_opaque_exact_identity() {
    let interner = Interner::new();
    let il = normalized_swift(
        r#"struct Values {
    func flatMap(_ transform: ([Bool]) -> [Bool]) -> [Bool] { [] }
}
func f(_ groups: [[Bool]]) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group }
}
func g(_ groups: Values) -> [Bool] {
    groups.flatMap { (group: [Bool]) in group }
}
"#,
        &interner,
    );
    let flat_maps = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            if node.kind != NodeKind::Call {
                return None;
            }
            let call = NodeId(index as u32);
            let callee = il.children(call).first().copied()?;
            matches!(
                (il.kind(callee), il.node(callee).payload),
                (NodeKind::Field, Payload::Name(name)) if interner.resolve(name) == "flatMap"
            )
            .then_some(call)
        })
        .collect::<Vec<_>>();
    assert_eq!(flat_maps.len(), 2, "both ambiguous flatMap calls stay raw");
    let facts = StrictFacts::collect(&il, &interner);
    for flat_map in flat_maps {
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, flat_map),
            "a surviving flatMap selector must not borrow opaque exact method identity"
        );
    }
}
