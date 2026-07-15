//! Extract detection units from a normalized file and compute their structural
//! features: a multiset of local **subtree-shape** hashes (tree 2-grams: a node
//! tag combined with its children's tags), a pre-order **linearization** of node
//! tags for alignment, and a **MinHash** signature for candidate generation.

mod features;
mod fragments;
mod gates;
mod model;
mod product;
mod timing;
mod tree;

#[cfg(test)]
pub(crate) use crate::exact_policy::EXACT_VALUE_MIN;
use crate::fragment::{FragmentKind, ProofFacts};
use crate::strict_exact::{strict_exact_safe_tree, StrictFacts};
use features::{unit_minhash, unit_shape_features};
#[cfg(test)]
pub(crate) use fragments::exact_statement_fragment_root;
pub(crate) use fragments::top_level_statement_fragment_context_safe;
use fragments::{collect_extra_unit_roots, strict_exact_self_field_fragment_safe};
pub(crate) use gates::large_test_file;
pub use gates::ProductUnitAdmission;
use gates::{
    post_value_fingerprint_rejection, pre_value_fingerprint_rejection,
    skip_before_value_fingerprint, PreValueFingerprintGate,
};
pub(crate) use model::abstraction_family_witness;
pub use model::UnitFeat;
use nose_il::{Il, Interner, NodeId, NodeKind, Payload, Span, Symbol, UnitKind, UnitOrigin};
use nose_semantics::ValueLaw;
pub(crate) use product::block_units_for_file;
pub use product::{
    default_product_unit_admission, default_product_value_fingerprint_context,
    ProductUnitAdmissionInput,
};
use std::time::Instant;
use timing::{UnitTimer, UnitTimingSample, UnitTimingSkipSample};
use tree::collect_pre;
pub(crate) use tree::{build_parent_index, subtree_spans_within};

#[derive(Clone, Copy)]
struct UnitRoot {
    root: NodeId,
    kind: UnitKind,
    name: Option<Symbol>,
    origin: UnitOrigin,
    /// The exact-fragment classification, when this root was admitted as an exact
    /// sub-function fragment. `None` for ordinary function/method/class/block units.
    /// `Some(_)` is the authoritative "this is an exact fragment" signal; the boolean
    /// `fragment_kind.is_some()` replaces the previous standalone `exact_fragment` flag.
    fragment_kind: Option<FragmentKind>,
}

fn collect_unit_roots(
    il: &Il,
    interner: &Interner,
    block_units: bool,
) -> (Vec<UnitRoot>, Option<Vec<Option<NodeId>>>) {
    let mut roots: Vec<UnitRoot> = il
        .units
        .iter()
        .map(|u| UnitRoot {
            root: u.root,
            kind: u.kind,
            name: u.name,
            origin: u.origin,
            fragment_kind: None,
        })
        .collect();
    let parents = if block_units {
        let parents = build_parent_index(il);
        collect_extra_unit_roots(il, il.root, &parents, interner, &mut roots);
        Some(parents)
    } else {
        None
    };
    (roots, parents)
}

fn value_fingerprint_context_for_roots(
    il: &Il,
    interner: &Interner,
    root_count: usize,
) -> Option<nose_normalize::ValueFingerprintContext> {
    (root_count > 1).then(|| nose_normalize::ValueFingerprintContext::new(il, interner))
}

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
    seeds: &'a [u64],
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
    seeds: &[u64],
    min_lines: u32,
    min_tokens: usize,
    block_units: bool,
    features: ExtractFeatures,
) -> Vec<UnitFeat> {
    // Frontend-tagged functions/methods/classes, and (when enabled) substantial
    // sub-function blocks (loops / ifs / try) plus exact-safe statement fragments.
    // The ceiling funnel showed ~56% of gold pairs have a region that is a
    // sub-function block, undetectable unless extracted as its own unit. Statement
    // fragments stay stricter: they must satisfy the exact semantic gate before they
    // are kept, so opaque surrounding code can no longer hide a provable return/effect
    // expression without expanding the fuzzy surface.
    let (roots, parents) = collect_unit_roots(il, interner, block_units);

    let facts = StrictFacts::collect(il, interner);
    let value_context = value_fingerprint_context_for_roots(il, interner, roots.len());
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
    let test_module_spans = inline_test_module_spans(il, interner);
    let mut out = Vec::new();
    let mut emitted_roots: Vec<NodeId> = Vec::new();
    for unit_root in roots {
        let root = unit_root.root;
        if let Some(mut unit) = extract_unit(&ctx, unit_root, &mut unit_timer) {
            unit.in_test_module = test_module_spans
                .iter()
                .any(|&(s, e)| s <= unit.start_line && unit.end_line <= e);
            out.push(unit);
            emitted_roots.push(root);
        }
    }
    fill_called_helper_returns(il, interner, &mut out, &emitted_roots);
    unit_timer.report_summary(&il.meta.path);
    out
}

/// Above this many normalized nodes a file is treated as pathological for witness
/// purposes (generated/minified): the graded witness is best-effort enrichment, so it
/// is skipped rather than paying an outsized cost on a file no one refactors by hand.
const WITNESS_MAX_FILE_NODES: usize = 60_000;

/// Export the value DAGs of the units at the given `(start_line, end_line)` spans, for
/// the #315 graded witness. `il` is the file's RAW IL (this normalizes it the same way
/// detection does), and the result is aligned with `wanted`: `Some((dag, exact_safe))`
/// when a unit root matches the span, `None` otherwise (no match, or a pathological
/// file skipped wholesale). The per-file resolution context (referents, inline/global
/// context) is built once and shared across the requested roots.
pub fn unit_dags_at(
    il: &Il,
    interner: &Interner,
    opts: &crate::DetectOptions,
    wanted: &[(u32, u32)],
) -> Vec<Option<(nose_normalize::ValueDag, bool)>> {
    if large_test_file(il) {
        return vec![None; wanted.len()];
    }
    let norm_opts = nose_normalize::NormalizeOptions {
        cfg_norm: opts.cfg_norm,
        dce: opts.dce,
        ..Default::default()
    };
    let n = nose_normalize::normalize(il, interner, &norm_opts);
    if n.nodes.len() > WITNESS_MAX_FILE_NODES {
        return vec![None; wanted.len()];
    }
    let (roots, _) = collect_unit_roots(&n, interner, opts.block_units);
    let facts = StrictFacts::collect(&n, interner);
    let context = value_fingerprint_context_for_roots(&n, interner, roots.len());
    let referents = nose_normalize::FileReferents::new(&n, interner);
    // Span -> root (first wins, matching extraction order).
    let mut by_lines: rustc_hash::FxHashMap<(u32, u32), NodeId> = rustc_hash::FxHashMap::default();
    for r in &roots {
        let s = n.node(r.root).span;
        by_lines.entry((s.start_line, s.end_line)).or_insert(r.root);
    }
    wanted
        .iter()
        .map(|&span| {
            let root = *by_lines.get(&span)?;
            let exact_safe = strict_exact_safe_tree(&n, interner, &facts, root);
            let dag = nose_normalize::value_dag(&n, root, interner, context.as_ref(), &referents);
            Some((dag, exact_safe))
        })
        .collect()
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

/// Source-line spans of inline test modules (`mod tests` / `mod test`) — the Rust
/// convention for in-file test scaffolding. Module nodes keep their names through
/// lowering, so a span check is enough; other languages simply have no named
/// `tests` module inside a file.
fn inline_test_module_spans(il: &Il, interner: &Interner) -> Vec<(u32, u32)> {
    let mut spans = Vec::new();
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
        value,
        lits,
        returns,
        anchors,
        semantic_laws,
        (pure_single_return, cond_sinks, used_length_contract),
    ) = if let Some(context) = ctx.value_context {
        nose_normalize::value_fingerprint_lits_anchors_laws_with_context(
            ctx.il,
            root,
            ctx.interner,
            context,
        )
    } else {
        nose_normalize::value_fingerprint_lits_anchors_laws(ctx.il, root, ctx.interner)
    };
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
    let minhash = unit_minhash(&value, &shapes, ctx.features.shape_features, ctx.seeds);

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
        token_count: pre.len(),
        shapes,
        shape_minhash,
        value,
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
        exact_safe,
        fragment_kind,
        proof_facts,
    })
}

#[cfg(test)]
mod tests;
