use super::*;

#[test]
fn type_domain_contracts_are_language_scoped_and_exact_enough() {
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Array<string>"),
        Some(DomainEvidence::Array)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: string[]"),
        Some(DomainEvidence::Array)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Iterable<string>"),
        Some(DomainEvidence::Iterable)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Iterator<string>"),
        Some(DomainEvidence::Iterator)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Promise<string>"),
        Some(DomainEvidence::PromiseLike)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Record<string, number>"),
        Some(DomainEvidence::Record)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Result<string, Error>"),
        Some(DomainEvidence::Result)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: boolean"),
        Some(DomainEvidence::Boolean)
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Bitmap<string, number>"),
        None
    );
    assert_eq!(
        type_domain_from_source_text(Lang::TypeScript, "xs: Blacklist<string>"),
        None
    );
}

fn assert_type_domain(lang: Lang, source: &str, expected: Option<DomainEvidence>) {
    assert_eq!(
        type_domain_from_source_text(lang, source),
        expected,
        "type-domain mismatch for {lang:?}: {source}"
    );
}

#[test]
fn type_domain_contracts_cover_java_signatures() {
    assert_type_domain(
        Lang::Java,
        "@Nonnull List<String> xs",
        Some(DomainEvidence::Collection),
    );
    assert_type_domain(
        Lang::Java,
        "Iterator<String> xs",
        Some(DomainEvidence::Iterator),
    );
    assert_type_domain(
        Lang::Java,
        "CompletableFuture<String> xs",
        Some(DomainEvidence::FutureLike),
    );
    assert_type_domain(Lang::Java, "boolean value", Some(DomainEvidence::Boolean));
    assert_type_domain(
        Lang::Java,
        "@Ann(\"...\") String value",
        Some(DomainEvidence::String),
    );
}

#[test]
fn type_domain_contracts_cover_rust_signatures() {
    assert_type_domain(
        Lang::Rust,
        "std::collections::HashMap<String, i32>",
        Some(DomainEvidence::Map),
    );
    assert_type_domain(Lang::Rust, "HashSet<i32>", Some(DomainEvidence::Set));
    assert_type_domain(
        Lang::Rust,
        "Result<String, Error>",
        Some(DomainEvidence::Result),
    );
    assert_type_domain(
        Lang::Rust,
        "impl Iterator<Item = i32>",
        Some(DomainEvidence::Iterator),
    );
    assert_type_domain(Lang::Rust, "std::pin::Pin<Box<T>>", None);
    assert_type_domain(Lang::Rust, "bool", Some(DomainEvidence::Boolean));
}

#[test]
fn type_domain_contracts_cover_c_and_go_signatures() {
    assert_type_domain(Lang::C, "int *xs", None);
    assert_type_domain(Lang::C, "int xs", Some(DomainEvidence::Integer));
    assert_type_domain(Lang::C, "_Bool ok", Some(DomainEvidence::Boolean));
    assert_type_domain(Lang::Go, "value bool", Some(DomainEvidence::Boolean));
    assert_type_domain(
        Lang::Go,
        "type User struct { id int }",
        Some(DomainEvidence::Record),
    );
}

#[test]
fn type_domain_contracts_cover_swift_signatures() {
    assert_type_domain(Lang::Swift, "xs: [Int]", Some(DomainEvidence::Collection));
    assert_type_domain(
        Lang::Swift,
        "xs: Array<String>",
        Some(DomainEvidence::Collection),
    );
    assert_type_domain(
        Lang::Swift,
        "lookup: [String: Int]",
        Some(DomainEvidence::Map),
    );
    assert_type_domain(
        Lang::Swift,
        "lookup: Dictionary<String, Int>",
        Some(DomainEvidence::Map),
    );
    assert_type_domain(Lang::Swift, "seen: Set<String>", Some(DomainEvidence::Set));
    assert_type_domain(Lang::Swift, "value: Int?", Some(DomainEvidence::Option));
    assert_type_domain(
        Lang::Swift,
        "result: Result<Int, Error>",
        Some(DomainEvidence::Result),
    );
    assert_type_domain(
        Lang::Swift,
        "items: AnySequence<Int>",
        Some(DomainEvidence::Iterable),
    );
    assert_type_domain(
        Lang::Swift,
        "items: AnyCollection<Int>",
        Some(DomainEvidence::Collection),
    );
    assert_type_domain(Lang::Swift, "text: String", Some(DomainEvidence::String));
    assert_type_domain(Lang::Swift, "ok: Bool", Some(DomainEvidence::Boolean));
    assert_type_domain(Lang::Swift, "xs: Bitmap<String>", None);
}

#[test]
fn python_stdlib_type_domain_rows_are_module_scoped() {
    let iterable = python_stdlib_type_domain_contract("typing", "Iterable")
        .expect("typing.Iterable should be a first-party pack row");
    assert_eq!(iterable.pack_id, PYTHON_STDLIB_TYPE_DOMAIN_PACK_ID);
    assert_eq!(iterable.producer_id, PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID);
    assert_eq!(iterable.domain, DomainEvidence::Iterable);
    assert_eq!(
        python_stdlib_type_domain("collections.abc", "Mapping"),
        Some(DomainEvidence::Map)
    );
    assert_eq!(
        python_stdlib_type_domain("collections.abc", "Awaitable"),
        Some(DomainEvidence::FutureLike)
    );
    assert_eq!(
        python_stdlib_type_domain("asyncio", "Future"),
        Some(DomainEvidence::FutureLike)
    );
    assert_eq!(python_stdlib_type_domain("typing", "Blacklist"), None);
    assert_eq!(python_stdlib_type_domain("typing", "Future"), None);
    assert_eq!(python_stdlib_type_domain("collections.abc", "Dict"), None);
    assert_eq!(
        python_stdlib_type_domain("collections.abc", "FrozenSet"),
        None
    );
    assert_eq!(python_stdlib_type_domain("collections.abc", "Deque"), None);
    assert_eq!(python_stdlib_type_domain("collections.abc", "List"), None);
    assert_eq!(python_stdlib_type_domain("collections.abc", "Tuple"), None);
    assert_eq!(python_stdlib_type_domain("asyncio", "Awaitable"), None);
    assert_eq!(python_stdlib_type_domain("asyncio", "Coroutine"), None);
    assert_eq!(python_stdlib_type_domain("custom.typing", "Iterable"), None);
}

#[test]
fn nominal_type_domain_evidence_is_dependency_backed_and_fail_closed() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(
        NodeKind::Var,
        Payload::Name(interner.intern("value")),
        sp(12),
        &[],
    );
    let root = b.add(NodeKind::Block, Payload::None, sp(11), &[receiver]);
    let mut il = finish_il(b, root, Lang::TypeScript);
    let widget = stable_symbol_hash("pkg.Widget");
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
            type_hash: widget,
            domain: DomainEvidence::Record,
        }),
        EvidenceStatus::Asserted,
    ));

    assert_eq!(
        nominal_type_domain_at_node(&il, receiver, widget),
        Some(DomainEvidence::Record)
    );

    il.evidence.push(evidence(
        1,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Domain(DomainEvidence::Record),
        EvidenceStatus::Ambiguous,
    ));
    let gadget = stable_symbol_hash("pkg.Gadget");
    il.evidence.push(evidence_with_dependencies(
        2,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
            type_hash: gadget,
            domain: DomainEvidence::Record,
        }),
        EvidenceStatus::Asserted,
        vec![EvidenceId(1)],
    ));
    assert_eq!(
        nominal_type_domain_at_node(&il, receiver, gadget),
        None,
        "dependency-broken nominal type-domain records must fail closed"
    );

    il.evidence.push(evidence(
        3,
        EvidenceAnchor::node(sp(12), NodeKind::Var),
        EvidenceKind::Type(TypeEvidenceKind::NominalDomain {
            type_hash: widget,
            domain: DomainEvidence::Map,
        }),
        EvidenceStatus::Asserted,
    ));
    assert_eq!(
        nominal_type_domain_at_node(&il, receiver, widget),
        None,
        "conflicting nominal type-domain records must fail closed"
    );
}
