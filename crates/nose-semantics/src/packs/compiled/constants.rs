use super::*;
use crate::{
    JS_LIKE_BUILTIN_PROMISE_CATCH_CONTRACT_ID, JS_LIKE_BUILTIN_PROMISE_FINALLY_CONTRACT_ID,
    JS_LIKE_BUILTIN_PROMISE_REJECT_CONTRACT_ID,
};

mod language_metadata;
mod protocol_coordinates;
mod rust;
mod sequence_hof;
mod string_affix;
mod swift;
mod type_domains;
pub(super) use language_metadata::*;
pub(super) use protocol_coordinates::*;
pub(super) use rust::*;
pub(super) use sequence_hof::*;
pub(super) use string_affix::*;
pub(super) use swift::*;
pub(super) use type_domains::*;

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
    JS_LIKE_BUILTIN_PROMISE_REJECT_CONTRACT_ID,
    JS_LIKE_BUILTIN_PROMISE_THEN_CONTRACT_ID,
    JS_LIKE_BUILTIN_PROMISE_CATCH_CONTRACT_ID,
    JS_LIKE_BUILTIN_PROMISE_FINALLY_CONTRACT_ID,
];
pub(super) const JS_LIKE_BUILTIN_PROMISE_CONFORMANCE_REFS: &[&str] = &[
    "js-promise-resolve-positive",
    "js-promise-reject-catch-positive",
    "js-promise-then-positive",
    "js-promise-finally-positive",
    "js-promise-resolve-shadowed-hard-negative",
    "js-promise-then-missing-receiver-hard-negative",
    "js-promise-resolve-thenable-hard-negative",
    "js-promise-finally-thenable-hard-negative",
];
pub(super) const JS_LIKE_BUILTIN_ARRAY_PRODUCER_IDS: &[&str] = &[JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID];
pub(super) const JS_LIKE_BUILTIN_ARRAY_CONTRACT_IDS: &[&str] = &[
    JS_LIKE_BUILTIN_ARRAY_FROM_CONTRACT_ID,
    JS_LIKE_BUILTIN_ARRAY_IS_ARRAY_CONTRACT_ID,
    JS_LIKE_BUILTIN_ARRAY_HOF_CONTRACT_ID,
    JS_LIKE_BUILTIN_ARRAY_BOOL_REDUCTION_CONTRACT_ID,
];
pub(super) const JS_LIKE_BUILTIN_ARRAY_CONFORMANCE_REFS: &[&str] = &[
    "js-array-from-positive",
    "js-array-is-array-positive",
    "js-array-map-hof-positive",
    "js-array-filter-hof-positive",
    "js-array-flatmap-hof-positive",
    "js-array-some-terminal-positive",
    "js-array-every-terminal-positive",
    "js-array-from-shadowed-hard-negative",
    "js-array-from-unsupported-arity-hard-negative",
    "js-array-is-array-shadowed-hard-negative",
    "js-array-hof-generic-pack-hard-negative",
    "js-array-hof-missing-receiver-proof-hard-negative",
    "js-array-hof-collection-receiver-hard-negative",
    "js-array-hof-this-arg-hard-negative",
    "js-array-hof-sparse-array-hard-negative",
    "js-array-hof-borrowed-prototype-hard-negative",
    "js-array-hof-effectful-callback-hard-negative",
    "js-array-find-deferred-hard-negative",
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
pub(super) const PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_IDS: &[&str] =
    &[PYTHON_ITERATOR_BUILTIN_PROTOCOL_PRODUCER_ID];
pub(super) const PYTHON_ITERATOR_BUILTIN_PROTOCOL_CONTRACT_IDS: &[&str] = &[
    PYTHON_ITERATOR_BUILTIN_CONTRACT_ID,
    FREE_FUNCTION_HOF_CONTRACT_ID,
];
pub(super) const PYTHON_ITERATOR_BUILTIN_PROTOCOL_CONFORMANCE_REFS: &[&str] = &[
    "python-iterator-builtin-map-positive",
    "python-iterator-builtin-filter-positive",
    "python-iterator-builtin-zip-positive",
    "python-iterator-builtin-enumerate-positive",
    "python-iterator-builtin-any-terminal-positive",
    "python-iterator-builtin-all-terminal-positive",
    "python-iterator-builtin-materializer-positive",
    "python-iterator-builtin-shadowed-hard-negative",
    "python-iterator-builtin-wildcard-import-hard-negative",
    "python-iterator-builtin-missing-source-proof-hard-negative",
    "python-iterator-builtin-callback-not-lambda-hard-negative",
    "python-iterator-builtin-missing-materializer-proof-hard-negative",
    "python-iterator-builtin-materializer-identity-hard-negative",
    "python-iterator-builtin-source-api-dependency-closure-hard-negative",
    "python-iterator-builtin-multi-iterable-map-unsupported-hard-negative",
    "python-iterator-builtin-sorted-reversed-unsupported-hard-negative",
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
    "map-key-view-js-object-keys-positive",
    "map-key-view-non-map-receiver-hard-negative",
    "map-key-view-unsupported-arity-hard-negative",
    "map-key-view-js-object-keys-shadowed-hard-negative",
    "map-key-view-js-object-keys-proto-hard-negative",
    "map-key-view-js-object-keys-mutated-hard-negative",
    "map-key-view-js-object-keys-delete-hard-negative",
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
    "go-stdlib-namespace-call-slices-contains-positive",
    "go-stdlib-namespace-call-strings-contains-positive",
    "go-stdlib-namespace-call-missing-import-hard-negative",
    "go-stdlib-namespace-call-wrong-pack-hard-negative",
];
pub(super) const JAVA_STDLIB_MAP_FACTORY_PRODUCER_IDS: &[&str] =
    &[JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID];
pub(super) const JAVA_STDLIB_MAP_FACTORY_CONTRACT_IDS: &[&str] = &[
    JAVA_STDLIB_MAP_FACTORY_OF_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_OF_ENTRIES_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_EMPTY_MAP_CONTRACT_ID,
    JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_SINGLETON_MAP_CONTRACT_ID,
];
pub(super) const JAVA_STDLIB_MAP_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "java-map-of-positive",
    "java-map-of-entries-positive",
    "java-collections-empty-map-positive",
    "java-collections-singleton-map-positive",
    "java-map-missing-import-hard-negative",
    "java-map-entry-boundary-hard-negative",
    "java-collections-map-missing-import-hard-negative",
    "java-collections-map-wrong-domain-hard-negative",
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
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_LIST_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_SET_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_CONTRACT_ID,
    JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_LIST_CONTRACT_ID,
];
pub(super) const JAVA_STDLIB_COLLECTION_FACTORY_CONFORMANCE_REFS: &[&str] = &[
    "java-list-of-positive",
    "java-set-of-positive",
    "java-arrays-as-list-positive",
    "java-collections-empty-list-positive",
    "java-collections-empty-set-positive",
    "java-collections-singleton-positive",
    "java-collections-singleton-list-positive",
    "java-collection-missing-import-hard-negative",
    "java-collection-constructor-boundary-hard-negative",
    "java-collections-missing-import-hard-negative",
    "java-collections-shadowed-collections-hard-negative",
    "java-collections-wrong-domain-hard-negative",
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
