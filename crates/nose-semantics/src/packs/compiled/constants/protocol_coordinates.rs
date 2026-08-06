pub(in crate::packs::compiled) const MAP_GET_DEFAULT_PROTOCOL_LANGUAGES: &[&str] =
    &["python", "ruby", "java"];
pub(in crate::packs::compiled) const FREE_FUNCTION_BUILTIN_PROTOCOL_LANGUAGES: &[&str] =
    &["python", "go", "swift"];
pub(in crate::packs::compiled) const PYTHON_ITERATOR_BUILTIN_PROTOCOL_LANGUAGES: &[&str] =
    &["python"];
pub(in crate::packs::compiled) const RECEIVER_MEMBERSHIP_PROTOCOL_LANGUAGES: &[&str] = &[
    "python",
    "ruby",
    "java",
    "rust",
    "swift",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
];
pub(in crate::packs::compiled) const MAP_KEY_VIEW_PROTOCOL_LANGUAGES: &[&str] = &[
    "python",
    "ruby",
    "java",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
];
pub(in crate::packs::compiled) const PROPERTY_BUILTIN_PROTOCOL_LANGUAGES: &[&str] = &[
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
    "java",
    "swift",
];
pub(in crate::packs::compiled) const BUILTIN_METHOD_CALL_PROTOCOL_LANGUAGES: &[&str] = &[
    "python",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
    "rust",
    "java",
    "ruby",
    "swift",
];
pub(in crate::packs::compiled) const STRING_AFFIX_PREDICATE_PROTOCOL_LANGUAGES: &[&str] = &[
    "python",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
    "go",
    "rust",
    "java",
    "ruby",
    "swift",
];
pub(in crate::packs::compiled) const MAP_GET_PROTOCOL_LANGUAGES: &[&str] = &[
    "java",
    "rust",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
];
pub(in crate::packs::compiled) const NO_LANGUAGES: &[&str] = &[];
pub(in crate::packs::compiled) const PYTHON_LANGUAGE: &[&str] = &["python"];
pub(in crate::packs::compiled) const RUBY_LANGUAGE: &[&str] = &["ruby"];
pub(in crate::packs::compiled) const RUST_LANGUAGE: &[&str] = &["rust"];
pub(in crate::packs::compiled) const NO_PACKAGES: &[&str] = &[];
pub(in crate::packs::compiled) const JAVA_STDLIB_MAP_FACTORY_PACKAGES: &[&str] = &["java.util"];
pub(in crate::packs::compiled) const JAVA_STDLIB_MAP_ENTRY_PACKAGES: &[&str] = &["java.util"];
pub(in crate::packs::compiled) const JAVA_STDLIB_COLLECTION_FACTORY_PACKAGES: &[&str] =
    &["java.util"];
pub(in crate::packs::compiled) const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACKAGES: &[&str] =
    &["java.util"];
pub(in crate::packs::compiled) const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACKAGES: &[&str] =
    &["com.google.common.collect"];
pub(in crate::packs::compiled) const JAVA_STDLIB_MATH_PACKAGES: &[&str] = &["java.lang"];
pub(in crate::packs::compiled) const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACKAGES: &[&str] =
    &["java.util"];
pub(in crate::packs::compiled) const ITERATOR_IDENTITY_ADAPTER_PACKAGES: &[&str] =
    &["core::iter", "java.util.stream"];
pub(in crate::packs::compiled) const MAP_GET_PROTOCOL_PACKAGES: &[&str] =
    &["Map", "java.util", "std::collections"];
pub(in crate::packs::compiled) const MAP_GET_DEFAULT_PROTOCOL_PACKAGES: &[&str] =
    &["dict", "Hash", "java.util"];
pub(in crate::packs::compiled) const FREE_FUNCTION_BUILTIN_PROTOCOL_PACKAGES: &[&str] =
    &["builtins", "go.predeclared", "Swift"];
pub(in crate::packs::compiled) const PYTHON_ITERATOR_BUILTIN_PROTOCOL_PACKAGES: &[&str] =
    &["builtins"];
pub(in crate::packs::compiled) const RECEIVER_MEMBERSHIP_PROTOCOL_PACKAGES: &[&str] = &[
    "Array",
    "Collection",
    "Hash",
    "Map",
    "Set",
    "Swift.Collection",
    "dict",
    "java.util",
    "std::collections",
];
pub(in crate::packs::compiled) const MAP_KEY_VIEW_PROTOCOL_PACKAGES: &[&str] =
    &["dict", "Hash", "Map", "Object", "java.util"];
pub(in crate::packs::compiled) const PROPERTY_BUILTIN_PROTOCOL_PACKAGES: &[&str] =
    &["Array", "Collection", "Swift.Collection", "java.lang"];
pub(in crate::packs::compiled) const BUILTIN_METHOD_CALL_PROTOCOL_PACKAGES: &[&str] =
    &["Collection", "Option", "String", "console", "functools"];
pub(in crate::packs::compiled) const STRING_AFFIX_PREDICATE_PROTOCOL_PACKAGES: &[&str] =
    &["String", "str", "Swift.String", "java.lang", "strings"];
pub(in crate::packs::compiled) const GO_STDLIB_NAMESPACE_CALL_PACKAGES: &[&str] =
    &["fmt", "slices", "strings"];
pub(in crate::packs::compiled) const JS_LIKE_BUILTIN_ARRAY_PACKAGES: &[&str] = &["Array"];
pub(in crate::packs::compiled) const JS_LIKE_BUILTIN_BOOLEAN_PACKAGES: &[&str] = &["Boolean"];
pub(in crate::packs::compiled) const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACKAGES: &[&str] =
    &["Map", "Set"];
pub(in crate::packs::compiled) const JS_LIKE_BUILTIN_PROMISE_PACKAGES: &[&str] = &["Promise"];
pub(in crate::packs::compiled) const JS_LIKE_BUILTIN_REGEX_PACKAGES: &[&str] = &["RegExp"];
pub(in crate::packs::compiled) const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACKAGES: &[&str] =
    &["Array"];
pub(in crate::packs::compiled) const PYTHON_BUILTIN_PACKAGES: &[&str] = &["builtins"];
pub(in crate::packs::compiled) const PYTHON_STDLIB_COLLECTION_FACTORY_PACKAGES: &[&str] =
    &["collections"];
pub(in crate::packs::compiled) const PYTHON_STDLIB_MATH_PACKAGES: &[&str] = &["math"];
pub(in crate::packs::compiled) const PYTHON_STDLIB_TYPE_DOMAIN_PACKAGES: &[&str] =
    &["typing", "collections.abc", "asyncio"];
pub(in crate::packs::compiled) const RUBY_STDLIB_SET_PACKAGES: &[&str] = &["set"];
pub(in crate::packs::compiled) const NO_IDS: &[&str] = &[];
