use super::*;
use crate::falsify::domains::{
    domain_pool, domains_are_hosted, domains_are_hosted_with_projections, float_values,
    value_conforms,
};

#[test]
fn erased_static_width_and_payload_domains_fail_closed() {
    use DomainEvidence as D;
    for (lang, source_type, expected) in [
        (Lang::Rust, "x: u8", D::Integer),
        (Lang::Java, "byte x", D::Integer),
        (Lang::Swift, "x: UInt8", D::Integer),
        (Lang::Rust, "xs: Vec<i32>", D::Collection),
        (Lang::Rust, "x: Option<i32>", D::Option),
    ] {
        assert_eq!(
            nose_semantics::type_domain_from_source_text(lang, source_type),
            Some(expected)
        );
        assert!(!domains_are_hosted(lang, &[Some(expected)]));
    }
    for lang in [Lang::Rust, Lang::Java, Lang::Swift, Lang::Go, Lang::C] {
        for domain in [D::Integer, D::Float, D::Collection, D::Iterable, D::Option] {
            assert!(
                !domains_are_hosted(lang, &[Some(domain)]),
                "{lang:?} {domain:?} loses source constraints"
            );
        }
    }
    assert!(domains_are_hosted(
        Lang::Python,
        &[Some(D::Integer), Some(D::Float)]
    ));
    assert!(domains_are_hosted(Lang::Rust, &[Some(D::String)]));
    assert!(domains_are_hosted(Lang::Swift, &[Some(D::String)]));
    assert_eq!(
        nose_semantics::type_domain_from_source_text(Lang::Swift, "x: Character"),
        None
    );
    assert_eq!(
        nose_semantics::type_domain_from_source_text(Lang::Swift, "x: Substring"),
        None
    );
    assert!(domains_are_hosted(Lang::TypeScript, &[Some(D::Number)]));

    let (mut rust_a, interner, rust_a_root) = two_arg_binop(Op::BitAnd, (0, 1), Lang::Rust);
    let (mut rust_b, _, rust_b_root) = two_arg_binop(Op::BitAnd, (1, 0), Lang::Rust);
    set_param_domain(&mut rust_a, rust_a_root, D::Integer);
    set_param_domain(&mut rust_b, rust_b_root, D::Integer);
    assert!(falsify_pair(
        (&rust_a, rust_a_root),
        (&rust_b, rust_b_root),
        &interner,
        &[],
        64,
        DEFAULT_FALSIFY_SEED,
    )
    .is_none());
}

#[test]
fn cardinality_projection_hosts_only_sequence_length() {
    use nose_detect::OracleInputProjection::{Cardinality, Declared};
    use DomainEvidence as D;

    assert!(!domains_are_hosted_with_projections(
        Lang::Rust,
        &[Some(D::Collection)],
        &[Declared],
    ));
    for domain in [D::Array, D::Collection, D::Iterable] {
        assert!(domains_are_hosted_with_projections(
            Lang::Rust,
            &[Some(domain)],
            &[Cardinality],
        ));
    }
    for domain in [D::ByteArray, D::Map, D::Set, D::Option, D::Integer] {
        assert!(
            !domains_are_hosted_with_projections(Lang::Rust, &[Some(domain)], &[Cardinality],),
            "cardinality projection must not broaden {domain:?}"
        );
    }
    assert!(!domains_are_hosted_with_projections(
        Lang::Rust,
        &[Some(D::Collection)],
        &[],
    ));
}

#[test]
fn unused_inputs_must_form_a_trailing_suffix() {
    use nose_detect::OracleInputProjection::{Declared, UnusedTrailing};
    use DomainEvidence as D;

    assert!(domains_are_hosted_with_projections(
        Lang::Rust,
        &[Some(D::String), Some(D::Integer)],
        &[Declared, UnusedTrailing],
    ));
    assert!(!domains_are_hosted_with_projections(
        Lang::Rust,
        &[Some(D::Integer), Some(D::String)],
        &[UnusedTrailing, Declared],
    ));
}

#[test]
fn every_supported_domain_has_distinct_boundary_values() {
    use DomainEvidence as D;
    let domains = [D::Integer, D::Float, D::Number, D::Boolean, D::String];
    for domain in domains {
        let pool = domain_pool(Some(domain), &[]);
        assert!(pool.len() >= 2, "{domain:?} domain is under-sampled");
        assert!(pool.iter().all(|value| value_conforms(value, Some(domain))));
    }
    let float_receipts: Vec<String> = float_values().iter().map(format_value).collect();
    assert!(float_receipts.contains(&"float:0e0".to_string()));
    assert!(float_receipts.contains(&"float:-0".to_string()));
    assert!(float_receipts.contains(&"float:nan".to_string()));

    for domain in [
        D::Array,
        D::ByteArray,
        D::Collection,
        D::FutureLike,
        D::Iterable,
        D::Iterator,
        D::Map,
        D::Option,
        D::PromiseLike,
        D::Record,
        D::Result,
        D::Set,
    ] {
        assert!(domain_pool(Some(domain), &[]).is_empty());
    }
}
