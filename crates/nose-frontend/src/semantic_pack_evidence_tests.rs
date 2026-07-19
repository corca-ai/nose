use nose_il::{Corpus, FileId, Interner, Lang};
use nose_semantics::{SemanticPackEvidenceIndex, SemanticPackSet};
use std::path::PathBuf;

const PACK_ID: &str = "com.example.java-guava-typed-factories";
const LIST_ROW: &str = "java.guava.immutable-list.of";
const STATIC_LIST_ROW: &str = "java.guava.immutable-list.static-of";
const MAP_ROW: &str = "java.guava.immutable-map.of";

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn evidence_for(source: &str, packs: &SemanticPackSet) -> SemanticPackEvidenceIndex {
    let interner = Interner::new();
    let il = crate::lower_source(
        FileId(0),
        "Fixture.java",
        source.as_bytes(),
        Lang::Java,
        &interner,
    )
    .expect("Java fixture should lower");
    SemanticPackEvidenceIndex::build(packs, &Corpus::new(interner, vec![il]))
}

fn corpus_for(source: &str) -> Corpus {
    let interner = Interner::new();
    let il = crate::lower_source(
        FileId(0),
        "Fixture.java",
        source.as_bytes(),
        Lang::Java,
        &interner,
    )
    .expect("Java fixture should lower");
    Corpus::new(interner, vec![il])
}

#[test]
fn locked_dependency_and_builtin_import_facts_produce_one_occurrence() {
    let packs =
        SemanticPackSet::new_locked(&workspace_path("docs/examples/semantic-pack-lock-v1.json"))
            .expect("checked-in lock should validate");
    let index = evidence_for(
        "import com.google.common.collect.ImmutableList;\n\
         class Example { Object f() { return ImmutableList.of(\"a\", \"b\"); } }",
        &packs,
    );

    assert_eq!(index.dependencies().len(), 1);
    assert_eq!(index.dependencies()[0].declared_version, "33.0.0-jre");
    assert_eq!(index.dependencies()[0].matched_version, "33.0.0");
    let row = index.row(PACK_ID, LIST_ROW).expect("selected list row");
    assert_eq!(row.blocker, None);
    assert_eq!(row.occurrence_count(), 1);
    let occurrence = &index.occurrences_for_row(PACK_ID, LIST_ROW)[0];
    assert_eq!(occurrence.arity, 2);
    assert!(occurrence.receiver_evidence.is_some());
    assert!(occurrence.receiver_span.is_some());
    assert!(!occurrence.effect_evidence.is_empty());
    assert_eq!(index.row(PACK_ID, MAP_ROW).unwrap().occurrence_count(), 0);
}

#[test]
fn explicit_static_import_produces_a_receiver_free_occurrence() {
    let packs =
        SemanticPackSet::new_locked(&workspace_path("docs/examples/semantic-pack-lock-v1.json"))
            .expect("checked-in lock should validate");
    let index = evidence_for(
        "import static com.google.common.collect.ImmutableList.of;\n\
         class Example { Object f() { return of(\"a\"); } }",
        &packs,
    );

    let occurrences = index.occurrences_for_row(PACK_ID, STATIC_LIST_ROW);
    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].arity, 1);
    assert_eq!(occurrences[0].receiver_evidence, None);
    assert_eq!(occurrences[0].receiver_span, None);
    assert_eq!(index.row(PACK_ID, LIST_ROW).unwrap().occurrence_count(), 0);
}

#[test]
fn wrong_package_local_shadow_and_unlocked_metadata_stay_closed() {
    let locked =
        SemanticPackSet::new_locked(&workspace_path("docs/examples/semantic-pack-lock-v1.json"))
            .expect("checked-in lock should validate");
    let wrong_package = evidence_for(
        "import example.collect.ImmutableList;\n\
         class Example { Object f() { return ImmutableList.of(\"a\"); } }",
        &locked,
    );
    assert_eq!(
        wrong_package
            .row(PACK_ID, LIST_ROW)
            .unwrap()
            .occurrence_count(),
        0
    );

    let shadowed = evidence_for(
        "import com.google.common.collect.ImmutableList;\n\
         class ImmutableList { static Object of(Object x) { return x; } }\n\
         class Example { Object f() { return ImmutableList.of(\"a\"); } }",
        &locked,
    );
    assert_eq!(
        shadowed.row(PACK_ID, LIST_ROW).unwrap().occurrence_count(),
        0
    );

    let wildcard = evidence_for(
        "import com.google.common.collect.*;\n\
         class Example { Object f() { return ImmutableList.of(\"a\"); } }",
        &locked,
    );
    assert_eq!(
        wildcard.row(PACK_ID, LIST_ROW).unwrap().occurrence_count(),
        0
    );

    let unsupported_arity = evidence_for(
        "import com.google.common.collect.ImmutableList;\n\
         class Example { Object f() { return ImmutableList.of(\
           0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12); } }",
        &locked,
    );
    assert_eq!(
        unsupported_arity
            .row(PACK_ID, LIST_ROW)
            .unwrap()
            .occurrence_count(),
        0
    );

    let unlocked = SemanticPackSet::new_local(&[workspace_path(
        "docs/examples/semantic-packs/v1/guava-immutable-collections.json",
    )])
    .expect("typed manifest should load as metadata");
    let metadata_only = evidence_for(
        "import com.google.common.collect.ImmutableList;\n\
         class Example { Object f() { return ImmutableList.of(\"a\"); } }",
        &unlocked,
    );
    assert!(metadata_only.rows().is_empty());
    assert!(metadata_only.occurrences().is_empty());
}

#[test]
fn locked_near_registry_joins_external_and_builtin_protocol_evidence() {
    let packs =
        SemanticPackSet::new_locked(&workspace_path("docs/examples/semantic-pack-lock-v1.json"))
            .expect("checked-in lock should validate");
    let corpus = corpus_for(
        "import com.google.common.collect.ImmutableList;\n\
         class Example { Object f() { return ImmutableList.of(\"a\", \"b\"); } }",
    );
    let evidence = SemanticPackEvidenceIndex::build(&packs, &corpus);
    let row = evidence.row(PACK_ID, LIST_ROW).expect("selected list row");
    assert_eq!(row.row_digest.len(), 71);
    assert!(row.row_digest.starts_with("sha256:"));

    let registry = nose_semantics::SemanticPackNearRegistry::build(&packs, &evidence, &corpus);
    assert!(registry.is_active());
    let protocols = registry.protocols_for_unit("Fixture.java", 1, 2);
    assert!(protocols
        .iter()
        .any(|protocol| protocol.provenance.is_some()));
    assert!(protocols
        .iter()
        .any(|protocol| protocol.provenance.is_none()));
    let report = registry.report_with_influential(
        protocols
            .iter()
            .filter_map(|protocol| protocol.provenance.as_ref()),
    );
    let counts = report.pack(PACK_ID).expect("near pack counts");
    assert_eq!(counts.selected_rows, 3);
    assert_eq!(counts.admitted_rows, 3);
    assert_eq!(counts.rejected_rows, 0);
    assert_eq!(counts.admitted_occurrences, 1);
    assert_eq!(counts.influential_occurrences, 1);
}
