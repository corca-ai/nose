use super::*;

pub(super) const C_LANGUAGE: &[&str] = &["c"];
pub(super) const C_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["c", "h"];
pub(super) const PYTHON_BINDING_LANGS: &[Lang] = &[Lang::Python];
pub(super) const JS_TS_BINDING_LANGS: &[Lang] = &[Lang::JavaScript, Lang::TypeScript];
pub(super) const GO_BINDING_LANGS: &[Lang] = &[Lang::Go];
pub(super) const RUST_BINDING_LANGS: &[Lang] = &[Lang::Rust];
pub(super) const JAVA_BINDING_LANGS: &[Lang] = &[Lang::Java];
pub(super) const C_BINDING_LANGS: &[Lang] = &[Lang::C];
pub(super) const RUBY_BINDING_LANGS: &[Lang] = &[Lang::Ruby];
pub(super) const SWIFT_BINDING_LANGS: &[Lang] = &[Lang::Swift];
pub(super) const CSS_BINDING_LANGS: &[Lang] = &[Lang::Css];
pub(super) const HTML_EMBEDDED_BINDING_LANGS: &[Lang] = &[Lang::Html, Lang::Vue, Lang::Svelte];
pub(super) const PYTHON_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    PYTHON_LANGUAGE_CORE_PRODUCER_ID,
    PYTHON_SOURCE_FACT_PRODUCER_ID,
];
pub(super) const PYTHON_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[PYTHON_SOURCE_FACT_PRODUCER_ID];
pub(super) const JS_TS_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    JS_TS_LANGUAGE_CORE_PRODUCER_ID,
    JS_TS_SOURCE_FACT_PRODUCER_ID,
];
pub(super) const JS_TS_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[JS_TS_SOURCE_FACT_PRODUCER_ID];
pub(super) const GO_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[GO_LANGUAGE_CORE_PRODUCER_ID, GO_SOURCE_FACT_PRODUCER_ID];
pub(super) const GO_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[GO_SOURCE_FACT_PRODUCER_ID];
pub(super) const RUST_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[RUST_LANGUAGE_CORE_PRODUCER_ID, RUST_SOURCE_FACT_PRODUCER_ID];
pub(super) const RUST_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[RUST_SOURCE_FACT_PRODUCER_ID];
pub(super) const JAVA_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[JAVA_LANGUAGE_CORE_PRODUCER_ID, JAVA_SOURCE_FACT_PRODUCER_ID];
pub(super) const JAVA_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[JAVA_SOURCE_FACT_PRODUCER_ID];
pub(super) const C_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    C_LANGUAGE_CORE_PRODUCER_ID,
    C_SOURCE_FACT_PRODUCER_ID,
    C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
];
pub(super) const C_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[
    C_SOURCE_FACT_PRODUCER_ID,
    C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID,
];
pub(super) const RUBY_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[RUBY_LANGUAGE_CORE_PRODUCER_ID, RUBY_SOURCE_FACT_PRODUCER_ID];
pub(super) const RUBY_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[RUBY_SOURCE_FACT_PRODUCER_ID];
pub(super) const SWIFT_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    SWIFT_LANGUAGE_CORE_PRODUCER_ID,
    SWIFT_SOURCE_FACT_PRODUCER_ID,
];
pub(super) const SWIFT_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[SWIFT_SOURCE_FACT_PRODUCER_ID];
pub(super) const CSS_LANGUAGE_PRODUCER_IDS: &[&str] =
    &[CSS_LANGUAGE_CORE_PRODUCER_ID, CSS_SOURCE_FACT_PRODUCER_ID];
pub(super) const CSS_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] = &[CSS_SOURCE_FACT_PRODUCER_ID];
pub(super) const HTML_EMBEDDED_LANGUAGE_PRODUCER_IDS: &[&str] = &[
    HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
    HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
];
pub(super) const HTML_EMBEDDED_LANGUAGE_SOURCE_FACT_PRODUCER_IDS: &[&str] =
    &[HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID];
pub(super) const C_LANGUAGE_CONFORMANCE_REFS: &[&str] = &[
    "c-unsigned32-byte-lane-cast-positive",
    "c-unsigned32-alias-cast-positive",
    "c-unsigned32-signed-cast-hard-negative",
    "c-unsigned32-non-byte-lane-hard-negative",
];
pub(super) const PYTHON_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["py", "pyi"];
pub(super) const JS_TS_LANGUAGE_FILE_EXTENSIONS: &[&str] =
    &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];
pub(super) const GO_LANGUAGE: &[&str] = &["go"];
pub(super) const GO_STDLIB_NAMESPACE_CALL_LANGUAGE: &[&str] = &["go"];
pub(super) const GO_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["go"];
pub(super) const RUST_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["rs"];
pub(super) const JAVA_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["java"];
pub(super) const RUBY_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["rb"];
pub(super) const SWIFT_LANGUAGE: &[&str] = &["swift"];
pub(super) const SWIFT_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["swift"];
pub(super) const CSS_LANGUAGE: &[&str] = &["css"];
pub(super) const CSS_LANGUAGE_FILE_EXTENSIONS: &[&str] = &["css"];
pub(super) const HTML_EMBEDDED_LANGUAGES: &[&str] = &["html", "vue", "svelte"];
pub(super) const HTML_EMBEDDED_LANGUAGE_FILE_EXTENSIONS: &[&str] =
    &["html", "htm", "vue", "svelte"];
pub(super) const JS_LIKE_LANGUAGE: &[&str] = &["javascript", "typescript"];
pub(super) const JAVA_LANGUAGE: &[&str] = &["java"];
pub(super) const JAVA_RUST_LANGUAGE: &[&str] = &["java", "rust"];
pub(super) const MAP_GET_DEFAULT_PROTOCOL_LANGUAGES: &[&str] = &["python", "ruby", "java"];
pub(super) const FREE_FUNCTION_BUILTIN_PROTOCOL_LANGUAGES: &[&str] = &["python", "go", "swift"];
pub(super) const RECEIVER_MEMBERSHIP_PROTOCOL_LANGUAGES: &[&str] = &[
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
pub(super) const MAP_KEY_VIEW_PROTOCOL_LANGUAGES: &[&str] = &[
    "python",
    "ruby",
    "java",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
];
pub(super) const PROPERTY_BUILTIN_PROTOCOL_LANGUAGES: &[&str] = &[
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
    "java",
    "swift",
];
pub(super) const BUILTIN_METHOD_CALL_PROTOCOL_LANGUAGES: &[&str] = &[
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
pub(super) const MAP_GET_PROTOCOL_LANGUAGES: &[&str] = &[
    "java",
    "rust",
    "javascript",
    "typescript",
    "vue",
    "svelte",
    "html",
];
pub(super) const NO_LANGUAGES: &[&str] = &[];
pub(super) const PYTHON_LANGUAGE: &[&str] = &["python"];
pub(super) const RUBY_LANGUAGE: &[&str] = &["ruby"];
pub(super) const RUST_LANGUAGE: &[&str] = &["rust"];
pub(super) const NO_PACKAGES: &[&str] = &[];
pub(super) const JAVA_STDLIB_MAP_FACTORY_PACKAGES: &[&str] = &["java.util"];
pub(super) const JAVA_STDLIB_MAP_ENTRY_PACKAGES: &[&str] = &["java.util"];
pub(super) const JAVA_STDLIB_COLLECTION_FACTORY_PACKAGES: &[&str] = &["java.util"];
pub(super) const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACKAGES: &[&str] = &["java.util"];
pub(super) const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACKAGES: &[&str] =
    &["com.google.common.collect"];
pub(super) const JAVA_STDLIB_MATH_PACKAGES: &[&str] = &["java.lang"];
pub(super) const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACKAGES: &[&str] = &["java.util"];
pub(super) const ITERATOR_IDENTITY_ADAPTER_PACKAGES: &[&str] = &["core::iter", "java.util.stream"];
pub(super) const MAP_GET_PROTOCOL_PACKAGES: &[&str] = &["Map", "java.util", "std::collections"];
pub(super) const MAP_GET_DEFAULT_PROTOCOL_PACKAGES: &[&str] = &["dict", "Hash", "java.util"];
pub(super) const FREE_FUNCTION_BUILTIN_PROTOCOL_PACKAGES: &[&str] =
    &["builtins", "go.predeclared", "Swift"];
pub(super) const RECEIVER_MEMBERSHIP_PROTOCOL_PACKAGES: &[&str] = &[
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
pub(super) const MAP_KEY_VIEW_PROTOCOL_PACKAGES: &[&str] = &["dict", "Hash", "Map", "java.util"];
pub(super) const PROPERTY_BUILTIN_PROTOCOL_PACKAGES: &[&str] =
    &["Array", "Collection", "Swift.Collection", "java.lang"];
pub(super) const BUILTIN_METHOD_CALL_PROTOCOL_PACKAGES: &[&str] =
    &["Collection", "Option", "String", "console", "functools"];
pub(super) const GO_STDLIB_NAMESPACE_CALL_PACKAGES: &[&str] = &["fmt", "slices", "strings"];
pub(super) const JS_LIKE_BUILTIN_ARRAY_PACKAGES: &[&str] = &["Array"];
pub(super) const JS_LIKE_BUILTIN_BOOLEAN_PACKAGES: &[&str] = &["Boolean"];
pub(super) const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACKAGES: &[&str] = &["Map", "Set"];
pub(super) const JS_LIKE_BUILTIN_PROMISE_PACKAGES: &[&str] = &["Promise"];
pub(super) const JS_LIKE_BUILTIN_REGEX_PACKAGES: &[&str] = &["RegExp"];
pub(super) const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACKAGES: &[&str] = &["Array"];
pub(super) const PYTHON_BUILTIN_PACKAGES: &[&str] = &["builtins"];
pub(super) const PYTHON_STDLIB_COLLECTION_FACTORY_PACKAGES: &[&str] = &["collections"];
pub(super) const PYTHON_STDLIB_MATH_PACKAGES: &[&str] = &["math"];
pub(super) const PYTHON_STDLIB_TYPE_DOMAIN_PACKAGES: &[&str] =
    &["typing", "collections.abc", "asyncio"];
pub(super) const RUBY_STDLIB_SET_PACKAGES: &[&str] = &["set"];
pub(super) const RUST_STDLIB_COLLECTION_FACTORY_PACKAGES: &[&str] = &["std::collections"];
pub(super) const RUST_STDLIB_MAP_FACTORY_PACKAGES: &[&str] = &["std::collections"];
pub(super) const RUST_STDLIB_OPTION_PACKAGES: &[&str] = &["std::option", "core::option"];
pub(super) const RUST_STDLIB_INTEGER_METHOD_PACKAGES: &[&str] = &["core::primitive"];
pub(super) const RUST_STDLIB_VEC_PACKAGES: &[&str] = &["std::vec", "alloc::vec"];
pub(super) const NO_IDS: &[&str] = &[];
pub(super) const PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_IDS: &[&str] =
    &[PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID];
pub(super) const PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_IDS: &[&str] =
    &[PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_ID];
pub(super) const PYTHON_BUILTIN_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "python-builtin-list-factory-positive",
    "python-builtin-set-factory-positive",
    "python-builtin-frozenset-factory-positive",
    "python-builtin-tuple-factory-positive",
    "python-builtin-list-shadowed-hard-negative",
    "python-builtin-list-wildcard-import-hard-negative",
];
pub(super) const PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS: &[&str] =
    &[PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID];
pub(super) const PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS: &[&str] =
    &[PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_ID];
pub(super) const PYTHON_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "python-collections-deque-imported-binding-positive",
    "python-collections-deque-imported-alias-positive",
    "python-collections-deque-imported-namespace-positive",
    "python-collections-deque-missing-import-hard-negative",
    "python-collections-deque-wrong-module-hard-negative",
];
pub(super) const PYTHON_STDLIB_MATH_PRODUCER_IDS: &[&str] = &[PYTHON_STDLIB_MATH_PRODUCER_ID];
pub(super) const PYTHON_STDLIB_MATH_CONTRACT_IDS: &[&str] = &[PYTHON_STDLIB_MATH_PROD_CONTRACT_ID];
pub(super) const PYTHON_STDLIB_MATH_CONFORMANCE_REFS: &[&str] = &[
    "python-math-prod-positive",
    "python-math-prod-local-shadow-hard-negative",
    "python-math-prod-wrong-namespace-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_PROMISE_PRODUCER_IDS: &[&str] =
    &[JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_PROMISE_CONTRACT_IDS: &[&str] = &[
    JS_LIKE_BUILTIN_PROMISE_RESOLVE_CONTRACT_ID,
    JS_LIKE_BUILTIN_PROMISE_THEN_CONTRACT_ID,
];
pub(super) const JS_LIKE_BUILTIN_PROMISE_CONFORMANCE_REFS: &[&str] = &[
    "js-promise-resolve-positive",
    "js-promise-then-positive",
    "js-promise-resolve-shadowed-hard-negative",
    "js-promise-then-missing-receiver-hard-negative",
    "js-promise-resolve-thenable-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_ARRAY_PRODUCER_IDS: &[&str] = &[JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_ARRAY_CONTRACT_IDS: &[&str] = &[
    JS_LIKE_BUILTIN_ARRAY_FROM_CONTRACT_ID,
    JS_LIKE_BUILTIN_ARRAY_IS_ARRAY_CONTRACT_ID,
];
pub(super) const JS_LIKE_BUILTIN_ARRAY_CONFORMANCE_REFS: &[&str] = &[
    "js-array-from-positive",
    "js-array-is-array-positive",
    "js-array-from-shadowed-hard-negative",
    "js-array-from-unsupported-arity-hard-negative",
    "js-array-is-array-shadowed-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_IDS: &[&str] =
    &[JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_IDS: &[&str] =
    &[JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_ID];
pub(super) const JS_LIKE_BUILTIN_BOOLEAN_CONFORMANCE_REFS: &[&str] = &[
    "js-boolean-coercion-positive",
    "js-boolean-coercion-shadowed-hard-negative",
    "js-boolean-coercion-unsupported-arity-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_REGEX_PRODUCER_IDS: &[&str] = &[JS_LIKE_BUILTIN_REGEX_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_REGEX_CONTRACT_IDS: &[&str] =
    &[JS_LIKE_BUILTIN_REGEX_TEST_CONTRACT_ID];
pub(super) const JS_LIKE_BUILTIN_REGEX_CONFORMANCE_REFS: &[&str] = &[
    "js-regex-test-positive",
    "js-regex-test-string-receiver-hard-negative",
    "js-regex-test-unsupported-arity-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_IDS: &[&str] =
    &[JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_CONTRACT_IDS: &[&str] = &[
    JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_INDEX_OF_CONTRACT_ID,
    JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_FIND_INDEX_CONTRACT_ID,
];
pub(super) const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_CONFORMANCE_REFS: &[&str] = &[
    "js-static-index-membership-index-of-positive",
    "js-static-index-membership-find-index-positive",
    "js-static-index-membership-non-literal-receiver-hard-negative",
    "js-static-index-membership-float-literal-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_IDS: &[&str] =
    &[JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_CONTRACT_IDS: &[&str] = &[
    JS_LIKE_BUILTIN_SET_CONSTRUCTOR_CONTRACT_ID,
    JS_LIKE_BUILTIN_MAP_CONSTRUCTOR_CONTRACT_ID,
];
pub(super) const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_CONFORMANCE_REFS: &[&str] = &[
    "js-set-constructor-positive",
    "js-map-constructor-positive",
    "js-set-constructor-shadowed-hard-negative",
    "js-map-constructor-shadowed-hard-negative",
    "js-collection-constructor-missing-construct-hard-negative",
];
pub(super) const RUBY_STDLIB_SET_PRODUCER_IDS: &[&str] = &[RUBY_STDLIB_SET_PRODUCER_ID];
pub(super) const RUBY_STDLIB_SET_CONTRACT_IDS: &[&str] = &[RUBY_STDLIB_SET_CONTRACT_ID];
pub(super) const RUBY_STDLIB_SET_CONFORMANCE_REFS: &[&str] = &[
    "ruby-set-new-include-positive",
    "ruby-set-new-member-positive",
    "ruby-set-local-positive",
    "ruby-set-missing-require-hard-negative",
    "ruby-set-shadowed-hard-negative",
    "ruby-set-mutated-hard-negative",
];
pub(super) const RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID];
pub(super) const RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS: &[&str] =
    &[RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_ID];
pub(super) const RUST_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "rust-std-collections-hashset-from-positive",
    "rust-std-collections-btreeset-from-positive",
    "rust-std-collections-vecdeque-from-positive",
    "rust-std-collections-shadowed-std-hard-negative",
    "rust-std-collections-type-alias-std-hard-negative",
];
pub(super) const RUST_STDLIB_MAP_FACTORY_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_MAP_FACTORY_PRODUCER_ID];
pub(super) const RUST_STDLIB_MAP_FACTORY_CONTRACT_IDS: &[&str] =
    &[RUST_STDLIB_MAP_FACTORY_CONTRACT_ID];
pub(super) const RUST_STDLIB_MAP_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "rust-std-map-hashmap-from-positive",
    "rust-std-map-btreemap-from-positive",
    "rust-std-map-shadowed-std-hard-negative",
    "rust-std-map-type-alias-std-hard-negative",
];
pub(super) const RUST_STDLIB_OPTION_PRODUCER_IDS: &[&str] = &[RUST_STDLIB_OPTION_PRODUCER_ID];
pub(super) const RUST_STDLIB_OPTION_CONTRACT_IDS: &[&str] = &[
    RUST_STDLIB_OPTION_SOME_CONTRACT_ID,
    RUST_STDLIB_OPTION_NONE_CONTRACT_ID,
    RUST_STDLIB_OPTION_AND_THEN_CONTRACT_ID,
];
pub(super) const RUST_STDLIB_OPTION_CONFORMANCE_REFS: &[&str] = &[
    "rust-option-some-positive",
    "rust-option-none-positive",
    "rust-option-and-then-positive",
    "rust-option-some-shadow-hard-negative",
    "rust-option-none-shadow-hard-negative",
    "rust-option-and-then-non-option-hard-negative",
];
pub(super) const RUST_STDLIB_INTEGER_METHOD_PRODUCER_IDS: &[&str] =
    &[RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID];
pub(super) const RUST_STDLIB_INTEGER_METHOD_CONTRACT_IDS: &[&str] = &[
    SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID,
];
pub(super) const RUST_STDLIB_INTEGER_METHOD_CONFORMANCE_REFS: &[&str] = &[
    "rust-integer-method-abs-positive",
    "rust-integer-method-min-positive",
    "rust-integer-method-max-positive",
    "rust-integer-method-clamp-positive",
    "rust-integer-method-non-integer-receiver-hard-negative",
    "rust-integer-method-unsupported-arity-hard-negative",
];
pub(super) const JAVA_STDLIB_MATH_PRODUCER_IDS: &[&str] = &[JAVA_STDLIB_MATH_PRODUCER_ID];
pub(super) const JAVA_STDLIB_MATH_CONTRACT_IDS: &[&str] = &[
    SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID,
    SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID,
];
pub(super) const JAVA_STDLIB_MATH_CONFORMANCE_REFS: &[&str] = &[
    "java-math-abs-positive",
    "java-math-min-positive",
    "java-math-max-positive",
    "java-math-shadowed-math-hard-negative",
    "java-math-non-integer-argument-hard-negative",
    "java-math-unsupported-arity-hard-negative",
];
pub(super) const MAP_GET_PROTOCOL_PRODUCER_IDS: &[&str] = &[MAP_GET_PROTOCOL_PRODUCER_ID];
pub(super) const MAP_GET_PROTOCOL_CONTRACT_IDS: &[&str] = &[MAP_GET_CONTRACT_ID];
pub(super) const MAP_GET_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "map-get-rust-positive",
    "map-get-java-positive",
    "map-get-js-positive",
    "map-get-non-map-receiver-hard-negative",
    "map-get-unsupported-arity-hard-negative",
];
pub(super) const MAP_GET_DEFAULT_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID];
pub(super) const MAP_GET_DEFAULT_PROTOCOL_CONTRACT_IDS: &[&str] = &[MAP_GET_DEFAULT_CONTRACT_ID];
pub(super) const MAP_GET_DEFAULT_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "map-get-default-python-get-positive",
    "map-get-default-ruby-fetch-positive",
    "map-get-default-java-get-or-default-positive",
    "map-get-default-non-map-receiver-hard-negative",
    "map-get-default-unsupported-arity-hard-negative",
];
pub(super) const FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID];
pub(super) const FREE_FUNCTION_BUILTIN_PROTOCOL_CONTRACT_IDS: &[&str] =
    &[FREE_FUNCTION_BUILTIN_CONTRACT_ID];
pub(super) const FREE_FUNCTION_BUILTIN_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "free-function-builtin-python-len-positive",
    "free-function-builtin-python-range-positive",
    "free-function-builtin-python-reduction-positive",
    "free-function-builtin-go-len-positive",
    "free-function-builtin-go-append-positive",
    "free-function-builtin-swift-abs-positive",
    "free-function-builtin-missing-symbol-hard-negative",
    "free-function-builtin-compatibility-pack-hard-negative",
    "free-function-builtin-wrong-producer-hard-negative",
    "free-function-builtin-unsupported-arity-hard-negative",
];
pub(super) const RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID];
pub(super) const RECEIVER_MEMBERSHIP_PROTOCOL_CONTRACT_IDS: &[&str] =
    &[RECEIVER_MEMBERSHIP_CONTRACT_ID];
pub(super) const RECEIVER_MEMBERSHIP_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "receiver-membership-java-contains-key-positive",
    "receiver-membership-rust-contains-key-positive",
    "receiver-membership-ruby-key-positive",
    "receiver-membership-ruby-has-key-positive",
    "receiver-membership-python-contains-positive",
    "receiver-membership-js-includes-positive",
    "receiver-membership-js-has-positive",
    "receiver-membership-java-contains-positive",
    "receiver-membership-swift-contains-positive",
    "receiver-membership-ruby-member-positive",
    "receiver-membership-missing-receiver-proof-hard-negative",
    "receiver-membership-unsupported-arity-hard-negative",
    "receiver-membership-go-slices-contains-out-of-scope-hard-negative",
];
pub(super) const MAP_KEY_VIEW_PROTOCOL_PRODUCER_IDS: &[&str] = &[MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID];
pub(super) const MAP_KEY_VIEW_PROTOCOL_CONTRACT_IDS: &[&str] = &[
    MAP_KEY_VIEW_COLLECTION_CONTRACT_ID,
    MAP_KEY_VIEW_ITERATOR_CONTRACT_ID,
];
pub(super) const MAP_KEY_VIEW_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "map-key-view-python-keys-positive",
    "map-key-view-ruby-keys-positive",
    "map-key-view-java-keyset-positive",
    "map-key-view-js-keys-positive",
    "map-key-view-non-map-receiver-hard-negative",
    "map-key-view-unsupported-arity-hard-negative",
];
pub(super) const PROPERTY_BUILTIN_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID];
pub(super) const PROPERTY_BUILTIN_PROTOCOL_CONTRACT_IDS: &[&str] = &[
    PROPERTY_BUILTIN_LEN_CONTRACT_ID,
    PROPERTY_BUILTIN_IS_EMPTY_CONTRACT_ID,
];
pub(super) const PROPERTY_BUILTIN_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "property-builtin-js-length-positive",
    "property-builtin-java-length-positive",
    "property-builtin-swift-count-positive",
    "property-builtin-swift-is-empty-positive",
    "property-builtin-missing-receiver-proof-hard-negative",
    "property-builtin-wrong-pack-hard-negative",
    "property-builtin-unsupported-property-hard-negative",
];
pub(super) const BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID];
pub(super) const BUILTIN_METHOD_CALL_PROTOCOL_CONTRACT_IDS: &[&str] =
    &[BUILTIN_METHOD_CALL_CONTRACT_ID];
pub(super) const BUILTIN_METHOD_CALL_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "builtin-method-call-python-append-positive",
    "builtin-method-call-js-push-positive",
    "builtin-method-call-rust-len-positive",
    "builtin-method-call-java-size-positive",
    "builtin-method-call-python-startswith-positive",
    "builtin-method-call-python-join-positive",
    "builtin-method-call-rust-unwrap-or-positive",
    "builtin-method-call-python-reduce-positive",
    "builtin-method-call-missing-receiver-proof-hard-negative",
    "builtin-method-call-wrong-pack-hard-negative",
    "builtin-method-call-unsupported-arity-hard-negative",
];
pub(super) const GO_STDLIB_NAMESPACE_CALL_PRODUCER_IDS: &[&str] =
    &[GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID];
pub(super) const GO_STDLIB_NAMESPACE_CALL_CONTRACT_IDS: &[&str] =
    &[GO_STDLIB_NAMESPACE_CALL_CONTRACT_ID];
pub(super) const GO_STDLIB_NAMESPACE_CALL_CONFORMANCE_REFS: &[&str] = &[
    "go-stdlib-namespace-call-fmt-print-positive",
    "go-stdlib-namespace-call-strings-has-prefix-positive",
    "go-stdlib-namespace-call-strings-has-suffix-positive",
    "go-stdlib-namespace-call-slices-contains-positive",
    "go-stdlib-namespace-call-missing-import-hard-negative",
    "go-stdlib-namespace-call-wrong-pack-hard-negative",
];
pub(super) const JAVA_STDLIB_MAP_FACTORY_PRODUCER_IDS: &[&str] =
    &[JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID];
pub(super) const JAVA_STDLIB_MAP_FACTORY_CONTRACT_IDS: &[&str] = &[
    JAVA_STDLIB_MAP_FACTORY_OF_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_OF_ENTRIES_CONTRACT_ID,
];
pub(super) const JAVA_STDLIB_MAP_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "java-map-of-positive",
    "java-map-of-entries-positive",
    "java-map-missing-import-hard-negative",
    "java-map-entry-boundary-hard-negative",
];
pub(super) const JAVA_STDLIB_MAP_ENTRY_PRODUCER_IDS: &[&str] = &[JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID];
pub(super) const JAVA_STDLIB_MAP_ENTRY_CONTRACT_IDS: &[&str] = &[JAVA_STDLIB_MAP_ENTRY_CONTRACT_ID];
pub(super) const JAVA_STDLIB_MAP_ENTRY_CONFORMANCE_REFS: &[&str] = &[
    "java-map-entry-positive",
    "java-map-entry-missing-import-hard-negative",
    "java-map-entry-shadowed-map-hard-negative",
];
pub(super) const JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_IDS: &[&str] =
    &[JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID];
pub(super) const JAVA_STDLIB_COLLECTION_FACTORY_CONTRACT_IDS: &[&str] = &[
    JAVA_STDLIB_COLLECTION_FACTORY_LIST_OF_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_SET_OF_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_ARRAYS_AS_LIST_CONTRACT_ID,
];
pub(super) const JAVA_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "java-list-of-positive",
    "java-set-of-positive",
    "java-arrays-as-list-positive",
    "java-collection-missing-import-hard-negative",
    "java-collection-constructor-boundary-hard-negative",
];
pub(super) const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_IDS: &[&str] =
    &[JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID];
pub(super) const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_CONTRACT_IDS: &[&str] = &[
    JAVA_GUAVA_IMMUTABLE_LIST_OF_CONTRACT_ID,
    JAVA_GUAVA_IMMUTABLE_SET_OF_CONTRACT_ID,
    JAVA_GUAVA_IMMUTABLE_MAP_OF_CONTRACT_ID,
];
pub(super) const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "java-guava-immutable-list-of-positive",
    "java-guava-immutable-set-of-positive",
    "java-guava-immutable-map-of-positive",
    "java-guava-immutable-copy-of-hard-negative",
    "java-guava-immutable-missing-import-hard-negative",
    "java-guava-immutable-wrong-package-hard-negative",
    "java-guava-immutable-shadowed-type-hard-negative",
];
pub(super) const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_IDS: &[&str] =
    &[JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID];
pub(super) const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_CONTRACT_IDS: &[&str] =
    &[JAVA_STDLIB_COLLECTION_CONSTRUCTOR_EMPTY_LIST_CONTRACT_ID];
pub(super) const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_CONFORMANCE_REFS: &[&str] = &[
    "java-arraylist-empty-constructor-positive",
    "java-linkedlist-empty-constructor-positive",
    "java-constructor-missing-import-hard-negative",
    "java-constructor-shadowed-type-hard-negative",
    "java-constructor-conflicting-import-hard-negative",
];
pub(super) const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_IDS: &[&str] =
    &[JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID];
pub(super) const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_IDS: &[&str] =
    &[JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_ID];
pub(super) const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONFORMANCE_REFS: &[&str] = &[
    "java-arrays-stream-positive",
    "java-arrays-stream-missing-import-hard-negative",
    "java-arrays-stream-shadowed-arrays-hard-negative",
];
pub(super) const ITERATOR_IDENTITY_ADAPTER_PRODUCER_IDS: &[&str] =
    &[ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID];
pub(super) const ITERATOR_IDENTITY_ADAPTER_CONTRACT_IDS: &[&str] =
    &[ITERATOR_IDENTITY_ADAPTER_CONTRACT_ID];
pub(super) const ITERATOR_IDENTITY_ADAPTER_CONFORMANCE_REFS: &[&str] = &[
    "rust-iterator-identity-iter-positive",
    "rust-iterator-identity-collect-positive",
    "java-iterator-identity-stream-positive",
    "iterator-identity-non-iterable-receiver-hard-negative",
    "iterator-identity-unsupported-arity-hard-negative",
];
pub(super) const RUST_STDLIB_VEC_PRODUCER_IDS: &[&str] = &[RUST_STDLIB_VEC_PRODUCER_ID];
pub(super) const RUST_STDLIB_VEC_CONTRACT_IDS: &[&str] = &[
    RUST_STDLIB_VEC_MACRO_CONTRACT_ID,
    RUST_STDLIB_VEC_NEW_CONTRACT_ID,
];
pub(super) const RUST_STDLIB_VEC_CONFORMANCE_REFS: &[&str] = &[
    "rust-vec-macro-factory-positive",
    "rust-vec-new-factory-positive",
    "rust-vec-macro-shadowed-hard-negative",
    "rust-vec-new-shadowed-hard-negative",
];
pub(super) const PYTHON_STDLIB_TYPE_DOMAIN_CONTRACT_IDS: &[&str] =
    &["python.stdlib.type-domain-alias.contract"];
pub(super) const PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_IDS: &[&str] =
    &[PYTHON_STDLIB_TYPE_DOMAIN_PRODUCER_ID];
pub(super) const PYTHON_STDLIB_TYPE_DOMAIN_HARD_NEGATIVE_REFS: &[&str] =
    &["python-typing-domain-wrong-module-hard-negative"];
pub(super) const NO_TYPE_DOMAIN_ALIAS_CONTRACTS: &[BuiltinTypeDomainAliasContract] = &[];
