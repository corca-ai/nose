use super::*;

fn imported_binding_symbol_count(il: &Il, local: &str, module: &str, exported: &str) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            record.status == nose_il::EvidenceStatus::Asserted
                && matches!(
                    record.anchor,
                    nose_il::EvidenceAnchor::Binding { local_hash, .. }
                        if local_hash == nose_il::stable_symbol_hash(local)
                )
                && matches!(
                    record.kind,
                    nose_il::EvidenceKind::Symbol(
                        nose_il::SymbolEvidenceKind::ImportedBinding {
                            module_hash,
                            exported_hash,
                        },
                    ) if module_hash == nose_il::stable_symbol_hash(module)
                        && exported_hash == nose_il::stable_symbol_hash(exported)
                )
        })
        .count()
}

fn imported_namespace_symbol_count(il: &Il, local: &str, module: &str) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            record.status == nose_il::EvidenceStatus::Asserted
                && matches!(
                    record.anchor,
                    nose_il::EvidenceAnchor::Binding { local_hash, .. }
                        if local_hash == nose_il::stable_symbol_hash(local)
                )
                && matches!(
                    record.kind,
                    nose_il::EvidenceKind::Symbol(
                        nose_il::SymbolEvidenceKind::ImportedNamespace { module_hash },
                    ) if module_hash == nose_il::stable_symbol_hash(module)
                )
        })
        .count()
}

fn reexport_binding_count(il: &Il, local: &str, module: &str, exported: &str) -> usize {
    il.evidence
        .iter()
        .filter(|record| {
            record.status == nose_il::EvidenceStatus::Asserted
                && matches!(
                    record.anchor,
                    nose_il::EvidenceAnchor::Binding { local_hash, .. }
                        if local_hash == nose_il::stable_symbol_hash(local)
                )
                && matches!(
                    record.kind,
                    nose_il::EvidenceKind::Import(
                        nose_il::ImportEvidenceKind::ReExportBinding {
                            target_module_hash,
                            target_exported_hash,
                        },
                    ) if target_module_hash == nose_il::stable_symbol_hash(module)
                        && target_exported_hash == nose_il::stable_symbol_hash(exported)
                )
        })
        .count()
}

#[test]
fn brace_use_emits_imported_binding_symbol_for_each_static_item() {
    let (_, il) = lower_rust(
        "use crate::detect_command::{cmd_detect, DetectArgs};\nuse std::{fs, path::Path as StdPath};\nfn f() {}",
    );

    assert_eq!(
        imported_binding_symbol_count(&il, "cmd_detect", "crate::detect_command", "cmd_detect"),
        1
    );
    assert_eq!(
        imported_binding_symbol_count(&il, "DetectArgs", "crate::detect_command", "DetectArgs"),
        1
    );
    assert_eq!(imported_binding_symbol_count(&il, "fs", "std", "fs"), 1);
    assert_eq!(
        imported_binding_symbol_count(&il, "StdPath", "std::path", "Path"),
        1
    );
}

#[test]
fn nested_brace_use_emits_imported_symbol_evidence_for_static_items() {
    let (_, il) = lower_rust(
        "use std::{io::{self, Read}, path::Path as StdPath};\nuse tokio::{runtime::{Builder, Runtime}};\nfn f() {}",
    );

    assert_eq!(imported_namespace_symbol_count(&il, "io", "std::io"), 1);
    assert_eq!(
        imported_binding_symbol_count(&il, "Read", "std::io", "Read"),
        1
    );
    assert_eq!(
        imported_binding_symbol_count(&il, "StdPath", "std::path", "Path"),
        1
    );
    assert_eq!(
        imported_binding_symbol_count(&il, "Builder", "tokio::runtime", "Builder"),
        1
    );
    assert_eq!(
        imported_binding_symbol_count(&il, "Runtime", "tokio::runtime", "Runtime"),
        1
    );
}

#[test]
fn public_use_emits_reexport_binding_evidence_for_direct_static_items() {
    let (_, il) = lower_rust(
        "pub use crate::constants::LIMIT;\npub(crate) use crate::constants::VALUES as PUBLIC_VALUES;\npub use crate::constants::{NAME, OTHER as RENAMED};\nfn f() {}",
    );

    assert_eq!(
        reexport_binding_count(&il, "LIMIT", "crate::constants", "LIMIT"),
        1
    );
    assert_eq!(
        reexport_binding_count(&il, "PUBLIC_VALUES", "crate::constants", "VALUES"),
        1
    );
    assert_eq!(
        reexport_binding_count(&il, "NAME", "crate::constants", "NAME"),
        1
    );
    assert_eq!(
        reexport_binding_count(&il, "RENAMED", "crate::constants", "OTHER"),
        1
    );
}

#[test]
fn public_use_emits_reexport_binding_evidence_for_nested_static_items() {
    let (_, nested) = lower_rust("pub use crate::{constants::{LIMIT}};\nfn f() {}");
    assert_eq!(
        reexport_binding_count(&nested, "LIMIT", "crate::constants", "LIMIT"),
        1
    );
}

#[test]
fn private_and_wildcard_use_do_not_emit_reexport_binding_evidence() {
    let (_, private_use) = lower_rust("use crate::constants::LIMIT;\nfn f() {}");
    assert_eq!(
        reexport_binding_count(&private_use, "LIMIT", "crate::constants", "LIMIT"),
        0
    );

    let (_, wildcard) = lower_rust("pub use crate::constants::*;\nfn f() {}");
    assert_eq!(
        reexport_binding_count(&wildcard, "LIMIT", "crate::constants", "LIMIT"),
        0
    );

    let (_, private_direct) = lower_rust("pub(self) use crate::constants::LIMIT;\nfn f() {}");
    assert_eq!(
        reexport_binding_count(&private_direct, "LIMIT", "crate::constants", "LIMIT"),
        0
    );

    let (_, private_nested) =
        lower_rust("pub(in crate::private) use crate::{constants::{LIMIT}};\nfn f() {}");
    assert_eq!(
        reexport_binding_count(&private_nested, "LIMIT", "crate::constants", "LIMIT"),
        0
    );
}

#[test]
fn wildcard_brace_use_stays_without_static_import_symbol_evidence() {
    let (_, wildcard) = lower_rust("use crate::items::*;\nfn f() {}");
    assert_eq!(
        imported_binding_symbol_count(&wildcard, "items", "crate::items", "items"),
        0
    );

    let (_, nested_wildcard) = lower_rust("use std::{io::*};\nfn f() {}");
    assert_eq!(
        imported_binding_symbol_count(&nested_wildcard, "Read", "std::io", "Read"),
        0
    );
}
