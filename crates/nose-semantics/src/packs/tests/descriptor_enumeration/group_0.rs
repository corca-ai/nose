use super::*;

pub(super) fn assert_group() {
    assert_source_fact_language_descriptor(
        PYTHON_LANGUAGE_PACK_ID,
        &["python"],
        &[nose_il::Lang::Python],
        &["py", "pyi"],
        "tree-sitter-python",
        "nose_frontend::python::lower",
        PYTHON_LANGUAGE_CORE_PRODUCER_ID,
        PYTHON_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        JS_TS_LANGUAGE_PACK_ID,
        &["javascript", "typescript"],
        &[nose_il::Lang::JavaScript, nose_il::Lang::TypeScript],
        &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"],
        "tree-sitter-javascript/tree-sitter-typescript",
        "nose_frontend::js_ts::lower",
        JS_TS_LANGUAGE_CORE_PRODUCER_ID,
        JS_TS_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        GO_LANGUAGE_PACK_ID,
        &["go"],
        &[nose_il::Lang::Go],
        &["go"],
        "tree-sitter-go",
        "nose_frontend::go::lower",
        GO_LANGUAGE_CORE_PRODUCER_ID,
        GO_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        RUST_LANGUAGE_PACK_ID,
        &["rust"],
        &[nose_il::Lang::Rust],
        &["rs"],
        "tree-sitter-rust",
        "nose_frontend::rust::lower",
        RUST_LANGUAGE_CORE_PRODUCER_ID,
        RUST_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        JAVA_LANGUAGE_PACK_ID,
        &["java"],
        &[nose_il::Lang::Java],
        &["java"],
        "tree-sitter-java",
        "nose_frontend::java::lower",
        JAVA_LANGUAGE_CORE_PRODUCER_ID,
        JAVA_SOURCE_FACT_PRODUCER_ID,
    );
    let c = builtin_pack_descriptor(C_LANGUAGE_PACK_ID).expect("C language descriptor");
    assert_eq!(c.kind, SemanticPackKind::LanguagePack);
    assert_eq!(c.supported_languages, &["c"]);
    assert!(c.supported_packages.is_empty());
    let language = c
        .language
        .expect("C descriptor should expose language binding");
    assert_eq!(language.langs, &[nose_il::Lang::C]);
    assert_eq!(language.file_extensions, &["c", "h"]);
    assert_eq!(language.parser, "tree-sitter-c");
    assert_eq!(language.lowering_entrypoint, "nose_frontend::c::lower");
    assert_eq!(
        c.evidence_producer_ids,
        &[
            C_LANGUAGE_CORE_PRODUCER_ID,
            C_SOURCE_FACT_PRODUCER_ID,
            C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID
        ]
    );
    assert_eq!(
        c.source_fact_producer_ids,
        &[
            C_SOURCE_FACT_PRODUCER_ID,
            C_UNSIGNED_32_CAST_SOURCE_PRODUCER_ID
        ]
    );
    assert_eq!(c.counts().evidence_producers, 3);
    assert_eq!(c.counts().contracts, 0);
    assert_eq!(c.counts().positive_fixtures, 2);
    assert_eq!(c.counts().hard_negatives, 2);
    assert!(c
        .conformance_refs()
        .contains(&"c-unsigned32-signed-cast-hard-negative"));
    assert_source_fact_language_descriptor(
        RUBY_LANGUAGE_PACK_ID,
        &["ruby"],
        &[nose_il::Lang::Ruby],
        &["rb"],
        "tree-sitter-ruby",
        "nose_frontend::ruby::lower",
        RUBY_LANGUAGE_CORE_PRODUCER_ID,
        RUBY_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        SWIFT_LANGUAGE_PACK_ID,
        &["swift"],
        &[nose_il::Lang::Swift],
        &["swift"],
        "tree-sitter-swift",
        "nose_frontend::swift::lower",
        SWIFT_LANGUAGE_CORE_PRODUCER_ID,
        SWIFT_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        CSS_LANGUAGE_PACK_ID,
        &["css"],
        &[nose_il::Lang::Css],
        &["css"],
        "tree-sitter-css",
        "nose_frontend::css::lower",
        CSS_LANGUAGE_CORE_PRODUCER_ID,
        CSS_SOURCE_FACT_PRODUCER_ID,
    );
    assert_source_fact_language_descriptor(
        HTML_EMBEDDED_LANGUAGE_PACK_ID,
        &["html", "vue", "svelte"],
        &[
            nose_il::Lang::Html,
            nose_il::Lang::Vue,
            nose_il::Lang::Svelte,
        ],
        &["html", "htm", "vue", "svelte"],
        "tree-sitter-html + embedded JS/TS/CSS extraction",
        "nose_frontend::embedded::lower_regions",
        HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
        HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
    );

    let python_builtins = builtin_pack_descriptor(PYTHON_BUILTIN_COLLECTION_FACTORY_PACK_ID)
        .expect("Python builtins descriptor");
    assert_eq!(python_builtins.kind, SemanticPackKind::StdlibPack);
    assert_eq!(python_builtins.supported_languages, &["python"]);
    assert_eq!(python_builtins.supported_packages, &["builtins"]);
    assert_eq!(
        python_builtins.evidence_producer_ids,
        &[PYTHON_BUILTIN_COLLECTION_FACTORY_PRODUCER_ID]
    );
    assert!(python_builtins.source_fact_producer_ids.is_empty());
    assert_eq!(
        python_builtins.contract_ids,
        &[PYTHON_BUILTIN_COLLECTION_FACTORY_CONTRACT_ID]
    );
    assert_eq!(python_builtins.counts().evidence_producers, 1);
    assert_eq!(python_builtins.counts().contracts, 1);
    assert_eq!(python_builtins.counts().positive_fixtures, 4);
    assert_eq!(python_builtins.counts().hard_negatives, 2);
    assert!(python_builtins
        .conformance_refs()
        .contains(&"python-builtin-list-wildcard-import-hard-negative"));

    let python_stdlib_collections =
        builtin_pack_descriptor(PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID)
            .expect("Python stdlib collection factory descriptor");
    assert_eq!(python_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(python_stdlib_collections.supported_languages, &["python"]);
    assert_eq!(
        python_stdlib_collections.supported_packages,
        &["collections"]
    );
    assert_eq!(
        python_stdlib_collections.evidence_producer_ids,
        &[PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID]
    );
    assert!(python_stdlib_collections
        .source_fact_producer_ids
        .is_empty());
    assert_eq!(
        python_stdlib_collections.contract_ids,
        &[PYTHON_STDLIB_COLLECTION_FACTORY_CONTRACT_ID]
    );
    assert_eq!(python_stdlib_collections.counts().evidence_producers, 1);
    assert_eq!(python_stdlib_collections.counts().contracts, 1);
    assert_eq!(python_stdlib_collections.counts().positive_fixtures, 3);
    assert_eq!(python_stdlib_collections.counts().hard_negatives, 2);
    assert!(python_stdlib_collections
        .conformance_refs()
        .contains(&"python-collections-deque-wrong-module-hard-negative"));

    let python_stdlib_math =
        builtin_pack_descriptor(PYTHON_STDLIB_MATH_PACK_ID).expect("Python stdlib math descriptor");
    assert_eq!(python_stdlib_math.kind, SemanticPackKind::StdlibPack);
    assert_eq!(python_stdlib_math.supported_languages, &["python"]);
    assert_eq!(python_stdlib_math.supported_packages, &["math"]);
    assert_eq!(
        python_stdlib_math.evidence_producer_ids,
        &[PYTHON_STDLIB_MATH_PRODUCER_ID]
    );
    assert!(python_stdlib_math.source_fact_producer_ids.is_empty());
    assert_eq!(
        python_stdlib_math.contract_ids,
        &[PYTHON_STDLIB_MATH_PROD_CONTRACT_ID]
    );
    assert_eq!(python_stdlib_math.counts().evidence_producers, 1);
    assert_eq!(python_stdlib_math.counts().contracts, 1);
    assert_eq!(python_stdlib_math.counts().positive_fixtures, 1);
    assert_eq!(python_stdlib_math.counts().hard_negatives, 2);
    assert!(python_stdlib_math
        .conformance_refs()
        .contains(&"python-math-prod-wrong-namespace-hard-negative"));

    let ruby_set =
        builtin_pack_descriptor(RUBY_STDLIB_SET_PACK_ID).expect("Ruby stdlib Set descriptor");
    assert_eq!(ruby_set.kind, SemanticPackKind::StdlibPack);
    assert_eq!(ruby_set.supported_languages, &["ruby"]);
    assert_eq!(ruby_set.supported_packages, &["set"]);
    assert_eq!(
        ruby_set.evidence_producer_ids,
        &[RUBY_STDLIB_SET_PRODUCER_ID]
    );
    assert!(ruby_set.source_fact_producer_ids.is_empty());
    assert_eq!(ruby_set.contract_ids, &[RUBY_STDLIB_SET_CONTRACT_ID]);
    assert_eq!(ruby_set.counts().evidence_producers, 1);
    assert_eq!(ruby_set.counts().contracts, 1);
    assert_eq!(ruby_set.counts().positive_fixtures, 3);
    assert_eq!(ruby_set.counts().hard_negatives, 3);
    assert!(ruby_set
        .conformance_refs()
        .contains(&"ruby-set-missing-require-hard-negative"));

    let rust_vec =
        builtin_pack_descriptor(RUST_STDLIB_VEC_PACK_ID).expect("Rust stdlib Vec descriptor");
    assert_eq!(rust_vec.kind, SemanticPackKind::StdlibPack);
    assert_eq!(rust_vec.supported_languages, &["rust"]);
    assert_eq!(rust_vec.supported_packages, &["std::vec", "alloc::vec"]);
    assert_eq!(
        rust_vec.evidence_producer_ids,
        &[RUST_STDLIB_VEC_PRODUCER_ID]
    );
    assert!(rust_vec.source_fact_producer_ids.is_empty());
    assert_eq!(
        rust_vec.contract_ids,
        &[
            RUST_STDLIB_VEC_MACRO_CONTRACT_ID,
            RUST_STDLIB_VEC_NEW_CONTRACT_ID
        ]
    );
    assert_eq!(rust_vec.counts().evidence_producers, 1);
    assert_eq!(rust_vec.counts().contracts, 2);
    assert_eq!(rust_vec.counts().positive_fixtures, 2);
    assert_eq!(rust_vec.counts().hard_negatives, 2);
    assert!(rust_vec
        .conformance_refs()
        .contains(&"rust-vec-new-shadowed-hard-negative"));

    let rust_option =
        builtin_pack_descriptor(RUST_STDLIB_OPTION_PACK_ID).expect("Rust stdlib Option descriptor");
    assert_eq!(rust_option.kind, SemanticPackKind::StdlibPack);
    assert_eq!(rust_option.supported_languages, &["rust"]);
    assert_eq!(
        rust_option.supported_packages,
        &["std::option", "core::option"]
    );
    assert_eq!(
        rust_option.evidence_producer_ids,
        &[RUST_STDLIB_OPTION_PRODUCER_ID]
    );
    assert!(rust_option.source_fact_producer_ids.is_empty());
    assert_eq!(
        rust_option.contract_ids,
        &[
            RUST_STDLIB_OPTION_SOME_CONTRACT_ID,
            RUST_STDLIB_OPTION_NONE_CONTRACT_ID,
            RUST_STDLIB_OPTION_AND_THEN_CONTRACT_ID,
        ]
    );
    assert_eq!(rust_option.counts().evidence_producers, 1);
    assert_eq!(rust_option.counts().contracts, 3);
    assert_eq!(rust_option.counts().positive_fixtures, 3);
    assert_eq!(rust_option.counts().hard_negatives, 3);
    assert!(rust_option
        .conformance_refs()
        .contains(&"rust-option-and-then-non-option-hard-negative"));

    let rust_integer_methods = builtin_pack_descriptor(RUST_STDLIB_INTEGER_METHOD_PACK_ID)
        .expect("Rust stdlib integer method descriptor");
    assert_eq!(rust_integer_methods.kind, SemanticPackKind::StdlibPack);
    assert_eq!(rust_integer_methods.supported_languages, &["rust"]);
    assert_eq!(
        rust_integer_methods.supported_packages,
        &["core::primitive"]
    );
    assert_eq!(
        rust_integer_methods.evidence_producer_ids,
        &[RUST_STDLIB_INTEGER_METHOD_PRODUCER_ID]
    );
    assert!(rust_integer_methods.source_fact_producer_ids.is_empty());
    assert_eq!(
        rust_integer_methods.contract_ids,
        &[
            SCALAR_INTEGER_METHOD_ABS_CONTRACT_ID,
            SCALAR_INTEGER_METHOD_MIN_CONTRACT_ID,
            SCALAR_INTEGER_METHOD_MAX_CONTRACT_ID,
            SCALAR_INTEGER_METHOD_CLAMP_CONTRACT_ID,
        ]
    );
    assert_eq!(rust_integer_methods.counts().evidence_producers, 1);
    assert_eq!(rust_integer_methods.counts().contracts, 4);
    assert_eq!(rust_integer_methods.counts().positive_fixtures, 4);
    assert_eq!(rust_integer_methods.counts().hard_negatives, 2);
    assert!(rust_integer_methods
        .conformance_refs()
        .contains(&"rust-integer-method-non-integer-receiver-hard-negative"));

    let rust_stdlib_collections = builtin_pack_descriptor(RUST_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Rust stdlib collection factory descriptor");
    assert_eq!(rust_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(rust_stdlib_collections.supported_languages, &["rust"]);
    assert_eq!(
        rust_stdlib_collections.supported_packages,
        &["std::collections"]
    );
    assert_eq!(
        rust_stdlib_collections.evidence_producer_ids,
        &[RUST_STDLIB_COLLECTION_FACTORY_PRODUCER_ID]
    );
    assert!(rust_stdlib_collections.source_fact_producer_ids.is_empty());
    assert_eq!(
        rust_stdlib_collections.contract_ids,
        &[RUST_STDLIB_COLLECTION_FACTORY_CONTRACT_ID]
    );
    assert_eq!(rust_stdlib_collections.counts().evidence_producers, 1);
    assert_eq!(rust_stdlib_collections.counts().contracts, 1);
    assert_eq!(rust_stdlib_collections.counts().positive_fixtures, 3);
    assert_eq!(rust_stdlib_collections.counts().hard_negatives, 2);
    assert!(rust_stdlib_collections
        .conformance_refs()
        .contains(&"rust-std-collections-shadowed-std-hard-negative"));

    let rust_stdlib_maps = builtin_pack_descriptor(RUST_STDLIB_MAP_FACTORY_PACK_ID)
        .expect("Rust stdlib map factory descriptor");
    assert_eq!(rust_stdlib_maps.kind, SemanticPackKind::StdlibPack);
    assert_eq!(rust_stdlib_maps.supported_languages, &["rust"]);
    assert_eq!(rust_stdlib_maps.supported_packages, &["std::collections"]);
    assert_eq!(
        rust_stdlib_maps.evidence_producer_ids,
        &[RUST_STDLIB_MAP_FACTORY_PRODUCER_ID]
    );
    assert!(rust_stdlib_maps.source_fact_producer_ids.is_empty());
    assert_eq!(
        rust_stdlib_maps.contract_ids,
        &[RUST_STDLIB_MAP_FACTORY_CONTRACT_ID]
    );
    assert_eq!(rust_stdlib_maps.counts().evidence_producers, 1);
    assert_eq!(rust_stdlib_maps.counts().contracts, 1);
    assert_eq!(rust_stdlib_maps.counts().positive_fixtures, 2);
    assert_eq!(rust_stdlib_maps.counts().hard_negatives, 2);
    assert!(rust_stdlib_maps
        .conformance_refs()
        .contains(&"rust-std-map-shadowed-std-hard-negative"));

    let swift_stdlib_collections = builtin_pack_descriptor(SWIFT_STDLIB_COLLECTION_FACTORY_PACK_ID)
        .expect("Swift stdlib collection factory descriptor");
    assert_eq!(swift_stdlib_collections.kind, SemanticPackKind::StdlibPack);
    assert_eq!(swift_stdlib_collections.supported_languages, &["swift"]);
    assert_eq!(
        swift_stdlib_collections.supported_packages,
        &["Array", "Set", "Dictionary", "Swift"]
    );
    assert_eq!(
        swift_stdlib_collections.evidence_producer_ids,
        &[SWIFT_STDLIB_COLLECTION_FACTORY_PRODUCER_ID]
    );
    assert!(swift_stdlib_collections.source_fact_producer_ids.is_empty());
    assert_eq!(
        swift_stdlib_collections.contract_ids,
        &[
            SWIFT_STDLIB_COLLECTION_FACTORY_ARRAY_CONTRACT_ID,
            SWIFT_STDLIB_COLLECTION_FACTORY_SET_CONTRACT_ID,
            SWIFT_STDLIB_DICTIONARY_UNIQUE_KEYS_CONTRACT_ID,
        ]
    );
    assert_eq!(swift_stdlib_collections.counts().evidence_producers, 1);
    assert_eq!(swift_stdlib_collections.counts().contracts, 3);
    assert_eq!(swift_stdlib_collections.counts().positive_fixtures, 3);
    assert_eq!(swift_stdlib_collections.counts().hard_negatives, 4);
    assert!(swift_stdlib_collections
        .conformance_refs()
        .contains(&"swift-dictionary-implicit-entry-shape-hard-negative"));
}
