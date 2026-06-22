use super::*;
use nose_il::{EvidenceProvenance, FileId, FileMeta, IlBuilder, Lang, Span};
use nose_semantics::{
    admitted_builder_append_method_call_args, BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
    BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID, GO_STDLIB_NAMESPACE_CALL_PACK_ID,
    GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID, RUST_STDLIB_OPTION_PACK_ID,
    RUST_STDLIB_OPTION_PRODUCER_ID,
};

fn sp(byte: u32) -> Span {
    Span::new(FileId(0), byte, byte + 1, byte, byte + 1)
}

fn method_call_il(
    interner: &mut Interner,
    lang: Lang,
    method: &str,
    arg_count: usize,
) -> (Il, NodeId, NodeId, Option<NodeId>) {
    let mut builder = IlBuilder::new(FileId(0));
    let name = interner.intern("r");
    let seed_span = sp(1);
    let seed = builder.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("array")),
        seed_span,
        &[],
    );
    let target = builder.add(NodeKind::Var, Payload::Name(name), sp(2), &[]);
    let assign = builder.add(NodeKind::Assign, Payload::None, sp(2), &[target, seed]);
    let receiver = builder.add(NodeKind::Var, Payload::Name(name), sp(3), &[]);
    let field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(3),
        &[receiver],
    );
    let args: Vec<NodeId> = (0..arg_count)
        .map(|idx| builder.add(NodeKind::Var, Payload::Cid((idx + 1) as u32), sp(4), &[]))
        .collect();
    let first_arg = args.first().copied();
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(field);
    children.extend(args);
    let call = builder.add(NodeKind::Call, Payload::None, sp(5), &children);
    let root = builder.add(NodeKind::Func, Payload::None, sp(6), &[assign, call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "method".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::sequence(seed_span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        pack_id,
        producer_id,
        Vec::new(),
    );
    (il, call, receiver, first_arg)
}

fn language_core_provenance(lang: Lang) -> EvidenceProvenance {
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    pack_provenance(pack_id, producer_id)
}

fn pack_provenance(pack_id: &str, producer_id: &str) -> EvidenceProvenance {
    EvidenceProvenance {
        emitter: EvidenceEmitter::Builtin,
        pack_hash: Some(stable_symbol_hash(pack_id)),
        rule_hash: Some(stable_symbol_hash(producer_id)),
    }
}

fn node_records(il: &Il, node: NodeId, kind: EvidenceKind) -> Vec<&EvidenceRecord> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence_anchored_at(anchor.span())
        .filter(|record| record.anchor == anchor && record.kind == kind)
        .collect()
}

fn node_domain_records(il: &Il, node: NodeId, domain: DomainEvidence) -> Vec<&EvidenceRecord> {
    node_records(il, node, EvidenceKind::Domain(domain))
}

fn library_api_records(il: &Il, node: NodeId) -> Vec<&EvidenceRecord> {
    let anchor = EvidenceAnchor::node(il.node(node).span, il.kind(node));
    il.evidence_anchored_at(anchor.span())
        .filter(|record| {
            record.anchor == anchor && matches!(record.kind, EvidenceKind::LibraryApi(_))
        })
        .collect()
}

fn asserted(records: Vec<&EvidenceRecord>) -> Vec<&EvidenceRecord> {
    records
        .into_iter()
        .filter(|record| record.status == EvidenceStatus::Asserted)
        .collect()
}

#[test]
fn builder_append_method_api_evidence_admits_first_party_rows() {
    for (lang, method) in [
        (Lang::Python, "append"),
        (Lang::JavaScript, "push"),
        (Lang::Java, "add"),
        (Lang::Rust, "push"),
    ] {
        let mut interner = Interner::new();
        let (mut il, call, receiver, item) = method_call_il(&mut interner, lang, method, 1);

        run(&mut il, &interner);

        let (admitted_receiver, admitted_item) =
            admitted_builder_append_method_call_args(&il, &interner, call)
                .expect("builder append method evidence");
        assert_eq!(admitted_receiver, receiver);
        assert_eq!(Some(admitted_item), item);

        let receiver_domains = node_domain_records(&il, receiver, DomainEvidence::Collection);
        assert_eq!(receiver_domains.len(), 1);
        assert_eq!(
            receiver_domains[0].provenance,
            language_core_provenance(lang)
        );
        assert!(!receiver_domains[0].dependencies.is_empty());

        let api = library_api_records(&il, call)
            .into_iter()
            .find(|record| record.status == EvidenceStatus::Asserted)
            .expect("builder append API evidence");
        assert_eq!(
            api.provenance,
            pack_provenance(
                BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
                BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID
            )
        );
        assert_eq!(api.dependencies, vec![receiver_domains[0].id]);
    }
}

#[test]
fn builder_append_receiver_domain_updates_legacy_first_party_row_in_place() {
    let mut interner = Interner::new();
    let (mut il, call, receiver, _) = method_call_il(&mut interner, Lang::JavaScript, "push", 1);
    let anchor = EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver));
    let legacy = il.find_or_push_first_party_evidence(
        anchor,
        EvidenceKind::Domain(DomainEvidence::Collection),
        BUILTIN_COMPAT_PACK_ID,
        "legacy_builder_append_receiver_domain",
        Vec::new(),
    );

    run(&mut il, &interner);

    assert!(admitted_builder_append_method_call_args(&il, &interner, call).is_some());
    let receiver_domains = node_domain_records(&il, receiver, DomainEvidence::Collection);
    assert_eq!(receiver_domains.len(), 1);
    assert_eq!(receiver_domains[0].id, legacy);
    assert_eq!(
        receiver_domains[0].provenance,
        language_core_provenance(Lang::JavaScript)
    );
    assert!(!receiver_domains[0].dependencies.is_empty());

    let api = library_api_records(&il, call)
        .into_iter()
        .find(|record| record.status == EvidenceStatus::Asserted)
        .expect("builder append API evidence");
    assert_eq!(api.dependencies, vec![legacy]);
}

#[test]
fn builder_append_receiver_domain_closes_duplicate_legacy_row_when_current_exists() {
    let mut interner = Interner::new();
    let (mut il, call, receiver, _) = method_call_il(&mut interner, Lang::JavaScript, "push", 1);
    let anchor = EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver));
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::JavaScript);
    let current = il.find_or_push_first_party_evidence(
        anchor,
        EvidenceKind::Domain(DomainEvidence::Collection),
        pack_id,
        producer_id,
        Vec::new(),
    );
    let legacy = il.find_or_push_first_party_evidence(
        anchor,
        EvidenceKind::Domain(DomainEvidence::Collection),
        BUILTIN_COMPAT_PACK_ID,
        "legacy_builder_append_receiver_domain",
        Vec::new(),
    );

    run(&mut il, &interner);

    let receiver_domains = node_domain_records(&il, receiver, DomainEvidence::Collection);
    let asserted_domains = asserted(receiver_domains.clone());
    assert_eq!(asserted_domains.len(), 1);
    assert_eq!(asserted_domains[0].id, current);
    assert_eq!(
        il.evidence_record_by_id(legacy).expect("legacy row").status,
        EvidenceStatus::Ambiguous
    );
    let api = library_api_records(&il, call)
        .into_iter()
        .find(|record| record.status == EvidenceStatus::Asserted)
        .expect("builder append API evidence");
    assert_eq!(api.dependencies, vec![current]);
}

#[test]
fn rust_option_result_domain_uses_language_core_provenance() {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let callee = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern("Some")),
        sp(1),
        &[],
    );
    let arg = builder.add(NodeKind::Var, Payload::Cid(1), sp(2), &[]);
    let call = builder.add(NodeKind::Call, Payload::None, sp(3), &[callee, arg]);
    let root = builder.add(NodeKind::Func, Payload::None, sp(4), &[call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "option".into(),
            lang: Lang::Rust,
        },
        Vec::new(),
        Vec::new(),
    );

    run(&mut il, &interner);

    let result_domains = node_domain_records(&il, call, DomainEvidence::Option);
    assert_eq!(result_domains.len(), 1);
    assert_eq!(
        result_domains[0].provenance,
        language_core_provenance(Lang::Rust)
    );
    let api = library_api_records(&il, call)
        .into_iter()
        .find(|record| record.status == EvidenceStatus::Asserted)
        .expect("Rust Option API evidence");
    assert_eq!(
        api.provenance,
        pack_provenance(RUST_STDLIB_OPTION_PACK_ID, RUST_STDLIB_OPTION_PRODUCER_ID)
    );
    assert_eq!(result_domains[0].dependencies, vec![api.id]);
}

fn receiver_method_result_domain_il(
    lang: Lang,
    receiver_name: &str,
    method: &str,
    arg_count: usize,
    receiver_domain: Option<DomainEvidence>,
) -> (Il, Interner, NodeId, NodeId) {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let receiver = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern(receiver_name)),
        sp(10),
        &[],
    );
    let field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern(method)),
        sp(11),
        &[receiver],
    );
    let mut children = vec![field];
    for idx in 0..arg_count {
        children.push(builder.add(
            NodeKind::Var,
            Payload::Cid((idx + 1) as u32),
            sp(12 + idx as u32),
            &[],
        ));
    }
    let call = builder.add(NodeKind::Call, Payload::None, sp(20), &children);
    let root = builder.add(NodeKind::Func, Payload::None, sp(21), &[call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "receiver-method".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    if let Some(domain) = receiver_domain {
        let (pack_id, producer_id) = language_core_evidence_provenance(lang);
        il.find_or_push_first_party_evidence(
            EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
            EvidenceKind::Domain(domain),
            pack_id,
            producer_id,
            Vec::new(),
        );
    }
    (il, interner, call, receiver)
}

#[test]
fn receiver_method_api_result_domains_are_emitted_from_admitted_contracts() {
    for (lang, receiver_name, method, args, receiver_domain, expected_domain) in [
        (
            Lang::TypeScript,
            "m",
            "keys",
            0,
            Some(DomainEvidence::Map),
            DomainEvidence::Iterator,
        ),
        (
            Lang::Java,
            "m",
            "keySet",
            0,
            Some(DomainEvidence::Map),
            DomainEvidence::Collection,
        ),
        (
            Lang::Rust,
            "n",
            "abs",
            0,
            Some(DomainEvidence::Integer),
            DomainEvidence::Integer,
        ),
        (
            Lang::Rust,
            "maybe",
            "and_then",
            1,
            Some(DomainEvidence::Option),
            DomainEvidence::Option,
        ),
        (
            Lang::TypeScript,
            "p",
            "then",
            1,
            Some(DomainEvidence::PromiseLike),
            DomainEvidence::PromiseLike,
        ),
    ] {
        let (mut il, interner, call, _) =
            receiver_method_result_domain_il(lang, receiver_name, method, args, receiver_domain);

        run(&mut il, &interner);

        let api = library_api_records(&il, call)
            .into_iter()
            .find(|record| record.status == EvidenceStatus::Asserted)
            .expect("receiver-method API evidence");
        let result_domains = node_domain_records(&il, call, expected_domain);
        assert_eq!(
            result_domains.len(),
            1,
            "{lang:?} {method} should emit one result-domain row"
        );
        assert_eq!(result_domains[0].provenance, language_core_provenance(lang));
        assert_eq!(result_domains[0].dependencies, vec![api.id]);
    }
}

#[test]
fn receiver_method_api_result_domains_stay_closed_without_receiver_proof() {
    for (lang, receiver_name, method, arg_count, missing_domain, rejected_domain) in [
        (
            Lang::TypeScript,
            "m",
            "keys",
            0,
            None,
            DomainEvidence::Iterator,
        ),
        (
            Lang::TypeScript,
            "m",
            "keys",
            0,
            Some(DomainEvidence::Collection),
            DomainEvidence::Iterator,
        ),
        (
            Lang::Rust,
            "maybe",
            "and_then",
            1,
            Some(DomainEvidence::Collection),
            DomainEvidence::Option,
        ),
    ] {
        let (mut il, interner, call, _) = receiver_method_result_domain_il(
            lang,
            receiver_name,
            method,
            arg_count,
            missing_domain,
        );

        run(&mut il, &interner);

        assert!(
            asserted(library_api_records(&il, call)).is_empty(),
            "{lang:?} {method} must not produce admitted API evidence without receiver proof"
        );
        assert!(
            node_domain_records(&il, call, rejected_domain).is_empty(),
            "{lang:?} {method} must not emit result-domain evidence without admitted API evidence"
        );
    }
}

fn chained_receiver_method_il(
    lang: Lang,
    receiver_name: &str,
    first_method: &str,
    first_arg_count: usize,
    second_method: &str,
    second_arg_count: usize,
    receiver_domain: DomainEvidence,
) -> (Il, Interner, NodeId, NodeId, NodeId) {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let receiver = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern(receiver_name)),
        sp(30),
        &[],
    );
    let first_field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern(first_method)),
        sp(31),
        &[receiver],
    );
    let mut first_children = vec![first_field];
    for idx in 0..first_arg_count {
        first_children.push(builder.add(
            NodeKind::Var,
            Payload::Cid((idx + 1) as u32),
            sp(32 + idx as u32),
            &[],
        ));
    }
    let first_call = builder.add(NodeKind::Call, Payload::None, sp(40), &first_children);
    let second_field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern(second_method)),
        sp(41),
        &[first_call],
    );
    let mut second_children = vec![second_field];
    for idx in 0..second_arg_count {
        second_children.push(builder.add(
            NodeKind::Var,
            Payload::Cid((idx + 10) as u32),
            sp(42 + idx as u32),
            &[],
        ));
    }
    let second_call = builder.add(NodeKind::Call, Payload::None, sp(50), &second_children);
    let root = builder.add(NodeKind::Func, Payload::None, sp(51), &[second_call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "chained-receiver-method".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    let (pack_id, producer_id) = language_core_evidence_provenance(lang);
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(receiver_domain),
        pack_id,
        producer_id,
        Vec::new(),
    );
    (il, interner, receiver, first_call, second_call)
}

#[test]
fn receiver_method_result_domains_feed_chained_builtin_admission() {
    for (
        lang,
        receiver_name,
        first_method,
        first_arg_count,
        second_method,
        second_arg_count,
        receiver_domain,
        first_result_domain,
    ) in [
        (
            Lang::Java,
            "m",
            "keySet",
            0,
            "contains",
            1,
            DomainEvidence::Map,
            DomainEvidence::Collection,
        ),
        (
            Lang::Rust,
            "n",
            "abs",
            0,
            "max",
            1,
            DomainEvidence::Integer,
            DomainEvidence::Integer,
        ),
        (
            Lang::Rust,
            "maybe",
            "and_then",
            1,
            "and_then",
            1,
            DomainEvidence::Option,
            DomainEvidence::Option,
        ),
        (
            Lang::TypeScript,
            "p",
            "then",
            1,
            "then",
            1,
            DomainEvidence::PromiseLike,
            DomainEvidence::PromiseLike,
        ),
    ] {
        let (mut il, interner, _, first_call, second_call) = chained_receiver_method_il(
            lang,
            receiver_name,
            first_method,
            first_arg_count,
            second_method,
            second_arg_count,
            receiver_domain,
        );

        run(&mut il, &interner);

        let first_api = library_api_records(&il, first_call)
            .into_iter()
            .find(|record| record.status == EvidenceStatus::Asserted)
            .expect("first receiver-method API evidence");
        let first_domains = node_domain_records(&il, first_call, first_result_domain);
        assert_eq!(
            first_domains.len(),
            1,
            "{lang:?} {first_method} should emit one result-domain row"
        );
        assert_eq!(first_domains[0].dependencies, vec![first_api.id]);

        let second_api = library_api_records(&il, second_call)
            .into_iter()
            .find(|record| record.status == EvidenceStatus::Asserted)
            .expect("second receiver-method API evidence");
        assert_eq!(
            second_api.dependencies,
            vec![first_domains[0].id],
            "{lang:?} {second_method} should consume the first call's result-domain proof"
        );
    }
}

#[test]
fn unshadowed_global_helper_symbol_updates_legacy_row_to_language_core() {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let console = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern("console")),
        sp(1),
        &[],
    );
    let field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern("log")),
        sp(1),
        &[console],
    );
    let arg = builder.add(NodeKind::Var, Payload::Cid(1), sp(2), &[]);
    let call = builder.add(NodeKind::Call, Payload::None, sp(3), &[field, arg]);
    let root = builder.add(NodeKind::Func, Payload::None, sp(4), &[call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "console.js".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );
    let symbol = SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash("console"),
    };
    let legacy = il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(console).span, il.kind(console)),
        EvidenceKind::Symbol(symbol),
        BUILTIN_COMPAT_PACK_ID,
        "legacy_unshadowed_global",
        Vec::new(),
    );

    run(&mut il, &interner);

    let symbols = node_records(&il, console, EvidenceKind::Symbol(symbol));
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].id, legacy);
    assert_eq!(
        symbols[0].provenance,
        language_core_provenance(Lang::JavaScript)
    );
    let api = library_api_records(&il, call)
        .into_iter()
        .find(|record| record.status == EvidenceStatus::Asserted)
        .expect("console.log API evidence");
    assert_eq!(
        api.provenance,
        pack_provenance(
            BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID,
            BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID
        )
    );
    assert_eq!(api.dependencies, vec![legacy]);
}

#[test]
fn imported_namespace_helper_symbol_updates_legacy_occurrence_to_language_core() {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let binding_span = sp(1);
    let namespace = builder.add(
        NodeKind::Var,
        Payload::Name(interner.intern("slices")),
        sp(2),
        &[],
    );
    let field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern("Contains")),
        sp(2),
        &[namespace],
    );
    let haystack = builder.add(NodeKind::Var, Payload::Cid(1), sp(3), &[]);
    let needle = builder.add(NodeKind::Var, Payload::Cid(2), sp(4), &[]);
    let call = builder.add(
        NodeKind::Call,
        Payload::None,
        sp(5),
        &[field, haystack, needle],
    );
    let root = builder.add(NodeKind::Func, Payload::None, sp(6), &[call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "slices.go".into(),
            lang: Lang::Go,
        },
        Vec::new(),
        Vec::new(),
    );
    let symbol = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash("slices"),
    };
    let binding = il.find_or_push_first_party_evidence(
        EvidenceAnchor::binding(binding_span, stable_symbol_hash("slices")),
        EvidenceKind::Symbol(symbol),
        BUILTIN_COMPAT_PACK_ID,
        "test_imported_namespace_binding",
        Vec::new(),
    );
    let legacy = il.find_or_push_first_party_evidence(
        EvidenceAnchor::node(il.node(namespace).span, il.kind(namespace)),
        EvidenceKind::Symbol(symbol),
        BUILTIN_COMPAT_PACK_ID,
        "legacy_imported_namespace_occurrence",
        vec![binding],
    );

    run(&mut il, &interner);

    let symbols = node_records(&il, namespace, EvidenceKind::Symbol(symbol));
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].id, legacy);
    assert_eq!(symbols[0].dependencies, vec![binding]);
    assert_eq!(symbols[0].provenance, language_core_provenance(Lang::Go));
    let api = library_api_records(&il, call)
        .into_iter()
        .find(|record| record.status == EvidenceStatus::Asserted)
        .expect("slices.Contains API evidence");
    assert_eq!(
        api.provenance,
        pack_provenance(
            GO_STDLIB_NAMESPACE_CALL_PACK_ID,
            GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID
        )
    );
    assert_eq!(api.dependencies, vec![legacy]);
}

#[test]
fn builder_append_method_api_evidence_is_language_and_arity_scoped() {
    for (lang, method, arg_count) in [
        (Lang::Ruby, "push", 1),
        (Lang::Python, "append", 2),
        (Lang::JavaScript, "push", 2),
    ] {
        let mut interner = Interner::new();
        let (mut il, call, _, _) = method_call_il(&mut interner, lang, method, arg_count);

        run(&mut il, &interner);

        assert!(admitted_builder_append_method_call_args(&il, &interner, call).is_none());
    }
}

#[test]
fn builder_append_method_api_evidence_closes_on_conflicting_sequence_surface_seed() {
    let mut interner = Interner::new();
    let (mut il, call, _, _) = method_call_il(&mut interner, Lang::JavaScript, "push", 1);
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::JavaScript);
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::sequence(sp(1)),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Map),
        pack_id,
        producer_id,
        Vec::new(),
    );

    run(&mut il, &interner);

    assert!(
        admitted_builder_append_method_call_args(&il, &interner, call).is_none(),
        "conflicting sequence-surface proof must not seed builder append API evidence"
    );
}

#[test]
fn builder_append_method_api_evidence_rejects_untagged_sequence_surface_seed() {
    let interner = Interner::new();
    let mut builder = IlBuilder::new(FileId(0));
    let name = interner.intern("r");
    let seed_span = sp(1);
    let seed = builder.add(NodeKind::Seq, Payload::None, seed_span, &[]);
    let target = builder.add(NodeKind::Var, Payload::Name(name), sp(2), &[]);
    let assign = builder.add(NodeKind::Assign, Payload::None, sp(2), &[target, seed]);
    let receiver = builder.add(NodeKind::Var, Payload::Name(name), sp(3), &[]);
    let field = builder.add(
        NodeKind::Field,
        Payload::Name(interner.intern("push")),
        sp(3),
        &[receiver],
    );
    let item = builder.add(NodeKind::Var, Payload::Cid(1), sp(4), &[]);
    let call = builder.add(NodeKind::Call, Payload::None, sp(5), &[field, item]);
    let root = builder.add(NodeKind::Func, Payload::None, sp(6), &[assign, call]);
    let mut il = builder.finish(
        root,
        FileMeta {
            path: "method".into(),
            lang: Lang::JavaScript,
        },
        Vec::new(),
        Vec::new(),
    );
    let (pack_id, producer_id) = language_core_evidence_provenance(Lang::JavaScript);
    il.find_or_push_first_party_evidence(
        EvidenceAnchor::sequence(seed_span),
        EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
        pack_id,
        producer_id,
        Vec::new(),
    );

    run(&mut il, &interner);

    assert!(
        admitted_builder_append_method_call_args(&il, &interner, call).is_none(),
        "untagged sequences must not become collection seeds from evidence alone"
    );
}
