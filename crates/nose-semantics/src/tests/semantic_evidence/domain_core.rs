use super::*;

#[test]
fn value_domain_inference_treats_retained_float_literals_as_numeric() {
    assert_eq!(
        inferred_domains_for_added_literal(Payload::LitInt(3)),
        vec![ValueDomain::Number]
    );
    assert_eq!(
        inferred_domains_for_added_literal(Payload::LitFloat(0xBEEF)),
        vec![ValueDomain::Number]
    );
}

#[test]
fn value_domain_inference_does_not_treat_python_repetition_as_numeric_proof() {
    assert_eq!(
        inferred_domains_for_multiplied_literal(Lang::Python),
        vec![ValueDomain::Unknown],
        "Python `*` can be string/list repetition, so untyped operands must fail closed"
    );
    assert_eq!(
        inferred_domains_for_multiplied_literal(Lang::Ruby),
        vec![ValueDomain::Unknown],
        "Ruby `*` can be repetition, so untyped operands must fail closed"
    );
    assert_eq!(
        inferred_domains_for_multiplied_literal(Lang::Rust),
        vec![ValueDomain::Number],
        "Rust `*` remains a strict numeric operator for the builtin language profile"
    );
}

fn inferred_domains_for_multiplied_literal(lang: Lang) -> Vec<ValueDomain> {
    let sp = Span::synthetic(FileId(0));
    let mut b = IlBuilder::new(FileId(0));
    let param = b.add(NodeKind::Param, Payload::Cid(0), sp, &[]);
    let varx = b.add(NodeKind::Var, Payload::Cid(0), sp, &[]);
    let lit = b.add(NodeKind::Lit, Payload::LitInt(2), sp, &[]);
    let mul = b.add(NodeKind::BinOp, Payload::Op(Op::Mul), sp, &[varx, lit]);
    let ret = b.add(NodeKind::Return, Payload::None, sp, &[mul]);
    let func = b.add(NodeKind::Func, Payload::None, sp, &[param, ret]);
    let il = b.finish(
        func,
        FileMeta {
            path: "t".into(),
            lang,
        },
        Vec::new(),
        Vec::new(),
    );
    semantics(lang)
        .operators()
        .infer_param_value_domains(&il, func)
}

#[test]
fn first_party_profile_wraps_each_language() {
    for &lang in ALL_LANGS {
        let profile = semantics(lang);
        assert_eq!(profile.lang(), lang);
        assert_eq!(profile.pack_id(), builtin_language_pack_id(lang));
        assert_eq!(profile.trust(), PackTrust::BuiltinDefault);
    }
}

#[test]
fn language_core_evidence_provenance_covers_each_language() {
    let expected = [
        (
            Lang::Python,
            PYTHON_LANGUAGE_PACK_ID,
            PYTHON_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::JavaScript,
            JS_TS_LANGUAGE_PACK_ID,
            JS_TS_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::TypeScript,
            JS_TS_LANGUAGE_PACK_ID,
            JS_TS_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (Lang::Go, GO_LANGUAGE_PACK_ID, GO_LANGUAGE_CORE_PRODUCER_ID),
        (
            Lang::Rust,
            RUST_LANGUAGE_PACK_ID,
            RUST_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::Java,
            JAVA_LANGUAGE_PACK_ID,
            JAVA_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (Lang::C, C_LANGUAGE_PACK_ID, C_LANGUAGE_CORE_PRODUCER_ID),
        (
            Lang::Ruby,
            RUBY_LANGUAGE_PACK_ID,
            RUBY_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::Swift,
            SWIFT_LANGUAGE_PACK_ID,
            SWIFT_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::Css,
            CSS_LANGUAGE_PACK_ID,
            CSS_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::Vue,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::Svelte,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
        ),
        (
            Lang::Html,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_LANGUAGE_CORE_PRODUCER_ID,
        ),
    ];
    for (lang, pack_id, producer_id) in expected {
        assert_eq!(
            language_core_evidence_provenance(lang),
            (pack_id, producer_id)
        );
    }
}

#[test]
fn language_source_fact_provenance_covers_each_language() {
    let expected = [
        (
            Lang::Python,
            PYTHON_LANGUAGE_PACK_ID,
            PYTHON_SOURCE_FACT_PRODUCER_ID,
        ),
        (
            Lang::JavaScript,
            JS_TS_LANGUAGE_PACK_ID,
            JS_TS_SOURCE_FACT_PRODUCER_ID,
        ),
        (
            Lang::TypeScript,
            JS_TS_LANGUAGE_PACK_ID,
            JS_TS_SOURCE_FACT_PRODUCER_ID,
        ),
        (Lang::Go, GO_LANGUAGE_PACK_ID, GO_SOURCE_FACT_PRODUCER_ID),
        (
            Lang::Rust,
            RUST_LANGUAGE_PACK_ID,
            RUST_SOURCE_FACT_PRODUCER_ID,
        ),
        (
            Lang::Java,
            JAVA_LANGUAGE_PACK_ID,
            JAVA_SOURCE_FACT_PRODUCER_ID,
        ),
        (Lang::C, C_LANGUAGE_PACK_ID, C_SOURCE_FACT_PRODUCER_ID),
        (
            Lang::Ruby,
            RUBY_LANGUAGE_PACK_ID,
            RUBY_SOURCE_FACT_PRODUCER_ID,
        ),
        (
            Lang::Swift,
            SWIFT_LANGUAGE_PACK_ID,
            SWIFT_SOURCE_FACT_PRODUCER_ID,
        ),
        (Lang::Css, CSS_LANGUAGE_PACK_ID, CSS_SOURCE_FACT_PRODUCER_ID),
        (
            Lang::Vue,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
        ),
        (
            Lang::Svelte,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
        ),
        (
            Lang::Html,
            HTML_EMBEDDED_LANGUAGE_PACK_ID,
            HTML_EMBEDDED_SOURCE_FACT_PRODUCER_ID,
        ),
    ];

    for (lang, pack_id, producer_id) in expected {
        assert_eq!(
            language_source_fact_provenance(lang),
            (pack_id, producer_id)
        );
    }
}

#[test]
fn domain_evidence_preserves_param_semantic_boundaries() {
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Array),
        DomainEvidence::Array
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Boolean),
        DomainEvidence::Boolean
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Float),
        DomainEvidence::Float
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::FutureLike),
        DomainEvidence::FutureLike
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Iterable),
        DomainEvidence::Iterable
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Iterator),
        DomainEvidence::Iterator
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Record),
        DomainEvidence::Record
    );
    assert_eq!(
        domain_evidence_from_param_semantic(ParamSemantic::Result),
        DomainEvidence::Result
    );
}

#[test]
fn domain_evidence_predicates_match_domain_families() {
    assert!(DomainEvidence::Array.is_array_collection_or_set());
    assert!(DomainEvidence::Boolean.is_boolean());
    assert!(DomainEvidence::Collection.is_array_or_collection());
    assert!(DomainEvidence::Set.is_collection_or_set());
    assert!(DomainEvidence::Float.is_float());
    assert!(DomainEvidence::FutureLike.is_future_like());
    assert!(DomainEvidence::PromiseLike.is_future_like());
    assert!(DomainEvidence::Iterable.is_iterable_or_iterator());
    assert!(DomainEvidence::Iterator.is_iterable_or_iterator());
    assert!(DomainEvidence::Map.is_map());
    assert!(DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("pkg.Widget")
    }
    .is_nominal(stable_symbol_hash("pkg.Widget")));
    assert!(DomainEvidence::Option.is_option());
    assert!(DomainEvidence::PromiseLike.is_promise_like());
    assert!(DomainEvidence::Record.is_record());
    assert!(DomainEvidence::Result.is_result());
}

#[test]
fn scalar_domain_evidence_predicates_keep_numeric_axes_separate() {
    assert!(DomainEvidence::String.is_string());
    assert!(DomainEvidence::ByteArray.is_byte_array());
    assert!(DomainEvidence::Integer.is_integer());
    assert!(DomainEvidence::Number.is_integer_or_number());
    assert!(DomainEvidence::Float.is_integer_or_number());
    assert!(DomainEvidence::Integer.is_integer_or_number());
    assert!(!DomainEvidence::Number.is_integer());
    assert!(!DomainEvidence::Array.is_collection_or_set());
    assert!(!DomainEvidence::Set.is_array_or_collection());
}

#[test]
fn domain_requirements_accept_matching_evidence() {
    assert!(DomainRequirement::BOOLEAN.accepts(DomainEvidence::Boolean));
    assert!(DomainRequirement::COLLECTION_OR_MAP.accepts(DomainEvidence::Array));
    assert!(DomainRequirement::COLLECTION_OR_MAP.accepts(DomainEvidence::Map));
    assert!(DomainRequirement::FLOAT.accepts(DomainEvidence::Float));
    assert!(DomainRequirement::FUTURE_LIKE.accepts(DomainEvidence::FutureLike));
    assert!(DomainRequirement::FUTURE_LIKE.accepts(DomainEvidence::PromiseLike));
    assert!(DomainRequirement::ITERABLE.accepts(DomainEvidence::Iterable));
    assert!(DomainRequirement::ITERATOR.accepts(DomainEvidence::Iterator));
    assert!(DomainRequirement::ITERABLE_OR_ITERATOR.accepts(DomainEvidence::Iterable));
    assert!(DomainRequirement::ITERABLE_OR_ITERATOR.accepts(DomainEvidence::Iterator));
    assert!(DomainRequirement::exact(DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("pkg.Widget")
    })
    .accepts(DomainEvidence::Nominal {
        type_hash: stable_symbol_hash("pkg.Widget")
    }));
}

#[test]
fn domain_requirements_reject_mismatches_and_map_value_domains() {
    assert!(DomainRequirement::NUMBER.accepts(DomainEvidence::Float));
    assert!(DomainRequirement::RECORD.accepts(DomainEvidence::Record));
    assert!(DomainRequirement::RESULT.accepts(DomainEvidence::Result));
    assert!(DomainRequirement::SET_OR_MAP.accepts(DomainEvidence::Set));
    assert!(DomainRequirement::SET_OR_MAP.accepts(DomainEvidence::Map));
    assert!(!DomainRequirement::SET_OR_MAP.accepts(DomainEvidence::Collection));
    assert!(DomainRequirement::PROMISE_LIKE.accepts(DomainEvidence::PromiseLike));
    assert!(!DomainRequirement::PROMISE_LIKE.accepts(DomainEvidence::String));
    assert_eq!(
        ValueDomain::from_domain_evidence(DomainEvidence::Boolean),
        Some(ValueDomain::Boolean)
    );
    assert_eq!(
        ValueDomain::from_domain_evidence(DomainEvidence::Float),
        Some(ValueDomain::Number)
    );
    assert_eq!(
        ValueDomain::from_domain_evidence(DomainEvidence::PromiseLike),
        None
    );
}

#[test]
fn domain_requirements_compose_pack_boundaries_without_new_variants() {
    const ARRAY_SCALAR_BOUNDARY: DomainRequirement =
        DomainRequirement::any_of(&[DomainEvidence::Array, DomainEvidence::Integer]);

    assert!(ARRAY_SCALAR_BOUNDARY.accepts(DomainEvidence::Array));
    assert!(ARRAY_SCALAR_BOUNDARY.accepts(DomainEvidence::Integer));
    assert!(!ARRAY_SCALAR_BOUNDARY.accepts(DomainEvidence::Map));
}

#[test]
fn named_domain_requirement_aliases_match_their_domain_sets() {
    const ALL_DOMAINS: &[DomainEvidence] = &[
        DomainEvidence::Array,
        DomainEvidence::Boolean,
        DomainEvidence::ByteArray,
        DomainEvidence::Collection,
        DomainEvidence::Float,
        DomainEvidence::FutureLike,
        DomainEvidence::Integer,
        DomainEvidence::Iterable,
        DomainEvidence::Iterator,
        DomainEvidence::Map,
        DomainEvidence::Number,
        DomainEvidence::Option,
        DomainEvidence::PromiseLike,
        DomainEvidence::Record,
        DomainEvidence::Result,
        DomainEvidence::Set,
        DomainEvidence::String,
    ];
    const CASES: &[(DomainRequirement, &[DomainEvidence])] = &[
        (DomainRequirement::ARRAY, &[DomainEvidence::Array]),
        (DomainRequirement::BOOLEAN, &[DomainEvidence::Boolean]),
        (DomainRequirement::BYTE_ARRAY, &[DomainEvidence::ByteArray]),
        (DomainRequirement::COLLECTION, &[DomainEvidence::Collection]),
        (
            DomainRequirement::COLLECTION_OR_SET,
            &[DomainEvidence::Collection, DomainEvidence::Set],
        ),
        (
            DomainRequirement::COLLECTION_OR_MAP,
            &[
                DomainEvidence::Array,
                DomainEvidence::Collection,
                DomainEvidence::Set,
                DomainEvidence::Map,
            ],
        ),
        (DomainRequirement::FLOAT, &[DomainEvidence::Float]),
        (
            DomainRequirement::FUTURE_LIKE,
            &[DomainEvidence::FutureLike, DomainEvidence::PromiseLike],
        ),
        (
            DomainRequirement::ARRAY_OR_COLLECTION,
            &[DomainEvidence::Array, DomainEvidence::Collection],
        ),
        (
            DomainRequirement::ARRAY_COLLECTION_OR_SET,
            &[
                DomainEvidence::Array,
                DomainEvidence::Collection,
                DomainEvidence::Set,
            ],
        ),
        (DomainRequirement::ITERABLE, &[DomainEvidence::Iterable]),
        (
            DomainRequirement::ITERABLE_OR_ITERATOR,
            &[DomainEvidence::Iterable, DomainEvidence::Iterator],
        ),
        (DomainRequirement::ITERATOR, &[DomainEvidence::Iterator]),
        (DomainRequirement::SET, &[DomainEvidence::Set]),
        (
            DomainRequirement::SET_OR_MAP,
            &[DomainEvidence::Set, DomainEvidence::Map],
        ),
        (DomainRequirement::MAP, &[DomainEvidence::Map]),
        (
            DomainRequirement::NUMBER,
            &[DomainEvidence::Number, DomainEvidence::Float],
        ),
        (DomainRequirement::OPTION, &[DomainEvidence::Option]),
        (
            DomainRequirement::PROMISE_LIKE,
            &[DomainEvidence::PromiseLike],
        ),
        (DomainRequirement::RECORD, &[DomainEvidence::Record]),
        (DomainRequirement::RESULT, &[DomainEvidence::Result]),
        (DomainRequirement::STRING, &[DomainEvidence::String]),
        (DomainRequirement::INTEGER, &[DomainEvidence::Integer]),
        (
            DomainRequirement::INTEGER_OR_NUMBER,
            &[
                DomainEvidence::Integer,
                DomainEvidence::Float,
                DomainEvidence::Number,
            ],
        ),
    ];

    for &(requirement, accepted) in CASES {
        for &domain in ALL_DOMAINS {
            assert_eq!(
                requirement.accepts(domain),
                accepted.contains(&domain),
                "{requirement:?} acceptance drifted for {domain:?}"
            );
        }
    }
}
