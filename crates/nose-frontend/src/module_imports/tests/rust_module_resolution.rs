use super::super::bindings::{assignment_name, assignment_rhs, collect_top_level_statements};
use super::super::{imported_immutable_snapshot_census, resolve_imported_immutable_bindings};
use super::support::snapshot_count;
use nose_il::{Corpus, FileId, Interner, Lang, NodeKind};

#[test]
fn rust_crate_import_resolves_mod_rs_literal_export() {
    assert_rust_literal_import_snapshot(
        "crates/demo/src/constants/mod.rs",
        "pub(crate) const LIMIT: i64 = 7;\n",
        "crates/demo/src/consumer.rs",
        "use crate::constants::LIMIT;\nfn read() -> i64 { LIMIT }\n",
        "LIMIT",
        NodeKind::Lit,
    );
}

#[test]
fn rust_super_import_resolves_parent_relative_literal_export() {
    assert_rust_literal_import_snapshot(
        "crates/demo/src/parent/constants.rs",
        "pub(crate) const LIMIT: i64 = 7;\n",
        "crates/demo/src/parent/child.rs",
        "use super::constants::LIMIT;\nfn read() -> i64 { LIMIT }\n",
        "LIMIT",
        NodeKind::Lit,
    );
}

#[test]
fn rust_super_import_resolves_parent_module_literal_export() {
    assert_rust_literal_import_snapshot(
        "crates/demo/src/units.rs",
        "pub(crate) const LIMIT: i64 = 7;\n",
        "crates/demo/src/units/features.rs",
        "use super::LIMIT;\nfn read() -> i64 { LIMIT }\n",
        "LIMIT",
        NodeKind::Lit,
    );
}

#[test]
fn rust_bare_child_import_resolves_nested_module_literal_export() {
    assert_rust_literal_import_snapshot(
        "crates/demo/src/units/fragments/context.rs",
        "pub(crate) const LIMIT: i64 = 7;\n",
        "crates/demo/src/units/fragments.rs",
        "use context::LIMIT;\nfn read() -> i64 { LIMIT }\n",
        "LIMIT",
        NodeKind::Lit,
    );
}

#[test]
fn rust_bare_sibling_import_resolves_literal_export_without_src_root() {
    assert_rust_literal_import_snapshot(
        "fixtures/demo/rust_values.rs",
        "pub const VALUES: [&str; 2] = [\"red\", \"blue\"];\n",
        "fixtures/demo/rust_imported_membership.rs",
        "use rust_values::VALUES;\n\npub fn membership(value: &str) -> bool {\n    VALUES.contains(&value)\n}\n",
        "VALUES",
        NodeKind::Seq,
    );
}

#[test]
fn rust_pub_use_reexport_resolves_literal_export_one_hop() {
    let interner = Interner::new();
    let values = lower_rust_file(
        FileId(0),
        "crates/demo/src/constants.rs",
        "pub(crate) const LIMIT: i64 = 7;\n",
        &interner,
    );
    let barrel = lower_rust_file(
        FileId(1),
        "crates/demo/src/lib.rs",
        "mod constants;\npub(crate) use constants::LIMIT as MAX;\n",
        &interner,
    );
    let importer = lower_rust_file(
        FileId(2),
        "crates/demo/src/consumer.rs",
        "use crate::MAX;\nfn read() -> i64 { MAX }\n",
        &interner,
    );

    let mut files = vec![values, barrel, importer];
    resolve_imported_immutable_bindings(&mut files, &interner);

    assert_eq!(snapshot_count(&files[2]), 1);
    let rhs = imported_binding_rhs(&files[2], &interner, "MAX");
    assert_eq!(files[2].kind(rhs), NodeKind::Lit);
    assert_eq!(files[2].node(rhs).span.file, FileId(0));
}

#[test]
fn rust_private_use_does_not_open_reexport_snapshot() {
    let interner = Interner::new();
    let values = lower_rust_file(
        FileId(0),
        "crates/demo/src/constants.rs",
        "pub(crate) const LIMIT: i64 = 7;\n",
        &interner,
    );
    let private_barrel = lower_rust_file(
        FileId(1),
        "crates/demo/src/lib.rs",
        "mod constants;\nuse constants::LIMIT;\n",
        &interner,
    );
    let importer = lower_rust_file(
        FileId(2),
        "crates/demo/src/consumer.rs",
        "use crate::LIMIT;\nfn read() -> i64 { LIMIT }\n",
        &interner,
    );

    let mut files = vec![values, private_barrel, importer];
    resolve_imported_immutable_bindings(&mut files, &interner);

    assert_eq!(snapshot_count(&files[2]), 0);
}

#[test]
fn rust_ambiguous_reexport_stays_closed_and_reported() {
    let interner = Interner::new();
    let files = vec![
        lower_rust_file(
            FileId(0),
            "crates/demo/src/a.rs",
            "pub(crate) const VALUE: i64 = 1;\n",
            &interner,
        ),
        lower_rust_file(
            FileId(1),
            "crates/demo/src/b.rs",
            "pub(crate) const VALUE: i64 = 2;\n",
            &interner,
        ),
        lower_rust_file(
            FileId(2),
            "crates/demo/src/lib.rs",
            "mod a;\nmod b;\npub use a::VALUE;\npub use b::VALUE;\n",
            &interner,
        ),
        lower_rust_file(
            FileId(3),
            "crates/demo/src/consumer.rs",
            "use crate::VALUE;\nfn read() -> i64 { VALUE }\n",
            &interner,
        ),
    ];
    let corpus = Corpus::new(interner, files);
    let census = imported_immutable_snapshot_census(&corpus);

    assert_reason_count(&census, "provider-reexport-ambiguous", 1);
}

#[test]
fn rust_reexport_to_callable_stays_non_value_boundary() {
    let interner = Interner::new();
    let files = vec![
        lower_rust_file(
            FileId(0),
            "crates/demo/src/functions.rs",
            "pub(crate) fn run() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(1),
            "crates/demo/src/lib.rs",
            "mod functions;\npub use functions::run;\n",
            &interner,
        ),
        lower_rust_file(
            FileId(2),
            "crates/demo/src/consumer.rs",
            "use crate::run;\nfn f() {}\n",
            &interner,
        ),
    ];
    let corpus = Corpus::new(interner, files);
    let census = imported_immutable_snapshot_census(&corpus);

    assert_reason_count(&census, "provider-reexport-callable-boundary", 1);
}

#[test]
fn rust_census_resolves_parent_module_relative_non_value_boundaries() {
    let interner = Interner::new();
    let files = vec![
        lower_rust_file(
            FileId(0),
            "crates/demo/src/units.rs",
            "pub(crate) struct UnitExtractCtx;\npub(crate) fn lower() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(1),
            "crates/demo/src/units/features.rs",
            "use super::UnitExtractCtx;\nuse super::lower;\nfn f() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(2),
            "crates/demo/src/report.rs",
            "pub(crate) struct RefactorFamily;\n",
            &interner,
        ),
        lower_rust_file(
            FileId(3),
            "crates/demo/src/report/tests/support.rs",
            "use super::super::RefactorFamily;\nfn f() {}\n",
            &interner,
        ),
    ];
    let corpus = Corpus::new(interner, files);
    let census = imported_immutable_snapshot_census(&corpus);

    assert_reason_count(&census, "provider-callable-export-boundary", 1);
    assert_reason_count(&census, "provider-module-missing", 0);
    assert_reason_count(&census, "provider-type-export-boundary", 2);
}

#[test]
fn rust_census_splits_non_value_and_unsupported_provider_boundaries() {
    let interner = Interner::new();
    let files = vec![
        lower_rust_file(
            FileId(0),
            "crates/demo/src/lib.rs",
            "mod helpers;\nmod model;\n",
            &interner,
        ),
        lower_rust_file(
            FileId(1),
            "crates/demo/src/helpers.rs",
            "pub(crate) fn run() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(2),
            "crates/demo/src/model.rs",
            "pub(crate) struct Thing { value: i64 }\n",
            &interner,
        ),
        lower_rust_file(
            FileId(3),
            "crates/demo/src/consumer.rs",
            "\
use crate::helpers;\n\
use crate::helpers::run;\n\
use crate::model::Thing;\n\
use anyhow::Result;\n\
use std::path::Path;\n\
use std::cell::RefCell;\n\
fn f() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(4),
            "crates/other-crate/src/lib.rs",
            "pub fn other() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(5),
            "crates/demo/src/workspace_consumer.rs",
            "use other_crate::other;\nfn f() {}\n",
            &interner,
        ),
        lower_rust_file(
            FileId(6),
            "crates/demo/src/workspace_type_consumer.rs",
            "use nose_il::UnitKind::Function;\nfn f() {}\n",
            &interner,
        ),
    ];
    let corpus = Corpus::new(interner, files);
    let census = imported_immutable_snapshot_census(&corpus);

    assert_reason_count(&census, "provider-callable-export-boundary", 1);
    assert_reason_count(&census, "provider-external-crate-boundary", 1);
    assert_reason_count(&census, "provider-module-namespace-boundary", 1);
    assert_reason_count(&census, "provider-rust-stdlib-boundary", 2);
    assert_reason_count(&census, "provider-type-export-boundary", 1);
    assert_reason_count(&census, "provider-workspace-crate-boundary", 2);
}

fn lower_rust_file(file: FileId, path: &str, src: &str, interner: &Interner) -> nose_il::Il {
    crate::lower_source(file, path, src.as_bytes(), Lang::Rust, interner)
        .expect("lower Rust test file")
}

fn assert_rust_literal_import_snapshot(
    provider_path: &str,
    provider_src: &str,
    importer_path: &str,
    importer_src: &str,
    local: &str,
    expected_kind: NodeKind,
) {
    let interner = Interner::new();
    let provider = lower_rust_file(FileId(0), provider_path, provider_src, &interner);
    let importer = lower_rust_file(FileId(1), importer_path, importer_src, &interner);

    let mut files = vec![provider, importer];
    resolve_imported_immutable_bindings(&mut files, &interner);

    assert_eq!(snapshot_count(&files[1]), 1);
    let rhs = imported_binding_rhs(&files[1], &interner, local);
    assert_eq!(files[1].kind(rhs), expected_kind);
    assert_eq!(files[1].node(rhs).span.file, FileId(0));
}

fn imported_binding_rhs(il: &nose_il::Il, interner: &Interner, name: &str) -> nose_il::NodeId {
    collect_top_level_statements(il)
        .into_iter()
        .find_map(|stmt| {
            assignment_name(il, stmt)
                .is_some_and(|symbol| interner.resolve(symbol) == name)
                .then(|| assignment_rhs(il, stmt))
                .flatten()
        })
        .expect("import binding RHS")
}

fn assert_reason_count(census: &super::super::ImportSnapshotCensus, reason: &str, expected: usize) {
    let actual = census
        .misses_by_reason
        .iter()
        .find(|row| row.key == reason)
        .map(|row| row.count)
        .unwrap_or_default();
    assert_eq!(actual, expected, "{reason}");
}
