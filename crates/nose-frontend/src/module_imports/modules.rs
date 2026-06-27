use nose_il::{stable_symbol_hash, Il, Interner, Lang, Symbol};
use nose_semantics::semantics;
use rustc_hash::FxHashSet;
use std::path::Path;

pub(super) fn file_module_hashes(il: &Il) -> Vec<u64> {
    let Some(spec) = semantics(il.meta.lang).modules().path_spec() else {
        return Vec::new();
    };
    let mut hashes = module_hashes_from_path(
        &il.meta.path,
        spec.extensions,
        spec.separator,
        spec.include_relative_dot,
        spec.drop_init_file,
    );
    if spec.rust_crate_self_aliases {
        if let Some(identity) = rust_module_identity(&il.meta.path) {
            hashes.extend(rust_absolute_module_hashes(&identity));
        }
        for module in module_names_from_path(
            &il.meta.path,
            spec.extensions,
            spec.separator,
            spec.drop_init_file,
        ) {
            hashes.push(stable_symbol_hash(&format!("crate::{module}")));
            hashes.push(stable_symbol_hash(&format!("self::{module}")));
        }
    }
    if il.meta.lang == Lang::Go {
        hashes.extend(go_directory_module_hashes(&il.meta.path));
    }
    dedupe_hashes(hashes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RustModuleIdentity {
    pub(super) crate_key: String,
    pub(super) parts: Vec<String>,
}

pub(super) fn rust_module_identity(path: &str) -> Option<RustModuleIdentity> {
    let mut parts = path_parts_without_extension(path, &["rs"])?;
    if parts.last().is_some_and(|part| part == "mod") {
        parts.pop();
    }
    let (crate_key, module_start) = rust_crate_key_and_module_start(&parts);
    let mut module_parts = parts[module_start..].to_vec();
    if module_parts
        .last()
        .is_some_and(|part| part == "lib" || part == "main")
    {
        module_parts.pop();
    }
    Some(RustModuleIdentity {
        crate_key,
        parts: module_parts,
    })
}

pub(super) fn rust_importable_module_hashes(
    provider: &RustModuleIdentity,
    importer: &RustModuleIdentity,
) -> Vec<u64> {
    if provider.crate_key != importer.crate_key {
        return Vec::new();
    }
    let mut hashes = rust_absolute_module_hashes(provider);
    if let Some(remaining) = provider.parts.strip_prefix(importer.parts.as_slice()) {
        if !remaining.is_empty() {
            hashes.push(stable_symbol_hash(&format!(
                "self::{}",
                remaining.join("::")
            )));
        }
    }
    for levels_up in 1..=importer.parts.len() {
        let base_len = importer.parts.len() - levels_up;
        let base = &importer.parts[..base_len];
        let Some(remaining) = provider.parts.strip_prefix(base) else {
            continue;
        };
        if remaining.is_empty() {
            continue;
        }
        let mut alias_parts = vec!["super"; levels_up];
        let remaining = remaining.iter().map(String::as_str);
        alias_parts.extend(remaining);
        hashes.push(stable_symbol_hash(&alias_parts.join("::")));
    }
    dedupe_hashes(hashes)
}

pub(super) fn rust_absolute_module_hashes(identity: &RustModuleIdentity) -> Vec<u64> {
    if identity.parts.is_empty() {
        return vec![stable_symbol_hash("crate"), stable_symbol_hash("self")];
    }
    let module = identity.parts.join("::");
    vec![
        stable_symbol_hash(&module),
        stable_symbol_hash(&format!("crate::{module}")),
    ]
}

fn rust_crate_key_and_module_start(parts: &[String]) -> (String, usize) {
    if let Some(src_idx) = parts.iter().rposition(|part| part == "src") {
        return (parts[..src_idx].join("/"), src_idx + 1);
    }
    if let Some(tests_idx) = parts.iter().rposition(|part| part == "tests") {
        return (parts[..=tests_idx].join("/"), tests_idx + 1);
    }
    if parts.len() > 1 {
        return (parts[..parts.len() - 1].join("/"), parts.len() - 1);
    }
    (String::new(), 0)
}

pub(super) fn java_class_module_hashes(
    il: &Il,
    interner: &Interner,
    class_name: Symbol,
) -> Vec<u64> {
    let class_name = interner.resolve(class_name);
    let mut hashes = vec![stable_symbol_hash(class_name)];
    if let Some(mut parts) = path_parts_without_extension(&il.meta.path, &["java"]) {
        if let Some(last) = parts.last_mut() {
            *last = class_name.to_string();
        }
        for module in suffix_module_names(&parts, ".") {
            hashes.push(stable_symbol_hash(&module));
        }
    }
    dedupe_hashes(hashes)
}

fn module_hashes_from_path(
    path: &str,
    extensions: &[&str],
    separator: &str,
    include_relative_dot: bool,
    drop_python_init: bool,
) -> Vec<u64> {
    let hashes = module_names_from_path(path, extensions, separator, drop_python_init)
        .into_iter()
        .flat_map(|module| {
            if include_relative_dot {
                vec![
                    stable_symbol_hash(&module),
                    stable_symbol_hash(&format!("./{module}")),
                ]
            } else {
                vec![stable_symbol_hash(&module)]
            }
        })
        .collect::<Vec<_>>();
    dedupe_hashes(hashes)
}

fn module_names_from_path(
    path: &str,
    extensions: &[&str],
    separator: &str,
    drop_python_init: bool,
) -> Vec<String> {
    let Some(mut parts) = path_parts_without_extension(path, extensions) else {
        return Vec::new();
    };
    if drop_python_init && parts.last().is_some_and(|part| part == "__init__") {
        parts.pop();
    }
    suffix_module_names(&parts, separator)
}

fn path_parts_without_extension(path: &str, extensions: &[&str]) -> Option<Vec<String>> {
    let path = Path::new(path);
    let ext = path.extension().and_then(|ext| ext.to_str())?;
    if !extensions.contains(&ext) {
        return None;
    }
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|part| !part.is_empty() && *part != "/")
        .map(ToOwned::to_owned)
        .collect();
    let last = parts.last_mut()?;
    let stem = Path::new(last)
        .file_stem()
        .and_then(|stem| stem.to_str())?
        .to_string();
    *last = stem;
    Some(parts)
}

fn suffix_module_names(parts: &[String], separator: &str) -> Vec<String> {
    let mut out = Vec::new();
    for start in 0..parts.len() {
        let module = parts[start..].join(separator);
        if !module.is_empty() {
            out.push(module);
        }
    }
    out
}

fn go_directory_module_hashes(path: &str) -> Vec<u64> {
    let Some(mut parts) = path_parts_without_extension(path, &["go"]) else {
        return Vec::new();
    };
    parts.pop();
    suffix_module_names(&parts, "/")
        .into_iter()
        .map(|module| stable_symbol_hash(&module))
        .collect()
}

fn dedupe_hashes(hashes: Vec<u64>) -> Vec<u64> {
    let mut seen = FxHashSet::default();
    hashes
        .into_iter()
        .filter(|hash| seen.insert(*hash))
        .collect()
}
