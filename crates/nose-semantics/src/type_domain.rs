use nose_il::{DomainEvidence, Lang};

pub fn type_domain_from_source_text(lang: Lang, text: &str) -> Option<DomainEvidence> {
    match lang {
        Lang::TypeScript => ts_type_domain(text),
        Lang::Python => python_type_domain(text),
        Lang::Rust => rust_type_domain(text),
        Lang::Java => java_type_domain(text),
        Lang::Go => go_type_domain(text),
        Lang::C => c_type_domain(text),
        Lang::JavaScript | Lang::Ruby | Lang::Vue | Lang::Svelte | Lang::Html => None,
    }
}

pub fn python_stdlib_type_domain(module: &str, exported: &str) -> Option<DomainEvidence> {
    let module = module.trim();
    let exported = exported.trim();
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "Dict" | "Mapping" | "MutableMapping")
    {
        return Some(DomainEvidence::Map);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "Iterable" | "AsyncIterable")
    {
        return Some(DomainEvidence::Iterable);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "Iterator" | "AsyncIterator")
    {
        return Some(DomainEvidence::Iterator);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "FrozenSet" | "MutableSet" | "Set")
    {
        return Some(DomainEvidence::Set);
    }
    if matches!(module, "typing") && matches!(exported, "Optional") {
        return Some(DomainEvidence::Option);
    }
    if matches!(module, "typing") && matches!(exported, "TypedDict") {
        return Some(DomainEvidence::Record);
    }
    if matches!(module, "typing" | "asyncio")
        && matches!(exported, "Awaitable" | "Coroutine" | "Future")
    {
        return Some(DomainEvidence::FutureLike);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(
            exported,
            "Collection"
                | "Container"
                | "Deque"
                | "List"
                | "MutableSequence"
                | "Sequence"
                | "Tuple"
        )
    {
        return Some(DomainEvidence::Collection);
    }
    None
}

fn ts_type_domain(text: &str) -> Option<DomainEvidence> {
    let ty = annotation_suffix(text);
    let ty = strip_ts_prefixes(&ty);
    if ty.ends_with("[]") || ty.starts_with("array<") || ty.starts_with("readonlyarray<") {
        return Some(DomainEvidence::Array);
    }
    if ty.starts_with("map<") || ty.starts_with("readonlymap<") {
        return Some(DomainEvidence::Map);
    }
    if ty.starts_with("set<") || ty.starts_with("readonlyset<") {
        return Some(DomainEvidence::Set);
    }
    if ty.starts_with("iterable<") || ty.starts_with("asynciterable<") {
        return Some(DomainEvidence::Iterable);
    }
    if ty.starts_with("iterator<") || ty.starts_with("asynciterator<") {
        return Some(DomainEvidence::Iterator);
    }
    if ty.starts_with("promise<") {
        return Some(DomainEvidence::PromiseLike);
    }
    if ty.starts_with("record<") {
        return Some(DomainEvidence::Record);
    }
    if ty.starts_with("result<") {
        return Some(DomainEvidence::Result);
    }
    if ty == "boolean" {
        return Some(DomainEvidence::Boolean);
    }
    if ty == "string" {
        return Some(DomainEvidence::String);
    }
    if ty == "number" {
        return Some(DomainEvidence::Number);
    }
    None
}

fn python_type_domain(text: &str) -> Option<DomainEvidence> {
    let ty = annotation_suffix(text);
    let ty = strip_python_string_annotation(&ty);
    let ty = ty.strip_prefix("typing.").unwrap_or(ty);
    let ty = ty.strip_prefix("collections.abc.").unwrap_or(ty);
    if ty.starts_with("dict[") || ty.starts_with("mapping[") || ty.starts_with("mutablemapping[") {
        return Some(DomainEvidence::Map);
    }
    if ty.starts_with("typeddict[") {
        return Some(DomainEvidence::Record);
    }
    if ty.starts_with("set[") || ty.starts_with("frozenset[") || ty.starts_with("mutableset[") {
        return Some(DomainEvidence::Set);
    }
    if ty.starts_with("iterable[") || ty.starts_with("asynciterable[") {
        return Some(DomainEvidence::Iterable);
    }
    if ty.starts_with("iterator[") || ty.starts_with("asynciterator[") {
        return Some(DomainEvidence::Iterator);
    }
    if ty.starts_with("optional[") {
        return Some(DomainEvidence::Option);
    }
    if ty.starts_with("awaitable[") || ty.starts_with("coroutine[") || ty.starts_with("future[") {
        return Some(DomainEvidence::FutureLike);
    }
    if ty.starts_with("result[") {
        return Some(DomainEvidence::Result);
    }
    if ty.starts_with("list[")
        || ty.starts_with("tuple[")
        || ty.starts_with("collection[")
        || ty.starts_with("container[")
        || ty.starts_with("deque[")
        || ty.starts_with("sequence[")
        || ty.starts_with("mutablesequence[")
    {
        return Some(DomainEvidence::Collection);
    }
    match ty {
        "bool" => Some(DomainEvidence::Boolean),
        "str" => Some(DomainEvidence::String),
        "int" => Some(DomainEvidence::Integer),
        "float" => Some(DomainEvidence::Float),
        _ => None,
    }
}

fn rust_type_domain(text: &str) -> Option<DomainEvidence> {
    let ty = annotation_suffix(text);
    let ty = strip_rust_ref_prefix(&ty);
    if ty.starts_with('[') || ty.starts_with("vec<") || ty.starts_with("vecdeque<") {
        return Some(DomainEvidence::Collection);
    }
    let type_name = rust_type_name(ty);
    if matches!(type_name, "vec" | "vecdeque") {
        return Some(DomainEvidence::Collection);
    }
    if matches!(type_name, "iter" | "iterator" | "intoiter" | "impliterator") {
        return Some(DomainEvidence::Iterator);
    }
    if type_name == "intoiterator" {
        return Some(DomainEvidence::Iterable);
    }
    if matches!(type_name, "hashmap" | "btreemap") {
        return Some(DomainEvidence::Map);
    }
    if matches!(type_name, "hashset" | "btreeset") {
        return Some(DomainEvidence::Set);
    }
    if type_name == "option" {
        return Some(DomainEvidence::Option);
    }
    if type_name == "result" {
        return Some(DomainEvidence::Result);
    }
    if matches!(type_name, "future" | "ready") {
        return Some(DomainEvidence::FutureLike);
    }
    if type_name == "bool" {
        return Some(DomainEvidence::Boolean);
    }
    if matches!(type_name, "str" | "string") {
        return Some(DomainEvidence::String);
    }
    if rust_integer_type(type_name) {
        return Some(DomainEvidence::Integer);
    }
    if matches!(type_name, "f32" | "f64") {
        return Some(DomainEvidence::Float);
    }
    None
}

fn java_type_domain(text: &str) -> Option<DomainEvidence> {
    if java_array_type(text) {
        return Some(DomainEvidence::Array);
    }
    let ty = java_type_identifier(text)?;
    let ty = ty.strip_prefix("java.util.").unwrap_or(&ty);
    let ty = ty.strip_prefix("java.lang.").unwrap_or(ty);
    match ty {
        "map" | "hashmap" | "linkedhashmap" | "treemap" | "concurrenthashmap" => {
            Some(DomainEvidence::Map)
        }
        "set" | "hashset" | "linkedhashset" | "treeset" => Some(DomainEvidence::Set),
        "iterable" => Some(DomainEvidence::Iterable),
        "iterator" | "listiterator" => Some(DomainEvidence::Iterator),
        "list" | "arraylist" | "linkedlist" | "collection" | "deque" | "queue" => {
            Some(DomainEvidence::Collection)
        }
        "completablefuture" | "completionstage" | "future" => Some(DomainEvidence::FutureLike),
        "optional" => Some(DomainEvidence::Option),
        "record" => Some(DomainEvidence::Record),
        "boolean" => Some(DomainEvidence::Boolean),
        "string" => Some(DomainEvidence::String),
        "byte" | "short" | "int" | "integer" | "long" => Some(DomainEvidence::Integer),
        "float" | "double" => Some(DomainEvidence::Float),
        _ => None,
    }
}

fn go_type_domain(text: &str) -> Option<DomainEvidence> {
    let compact = compact_lower(text);
    if compact.contains("map[") {
        return Some(DomainEvidence::Map);
    }
    if compact.contains("[]") {
        return Some(DomainEvidence::Collection);
    }
    if compact.contains("struct{") {
        return Some(DomainEvidence::Record);
    }
    let ty = last_identifier(text)?;
    if go_integer_type(&ty) {
        return Some(DomainEvidence::Integer);
    }
    match ty.as_str() {
        "bool" => Some(DomainEvidence::Boolean),
        "float32" | "float64" => Some(DomainEvidence::Float),
        "string" => Some(DomainEvidence::String),
        _ => None,
    }
}

fn c_type_domain(text: &str) -> Option<DomainEvidence> {
    if text.contains('*') || text.contains('[') {
        return None;
    }
    let tokens = identifier_tokens(text);
    if tokens.is_empty() {
        return None;
    }
    if c_integer_tokens(&tokens) {
        return Some(DomainEvidence::Integer);
    }
    if tokens
        .iter()
        .any(|token| matches!(*token, "float" | "double"))
    {
        return Some(DomainEvidence::Float);
    }
    if tokens
        .iter()
        .any(|token| matches!(*token, "bool" | "_Bool"))
    {
        return Some(DomainEvidence::Boolean);
    }
    None
}

fn compact_lower(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn annotation_suffix(text: &str) -> String {
    let compact = compact_lower(text);
    let suffix = compact
        .rsplit_once(':')
        .map(|(_, ty)| ty)
        .unwrap_or(compact.as_str());
    suffix.split('=').next().unwrap_or(suffix).to_string()
}

fn strip_ts_prefixes(mut ty: &str) -> &str {
    while let Some(rest) = ty.strip_prefix("readonly") {
        ty = rest;
    }
    ty
}

fn strip_python_string_annotation(ty: &str) -> &str {
    ty.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            ty.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(ty)
}

fn strip_rust_ref_prefix(mut ty: &str) -> &str {
    while let Some(rest) = ty.strip_prefix('&') {
        ty = rest;
        if let Some(rest) = ty.strip_prefix("mut") {
            ty = rest;
        }
    }
    ty
}

fn rust_type_name(ty: &str) -> &str {
    let head = ty.split(['<', '[', '(']).next().unwrap_or(ty);
    head.rsplit("::").next().unwrap_or(head)
}

fn java_array_type(text: &str) -> bool {
    let Some(surface) = java_type_surface(text) else {
        return false;
    };
    let compact = compact_lower(surface);
    compact.contains("[]") || compact.contains("...")
}

fn java_type_identifier(text: &str) -> Option<String> {
    let rest = java_type_surface(text)?;
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(rest.len());
    let first = &rest[..end];
    (!first.is_empty()).then(|| first.to_ascii_lowercase())
}

fn java_type_surface(text: &str) -> Option<&str> {
    let mut rest = text.trim_start();
    loop {
        let trimmed = rest.trim_start();
        if let Some(after_annotation) = strip_java_leading_annotation(trimmed) {
            rest = after_annotation;
            continue;
        }
        if let Some(after_modifier) = strip_java_modifier(trimmed) {
            rest = after_modifier;
            continue;
        }
        rest = trimmed;
        break;
    }
    (!rest.is_empty()).then_some(rest)
}

fn strip_java_modifier(text: &str) -> Option<&str> {
    const MODIFIERS: &[&str] = &[
        "final",
        "public",
        "private",
        "protected",
        "static",
        "volatile",
        "transient",
    ];
    let lower = text.to_ascii_lowercase();
    MODIFIERS.iter().find_map(|modifier| {
        lower
            .strip_prefix(modifier)
            .filter(|rest| {
                rest.chars()
                    .next()
                    .is_some_and(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
            })
            .map(|_| &text[modifier.len()..])
    })
}

fn strip_java_leading_annotation(text: &str) -> Option<&str> {
    let mut rest = text.strip_prefix('@')?;
    let name_len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(rest.len());
    if name_len == 0 {
        return None;
    }
    rest = &rest[name_len..];
    let trimmed = rest.trim_start();
    if !trimmed.starts_with('(') {
        return Some(trimmed);
    }
    let mut depth = 0usize;
    let mut quote = None;
    for (idx, ch) in trimmed.char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&trimmed[idx + ch.len_utf8()..]);
                }
            }
            _ => {}
        }
    }
    None
}

fn last_identifier(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .rfind(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
}

fn identifier_tokens(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|token| !token.is_empty())
        .map(|token| token.trim())
        .collect()
}

fn rust_integer_type(ty: &str) -> bool {
    matches!(
        ty,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
    )
}

fn go_integer_type(ty: &str) -> bool {
    matches!(
        ty,
        "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uintptr"
            | "byte"
            | "rune"
    )
}

fn c_integer_tokens(tokens: &[&str]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            *token,
            "char"
                | "short"
                | "int"
                | "long"
                | "int8_t"
                | "int16_t"
                | "int32_t"
                | "int64_t"
                | "uint8_t"
                | "uint16_t"
                | "uint32_t"
                | "uint64_t"
        )
    })
}
