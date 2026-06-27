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
