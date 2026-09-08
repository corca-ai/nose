use super::*;

fn leaf_il() -> Il {
    let mut b = IlBuilder::new(FileId(0));
    let span = Span::new(FileId(0), 0, 1, 1, 1);
    let root = b.add(NodeKind::Module, Payload::None, span, &[]);
    b.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn well_formed_il_validates() {
    assert!(leaf_il().validate().is_ok());
}

#[test]
fn editing_a_cached_arena_refreshes_span_and_binding_queries() {
    let mut il = leaf_il();
    let old = il.node(il.root).span;
    let new = Span::new(FileId(0), 5, 9, 2, 2);
    assert_eq!(il.nodes_spanning(old).count(), 1);
    assert_eq!(il.params_with_cid(7).count(), 0);
    let root = il.root;
    let contents = il.edit();
    contents.nodes[root.0 as usize].span = new;
    contents.nodes[root.0 as usize].kind = NodeKind::Param;
    contents.nodes[root.0 as usize].payload = Payload::Cid(7);
    assert_eq!(il.nodes_spanning(old).count(), 0);
    assert_eq!(il.nodes_spanning(new).count(), 1);
    assert_eq!(il.params_with_cid(7).collect::<Vec<_>>(), vec![root]);
    assert_eq!(il.clone().nodes_spanning(new).count(), 1);
    // Direct field mutation goes through the same invalidation boundary.
    il.nodes[root.0 as usize].span = old;
    assert_eq!(il.nodes_spanning(new).count(), 0);
    assert_eq!(il.nodes_spanning(old).count(), 1);
    let json = serde_json::to_value(&il).unwrap();
    assert!(json.get("nodes").is_some());
    assert!(json.get("contents").is_none());
    let restored: Il = serde_json::from_value(json).unwrap();
    assert_eq!(restored.nodes_spanning(old).count(), 1);
}

#[test]
fn evidence_record_edits_refresh_identity_queries_and_expose_live_metadata() {
    let mut il = leaf_il();
    let old = il.node(il.root).span;
    let new = Span::new(FileId(0), 10, 14, 2, 2);
    il.find_or_push_builtin_evidence(
        EvidenceAnchor::node(old, NodeKind::Module),
        EvidenceKind::Domain(DomainEvidence::Collection),
        "test",
        "test",
        Vec::new(),
    );
    assert_eq!(il.evidence_anchored_at(old).count(), 1);
    {
        let mut record = il.evidence_record_mut(0);
        record.anchor = EvidenceAnchor::node(new, NodeKind::Module);
        record.id = EvidenceId(5);
    }
    assert_eq!(il.evidence_anchored_at(old).count(), 0);
    assert_eq!(il.evidence_anchored_at(new).count(), 1);
    assert!(il.evidence_record_by_id(EvidenceId(0)).is_none());
    il.evidence_record_mut(0).status = EvidenceStatus::Ambiguous;
    assert_eq!(
        il.evidence_record_by_id(EvidenceId(5)).unwrap().status,
        EvidenceStatus::Ambiguous
    );
}

#[test]
fn span_contains_only_nested_ranges_in_the_same_file() {
    let outer = Span::new(FileId(0), 10, 20, 1, 2);
    assert!(outer.contains(Span::new(FileId(0), 10, 20, 1, 2)));
    assert!(outer.contains(Span::new(FileId(0), 12, 18, 1, 2)));
    assert!(!outer.contains(Span::new(FileId(0), 9, 18, 1, 2)));
    assert!(!outer.contains(Span::new(FileId(1), 12, 18, 1, 2)));
}

#[test]
fn evidence_kind_maps_every_embedded_span() {
    let first = Span::new(FileId(1), 10, 20, 2, 3);
    let second = Span::new(FileId(1), 30, 40, 4, 5);
    let kinds = [
        (
            EvidenceKind::Guard(GuardEvidenceKind::BoundOrder {
                lower_span: first,
                upper_span: second,
                activation: BoundOrderGuardActivation::WhenTrue,
            }),
            2,
        ),
        (
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span: first,
                name_hash: 1,
            }),
            1,
        ),
        (
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectMethod {
                target_span: first,
                receiver_type_hash: 2,
                method_hash: 3,
            }),
            1,
        ),
        (
            EvidenceKind::PromiseSettledValue(PromiseSettledValueEvidenceKind {
                channel: PromiseSettlementChannel::Fulfilled,
                payload_span: first,
                payload_kind: NodeKind::Lit,
            }),
            1,
        ),
    ];

    for (kind, expected_spans) in kinds {
        let mut mapped_spans = 0;
        let mapped = kind.map_spans(|mut span| {
            mapped_spans += 1;
            span.file = FileId(9);
            span
        });
        assert_eq!(mapped_spans, expected_spans);

        let mut verified_spans = 0;
        mapped.map_spans(|span| {
            verified_spans += 1;
            assert_eq!(span.file, FileId(9));
            span
        });
        assert_eq!(verified_spans, expected_spans);
    }

    let unchanged = EvidenceKind::Domain(DomainEvidence::Collection);
    assert_eq!(unchanged.map_spans(|_| unreachable!()), unchanged);
}

#[test]
fn dangling_child_is_caught() {
    let mut il = leaf_il();
    il.edit().edges.push(NodeId(999)); // child id past the arena
    il.edit().nodes[0].child_len = 1;
    assert!(il.validate().is_err(), "a dangling child id must fail");
}

#[test]
fn out_of_bounds_root_is_caught() {
    let mut il = leaf_il();
    il.edit().root = NodeId(42);
    assert!(il.validate().is_err(), "an invalid root must fail");
}

#[test]
fn child_range_past_edges_is_caught() {
    let mut il = leaf_il();
    il.edit().nodes[0].child_len = 5; // claims children that don't exist
    assert!(
        il.validate().is_err(),
        "an out-of-range child span must fail"
    );
}

#[test]
fn cached_scope_chain_follows_lexical_parents_without_crossing_siblings() {
    let mut b = IlBuilder::new(FileId(0));
    let left_ref = b.add(
        NodeKind::Var,
        Payload::Cid(0),
        Span::new(FileId(0), 30, 31, 1, 1),
        &[],
    );
    let left = b.add(
        NodeKind::Lambda,
        Payload::None,
        Span::new(FileId(0), 20, 40, 1, 1),
        &[left_ref],
    );
    let right = b.add(
        NodeKind::Lambda,
        Payload::None,
        Span::new(FileId(0), 50, 70, 1, 1),
        &[],
    );
    let outer = b.add(
        NodeKind::Func,
        Payload::None,
        Span::new(FileId(0), 10, 80, 1, 1),
        &[left, right],
    );
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        Span::new(FileId(0), 0, 90, 1, 1),
        &[outer],
    );
    let il = b.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(il.nearest_scope(left_ref), Some(left));
    assert_eq!(il.parent_scope(left), Some(outer));
    assert_eq!(il.parent_scope(right), Some(outer));
    assert_eq!(il.parent_scope(outer), None);
}

#[test]
fn equal_span_scope_chain_preserves_width_then_id_preference() {
    let mut b = IlBuilder::new(FileId(0));
    let span = Span::new(FileId(0), 10, 20, 1, 1);
    let first = b.add(NodeKind::Lambda, Payload::None, span, &[]);
    let second = b.add(NodeKind::Lambda, Payload::None, span, &[]);
    let value = b.add(NodeKind::Var, Payload::Cid(0), span, &[]);
    let root = b.add(
        NodeKind::Module,
        Payload::None,
        span,
        &[first, second, value],
    );
    let il = b.finish(
        root,
        FileMeta {
            path: "t".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(il.nearest_scope(value), Some(first));
    assert_eq!(il.parent_scope(first), Some(second));
    assert_eq!(il.parent_scope(second), None);
}

#[test]
fn scope_binding_index_covers_params_destructuring_and_foreach_patterns() {
    let interner = Interner::new();
    let param_name = interner.intern("param");
    let destructured_name = interner.intern("destructured");
    let loop_name = interner.intern("item");
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(
        NodeKind::Param,
        Payload::Name(param_name),
        Span::new(FileId(0), 1, 2, 1, 1),
        &[],
    );
    let _orphan_duplicate_param = b.add(
        NodeKind::Param,
        Payload::Name(param_name),
        Span::new(FileId(0), 1, 2, 1, 1),
        &[],
    );
    let destructured = b.add(
        NodeKind::Var,
        Payload::Name(destructured_name),
        Span::new(FileId(0), 10, 11, 2, 2),
        &[],
    );
    let cid_target = b.add(
        NodeKind::Var,
        Payload::Cid(7),
        Span::new(FileId(0), 12, 13, 2, 2),
        &[],
    );
    let assign_target = b.add(
        NodeKind::Seq,
        Payload::None,
        Span::new(FileId(0), 10, 13, 2, 2),
        &[destructured, cid_target],
    );
    let value = b.add(
        NodeKind::Lit,
        Payload::LitInt(1),
        Span::new(FileId(0), 16, 17, 2, 2),
        &[],
    );
    let assign = b.add(
        NodeKind::Assign,
        Payload::None,
        Span::new(FileId(0), 10, 17, 2, 2),
        &[assign_target, value],
    );
    let loop_target = b.add(
        NodeKind::Var,
        Payload::Name(loop_name),
        Span::new(FileId(0), 30, 31, 3, 3),
        &[],
    );
    let iterable = b.add(
        NodeKind::Var,
        Payload::Cid(9),
        Span::new(FileId(0), 35, 36, 3, 3),
        &[],
    );
    let loop_body = b.add(
        NodeKind::Block,
        Payload::None,
        Span::new(FileId(0), 40, 50, 4, 4),
        &[],
    );
    let loop_node = b.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::ForEach),
        Span::new(FileId(0), 30, 50, 3, 4),
        &[loop_target, iterable, loop_body],
    );
    let scope = b.add(
        NodeKind::Func,
        Payload::None,
        Span::new(FileId(0), 0, 60, 1, 4),
        &[param, assign, loop_node],
    );
    let mut il = b.finish(
        scope,
        FileMeta {
            path: "t".into(),
            lang: Lang::TypeScript,
        },
        Vec::new(),
        Vec::new(),
    );

    assert!(il.scope_binds_name(scope, param_name));
    assert_eq!(il.scope_name_param_count(scope, param_name), 1);
    assert!(!il.scope_writes_name(scope, param_name));
    assert!(il.scope_writes_name(scope, destructured_name));
    assert!(il.scope_writes_name(scope, loop_name));
    assert!(il.scope_writes_cid(scope, 7));

    il.edit().nodes[destructured.0 as usize].payload = Payload::Cid(11);
    il.invalidate_scope_binding_index();
    assert!(!il.scope_writes_name(scope, destructured_name));
    assert!(il.scope_writes_cid(scope, 11));
}

#[test]
fn builtin_evidence_dedupe_preserves_provenance_boundary() {
    let mut il = leaf_il();
    let anchor = EvidenceAnchor::node(il.node(il.root).span, NodeKind::Module);
    let kind = EvidenceKind::Domain(DomainEvidence::Collection);
    il.push_evidence(EvidenceRecord {
        id: EvidenceId(0),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::External,
            pack_hash: Some(stable_symbol_hash("external.pack")),
            rule_hash: Some(stable_symbol_hash("external.rule")),
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    });

    let first =
        il.find_or_push_builtin_evidence(anchor, kind, "nose.first_party", "rule.a", Vec::new());
    let duplicate =
        il.find_or_push_builtin_evidence(anchor, kind, "nose.first_party", "rule.a", Vec::new());
    let different_rule =
        il.find_or_push_builtin_evidence(anchor, kind, "nose.first_party", "rule.b", Vec::new());

    assert_eq!(first, EvidenceId(1));
    assert_eq!(duplicate, first);
    assert_eq!(different_rule, EvidenceId(2));
}

#[test]
fn legacy_first_party_evidence_helper_alias_matches_builtin_helper() {
    let mut il = leaf_il();
    let anchor = EvidenceAnchor::node(il.node(il.root).span, NodeKind::Module);
    let kind = EvidenceKind::Domain(DomainEvidence::Collection);

    let builtin =
        il.find_or_push_builtin_evidence(anchor, kind, "nose.first_party", "rule.a", Vec::new());
    let legacy = il.find_or_push_first_party_evidence(
        anchor,
        kind,
        "nose.first_party",
        "rule.a",
        Vec::new(),
    );

    assert_eq!(legacy, builtin);
}

#[test]
fn builtin_evidence_emitter_keeps_legacy_wire_name() {
    assert_eq!(EvidenceEmitter::FirstParty, EvidenceEmitter::Builtin);
    assert_eq!(
        serde_json::to_string(&EvidenceEmitter::Builtin).unwrap(),
        "\"FirstParty\""
    );
    assert_eq!(
        serde_json::from_str::<EvidenceEmitter>("\"FirstParty\"").unwrap(),
        EvidenceEmitter::Builtin
    );
    assert_eq!(
        serde_json::from_str::<EvidenceEmitter>("\"Builtin\"").unwrap(),
        EvidenceEmitter::Builtin
    );
}

/// `clear()` + re-push rewrites the indexed prefix without shrinking below
/// the indexed length — the staleness sentinel must trigger a rebuild, not
/// serve buckets for records that no longer exist.
#[test]
fn evidence_index_survives_clear_and_repush() {
    let mut il = leaf_il();
    let span = il.node(il.root).span;
    let record = |id: u32, anchor| EvidenceRecord {
        id: EvidenceId(id),
        anchor,
        kind: EvidenceKind::Domain(DomainEvidence::Collection),
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::Builtin,
            pack_hash: None,
            rule_hash: None,
        },
        dependencies: Vec::new(),
        status: EvidenceStatus::Asserted,
    };

    il.push_evidence(record(0, EvidenceAnchor::node(span, NodeKind::Module)));
    // Build the index, then invalidate it the rude way.
    assert_eq!(il.evidence_anchored_at(span).count(), 1);
    il.edit().evidence.clear();
    il.push_evidence(record(0, EvidenceAnchor::binding(span, 7)));
    il.push_evidence(record(1, EvidenceAnchor::node(span, NodeKind::Module)));

    assert_eq!(il.evidence_anchored_at(span).count(), 2);
    assert_eq!(il.evidence_binding_anchored(7).count(), 1);
}
