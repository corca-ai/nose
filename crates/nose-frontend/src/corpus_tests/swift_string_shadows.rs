use super::*;

fn assert_swift_string_binding_statuses(
    dir: &Path,
    consumer: &Path,
    unqualified_name: &str,
    expected_unqualified: EvidenceStatus,
    expected_qualified: EvidenceStatus,
) {
    let corpus = lower_corpus_filtered(&[dir], &[]);
    let il = corpus
        .files
        .iter()
        .find(|il| il.meta.path == consumer.to_string_lossy())
        .expect("consumer Swift file should be lowered");
    let status = |name: &str, kind: TypeEvidenceKind| {
        il.evidence.iter().find_map(|record| {
            (matches!(
                record.anchor,
                EvidenceAnchor::Binding { local_hash, .. }
                    if local_hash == stable_symbol_hash(name)
            ) && record.kind == EvidenceKind::Type(kind))
            .then_some(record.status)
        })
    };
    assert_eq!(
        status(
            unqualified_name,
            TypeEvidenceKind::SwiftUnqualifiedStringBinding,
        ),
        Some(expected_unqualified)
    );
    assert_eq!(
        status("qualified", TypeEvidenceKind::SwiftQualifiedStringBinding),
        Some(expected_qualified)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn lower_corpus_closes_cross_file_string_typealias_binding_proof() {
    let dir = temp_dir("swift_cross_file_string_typealias");
    fs::write(dir.join("Alias.swift"), "typealias String = Character\n").unwrap();
    let consumer = dir.join("Consumer.swift");
    fs::write(
        &consumer,
        "let alias: String = \"x\"\nlet qualified: Swift.String = \"x\"\n",
    )
    .unwrap();

    assert_swift_string_binding_statuses(
        &dir,
        &consumer,
        "alias",
        EvidenceStatus::Ambiguous,
        EvidenceStatus::Asserted,
    );
}

#[test]
fn lower_corpus_closes_selectively_imported_swift_string_names() {
    let dir = temp_dir("swift_selective_import_string_shadows");
    let consumer = dir.join("Consumer.swift");
    fs::write(
        &consumer,
        "import /* selective shadow */ struct ShadowString.String\n\
         import /* selective shadow */ struct ShadowSwift.Swift\n\
         let imported: String = \"x\"\n\
         let qualified: Swift.String = \"x\"\n",
    )
    .unwrap();

    assert_swift_string_binding_statuses(
        &dir,
        &consumer,
        "imported",
        EvidenceStatus::Ambiguous,
        EvidenceStatus::Ambiguous,
    );
}

#[test]
fn lower_corpus_closes_escaped_selective_swift_string_names() {
    for (label, import, unqualified, qualified) in [
        (
            "swift_escaped_selective_string_shadow",
            "import struct ShadowString.`String`",
            EvidenceStatus::Ambiguous,
            EvidenceStatus::Asserted,
        ),
        (
            "swift_escaped_selective_swift_shadow",
            "import struct ShadowSwift.`Swift`",
            EvidenceStatus::Asserted,
            EvidenceStatus::Ambiguous,
        ),
    ] {
        let dir = temp_dir(label);
        let consumer = dir.join("Consumer.swift");
        fs::write(
            &consumer,
            format!(
                "{import}\nlet imported: String = \"x\"\nlet qualified: Swift.String = \"x\"\n"
            ),
        )
        .unwrap();

        assert_swift_string_binding_statuses(&dir, &consumer, "imported", unqualified, qualified);
    }
}

#[test]
fn lower_corpus_closes_string_names_exposed_by_ordinary_imports() {
    let dir = temp_dir("swift_ordinary_import_string_shadows");
    let consumer = dir.join("Consumer.swift");
    fs::write(
        &consumer,
        "import ShadowTypes\n\
         let imported: String = \"x\"\n\
         let qualified: Swift.String = \"x\"\n",
    )
    .unwrap();

    assert_swift_string_binding_statuses(
        &dir,
        &consumer,
        "imported",
        EvidenceStatus::Ambiguous,
        EvidenceStatus::Ambiguous,
    );
}
