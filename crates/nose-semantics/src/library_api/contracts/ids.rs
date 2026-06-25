pub const PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID: &str =
    "nose.python.builtins.collection_factories";
pub const PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID: &str =
    "python.builtins.collection-factory-api";
pub const PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_ID: &str = "python.builtin.collection_factory";
pub const PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID: &str =
    "nose.python.stdlib.collection_factories";
pub const PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str =
    "python.stdlib.collection-factory-api";
pub const PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_ID: &str = "python.imported.collection_factory";
pub const PYTHON_STDLIB_MATH_PACK_ID: &str = "nose.python.stdlib.math";
pub const PYTHON_STDLIB_MATH_PRODUCER_ID: &str = "python.stdlib.math-api";
pub const PYTHON_STDLIB_MATH_PROD_CONTRACT_ID: &str = "python.math.prod.reduction";
pub const JS_LIKE_BUILTIN_PROMISE_PACK_ID: &str = "nose.javascript.builtins.promise";
pub const JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID: &str = "javascript.builtins.promise-api";
pub const JS_LIKE_BUILTIN_PROMISE_RESOLVE_CONTRACT_ID: &str = "js_like.promise.factory.resolve";
pub const JS_LIKE_BUILTIN_PROMISE_THEN_CONTRACT_ID: &str = "js_like.promise.then";
pub const JS_LIKE_BUILTIN_ARRAY_PACK_ID: &str = "nose.javascript.builtins.array";
pub const JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID: &str = "javascript.builtins.array-api";
pub const JS_LIKE_BUILTIN_ARRAY_FROM_CONTRACT_ID: &str = "map_key_view.wrapper";
pub const JS_LIKE_BUILTIN_ARRAY_IS_ARRAY_CONTRACT_ID: &str = "js_like.array.is_array";
pub const JS_LIKE_BUILTIN_BOOLEAN_PACK_ID: &str = "nose.javascript.builtins.boolean";
pub const JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID: &str = "javascript.builtins.boolean-api";
pub const JS_LIKE_BUILTIN_BOOLEAN_CONTRACT_ID: &str = "js_like.boolean.coercion";
pub const JS_LIKE_BUILTIN_REGEX_PACK_ID: &str = "nose.javascript.builtins.regex";
pub const JS_LIKE_BUILTIN_REGEX_PRODUCER_ID: &str = "javascript.builtins.regex-api";
pub const JS_LIKE_BUILTIN_REGEX_TEST_CONTRACT_ID: &str = "js_like.regex.test";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PACK_ID: &str =
    "nose.javascript.builtins.static_index_membership";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID: &str =
    "javascript.builtins.static-index-membership-api";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_INDEX_OF_CONTRACT_ID: &str =
    "js_like.static_index_membership.index_of";
pub const JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_FIND_INDEX_CONTRACT_ID: &str =
    "js_like.static_index_membership.find_index";
pub const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PACK_ID: &str =
    "nose.javascript.builtins.collection_constructors";
pub const JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID: &str =
    "javascript.builtins.collection-constructor-api";
pub const JS_LIKE_BUILTIN_SET_CONSTRUCTOR_CONTRACT_ID: &str = "js_like.set.constructor";
pub const JS_LIKE_BUILTIN_MAP_CONSTRUCTOR_CONTRACT_ID: &str = "js_like.map.constructor";
pub const MAP_GET_PROTOCOL_PACK_ID: &str = "nose.protocols.map_get";
pub const MAP_GET_PROTOCOL_PRODUCER_ID: &str = "protocols.map-get-api";
pub const MAP_GET_CONTRACT_ID: &str = "map.get";
pub const MAP_GET_DEFAULT_PROTOCOL_PACK_ID: &str = "nose.protocols.map_get_default";
pub const MAP_GET_DEFAULT_PROTOCOL_PRODUCER_ID: &str = "protocols.map-get-default-api";
pub const MAP_GET_DEFAULT_CONTRACT_ID: &str = "map.get_default";
pub const FREE_FUNCTION_BUILTIN_PROTOCOL_PACK_ID: &str = "nose.protocols.free_function_builtins";
pub const FREE_FUNCTION_BUILTIN_PROTOCOL_PRODUCER_ID: &str = "protocols.free-function-builtin-api";
pub const FREE_FUNCTION_BUILTIN_CONTRACT_ID: &str = "free_function_builtin.call";
pub const RECEIVER_MEMBERSHIP_PROTOCOL_PACK_ID: &str = "nose.protocols.receiver_membership";
pub const RECEIVER_MEMBERSHIP_PROTOCOL_PRODUCER_ID: &str = "protocols.receiver-membership-api";
pub const RECEIVER_MEMBERSHIP_CONTRACT_ID: &str = "receiver_membership.contains";
pub const MAP_KEY_VIEW_PROTOCOL_PACK_ID: &str = "nose.protocols.map_key_views";
pub const MAP_KEY_VIEW_PROTOCOL_PRODUCER_ID: &str = "protocols.map-key-view-api";
pub const MAP_KEY_VIEW_COLLECTION_CONTRACT_ID: &str = "map_key_view.collection";
pub const MAP_KEY_VIEW_ITERATOR_CONTRACT_ID: &str = "map_key_view.iterator";
pub const PROPERTY_BUILTIN_PROTOCOL_PACK_ID: &str = "nose.protocols.property_builtins";
pub const PROPERTY_BUILTIN_PROTOCOL_PRODUCER_ID: &str = "protocols.property-builtin-api";
pub const PROPERTY_BUILTIN_LEN_CONTRACT_ID: &str = "property_builtin.len";
pub const PROPERTY_BUILTIN_IS_EMPTY_CONTRACT_ID: &str = "property_builtin.is_empty";
pub const BUILTIN_METHOD_CALL_PROTOCOL_PACK_ID: &str = "nose.protocols.builtin_method_calls";
pub const BUILTIN_METHOD_CALL_PROTOCOL_PRODUCER_ID: &str = "protocols.builtin-method-call-api";
pub const BUILTIN_METHOD_CALL_CONTRACT_ID: &str = "builtin_method.call";
pub const GO_STDLIB_NAMESPACE_CALL_PACK_ID: &str = "nose.go.stdlib.namespace_calls";
pub const GO_STDLIB_NAMESPACE_CALL_PRODUCER_ID: &str = "go.stdlib.namespace-call-api";
pub const GO_STDLIB_NAMESPACE_CALL_CONTRACT_ID: &str = "go.stdlib.namespace_call";
pub const SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID: &str = "scalar_integer_method.abs";
pub const SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID: &str = "scalar_integer_method.min";
pub const SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID: &str = "scalar_integer_method.max";
pub const SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID: &str = "scalar_integer_method.clamp";
pub const JAVA_STDLIB_MATH_PACK_ID: &str = "nose.java.stdlib.math";
pub const JAVA_STDLIB_MATH_PRODUCER_ID: &str = "java.stdlib.math-api";
pub const RUBY_STDLIB_SET_PACK_ID: &str = "nose.ruby.stdlib.set";
pub const RUBY_STDLIB_SET_PRODUCER_ID: &str = "ruby.stdlib.set-factory-api";
pub const RUBY_STDLIB_SET_CONTRACT_ID: &str = "ruby.set_factory";
pub const RUST_STDLIB_VEC_PACK_ID: &str = "nose.rust.stdlib.vec";
pub const RUST_STDLIB_VEC_PRODUCER_ID: &str = "rust.stdlib.vec-factory-api";
pub const RUST_STDLIB_VEC_MACRO_CONTRACT_ID: &str = "rust.vec.macro_factory";
pub const RUST_STDLIB_VEC_NEW_CONTRACT_ID: &str = "rust.vec.new_factory";
pub const RUST_STDLIB_OPTION_PACK_ID: &str = "nose.rust.stdlib.option";
pub const RUST_STDLIB_OPTION_PRODUCER_ID: &str = "rust.stdlib.option-api";
pub const RUST_STDLIB_OPTION_SOME_CONTRACT_ID: &str = "rust.option.some.constructor";
pub const RUST_STDLIB_OPTION_NONE_CONTRACT_ID: &str = "rust.option.none.sentinel";
pub const RUST_STDLIB_OPTION_AND_THEN_CONTRACT_ID: &str = "rust.option.and_then";
pub const RUST_STDLIB_RESULT_PACK_ID: &str = "nose.rust.stdlib.result";
pub const RUST_STDLIB_RESULT_PRODUCER_ID: &str = "rust.stdlib.result-api";
pub const RUST_STDLIB_RESULT_OK_CONTRACT_ID: &str = "rust.result.ok.constructor";
pub const RUST_STDLIB_RESULT_ERR_CONTRACT_ID: &str = "rust.result.err.constructor";
pub const RUST_STDLIB_RESULT_IS_OK_CONTRACT_ID: &str = "rust.result.is_ok";
pub const RUST_STDLIB_RESULT_IS_ERR_CONTRACT_ID: &str = "rust.result.is_err";
pub const RUST_STDLIB_INTEGER_METHOD_PACK_ID: &str = "nose.rust.stdlib.integer_methods";
pub const RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID: &str = "rust.stdlib.integer-method-api";
pub const ITERATOR_IDENTITY_ADAPTER_PACK_ID: &str = "nose.protocols.iterator_identity_adapters";
pub const ITERATOR_IDENTITY_ADAPTER_PRODUCER_ID: &str = "protocols.iterator-identity-adapter-api";
pub const ITERATOR_IDENTITY_ADAPTER_CONTRACT_ID: &str = "iterator.identity_adapter";
pub const RUST_STDLIB_COLLECTION_FACTORY_PACK_ID: &str = "nose.rust.stdlib.collection_factories";
pub const RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str = "rust.stdlib.collection-factory-api";
pub const RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_ID: &str = "rust.std.collection_factory";
pub const RUST_STDLIB_MAP_FACTORY_PACK_ID: &str = "nose.rust.stdlib.map_factories";
pub const RUST_STDLIB_MAP_FACTORY_PRODUCER_ID: &str = "rust.stdlib.map-factory-api";
pub const RUST_STDLIB_MAP_FACTORY_CONTRACT_ID: &str = "rust.std.map_factory";
pub const SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID: &str = "nose.swift.stdlib.collection_factories";
pub const SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str = "swift.stdlib.collection-factory-api";
pub const SWIFT_STDLIB_COLLECTION_FACTORY_ARRAY_CONTRACT_ID: &str =
    "swift.collection_factory.array";
pub const SWIFT_STDLIB_COLLECTION_FACTORY_SET_CONTRACT_ID: &str = "swift.collection_factory.set";
pub const SWIFT_STDLIB_DICTIONARY_UNIQUE_KEYS_CONTRACT_ID: &str =
    "swift.map_factory.dictionary_unique_keys_with_values";
pub const JAVA_STDLIB_MAP_FACTORY_PACK_ID: &str = "nose.java.stdlib.map_factories";
pub const JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID: &str = "java.stdlib.map-factory-api";
pub const JAVA_STDLIB_MAP_FACTORY_OF_CONTRACT_ID: &str = "java.map_factory.of";
pub const JAVA_STDLIB_MAP_FACTORY_OF_ENTRIES_CONTRACT_ID: &str = "java.map_factory.of_entries";
pub const JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_EMPTY_MAP_CONTRACT_ID: &str =
    "java.map_factory.collections_empty_map";
pub const JAVA_STDLIB_MAP_FACTORY_COLLECTIONS_SINGLETON_MAP_CONTRACT_ID: &str =
    "java.map_factory.collections_singleton_map";
pub const JAVA_STDLIB_MAP_ENTRY_PACK_ID: &str = "nose.java.stdlib.map_entries";
pub const JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID: &str = "java.stdlib.map-entry-api";
pub const JAVA_STDLIB_MAP_ENTRY_CONTRACT_ID: &str = "java.map_entry_factory";
pub const JAVA_STDLIB_COLLECTION_FACTORY_PACK_ID: &str = "nose.java.stdlib.collection_factories";
pub const JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID: &str = "java.stdlib.collection-factory-api";
pub const JAVA_STDLIB_COLLECTION_FACTORY_LIST_OF_CONTRACT_ID: &str =
    "java.collection_factory.list_of";
pub const JAVA_STDLIB_COLLECTION_FACTORY_SET_OF_CONTRACT_ID: &str =
    "java.collection_factory.set_of";
pub const JAVA_STDLIB_COLLECTION_FACTORY_ARRAYS_AS_LIST_CONTRACT_ID: &str =
    "java.collection_factory.arrays_as_list";
pub const JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_LIST_CONTRACT_ID: &str =
    "java.collection_factory.collections_empty_list";
pub const JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_EMPTY_SET_CONTRACT_ID: &str =
    "java.collection_factory.collections_empty_set";
pub const JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_CONTRACT_ID: &str =
    "java.collection_factory.collections_singleton";
pub const JAVA_STDLIB_COLLECTION_FACTORY_COLLECTIONS_SINGLETON_LIST_CONTRACT_ID: &str =
    "java.collection_factory.collections_singleton_list";
pub const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID: &str =
    "nose.java.ecosystem.guava.immutable_collection_factories";
pub const JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID: &str =
    "java.guava.immutable-collection-factory-api";
pub const JAVA_GUAVA_IMMUTABLE_LIST_OF_CONTRACT_ID: &str =
    "java.collection_factory.guava_immutable_list_of";
pub const JAVA_GUAVA_IMMUTABLE_SET_OF_CONTRACT_ID: &str =
    "java.collection_factory.guava_immutable_set_of";
pub const JAVA_GUAVA_IMMUTABLE_MAP_OF_CONTRACT_ID: &str = "java.map_factory.guava_immutable_map_of";
pub const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PACK_ID: &str =
    "nose.java.stdlib.collection_constructors";
pub const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_PRODUCER_ID: &str =
    "java.stdlib.collection-constructor-api";
pub const JAVA_STDLIB_COLLECTION_CONSTRUCTOR_EMPTY_LIST_CONTRACT_ID: &str =
    "java.collection_constructor.empty_list";
pub const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PACK_ID: &str =
    "nose.java.stdlib.static_collection_adapters";
pub const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID: &str =
    "java.stdlib.static-collection-adapter-api";
pub const JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_CONTRACT_ID: &str = "static.collection_adapter";
