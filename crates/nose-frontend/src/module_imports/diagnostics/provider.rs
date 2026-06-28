use super::super::bindings::{
    assignment_name, assignment_rhs, collect_statements_for_root, BindingUseIndex,
};
use super::super::exports::LiteralExports;
use super::super::modules::java_class_module_hashes;
use super::super::FileImportContext;
use nose_il::{stable_symbol_hash, Il, Interner, Lang, NodeId, NodeKind, Payload, Unit, UnitKind};
use nose_semantics::{
    imported_literal_export_rejection_reason, imported_literal_export_safe, semantics,
};

pub(super) struct ProviderMiss {
    pub(super) reason: &'static str,
    pub(super) provider_file: Option<String>,
    pub(super) provider_line: Option<u32>,
}

pub(super) fn provider_miss(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: &LiteralExports,
    importer_file_idx: usize,
    module_hash: u64,
    exported_hash: u64,
) -> ProviderMiss {
    provider_miss_inner(
        files,
        interner,
        contexts,
        Some(exports),
        importer_file_idx,
        module_hash,
        exported_hash,
    )
}

fn provider_miss_inner(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: Option<&LiteralExports>,
    importer_file_idx: usize,
    module_hash: u64,
    exported_hash: u64,
) -> ProviderMiss {
    let importer_lang = files[importer_file_idx].meta.lang;
    let mut scan = ProviderScan::default();

    for (file_idx, il) in files.iter().enumerate() {
        inspect_provider_file(
            ProviderFile {
                interner,
                contexts,
                il,
                file_idx,
                importer_file_idx,
                importer_lang,
                module_hash,
                exported_hash,
            },
            &mut scan,
        );
    }

    if scan.safe_candidates.len() > 1 {
        return scan
            .safe_candidates
            .remove(0)
            .with_reason("provider-export-ambiguous");
    }
    if scan.safe_candidates.len() == 1 {
        return scan
            .safe_candidates
            .remove(0)
            .with_reason("eligible-but-not-snapshotted");
    }
    if !scan.module_seen {
        if importer_lang == Lang::Rust && rust_stdlib_module_hash(module_hash) {
            return ProviderMiss::without_provider("provider-rust-stdlib-boundary");
        }
        if importer_lang == Lang::Rust
            && rust_workspace_crate_module_hash(contexts, importer_file_idx, module_hash)
        {
            return ProviderMiss::without_provider("provider-workspace-crate-boundary");
        }
        return ProviderMiss::without_provider("provider-module-missing");
    }
    if !scan.same_lang_module_seen {
        return ProviderMiss::without_provider("cross-language-boundary");
    }
    if let Some(exports) = exports {
        if let Some(miss) = reexport_provider_miss(
            files,
            interner,
            contexts,
            exports,
            importer_file_idx,
            module_hash,
            exported_hash,
        ) {
            return miss;
        }
    }
    if !scan.export_seen {
        return ProviderMiss::without_provider("provider-export-missing");
    }
    scan.duplicate_seen
        .or(scan.self_seen)
        .or(scan.unsafe_seen)
        .or(scan.rhs_missing_seen)
        .or(scan.non_value_export_seen)
        .or(scan.export_rejection_seen)
        .unwrap_or_else(|| ProviderMiss::without_provider("provider-export-missing"))
}

fn reexport_provider_miss(
    files: &[Il],
    interner: &Interner,
    contexts: &[FileImportContext],
    exports: &LiteralExports,
    importer_file_idx: usize,
    module_hash: u64,
    exported_hash: u64,
) -> Option<ProviderMiss> {
    let candidates =
        exports.reexport_record_candidates(contexts, importer_file_idx, module_hash, exported_hash);
    let [reexport] = candidates.as_slice() else {
        return (!candidates.is_empty()).then(|| {
            let first = candidates[0];
            ProviderMiss {
                reason: "provider-reexport-ambiguous",
                provider_file: Some(files[first.file_idx].meta.path.clone()),
                provider_line: Some(first.provider_line),
            }
        });
    };
    let target = provider_miss_inner(
        files,
        interner,
        contexts,
        None,
        reexport.file_idx,
        reexport.target_module_hash,
        reexport.target_exported_hash,
    );
    let reason = reexport_reason(target.reason);
    if target.provider_file.is_some() {
        Some(target.with_reason(reason))
    } else {
        Some(ProviderMiss {
            reason,
            provider_file: Some(files[reexport.file_idx].meta.path.clone()),
            provider_line: Some(reexport.provider_line),
        })
    }
}

fn reexport_reason(reason: &'static str) -> &'static str {
    match reason {
        "eligible-but-not-snapshotted" => "eligible-but-not-snapshotted",
        "provider-callable-export-boundary" => "provider-reexport-callable-boundary",
        "provider-type-export-boundary" => "provider-reexport-type-boundary",
        "provider-module-namespace-boundary" => "provider-reexport-module-namespace-boundary",
        "provider-module-missing" => "provider-reexport-target-module-missing",
        "provider-export-missing" => "provider-reexport-target-export-missing",
        "provider-rust-stdlib-boundary" => "provider-reexport-rust-stdlib-boundary",
        "provider-workspace-crate-boundary" => "provider-reexport-workspace-crate-boundary",
        "provider-binding-unsafe" => "provider-reexport-target-binding-unsafe",
        "self-import-boundary" => "provider-reexport-self-boundary",
        "cross-language-boundary" => "provider-reexport-cross-language-boundary",
        _ => "provider-reexport-target-not-import-literal-safe",
    }
}

struct ProviderFile<'a> {
    interner: &'a Interner,
    contexts: &'a [FileImportContext],
    il: &'a Il,
    file_idx: usize,
    importer_file_idx: usize,
    importer_lang: Lang,
    module_hash: u64,
    exported_hash: u64,
}

fn inspect_provider_file(file: ProviderFile<'_>, scan: &mut ProviderScan) {
    let context = &file.contexts[file.file_idx];
    let Some(top_level) = context.top_level.as_deref() else {
        return;
    };
    let Some(binding_uses) = context.binding_uses.as_ref() else {
        return;
    };
    inspect_provider_scope(
        ProviderScope {
            il: file.il,
            interner: file.interner,
            file_idx: file.file_idx,
            importer_file_idx: file.importer_file_idx,
            importer_lang: file.importer_lang,
            module_matches: context.module_matches_import_from(
                &file.contexts[file.importer_file_idx],
                file.module_hash,
            ),
            statements: top_level,
            binding_uses,
            exported_hash: file.exported_hash,
        },
        scan,
    );
    if !semantics(file.il.meta.lang)
        .modules()
        .java_class_literal_exports()
    {
        return;
    }
    for unit in &file.il.units {
        inspect_java_class_provider_scope(&file, binding_uses, unit, scan);
    }
}

fn inspect_java_class_provider_scope(
    file: &ProviderFile<'_>,
    binding_uses: &BindingUseIndex,
    unit: &Unit,
    scan: &mut ProviderScan,
) {
    if unit.kind != UnitKind::Class {
        return;
    }
    let Some(class_name) = unit.name else {
        return;
    };
    let class_module_hashes = java_class_module_hashes(file.il, file.interner, class_name);
    if class_module_hashes.is_empty() {
        return;
    }
    let statements = collect_statements_for_root(file.il, unit.root);
    inspect_provider_scope(
        ProviderScope {
            il: file.il,
            interner: file.interner,
            file_idx: file.file_idx,
            importer_file_idx: file.importer_file_idx,
            importer_lang: file.importer_lang,
            module_matches: class_module_hashes.contains(&file.module_hash),
            statements: &statements,
            binding_uses,
            exported_hash: file.exported_hash,
        },
        scan,
    );
}

struct ProviderScope<'a> {
    il: &'a Il,
    interner: &'a Interner,
    file_idx: usize,
    importer_file_idx: usize,
    importer_lang: Lang,
    module_matches: bool,
    statements: &'a [NodeId],
    binding_uses: &'a BindingUseIndex,
    exported_hash: u64,
}

#[derive(Default)]
struct ProviderScan {
    module_seen: bool,
    same_lang_module_seen: bool,
    export_seen: bool,
    duplicate_seen: Option<ProviderMiss>,
    self_seen: Option<ProviderMiss>,
    unsafe_seen: Option<ProviderMiss>,
    rhs_missing_seen: Option<ProviderMiss>,
    non_value_export_seen: Option<ProviderMiss>,
    export_rejection_seen: Option<ProviderMiss>,
    safe_candidates: Vec<ProviderMiss>,
}

fn inspect_provider_scope(scope: ProviderScope<'_>, scan: &mut ProviderScan) {
    if !scope.module_matches {
        return;
    }
    scan.module_seen = true;
    if scope.il.meta.lang != scope.importer_lang {
        return;
    }
    scan.same_lang_module_seen = true;

    let matching: Vec<NodeId> = scope
        .statements
        .iter()
        .copied()
        .filter(|&stmt| {
            assignment_name(scope.il, stmt).is_some_and(|name| {
                stable_symbol_hash(scope.interner.resolve(name)) == scope.exported_hash
            })
        })
        .collect();
    if matching.is_empty() {
        if let Some(miss) = rust_non_literal_export_miss(&scope) {
            scan.export_seen = true;
            if scan.non_value_export_seen.is_none() {
                scan.non_value_export_seen = Some(miss);
            }
        }
        return;
    }
    scan.export_seen = true;
    if matching.len() != 1 {
        if scan.duplicate_seen.is_none() {
            scan.duplicate_seen = Some(location_miss(
                scope.il,
                matching[0],
                "provider-export-ambiguous",
            ));
        }
        return;
    }

    let stmt = matching[0];
    if scope.file_idx == scope.importer_file_idx {
        if scan.self_seen.is_none() {
            scan.self_seen = Some(location_miss(scope.il, stmt, "self-import-boundary"));
        }
        return;
    }
    let Some(name) = assignment_name(scope.il, stmt) else {
        return;
    };
    if scope
        .binding_uses
        .exported_binding_unsafe(scope.il, name, stmt)
    {
        if scan.unsafe_seen.is_none() {
            scan.unsafe_seen = Some(location_miss(scope.il, stmt, "provider-binding-unsafe"));
        }
        return;
    }
    let Some(rhs) = assignment_rhs(scope.il, stmt) else {
        if scan.rhs_missing_seen.is_none() {
            scan.rhs_missing_seen = Some(location_miss(scope.il, stmt, "provider-rhs-missing"));
        }
        return;
    };
    if !imported_literal_export_safe(scope.il, scope.interner, rhs) {
        if scan.export_rejection_seen.is_none() {
            let reason = imported_literal_export_rejection_reason(scope.il, scope.interner, rhs)
                .unwrap_or("provider-export-not-snapshot-safe");
            scan.export_rejection_seen = Some(location_miss(scope.il, stmt, reason));
        }
        return;
    }
    let safe = location_miss(scope.il, stmt, "eligible-but-not-snapshotted");
    scan.safe_candidates.push(safe);
}

fn rust_non_literal_export_miss(scope: &ProviderScope<'_>) -> Option<ProviderMiss> {
    if scope.il.meta.lang != Lang::Rust {
        return None;
    }
    for stmt in scope
        .statements
        .iter()
        .copied()
        .chain(scope.il.children(scope.il.root).iter().copied())
    {
        if let Some(miss) = rust_non_literal_export_item_miss(scope, stmt) {
            return Some(miss);
        }
    }
    None
}

fn rust_non_literal_export_item_miss(
    scope: &ProviderScope<'_>,
    stmt: NodeId,
) -> Option<ProviderMiss> {
    if rust_function_item_matches(scope.il, scope.interner, stmt, scope.exported_hash) {
        return Some(location_miss(
            scope.il,
            stmt,
            "provider-callable-export-boundary",
        ));
    }
    if rust_named_node_matches(scope.il, scope.interner, stmt, scope.exported_hash) {
        let reason = match scope.il.kind(stmt) {
            NodeKind::Module => "provider-module-namespace-boundary",
            NodeKind::Block => "provider-type-export-boundary",
            _ => return None,
        };
        return Some(location_miss(scope.il, stmt, reason));
    }
    None
}

fn rust_function_item_matches(
    il: &Il,
    interner: &Interner,
    stmt: NodeId,
    exported_hash: u64,
) -> bool {
    il.units.iter().any(|unit| {
        unit.kind == UnitKind::Function
            && unit.root == stmt
            && unit
                .name
                .is_some_and(|name| stable_symbol_hash(interner.resolve(name)) == exported_hash)
    })
}

fn rust_named_node_matches(il: &Il, interner: &Interner, stmt: NodeId, exported_hash: u64) -> bool {
    matches!(
        il.node(stmt).payload,
        Payload::Name(name) if stable_symbol_hash(interner.resolve(name)) == exported_hash
    )
}

fn rust_stdlib_module_hash(module_hash: u64) -> bool {
    const STDLIB_MODULES: &[&str] = &[
        "alloc",
        "alloc::borrow",
        "alloc::boxed",
        "alloc::collections",
        "alloc::rc",
        "alloc::string",
        "alloc::sync",
        "alloc::vec",
        "core",
        "core::cmp",
        "core::convert",
        "core::fmt",
        "core::hash",
        "core::iter",
        "core::marker",
        "core::mem",
        "core::ops",
        "core::option",
        "core::result",
        "core::slice",
        "core::str",
        "std",
        "std::borrow",
        "std::cmp",
        "std::collections",
        "std::env",
        "std::ffi",
        "std::fmt",
        "std::fs",
        "std::hash",
        "std::io",
        "std::iter",
        "std::mem",
        "std::num",
        "std::ops",
        "std::path",
        "std::process",
        "std::rc",
        "std::str",
        "std::string",
        "std::sync",
        "std::thread",
        "std::time",
        "std::vec",
    ];
    STDLIB_MODULES
        .iter()
        .any(|module| stable_symbol_hash(module) == module_hash)
}

fn rust_workspace_crate_module_hash(
    contexts: &[FileImportContext],
    importer_file_idx: usize,
    module_hash: u64,
) -> bool {
    let Some(importer_crate) = contexts[importer_file_idx]
        .rust_module
        .as_ref()
        .map(|module| module.crate_key.as_str())
    else {
        return false;
    };
    contexts
        .iter()
        .filter_map(|context| context.rust_module.as_ref())
        .filter(|module| module.crate_key != importer_crate)
        .filter_map(|module| {
            let crate_name = module.crate_key.rsplit('/').next()?.replace('-', "_");
            let module = if module.parts.is_empty() {
                crate_name
            } else {
                format!("{crate_name}::{}", module.parts.join("::"))
            };
            Some(module)
        })
        .any(|module| stable_symbol_hash(&module) == module_hash)
}

fn location_miss(il: &Il, stmt: NodeId, reason: &'static str) -> ProviderMiss {
    ProviderMiss {
        reason,
        provider_file: Some(il.meta.path.clone()),
        provider_line: Some(il.node(stmt).span.start_line),
    }
}

impl ProviderMiss {
    pub(super) fn without_provider(reason: &'static str) -> Self {
        Self {
            reason,
            provider_file: None,
            provider_line: None,
        }
    }

    fn with_reason(mut self, reason: &'static str) -> Self {
        self.reason = reason;
        self
    }
}
