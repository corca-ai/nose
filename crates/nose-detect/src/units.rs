//! Extract detection units from a normalized file and compute their structural
//! features: a multiset of local **subtree-shape** hashes (tree 2-grams: a node
//! tag combined with its children's tags), a pre-order **linearization** of node
//! tags for alignment, and a **MinHash** signature for candidate generation.

mod dags;
mod features;
mod fragments;
mod gates;
mod model;
mod product;
mod roots;
mod timing;
mod tree;

#[cfg(test)]
pub(crate) use crate::exact_policy::EXACT_VALUE_MIN;
use crate::fragment::{FragmentKind, ProofFacts};
use crate::strict_exact::{strict_exact_safe_tree, StrictFacts};
pub use dags::unit_dags_at;
use features::{unit_minhash, unit_shape_features};
#[cfg(test)]
pub(crate) use fragments::exact_statement_fragment_root;
use fragments::strict_exact_self_field_fragment_safe;
pub(crate) use fragments::top_level_statement_fragment_context_safe;
pub(crate) use gates::large_test_file;
pub use gates::ProductUnitAdmission;
use gates::{
    post_value_fingerprint_rejection, pre_value_fingerprint_rejection,
    skip_before_value_fingerprint, PreValueFingerprintGate,
};
pub(crate) use model::abstraction_family_witness;
pub use model::UnitFeat;
use nose_il::{Il, Interner, NodeId, NodeKind, Payload, Span, UnitKind};
use nose_semantics::ValueLaw;
#[cfg(test)]
pub(crate) use product::default_product_oracle_fragments;
pub(crate) use product::{block_units_for_file, raw_il_is_empty_module};
pub use product::{
    default_product_oracle_fragment_candidates, default_product_unit_admission,
    default_product_value_fingerprint_context, ProductOracleFragment, ProductUnitAdmissionInput,
};
use roots::{collect_unit_roots, value_fingerprint_context_for_roots, UnitRoot};
use std::time::Instant;
use timing::{UnitTimer, UnitTimingSample, UnitTimingSkipSample};
use tree::collect_pre;
pub(crate) use tree::{build_parent_index, subtree_spans_within};

#[derive(Clone, Copy)]
pub(crate) struct ExtractFeatures {
    pub(crate) shape_features: bool,
    pub(crate) abstraction_witnesses: bool,
    pub(crate) connected_witnesses: bool,
}

/// Per-file inputs shared by every unit extraction in [`extract`].
struct UnitExtractCtx<'a> {
    il: &'a Il,
    interner: &'a Interner,
    /// `None` defers signatures until the corpus finalizes its owned feature arrays.
    seeds: Option<&'a [u64]>,
    min_lines: u32,
    min_tokens: usize,
    features: ExtractFeatures,
    parents: Option<&'a [Option<NodeId>]>,
    facts: &'a StrictFacts<'a>,
    value_context: Option<&'a nose_normalize::ValueFingerprintContext>,
    large_test_file: bool,
}

/// A unit root that survived the size/semantic gates, with the semantic
/// fingerprint and per-stage timings already computed.
struct GatedUnit {
    span: Span,
    pre: Vec<NodeId>,
    exact_safe: bool,
    value: Vec<u64>,
    review_value: Option<nose_normalize::ReviewValueFingerprint>,
    lits: Vec<u64>,
    returns: Vec<u64>,
    pure_single_return: bool,
    cond_sinks: Vec<u64>,
    used_length_contract: bool,
    anchors: Vec<nose_normalize::Anchor>,
    semantic_laws: Vec<ValueLaw>,
    unit_start: Option<Instant>,
    pre_ms: Option<f64>,
    safe_ms: Option<f64>,
    value_ms: Option<f64>,
}

/// Extract all units of `il` passing the size gates, with features computed.
pub(crate) fn extract(
    il: &Il,
    interner: &Interner,
    seeds: Option<&[u64]>,
    min_lines: u32,
    min_tokens: usize,
    block_units: bool,
    features: ExtractFeatures,
) -> Vec<UnitFeat> {
    extract_with_context(
        il,
        interner,
        seeds,
        min_lines,
        min_tokens,
        block_units,
        features,
    )
    .0
}

pub(crate) fn extract_with_context(
    il: &Il,
    interner: &Interner,
    seeds: Option<&[u64]>,
    min_lines: u32,
    min_tokens: usize,
    block_units: bool,
    features: ExtractFeatures,
) -> (
    Vec<UnitFeat>,
    Option<nose_normalize::ValueFingerprintContext>,
) {
    // Frontend-tagged functions/methods/classes, and (when enabled) substantial
    // sub-function blocks (loops / ifs / try) plus exact-safe statement fragments.
    // The ceiling funnel showed ~56% of gold pairs have a region that is a
    // sub-function block, undetectable unless extracted as its own unit. Statement
    // fragments stay stricter: they must satisfy the exact semantic gate before they
    // are kept, so opaque surrounding code can no longer hide a provable return/effect
    // expression without expanding the fuzzy surface.
    let (roots, parents) = collect_unit_roots(il, interner, block_units);
    if roots.is_empty() {
        return (Vec::new(), None);
    }

    let facts = StrictFacts::collect(il, interner);
    let value_context = value_fingerprint_context_for_roots(il, interner, roots.len());
    let (mut out, emitted_roots, unit_timer) = {
        let ctx = UnitExtractCtx {
            il,
            interner,
            seeds,
            min_lines,
            min_tokens,
            features,
            parents: parents.as_deref(),
            facts: &facts,
            value_context: value_context.as_ref(),
            large_test_file: large_test_file(il),
        };
        let mut unit_timer = UnitTimer::new();
        let mut out = Vec::new();
        let mut emitted_roots: Vec<NodeId> = Vec::new();
        for unit_root in roots {
            let root = unit_root.root;
            if let Some(unit) = extract_unit(&ctx, unit_root, &mut unit_timer) {
                out.push(unit);
                emitted_roots.push(root);
            }
        }
        (out, emitted_roots, unit_timer)
    };
    if out.is_empty() {
        unit_timer.report_summary(&il.meta.path);
        return (out, value_context);
    }
    let test_module_spans = test_context_spans(il, interner);
    for unit in &mut out {
        unit.in_test_module = test_module_spans
            .iter()
            .any(|&(s, e)| s <= unit.start_line && unit.end_line <= e);
    }
    fill_called_helper_returns(il, interner, &mut out, &emitted_roots);
    unit_timer.report_summary(&il.meta.path);
    (out, value_context)
}

/// Record, on every unit that could be a containment CONTAINER (it has anchors), the
/// return-sink hashes of each SAME-FILE function it provably calls
/// (`CallTarget::DirectFunction` evidence). A containment match on one of these hashes
/// is the unit *using* a helper — generalized inlining splices the callee's value graph
/// into the caller's fingerprint, so without this record every well-behaved caller of a
/// helper would read as "reinventing" it.
fn fill_called_helper_returns(
    il: &Il,
    interner: &Interner,
    units: &mut [UnitFeat],
    roots: &[NodeId],
) {
    use nose_semantics::direct_function_call_target_span_at_call;
    use rustc_hash::FxHashMap;
    // DirectFunction target spans are function ROOT spans; within one file the line
    // pair identifies the target unit.
    let by_span: FxHashMap<(u32, u32), Vec<u64>> = units
        .iter()
        .filter(|u| matches!(u.kind, UnitKind::Function | UnitKind::Method))
        .map(|u| ((u.start_line, u.end_line), u.returns.clone()))
        .collect();
    if by_span.is_empty() {
        return;
    }
    for (unit, &root) in units.iter_mut().zip(roots) {
        if unit.anchors.is_empty() {
            continue;
        }
        let mut called: Vec<u64> = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if il.kind(node) == NodeKind::Call {
                if let Some(span) = direct_function_call_target_span_at_call(il, interner, node) {
                    if let Some(returns) = by_span.get(&(span.start_line, span.end_line)) {
                        called.extend_from_slice(returns);
                    }
                }
            }
            stack.extend(il.children(node).iter().copied());
        }
        called.sort_unstable();
        called.dedup();
        unit.called_helper_returns = called;
    }
}

/// Source spans of frontend test-context evidence and conventional test functions/modules.
/// Nested regions inherit these facts even when their own unit has no name.
pub(crate) fn test_context_spans(il: &Il, interner: &Interner) -> Vec<(u32, u32)> {
    let mut spans: Vec<_> = il
        .units
        .iter()
        .filter(|unit| {
            unit.origin
                .has_evidence(nose_il::UnitEvidenceFlag::TestContext)
                || unit
                    .name
                    .is_some_and(|name| crate::test_paths::is_test_name(interner.resolve(name)))
        })
        .map(|unit| {
            let span = il.node(unit.root).span;
            (span.start_line, span.end_line)
        })
        .collect();
    for node in &il.nodes {
        if node.kind != NodeKind::Module {
            continue;
        }
        let Payload::Name(name) = node.payload else {
            continue;
        };
        let name = interner.resolve(name);
        if name.eq_ignore_ascii_case("tests") || name.eq_ignore_ascii_case("test") {
            spans.push((node.span.start_line, node.span.end_line));
        }
    }
    spans
}

fn exact_safe_for_unit(ctx: &UnitExtractCtx<'_>, root: NodeId, exact_fragment: bool) -> bool {
    strict_exact_safe_tree(ctx.il, ctx.interner, ctx.facts, root)
        || (exact_fragment
            && ctx.parents.is_some_and(|parents| {
                strict_exact_self_field_fragment_safe(
                    ctx.il,
                    ctx.interner,
                    ctx.facts,
                    parents,
                    root,
                )
            }))
}

fn bind_optional_fragment_control_identity(
    il: &Il,
    root: NodeId,
    fragment_kind: Option<FragmentKind>,
    value: &mut Vec<u64>,
) {
    if let Some(kind) = fragment_kind {
        crate::fragment::bind_fragment_control_identity(il, root, kind, value);
    }
}

fn unit_fingerprints(
    ctx: &UnitExtractCtx<'_>,
    root: NodeId,
    fragment: Option<FragmentKind>,
) -> (
    nose_normalize::FingerprintLawBundle,
    Option<nose_normalize::ReviewValueFingerprint>,
) {
    let (mut features, mut review) = nose_normalize::value_fingerprint_with_review(
        ctx.il,
        root,
        ctx.interner,
        ctx.value_context,
    );
    bind_optional_fragment_control_identity(ctx.il, root, fragment, &mut features.0);
    if let Some(review) = &mut review {
        bind_optional_fragment_control_identity(ctx.il, root, fragment, &mut review.values);
    }
    (features, review)
}

fn gate_unit(
    ctx: &UnitExtractCtx<'_>,
    unit_root: UnitRoot,
    unit_timer: &mut UnitTimer,
) -> Option<GatedUnit> {
    let UnitRoot {
        root,
        kind,
        name: _,
        origin,
        fragment_kind,
    } = unit_root;
    let exact_fragment = fragment_kind.is_some();
    let unit_start = unit_timer.start();
    let span = ctx.il.node(root).span;
    let lines = span.line_count();

    let pre_start = unit_timer.start();
    let mut pre = Vec::new();
    collect_pre(ctx.il, root, &mut pre);
    let pre_ms = UnitTimer::elapsed(pre_start);
    let skip = |unit_timer: &mut UnitTimer, safe_ms: Option<f64>, value_ms: Option<f64>| {
        unit_timer.report_skip(UnitTimingSkipSample {
            start: unit_start,
            kind: &kind,
            path: &ctx.il.meta.path,
            start_line: span.start_line,
            end_line: span.end_line,
            tokens: pre.len(),
            pre_ms,
            safe_ms,
            value_ms,
        });
    };

    // A declarative unit (a CSS rule; HTML element later) is a `Block`, but unlike an
    // imperative block its value fingerprint IS its meaning (the canonical declaration
    // set), so — like a dense functional one-liner — it may pass the size gate on the
    // `value.len() >= EXACT_VALUE_MIN` floor below rather than the syntactic floor.
    let declarative = matches!(ctx.il.kind(root), NodeKind::CssRule | NodeKind::HtmlElement);
    let syntactically_small = lines < ctx.min_lines || pre.len() < ctx.min_tokens;
    if skip_before_value_fingerprint(PreValueFingerprintGate {
        kind,
        origin,
        tokens: pre.len(),
        lines,
        syntactically_small,
        declarative,
        exact_fragment,
        large_test_file: ctx.large_test_file,
    }) {
        skip(unit_timer, None, None);
        return None;
    }

    let safe_start = unit_timer.start();
    let exact_safe = exact_safe_for_unit(ctx, root, exact_fragment);
    let safe_ms = UnitTimer::elapsed(safe_start);

    if exact_fragment && !exact_safe {
        skip(unit_timer, safe_ms, None);
        return None;
    }

    // The value graph is the semantic fingerprint (already sorted), with the
    // literal-only multiset for data-table detection. Computed before the size
    // gate so the gate can consult semantic richness (below).
    let value_start = unit_timer.start();
    let (
        (
            value,
            lits,
            returns,
            anchors,
            semantic_laws,
            (pure_single_return, cond_sinks, used_length_contract),
        ),
        review_value,
    ) = unit_fingerprints(ctx, root, fragment_kind);
    let value_ms = UnitTimer::elapsed(value_start);

    // Size gate. A short unit normally isn't a meaningful clone — EXCEPT a
    // frontend-tagged function whose body is behaviorally *dense*: a functional
    // one-liner like `return sum(v for v in xs if v>0)` is a real Type-4 clone of a
    // multi-line loop (the value graph converges them to an *identical* fingerprint),
    // just compressed below the line/token gate. Admit such a function when its value
    // fingerprint is rich enough to be matched by the oracle-certified exact-match
    // path (`value.len() >= 4`, the same floor that path uses) — this recovers the
    // compressed functional Type-4 forms without lowering the gate for trivial units
    // (`return x` has 1–2 atoms) or for blocks (kept strict; they are the noisy ones).
    // Control-flow blocks keep the same syntactic min-lines/min-size gate as
    // functions: measurement showed the real sub-function clones are small (24–40
    // tokens), so a stricter block gate drops signal (pool-precision 0.106→0.074,
    // AUC 0.42→0.17) faster than noise. Exact statement fragments are the narrow
    // exception: they may pass the dense gate only after `exact_safe` and the value
    // fingerprint floor prove that the fragment itself is a usable semantic unit.
    // A declarative unit is admitted on the same `value.len() >= EXACT_VALUE_MIN` floor
    // as a dense functional one-liner (a 1-declaration rule stays below it — intended).
    if post_value_fingerprint_rejection(
        kind,
        exact_fragment,
        declarative,
        syntactically_small,
        exact_safe,
        value.len(),
    )
    .is_some()
    {
        skip(unit_timer, safe_ms, value_ms);
        return None;
    }
    Some(GatedUnit {
        span,
        pre,
        exact_safe,
        value,
        review_value,
        lits,
        returns,
        pure_single_return,
        cond_sinks,
        used_length_contract,
        anchors,
        semantic_laws,
        unit_start,
        pre_ms,
        safe_ms,
        value_ms,
    })
}

fn extract_unit(
    ctx: &UnitExtractCtx<'_>,
    unit_root: UnitRoot,
    unit_timer: &mut UnitTimer,
) -> Option<UnitFeat> {
    let UnitRoot {
        root: _,
        kind,
        name: uname,
        origin,
        fragment_kind,
    } = unit_root;
    let GatedUnit {
        span,
        pre,
        exact_safe,
        value,
        review_value,
        lits,
        returns,
        pure_single_return,
        cond_sinks,
        used_length_contract,
        anchors,
        semantic_laws,
        unit_start,
        pre_ms,
        safe_ms,
        value_ms,
    } = gate_unit(ctx, unit_root, unit_timer)?;
    let feature_start = unit_timer.start();
    let (shapes, shape_minhash, linear, abstraction_tokens) = unit_shape_features(ctx, &pre);
    let connected_tokens = if ctx.features.connected_witnesses {
        crate::connected::mapped_tokens(ctx.il, ctx.interner, &pre)
    } else {
        Vec::new()
    };

    // Candidate generation keys on the value graph when present (so clones
    // that converge only semantically still become candidates).
    let minhash = ctx
        .seeds
        .map(|seeds| unit_minhash(&value, &shapes, ctx.features.shape_features, seeds))
        .unwrap_or_default();

    let display_name = uname
        .map(|s| ctx.interner.resolve(s).to_string())
        .unwrap_or_else(|| "-".to_string());
    unit_timer.report_keep(UnitTimingSample {
        start: unit_start,
        feature_start,
        kind: &kind,
        name: &display_name,
        path: &ctx.il.meta.path,
        start_line: span.start_line,
        end_line: span.end_line,
        tokens: pre.len(),
        value_atoms: value.len(),
        pre_ms,
        safe_ms,
        value_ms,
    });

    let proof_facts = fragment_kind.map(|fk| match fk {
        FragmentKind::SelfFieldBody => ProofFacts::self_field_body(),
        other => ProofFacts::context_gated(other),
    });
    Some(UnitFeat {
        in_test_module: false,
        path: ctx.il.meta.path.clone(),
        lang: ctx.il.meta.lang,
        kind,
        origin,
        name: uname.map(|s| ctx.interner.resolve(s).to_string()),
        start_line: span.start_line,
        end_line: span.end_line,
        source_region: ctx
            .il
            .source
            .as_ref()
            .and_then(|source| source.region(span.start_byte, span.end_byte)),
        source_document: ctx.il.source.clone(),
        token_count: pre.len(),
        shapes,
        shape_minhash,
        value,
        review_value,
        minhash,
        linear,
        connected_tokens,
        abstraction_tokens,
        lits,
        returns,
        pure_single_return,
        cond_sinks,
        used_length_contract,
        called_helper_returns: Vec::new(),
        anchors,
        semantic_laws,
        semantic_pack_near_protocols: Vec::new(),
        exact_safe,
        fragment_kind,
        proof_facts,
    })
}

#[cfg(test)]
mod tests;
