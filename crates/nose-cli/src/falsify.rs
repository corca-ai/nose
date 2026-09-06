//! Domain-aware falsification for the offline `nose verify --falsify` gate (#858).
//!
//! Each parameter is fed values from its declared source domain. Relation-first rows cover the
//! laws most likely to be normalized unsafely; a seeded Cartesian search then explores the rest.
//! A concrete disagreement is shrunk in a stable order and printed with its seed, making nightly
//! failures byte-reproducible without adding code to the shipped query path.

use nose_il::{DomainEvidence, Il, Interner, NodeId};
#[cfg(test)]
use nose_normalize::run_unit;
use nose_normalize::{behavior_has_sym, Behavior, PreparedInterpreter, Value, F64};
use std::collections::HashSet;

mod collections;
mod domains;
pub(crate) use collections::collection_input_projections;
#[cfg(test)]
use domains::domains_are_hosted;
pub(crate) use domains::{domains_are_hosted_with_projections, effective_domain_contract};
use domains::{parameter_domains, projected_domain_pool, relation_rows};

pub(crate) const DEFAULT_FALSIFY_SEED: u64 = 0x4e4f_5345_0020_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FalsifyWitness {
    pub(crate) seed: u64,
    /// Zero-based position in the de-duplicated deterministic candidate stream.
    pub(crate) case_index: usize,
    pub(crate) inputs: Vec<Value>,
    pub(crate) shrunk_inputs: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FalsifyOutcome {
    Witness(FalsifyWitness),
    Exhausted { cases: usize },
    Skipped { reason: &'static str },
}

#[derive(Clone, Copy)]
pub(crate) enum FalsifyObservation {
    Behavior,
    BehaviorAndExit,
}

#[derive(Clone, Copy)]
pub(crate) enum ModuleStringBindings {
    Exclude,
    Include,
}

#[derive(Clone, Copy)]
pub(crate) struct FalsifyTarget<'a> {
    pub(crate) il: &'a Il,
    pub(crate) root: NodeId,
    pub(crate) projections: &'a [nose_detect::OracleInputProjection],
}

pub(crate) struct FalsifyRequest<'a> {
    pub(crate) left: FalsifyTarget<'a>,
    pub(crate) right: FalsifyTarget<'a>,
    pub(crate) interner: &'a Interner,
    pub(crate) probes: &'a [Value],
    pub(crate) budget: usize,
    pub(crate) seed: u64,
    pub(crate) observation: FalsifyObservation,
    pub(crate) module_strings: ModuleStringBindings,
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

struct ReplayUnit<'a> {
    interpreter: PreparedInterpreter<'a>,
    root: NodeId,
}

struct ReplayPair<'a> {
    left: ReplayUnit<'a>,
    right: ReplayUnit<'a>,
    observe_exit: bool,
}

impl ReplayPair<'_> {
    fn concrete_disagreement(&self, row: &[Value]) -> bool {
        if self.observe_exit {
            let (Some((behavior_a, exit_a)), Some((behavior_b, exit_b))) = (
                self.left
                    .interpreter
                    .run_observing_exit(self.left.root, row),
                self.right
                    .interpreter
                    .run_observing_exit(self.right.root, row),
            ) else {
                return false;
            };
            return exit_a != exit_b || behaviors_concretely_differ(&behavior_a, &behavior_b);
        }
        let (Some(behavior_a), Some(behavior_b)) = (
            self.left.interpreter.run(self.left.root, row),
            self.right.interpreter.run(self.right.root, row),
        ) else {
            return false;
        };
        behaviors_concretely_differ(&behavior_a, &behavior_b)
    }
}

fn behaviors_concretely_differ(a: &Behavior, b: &Behavior) -> bool {
    !behaviors_falsification_equal(a, b) && !behavior_has_sym(a) && !behavior_has_sym(b)
}

/// Falsification observes the sign of zero because source runtimes expose it through operations
/// such as `Object.is` and `copysign`. NaN payloads remain intentionally canonical: the oracle
/// models one deterministic NaN class rather than platform-specific payload propagation.
fn behaviors_falsification_equal(a: &Behavior, b: &Behavior) -> bool {
    values_falsification_equal(&a.ret, &b.ret)
        && a.effects.len() == b.effects.len()
        && a.effects
            .iter()
            .zip(&b.effects)
            .all(|(left, right)| values_falsification_equal(left, right))
        && a.fields.len() == b.fields.len()
        && a.fields
            .iter()
            .zip(&b.fields)
            .all(|(left, right)| left.0 == right.0 && values_falsification_equal(&left.1, &right.1))
}

fn values_falsification_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(F64(left)), Value::Float(F64(right))) => {
            (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
        }
        (Value::List(left), Value::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(a, b)| values_falsification_equal(a, b))
        }
        _ => a == b,
    }
}

fn shrink(
    replay: &ReplayPair<'_>,
    domains: &[Option<DomainEvidence>],
    projections: &[nose_detect::OracleInputProjection],
    original: &[Value],
) -> Vec<Value> {
    let mut current = original.to_vec();
    for index in 0..current.len() {
        let candidates = projected_domain_pool(
            domains.get(index).copied().flatten(),
            projections
                .get(index)
                .copied()
                .unwrap_or(nose_detect::OracleInputProjection::Declared),
            &[],
        );
        let upper = candidates
            .iter()
            .position(|candidate| candidate == &current[index])
            .unwrap_or(candidates.len());
        for candidate in candidates.into_iter().take(upper) {
            let mut attempt = current.clone();
            attempt[index] = candidate;
            if replay.concrete_disagreement(&attempt) {
                current = attempt;
                break;
            }
        }
    }
    current
}

/// Search for a concrete distinguishing input. Declared domains must agree exactly; callers may
/// not use a hash collision or a cross-domain execution as hard soundness evidence.
#[cfg(test)]
pub(crate) fn falsify_pair(
    left: (&Il, NodeId),
    right: (&Il, NodeId),
    interner: &Interner,
    probes: &[Value],
    budget: usize,
    seed: u64,
) -> Option<FalsifyWitness> {
    let (il_a, root_a) = left;
    let (il_b, root_b) = right;
    let domains_a = parameter_domains(il_a, root_a);
    let domains_b = parameter_domains(il_b, root_b);
    if !domains_are_hosted(il_a.meta.lang, &domains_a)
        || !domains_are_hosted(il_b.meta.lang, &domains_b)
    {
        return None;
    }
    let projections_a = vec![nose_detect::OracleInputProjection::Declared; domains_a.len()];
    let projections_b = vec![nose_detect::OracleInputProjection::Declared; domains_b.len()];
    match falsify_pair_with_projections(FalsifyRequest {
        left: FalsifyTarget {
            il: il_a,
            root: root_a,
            projections: &projections_a,
        },
        right: FalsifyTarget {
            il: il_b,
            root: root_b,
            projections: &projections_b,
        },
        interner,
        probes,
        budget,
        seed,
        observation: FalsifyObservation::Behavior,
        module_strings: ModuleStringBindings::Include,
    }) {
        FalsifyOutcome::Witness(witness) => Some(witness),
        FalsifyOutcome::Exhausted { .. } | FalsifyOutcome::Skipped { .. } => None,
    }
}

pub(crate) fn falsify_pair_with_projections(request: FalsifyRequest<'_>) -> FalsifyOutcome {
    let contract = match validate_falsify_contract(request.left, request.right, request.interner) {
        Ok(contract) => contract,
        Err(reason) => return FalsifyOutcome::Skipped { reason },
    };
    run_falsification_search(request, &contract)
}

struct FalsifyContract {
    domains: Vec<Option<DomainEvidence>>,
    projections: Vec<nose_detect::OracleInputProjection>,
}

fn validate_falsify_contract(
    left: FalsifyTarget<'_>,
    right: FalsifyTarget<'_>,
    interner: &Interner,
) -> Result<FalsifyContract, &'static str> {
    if !collections::valid_collection_projections(left, interner)
        || !collections::valid_collection_projections(right, interner)
    {
        return Err("array projection lacks source element evidence");
    }
    let domains_a = parameter_domains(left.il, left.root);
    let domains_b = parameter_domains(right.il, right.root);
    let (domains, projections) = effective_domain_contract(&domains_a, left.projections)
        .ok_or("invalid left projection contract")?;
    let (other_domains, other_projections) =
        effective_domain_contract(&domains_b, right.projections)
            .ok_or("invalid right projection contract")?;
    if domains != other_domains || projections != other_projections {
        return Err("effective domain contracts differ");
    }
    if !domains_are_hosted_with_projections(left.il.meta.lang, &domains_a, left.projections) {
        return Err("left domain contract is not hosted");
    }
    if !domains_are_hosted_with_projections(right.il.meta.lang, &domains_b, right.projections) {
        return Err("right domain contract is not hosted");
    }
    Ok(FalsifyContract {
        domains: domains.to_vec(),
        projections: projections.to_vec(),
    })
}

fn run_falsification_search(
    request: FalsifyRequest<'_>,
    contract: &FalsifyContract,
) -> FalsifyOutcome {
    let FalsifyRequest {
        left,
        right,
        interner,
        probes,
        budget,
        seed,
        observation,
        module_strings,
    } = request;
    let domains = &contract.domains;
    let projections = &contract.projections;
    let replay = ReplayPair {
        left: ReplayUnit {
            interpreter: PreparedInterpreter::new(
                left.il,
                interner,
                matches!(module_strings, ModuleStringBindings::Include),
            ),
            root: left.root,
        },
        right: ReplayUnit {
            interpreter: PreparedInterpreter::new(
                right.il,
                interner,
                matches!(module_strings, ModuleStringBindings::Include),
            ),
            root: right.root,
        },
        observe_exit: matches!(observation, FalsifyObservation::BehaviorAndExit),
    };
    let arity = domains.len().max(1);
    let mut pools: Vec<Vec<Value>> = (0..arity)
        .map(|index| {
            projected_domain_pool(
                domains.get(index).copied().flatten(),
                projections
                    .get(index)
                    .copied()
                    .unwrap_or(nose_detect::OracleInputProjection::Declared),
                probes,
            )
        })
        .collect();
    for (index, pool) in pools.iter_mut().enumerate() {
        rotate(pool, seed ^ index as u64);
    }
    let mut relations = relation_rows(domains, projections, arity);
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
        if replay.concrete_disagreement(&row) {
            let shrunk_inputs = shrink(&replay, domains, projections, &row);
            return FalsifyOutcome::Witness(FalsifyWitness {
                seed,
                case_index,
                inputs: row,
                shrunk_inputs,
            });
        }
        case_index += 1;
    }
    FalsifyOutcome::Exhausted { cases: case_index }
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
        Value::KeySet(keys) => format!(
            "keys:[{}]",
            keys.iter().map(format_value).collect::<Vec<_>>().join(", ")
        ),
        Value::Sym(value) => format!("sym:{value:016x}"),
    }
}

#[cfg(test)]
mod tests;
