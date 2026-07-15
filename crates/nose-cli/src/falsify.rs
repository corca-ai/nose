//! Domain-aware falsification for the offline `nose verify --falsify` gate (#858).
//!
//! Each parameter is fed values from its declared source domain. Relation-first rows cover the
//! laws most likely to be normalized unsafely; a seeded Cartesian search then explores the rest.
//! A concrete disagreement is shrunk in a stable order and printed with its seed, making nightly
//! failures byte-reproducible without adding code to the shipped query path.

use nose_il::{DomainEvidence, Il, Interner, NodeId, NodeKind, Payload};
use nose_normalize::{behavior_has_sym, run_unit, Behavior, Value, F64};
use std::collections::HashSet;

pub(crate) const DEFAULT_FALSIFY_SEED: u64 = 0x4e4f_5345_0020_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FalsifyWitness {
    pub(crate) seed: u64,
    /// Zero-based position in the de-duplicated deterministic candidate stream.
    pub(crate) case_index: usize,
    pub(crate) inputs: Vec<Value>,
    pub(crate) shrunk_inputs: Vec<Value>,
}

fn parameter_domains(il: &Il, root: NodeId) -> Vec<Option<DomainEvidence>> {
    il.children(root)
        .iter()
        .filter(|&&child| {
            il.kind(child) == NodeKind::Param && matches!(il.node(child).payload, Payload::Cid(_))
        })
        .map(|&param| nose_semantics::domain_evidence_for_param(il, param))
        .collect()
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

fn float_values() -> Vec<Value> {
    vec![
        Value::Float(F64(0.0)),
        Value::Float(F64(-0.0)),
        Value::Float(F64(1.0)),
        Value::Float(F64(-1.0)),
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

fn domain_pool(domain: Option<DomainEvidence>, probes: &[Value]) -> Vec<Value> {
    use DomainEvidence as D;
    let mut values = match domain {
        Some(D::Integer) => integer_values(),
        Some(D::Float) => float_values(),
        Some(D::Number) => {
            let mut values = integer_values();
            values.extend(float_values());
            values
        }
        Some(D::Boolean) => vec![Value::Bool(false), Value::Bool(true)],
        Some(D::String) => string_values(),
        Some(D::Array | D::ByteArray | D::Collection | D::Iterable | D::Iterator | D::Set) => {
            collection_values()
        }
        Some(D::Option) => {
            let mut values = vec![Value::Null];
            values.extend(integer_values().into_iter().take(3));
            values.extend(string_values().into_iter().take(3));
            values.extend(collection_values().into_iter().take(2));
            values
        }
        Some(
            D::FutureLike | D::Map | D::Nominal { .. } | D::PromiseLike | D::Record | D::Result,
        ) => vec![Value::Null],
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

fn value_conforms(value: &Value, domain: Option<DomainEvidence>) -> bool {
    use DomainEvidence as D;
    match domain {
        None | Some(D::Option) => true,
        Some(D::Integer) => matches!(value, Value::Int(_)),
        Some(D::Float) => matches!(value, Value::Float(_)),
        Some(D::Number) => matches!(value, Value::Int(_) | Value::Float(_)),
        Some(D::Boolean) => matches!(value, Value::Bool(_)),
        Some(D::String) => matches!(value, Value::Str(_)),
        Some(D::Array | D::ByteArray | D::Collection | D::Iterable | D::Iterator | D::Set) => {
            matches!(value, Value::List(_))
        }
        Some(
            D::FutureLike | D::Map | D::Nominal { .. } | D::PromiseLike | D::Record | D::Result,
        ) => matches!(value, Value::Null),
    }
}

fn relation_rows(domains: &[Option<DomainEvidence>], arity: usize) -> Vec<Vec<Value>> {
    let mut rows = Vec::new();
    let neutral = |index: usize| domain_pool(domains.get(index).copied().flatten(), &[])[0].clone();
    let row_with = |overrides: &[(usize, Value)]| {
        let mut row: Vec<Value> = (0..arity).map(&neutral).collect();
        for (index, value) in overrides {
            row[*index] = value.clone();
        }
        row
    };
    let accepts =
        |index: usize, value: &Value| value_conforms(value, domains.get(index).copied().flatten());

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

fn rotate<T>(values: &mut [T], seed: u64) {
    if !values.is_empty() {
        values.rotate_left((splitmix64(seed) as usize) % values.len());
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn concrete_disagreement(
    il_a: &Il,
    root_a: NodeId,
    il_b: &Il,
    root_b: NodeId,
    interner: &Interner,
    row: &[Value],
) -> bool {
    let (Some(behavior_a), Some(behavior_b)) = (
        run_unit(il_a, interner, root_a, row),
        run_unit(il_b, interner, root_b, row),
    ) else {
        return false;
    };
    behaviors_concretely_differ(&behavior_a, &behavior_b)
}

fn behaviors_concretely_differ(a: &Behavior, b: &Behavior) -> bool {
    a != b && !behavior_has_sym(a) && !behavior_has_sym(b)
}

fn shrink(
    il_a: &Il,
    root_a: NodeId,
    il_b: &Il,
    root_b: NodeId,
    interner: &Interner,
    domains: &[Option<DomainEvidence>],
    original: &[Value],
) -> Vec<Value> {
    let mut current = original.to_vec();
    for index in 0..current.len() {
        let candidates = domain_pool(domains.get(index).copied().flatten(), &[]);
        let upper = candidates
            .iter()
            .position(|candidate| candidate == &current[index])
            .unwrap_or(candidates.len());
        for candidate in candidates.into_iter().take(upper) {
            let mut attempt = current.clone();
            attempt[index] = candidate;
            if concrete_disagreement(il_a, root_a, il_b, root_b, interner, &attempt) {
                current = attempt;
                break;
            }
        }
    }
    current
}

/// Search for a concrete distinguishing input. Declared domains must agree exactly; callers may
/// not use a hash collision or a cross-domain execution as hard soundness evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn falsify_pair(
    il_a: &Il,
    root_a: NodeId,
    il_b: &Il,
    root_b: NodeId,
    interner: &Interner,
    probes: &[Value],
    budget: usize,
    seed: u64,
) -> Option<FalsifyWitness> {
    let domains = parameter_domains(il_a, root_a);
    if domains != parameter_domains(il_b, root_b) {
        return None;
    }
    let arity = domains.len().max(1);
    let mut pools: Vec<Vec<Value>> = (0..arity)
        .map(|index| domain_pool(domains.get(index).copied().flatten(), probes))
        .collect();
    for (index, pool) in pools.iter_mut().enumerate() {
        rotate(pool, seed ^ index as u64);
    }
    let mut relations = relation_rows(&domains, arity);
    rotate(&mut relations, seed ^ 0x5245_4c41_5449_4f4e);

    // `F64` intentionally canonicalizes signed zero for behavior equality, but the source
    // domain still contains both bit patterns. De-duplicate by the receipt encoding so `+0`
    // and `-0` both reach the interpreter while otherwise-identical rows are skipped.
    let mut seen = HashSet::new();
    let mut case_index = 0usize;
    let total = pools.iter().try_fold(1usize, |total, pool| {
        total.checked_mul(pool.len()).ok_or(())
    });
    let cartesian_limit = total.unwrap_or(usize::MAX).min(budget);
    let candidates = relations
        .into_iter()
        .chain((0..cartesian_limit).map(|encoded| {
            let mut remainder = encoded;
            pools
                .iter()
                .map(|pool| {
                    let value = pool[remainder % pool.len()].clone();
                    remainder /= pool.len();
                    value
                })
                .collect::<Vec<_>>()
        }));
    for row in candidates {
        if !seen.insert(format_inputs(&row)) {
            continue;
        }
        if case_index >= budget {
            break;
        }
        if concrete_disagreement(il_a, root_a, il_b, root_b, interner, &row) {
            let shrunk_inputs = shrink(il_a, root_a, il_b, root_b, interner, &domains, &row);
            return Some(FalsifyWitness {
                seed,
                case_index,
                inputs: row,
                shrunk_inputs,
            });
        }
        case_index += 1;
    }
    None
}

pub(crate) fn format_inputs(values: &[Value]) -> String {
    let formatted: Vec<String> = values.iter().map(format_value).collect();
    format!("[{}]", formatted.join(", "))
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Int(value) => format!("int:{value}"),
        Value::Float(F64(value)) if value.is_nan() => "float:nan".to_string(),
        Value::Float(F64(value)) if *value == f64::INFINITY => "float:+inf".to_string(),
        Value::Float(F64(value)) if *value == f64::NEG_INFINITY => "float:-inf".to_string(),
        Value::Float(F64(value)) if *value == 0.0 && value.is_sign_negative() => {
            "float:-0".to_string()
        }
        Value::Float(F64(value)) => format!("float:{value:e}"),
        Value::Bool(value) => format!("bool:{value}"),
        Value::Str(parts) => {
            let parts: Vec<String> = parts.iter().map(|part| format!("{part:016x}")).collect();
            format!("str:[{}]", parts.join("+"))
        }
        Value::List(values) => format!("list:{}", format_inputs(values)),
        Value::Null => "null".to_string(),
        Value::Err => "err".to_string(),
        Value::Sym(value) => format!("sym:{value:016x}"),
    }
}

#[cfg(test)]
mod tests;
