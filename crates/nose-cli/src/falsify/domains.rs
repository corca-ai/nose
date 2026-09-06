use nose_il::{DomainEvidence, Il, Lang, NodeId, NodeKind, Payload};
use nose_normalize::{Value, F64};

pub(super) fn parameter_domains(il: &Il, root: NodeId) -> Vec<Option<DomainEvidence>> {
    il.children(root)
        .iter()
        .filter(|&&child| {
            il.kind(child) == NodeKind::Param && matches!(il.node(child).payload, Payload::Cid(_))
        })
        .map(|&param| nose_semantics::domain_evidence_for_param(il, param))
        .collect()
}

/// Validate a projection vector and return the inputs that the oracle may actually vary.
/// `UnusedTrailing` is deliberately suffix-only: accepting a hole in the middle would change
/// positional argument binding rather than erase an unobserved declaration suffix.
pub(crate) fn effective_domain_contract<'a>(
    domains: &'a [Option<DomainEvidence>],
    projections: &'a [nose_detect::OracleInputProjection],
) -> Option<(
    &'a [Option<DomainEvidence>],
    &'a [nose_detect::OracleInputProjection],
)> {
    if domains.len() != projections.len() {
        return None;
    }
    let active = projections
        .iter()
        .position(|projection| *projection == nose_detect::OracleInputProjection::UnusedTrailing)
        .unwrap_or(projections.len());
    projections[active..]
        .iter()
        .all(|projection| *projection == nose_detect::OracleInputProjection::UnusedTrailing)
        .then_some((&domains[..active], &projections[..active]))
}

fn integer_values() -> Vec<Value> {
    vec![
        Value::Int(0),
        Value::Int(1),
        Value::Int(-1),
        Value::Int(i32::MAX as i64),
        Value::Int(i32::MIN as i64),
        Value::Int(0x1_0000_0000),
        Value::Int(-0x1_0000_0001),
        Value::Int(0xF_0000_0003),
        Value::Int(0xF_0000_0005),
    ]
}

pub(super) fn float_values() -> Vec<Value> {
    vec![
        Value::Float(F64(0.0)),
        Value::Float(F64(-0.0)),
        Value::Float(F64(1.0)),
        Value::Float(F64(-1.0)),
        Value::Float(F64(4_503_599_627_370_495.5)),
        Value::Float(F64(1e16)),
        Value::Float(F64(-1e16)),
        Value::Float(F64(f64::NAN)),
        Value::Float(F64(f64::INFINITY)),
        Value::Float(F64(f64::NEG_INFINITY)),
    ]
}

fn string_values() -> Vec<Value> {
    vec![
        Value::Str(Vec::new()),
        Value::Str(vec![0x5EED_0001]),
        Value::Str(vec![0x5EED_0002]),
        Value::Str(vec![0x5EED_0001, 0x5EED_0001]),
    ]
}

fn collection_values() -> Vec<Value> {
    vec![
        Value::List(Vec::new()),
        Value::List(vec![Value::Int(0)]),
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::List(vec![Value::Int(2), Value::Int(1)]),
        Value::List(vec![Value::Int(1), Value::Int(1)]),
        Value::List(vec![Value::Str(vec![0x5EED_0001])]),
    ]
}

fn push_unique(values: &mut Vec<Value>, value: Value) {
    if !values.contains(&value) {
        values.push(value);
    }
}

pub(super) fn domain_pool(domain: Option<DomainEvidence>, probes: &[Value]) -> Vec<Value> {
    use DomainEvidence as D;
    let mut values = match domain {
        Some(D::Integer) => integer_values(),
        Some(D::Float) => float_values(),
        Some(D::Number) => {
            let mut values = float_values();
            for integer in integer_values() {
                let Value::Int(integer) = integer else {
                    unreachable!("integer pool contains only integers")
                };
                push_unique(&mut values, Value::Float(F64(integer as f64)));
            }
            values
        }
        Some(D::Boolean) => vec![Value::Bool(false), Value::Bool(true)],
        Some(D::String) => string_values(),
        Some(
            D::Array
            | D::ByteArray
            | D::Collection
            | D::FutureLike
            | D::Iterable
            | D::Iterator
            | D::Map
            | D::Nominal { .. }
            | D::Option
            | D::PromiseLike
            | D::Record
            | D::Result
            | D::Set,
        ) => Vec::new(),
        None => {
            let mut values = integer_values();
            values.extend(float_values());
            values.extend([Value::Bool(false), Value::Bool(true), Value::Null]);
            values.extend(string_values());
            values.extend(collection_values());
            values
        }
    };
    for probe in probes {
        if value_conforms(probe, domain) {
            push_unique(&mut values, probe.clone());
        }
    }
    values
}

pub(super) fn value_conforms(value: &Value, domain: Option<DomainEvidence>) -> bool {
    use DomainEvidence as D;
    match domain {
        None => true,
        Some(D::Integer) => matches!(value, Value::Int(_)),
        Some(D::Float) => matches!(value, Value::Float(_)),
        Some(D::Number) => matches!(value, Value::Float(_)),
        Some(D::Boolean) => matches!(value, Value::Bool(_)),
        Some(D::String) => matches!(value, Value::Str(_)),
        Some(
            D::Array
            | D::ByteArray
            | D::Collection
            | D::FutureLike
            | D::Iterable
            | D::Iterator
            | D::Map
            | D::Nominal { .. }
            | D::Option
            | D::PromiseLike
            | D::Record
            | D::Result
            | D::Set,
        ) => false,
    }
}

#[cfg(test)]
pub(crate) fn domains_are_hosted(lang: Lang, domains: &[Option<DomainEvidence>]) -> bool {
    let projections = vec![nose_detect::OracleInputProjection::Declared; domains.len()];
    domains_are_hosted_with_projections(lang, domains, &projections)
}

pub(crate) fn domains_are_hosted_with_projections(
    lang: Lang,
    domains: &[Option<DomainEvidence>],
    projections: &[nose_detect::OracleInputProjection],
) -> bool {
    use DomainEvidence as D;
    let Some((domains, projections)) = effective_domain_contract(domains, projections) else {
        return false;
    };
    domains
        .iter()
        .zip(projections)
        .all(|(domain, projection)| match projection {
            nose_detect::OracleInputProjection::Cardinality => {
                matches!(domain, Some(D::Array | D::Collection | D::Iterable))
            }
            nose_detect::OracleInputProjection::ScalarArray(element) => {
                lang == Lang::TypeScript
                    && *domain == Some(D::Array)
                    && matches!(element, D::Boolean | D::Number | D::String)
            }
            nose_detect::OracleInputProjection::KeyedMembership(key) => {
                lang == Lang::TypeScript
                    && matches!(domain, Some(D::Map | D::Set))
                    && matches!(key, D::Boolean | D::Number | D::String)
            }
            nose_detect::OracleInputProjection::Declared => match domain {
                None => nose_semantics::semantics(lang).is_dynamically_typed(),
                Some(D::Boolean | D::String) => true,
                Some(D::Integer | D::Float) => matches!(
                    lang,
                    Lang::Python
                        | Lang::Ruby
                        | Lang::JavaScript
                        | Lang::Vue
                        | Lang::Svelte
                        | Lang::Html
                ),
                Some(D::Number) => lang == Lang::TypeScript,
                Some(
                    D::Array
                    | D::ByteArray
                    | D::Collection
                    | D::FutureLike
                    | D::Iterable
                    | D::Iterator
                    | D::Map
                    | D::Nominal { .. }
                    | D::Option
                    | D::PromiseLike
                    | D::Record
                    | D::Result
                    | D::Set,
                ) => false,
            },
            nose_detect::OracleInputProjection::UnusedTrailing => false,
        })
}

pub(super) fn projected_domain_pool(
    domain: Option<DomainEvidence>,
    projection: nose_detect::OracleInputProjection,
    probes: &[Value],
) -> Vec<Value> {
    match projection {
        nose_detect::OracleInputProjection::Cardinality => {
            let mut values = collection_values();
            for probe in probes {
                if matches!(probe, Value::List(_)) {
                    push_unique(&mut values, probe.clone());
                }
            }
            values
        }
        nose_detect::OracleInputProjection::ScalarArray(element) => {
            let elements = domain_pool(Some(element), probes);
            let mut values = vec![Value::List(Vec::new())];
            for a in &elements {
                values.push(Value::List(vec![a.clone()]));
                for b in &elements {
                    values.push(Value::List(vec![a.clone(), b.clone()]));
                }
            }
            values
        }
        nose_detect::OracleInputProjection::KeyedMembership(key) => projected_domain_pool(
            domain,
            nose_detect::OracleInputProjection::ScalarArray(key),
            probes,
        )
        .into_iter()
        .filter_map(|value| match value {
            Value::List(keys) => nose_normalize::keyed_membership_value(keys),
            _ => None,
        })
        .fold(Vec::new(), |mut values, value| {
            push_unique(&mut values, value);
            values
        }),
        nose_detect::OracleInputProjection::Declared => domain_pool(domain, probes),
        nose_detect::OracleInputProjection::UnusedTrailing => vec![Value::Null],
    }
}

pub(super) fn relation_rows(
    domains: &[Option<DomainEvidence>],
    projections: &[nose_detect::OracleInputProjection],
    arity: usize,
) -> Vec<Vec<Value>> {
    let mut rows = Vec::new();
    let neutral = |index: usize| {
        projected_domain_pool(
            domains.get(index).copied().flatten(),
            projections
                .get(index)
                .copied()
                .unwrap_or(nose_detect::OracleInputProjection::Declared),
            &[],
        )[0]
        .clone()
    };
    let row_with = |overrides: &[(usize, Value)]| {
        let mut row: Vec<Value> = (0..arity).map(&neutral).collect();
        for (index, value) in overrides {
            row[*index] = value.clone();
        }
        row
    };
    let accepts = |index: usize, value: &Value| {
        let domain = domains.get(index).copied().flatten();
        match projections
            .get(index)
            .copied()
            .unwrap_or(nose_detect::OracleInputProjection::Declared)
        {
            nose_detect::OracleInputProjection::Cardinality => matches!(value, Value::List(_)),
            nose_detect::OracleInputProjection::ScalarArray(element) => {
                matches!(value, Value::List(values) if values.iter().all(|v| value_conforms(v, Some(element))))
            }
            nose_detect::OracleInputProjection::KeyedMembership(key) => {
                matches!(value, Value::KeySet(values) if values.iter().all(|v| value_conforms(v, Some(key))))
            }
            nose_detect::OracleInputProjection::Declared => value_conforms(value, domain),
            nose_detect::OracleInputProjection::UnusedTrailing => false,
        }
    };

    let string_a = Value::Str(vec![0x5EED_0001]);
    let string_b = Value::Str(vec![0x5EED_0002]);
    if arity >= 2 && accepts(0, &string_a) && accepts(1, &string_b) {
        rows.push(row_with(&[(0, string_a), (1, string_b)]));
    }
    let list_a = Value::List(vec![Value::Int(1), Value::Int(2)]);
    let list_b = Value::List(vec![Value::Int(2), Value::Int(1)]);
    if arity >= 2 && accepts(0, &list_a) && accepts(1, &list_b) {
        rows.push(row_with(&[(0, list_a), (1, list_b)]));
    }
    let high_a = Value::Int(0xF_0000_0003);
    let high_b = Value::Int(0xF_0000_0005);
    if arity >= 2 && accepts(0, &high_a) && accepts(1, &high_b) {
        rows.push(row_with(&[(0, high_a), (1, high_b)]));
    }
    let float_a = Value::Float(F64(1e16));
    let float_b = Value::Float(F64(-1e16));
    let float_c = Value::Float(F64(1.0));
    if arity >= 3 && accepts(0, &float_a) && accepts(1, &float_b) && accepts(2, &float_c) {
        rows.push(row_with(&[(0, float_a), (1, float_b), (2, float_c)]));
    }
    rows
}
