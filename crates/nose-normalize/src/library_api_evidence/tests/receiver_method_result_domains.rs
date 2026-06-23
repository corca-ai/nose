use super::*;

pub(super) fn receiver_method_result_domain_il(
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
