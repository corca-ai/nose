use super::map_factory_fixtures::*;
use super::support::*;

#[test]
fn map_like_literal_respects_sequence_surface_evidence() {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let map = b.add(
        NodeKind::Seq,
        Payload::Name(interner.intern("dictionary")),
        sp(),
        &[],
    );
    let root = b.add(NodeKind::Module, Payload::None, sp(), &[map]);
    let mut il = b.finish(
        root,
        FileMeta {
            path: "t".to_string(),
            lang: Lang::Python,
        },
        Vec::new(),
        Vec::new(),
    );

    assert!(
        !map_like_literal(&il, &interner, map),
        "raw map tag alone must not prove a semantic map surface"
    );

    il.push_evidence(sequence_surface_evidence(
        0,
        Lang::Python,
        sp(),
        SequenceSurfaceKind::Map,
    ));
    assert!(map_like_literal(&il, &interner, map));

    il.push_evidence(sequence_surface_evidence(
        1,
        Lang::Python,
        sp(),
        SequenceSurfaceKind::Collection,
    ));
    assert!(
        !map_like_literal(&il, &interner, map),
        "conflicting sequence-surface evidence must block raw map tag fallback"
    );
}

#[test]
fn rust_std_map_factory_requires_entry_surface_and_shadow_proof() {
    let (mut il, interner, call) = rust_hash_map_from_call("tuple", false, true);
    assert!(
        !rust_std_map_factory_call(&il, &interner, call),
        "raw Rust std path proof must not prove the migrated factory"
    );
    push_rust_hash_map_library_api_evidence(&mut il);
    assert!(rust_std_map_factory_call(&il, &interner, call));

    let (mut il, interner, call) = rust_hash_map_from_call("array", false, true);
    push_rust_hash_map_library_api_evidence(&mut il);
    assert!(
        !rust_std_map_factory_call(&il, &interner, call),
        "HashMap::from exact map proof requires tuple-shaped entries"
    );

    let (mut il, interner, call) = rust_hash_map_from_call("tuple", true, true);
    push_rust_hash_map_library_api_evidence(&mut il);
    assert!(
        !rust_std_map_factory_call(&il, &interner, call),
        "a local std binding must close the Rust stdlib factory path"
    );
}

#[test]
fn rust_std_map_factory_requires_outer_entries_sequence_surface() {
    let (mut il, interner, call) = rust_hash_map_from_call("tuple", false, false);
    push_rust_hash_map_library_api_evidence(&mut il);
    assert!(
        !rust_std_map_factory_call(&il, &interner, call),
        "HashMap::from exact map proof requires evidence for the outer entry list surface"
    );
}
