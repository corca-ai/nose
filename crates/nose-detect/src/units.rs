//! Extract detection units from a normalized file and compute their structural
//! features: a multiset of local **subtree-shape** hashes (tree 2-grams: a node
//! tag combined with its children's tags), a pre-order **linearization** of node
//! tags for alignment, and a **MinHash** signature for candidate generation.

use crate::abstraction;
use crate::fragment::{FragmentKind, ProofFacts};
use crate::il_utils::{
    local_nontrivial_assignment, local_nontrivial_assignment_chain, node_mentions_any_cid,
};
#[cfg(test)]
use crate::strict_exact::{
    function_binding_safe, strict_exact_collection_contains_call_safe,
    strict_exact_java_collection_factory_safe, strict_exact_java_map_factory_safe,
    strict_exact_membership_collection_safe, strict_exact_python_collection_factory_safe,
    strict_exact_set_constructor_collection_safe,
};
use crate::strict_exact::{strict_exact_safe_tree, StrictFacts};
#[cfg(test)]
use nose_il::Builtin;
use nose_il::{Il, Interner, Lang, LoopKind, NodeId, NodeKind, Payload, Span, Symbol, UnitKind};
use nose_normalize::node_tag;
use nose_semantics::{
    admitted_builtin_semantics_at_call, builder_append_call_args, exact_java_return_this,
    exact_non_overloadable_index_assignment, exact_non_overloadable_index_assignment_parts,
    exact_self_field_write_assignment, opaque_argument_escape_args,
    receiver_mutation_call_receiver, ValueLaw,
};
#[cfg(test)]
use nose_semantics::{library_free_name_collection_factory_contract, LibraryApiCalleeContract};
use rustc_hash::FxHashSet;
use std::time::Instant;

/// A unit ready for comparison. Self-contained (owns its features and location)
/// so the detector can flatten units from many files into one vector. All feature
/// vectors are content-derived hashes (interner-independent), so a `UnitFeat` is
/// portable across runs — which is what lets the CLI cache it by source-content hash.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UnitFeat {
    pub path: String,
    pub lang: Lang,
    pub kind: UnitKind,
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub token_count: usize,
    /// Sorted multiset of local shape hashes (syntactic structure).
    pub shapes: Vec<u64>,
    /// MinHash signature over `shapes`, used by the Type-3 near-duplicate channel.
    pub shape_minhash: Vec<u64>,
    /// Sorted multiset of value-graph (GVN) hashes — the semantic substrate that
    /// is invariant to temporaries, statement order, and common-subexpression
    /// duplication.
    pub value: Vec<u64>,
    /// MinHash signature for candidate generation (over the value graph when
    /// available, else shapes).
    pub minhash: Vec<u64>,
    /// Pre-order node-tag sequence, for alignment scoring.
    pub linear: Vec<u64>,
    /// Pre-order typed tokens used only by the experimental abstraction witness layer.
    ///
    /// Unlike `linear`, this keeps a value-sensitive literal tag so a pair that differs
    /// only by `0` vs `1` can be explained as one literal hole without weakening the
    /// exact semantic fingerprint.
    #[serde(default)]
    pub(crate) abstraction_tokens: Vec<abstraction::WitnessToken>,
    /// Sorted multiset of literal (`Const`) value hashes. A high `lits/value`
    /// ratio marks a "data-table" unit (constant-dominated, e.g. a locale map),
    /// where the constants must match for a clone.
    pub lits: Vec<u64>,
    /// Sorted multiset of RETURN-sink value hashes — what the unit returns. True
    /// clones return the same computed values; used to demote near-identical units
    /// that differ only in their result (`<` vs `<=`, an extra effect).
    pub returns: Vec<u64>,
    /// The unit's heavy sub-DAG ANCHORS (sub-computations of ≥ `ANCHOR_MIN_WEIGHT` value-nodes),
    /// sorted/deduped by hash. Two units sharing a rare anchor share an extractable common
    /// sub-computation — a partial / sub-DAG clone that whole-unit Jaccard misses. Each carries
    /// its weight (to RANK the shared sub-DAG by size) and the source line range (to SHOW where
    /// the shared computation lives).
    #[serde(default)]
    pub anchors: Vec<nose_normalize::Anchor>,
    /// The unit sits inside an inline test module (`mod tests` / `mod test`) —
    /// Rust keeps tests inside production files, so a path heuristic alone tags
    /// their scaffolding `prod` (#226).
    #[serde(default)]
    pub in_test_module: bool,
    /// Pack-facing value laws that actually rewrote or bridged this unit's value graph.
    #[serde(default)]
    pub semantic_laws: Vec<ValueLaw>,
    /// Whether the value fingerprint is safe to use as a strict semantic proof.
    ///
    /// `semantic` mode must not report units whose fingerprint passed through lossy
    /// lowering (`Raw`, abstract literals such as JS regex, or opaque calls). Those
    /// units can still participate in `near` via structural scoring.
    #[serde(default)]
    pub exact_safe: bool,
    /// The exact-fragment classification, when this unit is a sub-function fragment.
    ///
    /// `None` for ordinary function/method/class/block units; `Some(_)` names *why* the
    /// fragment is an exact semantic clone (its [`FragmentKind`], with a stable
    /// [`reason_code`](FragmentKind::reason_code)). Surfaced so reporting and ranking can
    /// separate proof fragments from actionable refactors without re-deriving the shape.
    #[serde(default)]
    pub fragment_kind: Option<FragmentKind>,
    /// What the recognizer proved about this fragment at acceptance time. Present iff
    /// [`fragment_kind`](Self::fragment_kind) is `Some`.
    #[serde(default)]
    pub proof_facts: Option<ProofFacts>,
}

pub(crate) fn abstraction_family_witness<'a>(
    members: impl IntoIterator<Item = &'a UnitFeat>,
) -> Option<crate::AbstractionWitness> {
    let units = members
        .into_iter()
        .map(|unit| (unit.lang, unit.kind, unit.abstraction_tokens.as_slice()))
        .collect::<Vec<_>>();
    abstraction::family_witness(&units)
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Upper bound (pre-order node count) for a *block* unit. Blocks are meant to surface
/// sub-function fragments; broad nested bodies are covered by their enclosing unit and
/// can multiply value extraction cost across almost-identical regions.
const MAX_BLOCK_TOKENS: usize = 160;
/// Upper bound for a *class* container unit. Ordinary class/type clones stay eligible,
/// while very large class bodies are delegated to their method/function units.
const MAX_CLASS_TOKENS: usize = 8_000;
const EXACT_VALUE_MIN: usize = 4;

fn semantic_container_token_cap(kind: UnitKind) -> Option<usize> {
    match kind {
        UnitKind::Block => Some(MAX_BLOCK_TOKENS),
        UnitKind::Class => Some(MAX_CLASS_TOKENS),
        UnitKind::Function | UnitKind::Method => None,
    }
}

struct UnitTimer {
    sample_enabled: bool,
    summary_enabled: bool,
    summary: UnitTimingSummary,
}

impl UnitTimer {
    fn new() -> Self {
        let sample_enabled = std::env::var_os("NOSE_TIME_UNITS").is_some();
        let summary_enabled = std::env::var_os("NOSE_TIME_UNIT_SUMMARY").is_some();
        Self {
            sample_enabled,
            summary_enabled,
            summary: UnitTimingSummary::default(),
        }
    }

    fn start(&self) -> Option<Instant> {
        (self.sample_enabled || self.summary_enabled).then(Instant::now)
    }

    fn elapsed(start: Option<Instant>) -> Option<f64> {
        start.map(|t| t.elapsed().as_secs_f64() * 1e3)
    }

    fn report_skip(&mut self, sample: UnitTimingSkipSample<'_>) {
        let Some(start) = sample.start else {
            return;
        };
        let total_ms = start.elapsed().as_secs_f64() * 1e3;
        self.summary.record_skip(
            sample.kind,
            sample.tokens,
            total_ms,
            sample.pre_ms,
            sample.safe_ms,
            sample.value_ms,
        );
        if self.sample_enabled && total_ms >= 10.0 {
            let ms = |value: Option<f64>| {
                value
                    .map(|value| format!("{value:.1}ms"))
                    .unwrap_or_else(|| "-".to_string())
            };
            eprintln!(
                "  [unit] skip {:?} {}:{}-{} tokens={} pre={} safe={} value={} total={:.1}ms",
                sample.kind,
                sample.path,
                sample.start_line,
                sample.end_line,
                sample.tokens,
                ms(sample.pre_ms),
                ms(sample.safe_ms),
                ms(sample.value_ms),
                total_ms,
            );
        }
    }

    fn report_keep(&mut self, sample: UnitTimingSample<'_>) {
        let (Some(start), Some(pre_ms), Some(safe_ms), Some(value_ms), Some(feature_start)) = (
            sample.start,
            sample.pre_ms,
            sample.safe_ms,
            sample.value_ms,
            sample.feature_start,
        ) else {
            return;
        };
        let feature_ms = feature_start.elapsed().as_secs_f64() * 1e3;
        let total_ms = start.elapsed().as_secs_f64() * 1e3;
        self.summary.record_keep(
            sample.kind,
            sample.tokens,
            sample.value_atoms,
            total_ms,
            pre_ms,
            safe_ms,
            value_ms,
            feature_ms,
        );
        if self.sample_enabled && total_ms >= 10.0 {
            eprintln!(
                "  [unit] keep {:?} {} {}:{}-{} tokens={} value_atoms={} pre={pre_ms:.1}ms safe={safe_ms:.1}ms value={value_ms:.1}ms features={feature_ms:.1}ms total={total_ms:.1}ms",
                sample.kind,
                sample.name,
                sample.path,
                sample.start_line,
                sample.end_line,
                sample.tokens,
                sample.value_atoms,
            );
        }
    }

    fn report_summary(&self, path: &str) {
        if self.summary_enabled {
            self.summary.report(path);
        }
    }
}

#[derive(Clone, Copy, Default)]
struct UnitTimingBucket {
    seen: usize,
    kept: usize,
    skipped: usize,
    tokens: usize,
    value_atoms: usize,
    total_ms: f64,
    pre_ms: f64,
    safe_ms: f64,
    value_ms: f64,
    feature_ms: f64,
}

#[derive(Default)]
struct UnitTimingSummary {
    buckets: [UnitTimingBucket; 4],
}

impl UnitTimingSummary {
    fn bucket_mut(&mut self, kind: &UnitKind) -> &mut UnitTimingBucket {
        &mut self.buckets[unit_kind_index(kind)]
    }

    fn record_skip(
        &mut self,
        kind: &UnitKind,
        tokens: usize,
        total_ms: f64,
        pre_ms: Option<f64>,
        safe_ms: Option<f64>,
        value_ms: Option<f64>,
    ) {
        let bucket = self.bucket_mut(kind);
        bucket.seen += 1;
        bucket.skipped += 1;
        bucket.tokens += tokens;
        bucket.total_ms += total_ms;
        bucket.pre_ms += pre_ms.unwrap_or(0.0);
        bucket.safe_ms += safe_ms.unwrap_or(0.0);
        bucket.value_ms += value_ms.unwrap_or(0.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn record_keep(
        &mut self,
        kind: &UnitKind,
        tokens: usize,
        value_atoms: usize,
        total_ms: f64,
        pre_ms: f64,
        safe_ms: f64,
        value_ms: f64,
        feature_ms: f64,
    ) {
        let bucket = self.bucket_mut(kind);
        bucket.seen += 1;
        bucket.kept += 1;
        bucket.tokens += tokens;
        bucket.value_atoms += value_atoms;
        bucket.total_ms += total_ms;
        bucket.pre_ms += pre_ms;
        bucket.safe_ms += safe_ms;
        bucket.value_ms += value_ms;
        bucket.feature_ms += feature_ms;
    }

    fn report(&self, path: &str) {
        let total_ms: f64 = self.buckets.iter().map(|bucket| bucket.total_ms).sum();
        if total_ms < 10.0 {
            return;
        }
        for (kind, bucket) in [
            (UnitKind::Function, self.buckets[0]),
            (UnitKind::Method, self.buckets[1]),
            (UnitKind::Class, self.buckets[2]),
            (UnitKind::Block, self.buckets[3]),
        ] {
            if bucket.seen == 0 {
                continue;
            }
            eprintln!(
                "  [unit-summary] {:?} {} seen={} kept={} skipped={} tokens={} value_atoms={} total={:.1}ms pre={:.1}ms safe={:.1}ms value={:.1}ms features={:.1}ms",
                kind,
                path,
                bucket.seen,
                bucket.kept,
                bucket.skipped,
                bucket.tokens,
                bucket.value_atoms,
                bucket.total_ms,
                bucket.pre_ms,
                bucket.safe_ms,
                bucket.value_ms,
                bucket.feature_ms,
            );
        }
    }
}

fn unit_kind_index(kind: &UnitKind) -> usize {
    match kind {
        UnitKind::Function => 0,
        UnitKind::Method => 1,
        UnitKind::Class => 2,
        UnitKind::Block => 3,
    }
}

struct UnitTimingSample<'a> {
    start: Option<Instant>,
    feature_start: Option<Instant>,
    kind: &'a UnitKind,
    name: &'a str,
    path: &'a str,
    start_line: u32,
    end_line: u32,
    tokens: usize,
    value_atoms: usize,
    pre_ms: Option<f64>,
    safe_ms: Option<f64>,
    value_ms: Option<f64>,
}

struct UnitTimingSkipSample<'a> {
    start: Option<Instant>,
    kind: &'a UnitKind,
    path: &'a str,
    start_line: u32,
    end_line: u32,
    tokens: usize,
    pre_ms: Option<f64>,
    safe_ms: Option<f64>,
    value_ms: Option<f64>,
}

#[inline]
fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(7) ^ b).wrapping_mul(SEED)
}

#[derive(Clone, Copy)]
struct UnitRoot {
    root: NodeId,
    kind: UnitKind,
    name: Option<Symbol>,
    /// The exact-fragment classification, when this root was admitted as an exact
    /// sub-function fragment. `None` for ordinary function/method/class/block units.
    /// `Some(_)` is the authoritative "this is an exact fragment" signal; the boolean
    /// `fragment_kind.is_some()` replaces the previous standalone `exact_fragment` flag.
    fragment_kind: Option<FragmentKind>,
}

#[derive(Clone, Copy)]
pub(crate) struct ExtractFeatures {
    pub(crate) shape_features: bool,
    pub(crate) abstraction_witnesses: bool,
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
    let mut roots: Vec<UnitRoot> = il
        .units
        .iter()
        .map(|u| UnitRoot {
            root: u.root,
            kind: u.kind,
            name: u.name,
            fragment_kind: None,
        })
        .collect();
    let parents = if block_units {
        collect_block_units(il, il.root, &mut roots);
        let parents = build_parent_index(il);
        collect_exact_statement_fragment_units(il, il.root, &parents, interner, &mut roots);
        Some(parents)
    } else {
        None
    };

    let facts = StrictFacts::collect(il, interner);
    let value_context =
        (roots.len() > 1).then(|| nose_normalize::ValueFingerprintContext::new(il, interner));
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
    };
    let mut unit_timer = UnitTimer::new();
    let test_module_spans = inline_test_module_spans(il, interner);
    let mut out = Vec::new();
    for unit_root in roots {
        if let Some(mut unit) = extract_unit(&ctx, unit_root, &mut unit_timer) {
            unit.in_test_module = test_module_spans
                .iter()
                .any(|&(s, e)| s <= unit.start_line && unit.end_line <= e);
            out.push(unit);
        }
    }
    unit_timer.report_summary(&il.meta.path);
    out
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

/// Gate one unit root: collect its pre-order, apply the container/size/dense gates
/// (reporting skips), and compute the strict-exact verdict and value fingerprint.
fn gate_unit(
    ctx: &UnitExtractCtx<'_>,
    unit_root: UnitRoot,
    unit_timer: &mut UnitTimer,
) -> Option<GatedUnit> {
    let UnitRoot {
        root,
        kind,
        name: _,
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

    // Broad container units are covered by their nested primary units. Apply
    // this cap before strict/value extraction so discarded containers never pay
    // the dominant semantic fingerprint cost.
    if semantic_container_token_cap(kind).is_some_and(|cap| pre.len() > cap) {
        skip(unit_timer, None, None);
        return None;
    }

    let syntactically_small = lines < ctx.min_lines || pre.len() < ctx.min_tokens;
    let can_use_dense_gate =
        matches!(kind, UnitKind::Function | UnitKind::Method) || exact_fragment;
    if syntactically_small && !can_use_dense_gate {
        skip(unit_timer, None, None);
        return None;
    }

    let safe_start = unit_timer.start();
    let strict_exact_safe = strict_exact_safe_tree(ctx.il, ctx.interner, ctx.facts, root);
    let exact_safe = strict_exact_safe
        || (exact_fragment
            && ctx.parents.is_some_and(|parents| {
                strict_exact_self_field_fragment_safe(
                    ctx.il,
                    ctx.interner,
                    ctx.facts,
                    parents,
                    root,
                )
            }));
    let safe_ms = UnitTimer::elapsed(safe_start);
    // The value graph is the semantic fingerprint (already sorted), with the
    // literal-only multiset for data-table detection. Computed before the size
    // gate so the gate can consult semantic richness (below).
    let value_start = unit_timer.start();
    let (value, lits, returns, anchors, semantic_laws) = if let Some(context) = ctx.value_context {
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
    let dense_exact_unit = if exact_fragment {
        exact_safe && value.len() >= EXACT_VALUE_MIN
    } else {
        matches!(kind, UnitKind::Function | UnitKind::Method) && value.len() >= EXACT_VALUE_MIN
    };
    if (syntactically_small || exact_fragment) && !dense_exact_unit {
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
        fragment_kind,
    } = unit_root;
    let GatedUnit {
        span,
        pre,
        exact_safe,
        value,
        lits,
        returns,
        anchors,
        semantic_laws,
        unit_start,
        pre_ms,
        safe_ms,
        value_ms,
    } = gate_unit(ctx, unit_root, unit_timer)?;
    let feature_start = unit_timer.start();
    let (shapes, shape_minhash, linear, abstraction_tokens) = unit_shape_features(ctx, &pre);

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
        name: uname.map(|s| ctx.interner.resolve(s).to_string()),
        start_line: span.start_line,
        end_line: span.end_line,
        token_count: pre.len(),
        shapes,
        shape_minhash,
        value,
        minhash,
        linear,
        abstraction_tokens,
        lits,
        returns,
        anchors,
        semantic_laws,
        exact_safe,
        fragment_kind,
        proof_facts,
    })
}

fn unit_shape_features(
    ctx: &UnitExtractCtx<'_>,
    pre: &[NodeId],
) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<abstraction::WitnessToken>) {
    let il = ctx.il;
    let interner = ctx.interner;
    let features = ctx.features;
    if features.shape_features {
        let mut shapes = Vec::with_capacity(pre.len());
        let mut linear = Vec::with_capacity(pre.len());
        let mut abstraction_tokens = if features.abstraction_witnesses {
            Vec::with_capacity(pre.len())
        } else {
            Vec::new()
        };
        for &nid in pre {
            let n = il.node(nid);
            let tag = node_tag(n.kind, n.payload, interner);
            linear.push(tag);
            if features.abstraction_witnesses {
                abstraction_tokens.push(abstraction::token_for(il, interner, nid, tag));
            }
            let mut shape = tag;
            for &c in il.children(nid) {
                let cn = il.node(c);
                shape = combine(shape, node_tag(cn.kind, cn.payload, interner));
            }
            shapes.push(shape);
        }
        shapes.sort_unstable();
        let mut distinct_shapes = shapes.clone();
        distinct_shapes.dedup();
        (
            shapes,
            crate::minhash::sign(&distinct_shapes, ctx.seeds),
            linear,
            abstraction_tokens,
        )
    } else if features.abstraction_witnesses {
        let abstraction_tokens = pre
            .iter()
            .map(|&nid| {
                let n = il.node(nid);
                let tag = node_tag(n.kind, n.payload, interner);
                abstraction::token_for(il, interner, nid, tag)
            })
            .collect();
        (Vec::new(), Vec::new(), Vec::new(), abstraction_tokens)
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    }
}

fn unit_minhash(value: &[u64], shapes: &[u64], shape_features: bool, seeds: &[u64]) -> Vec<u64> {
    if value.is_empty() && !shape_features {
        Vec::new()
    } else {
        let mut distinct = if value.is_empty() {
            shapes.to_vec()
        } else {
            value.to_vec()
        };
        distinct.dedup();
        crate::minhash::sign(&distinct, seeds)
    }
}

/// Collect sub-function block roots (loops / ifs / try) as extra unit candidates.
fn collect_block_units(il: &Il, node: NodeId, out: &mut Vec<UnitRoot>) {
    let is_statement_if = il.kind(node) == NodeKind::If
        && il
            .children(node)
            .iter()
            .skip(1)
            .any(|&child| il.kind(child) == NodeKind::Block);
    if matches!(il.kind(node), NodeKind::Loop | NodeKind::Try) || is_statement_if {
        out.push(UnitRoot {
            root: node,
            kind: UnitKind::Block,
            name: None,
            fragment_kind: None,
        });
    }
    for &c in il.children(node) {
        collect_block_units(il, c, out);
    }
}

/// Collect single-statement fragments that can become exact semantic units even when
/// their enclosing function is unsafe because of unrelated siblings.
fn collect_exact_statement_fragment_units(
    il: &Il,
    node: NodeId,
    parents: &[Option<NodeId>],
    interner: &Interner,
    out: &mut Vec<UnitRoot>,
) {
    if il.kind(node) == NodeKind::Lambda {
        return;
    }
    if let Some(contract) =
        crate::fragment::recognize::recognize_contract(il, node, parents, interner)
    {
        let kind = contract.kind;
        // The contract path is the production authority. Keep the old predicate
        // matrix as a debug-only differential guard while it remains in-tree.
        debug_assert!(
            exact_statement_fragment_root(il, node, parents, interner)
                .is_some_and(|predicate_kind| predicate_kind == kind),
            "predicate path must agree with contract-first fragment production for {kind:?}"
        );
        push_or_upgrade_exact_fragment_root(out, node, kind);
    }
    for &c in il.children(node) {
        collect_exact_statement_fragment_units(il, c, parents, interner, out);
    }
}

fn push_or_upgrade_exact_fragment_root(out: &mut Vec<UnitRoot>, root: NodeId, kind: FragmentKind) {
    if let Some(existing) = out.iter_mut().find(|candidate| candidate.root == root) {
        existing.fragment_kind = Some(kind);
    } else {
        out.push(UnitRoot {
            root,
            kind: UnitKind::Block,
            name: None,
            fragment_kind: Some(kind),
        });
    }
}

/// Classify `node` as an exact sub-function fragment root, or `None` if it is not one.
///
/// `Some(kind)` is returned for exactly the nodes the previous boolean recognizer
/// accepted (`true`); the [`FragmentKind`] names which recognizer branch matched. This
/// is the single dispatch that lowers the standalone shape predicates into the fragment
/// substrate (issue #33, step 1).
pub(crate) fn exact_statement_fragment_root(
    il: &Il,
    node: NodeId,
    parents: &[Option<NodeId>],
    interner: &Interner,
) -> Option<FragmentKind> {
    if !subtree_spans_within(il, node, il.node(node).span) {
        return None;
    }
    if exact_function_body_self_field_fragment_root(il, interner, parents, node) {
        return Some(FragmentKind::SelfFieldBody);
    }
    if !top_level_statement_fragment_context_safe(il, node, parents, interner) {
        return None;
    }
    let kids = il.children(node);
    match il.kind(node) {
        NodeKind::Return => (kids.len() == 1
            && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit))
        .then_some(FragmentKind::DirectReturn),
        NodeKind::Throw => (kids.len() == 1
            && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit))
        .then_some(FragmentKind::DirectThrow),
        NodeKind::Assign => exact_assignment_fragment_kind(il, interner, node),
        NodeKind::ExprStmt => {
            exact_expr_statement_fragment_root(il, node).then_some(FragmentKind::ExprEffect)
        }
        NodeKind::If => exact_conditional_fragment_root(il, interner, node)
            .then_some(FragmentKind::ConditionalGuard),
        NodeKind::Loop => {
            exact_loop_effect_fragment_root(il, interner, node).then_some(FragmentKind::LoopEffect)
        }
        _ => None,
    }
}

fn exact_expr_statement_fragment_root(il: &Il, node: NodeId) -> bool {
    let kids = il.children(node);
    kids.len() == 1
        && !matches!(
            il.kind(kids[0]),
            NodeKind::Return
                | NodeKind::Throw
                | NodeKind::Break
                | NodeKind::Continue
                | NodeKind::Var
                | NodeKind::Lit
        )
}

fn exact_conditional_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let kids = il.children(node);
    if !(kids.len() == 2 || kids.len() == 3) {
        return false;
    }
    let mut has_exact_statement = false;
    for &branch in kids.iter().skip(1) {
        let Some(branch_has_exact_statement) =
            empty_or_single_direct_exact_statement_block(il, interner, branch)
        else {
            return false;
        };
        has_exact_statement |= branch_has_exact_statement;
    }
    has_exact_statement
}

fn empty_or_single_direct_exact_statement_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<bool> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(false);
    }
    if exact_ordered_append_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_index_assignment_effect_sequence_block(il, node) {
        return Some(true);
    }
    if exact_ordered_self_field_assignment_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_loop_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_mixed_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_conditional_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_conditional_mixed_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_loop_conditional_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if exact_ordered_loop_conditional_mixed_effect_sequence_block(il, interner, node) {
        return Some(true);
    }
    if kids.len() == 3 && exact_temp_chain_consumed_by_statement(il, kids[0], kids[1], kids[2]) {
        return Some(true);
    }
    if kids.len() == 2 && exact_temp_assignment_consumed_by_statement(il, kids[0], kids[1]) {
        return Some(true);
    }
    if kids.len() != 1 {
        return None;
    }
    match il.kind(kids[0]) {
        NodeKind::Return if il.children(kids[0]).len() <= 1 => Some(true),
        NodeKind::Throw if il.children(kids[0]).len() == 1 => Some(true),
        NodeKind::Assign if exact_assignment_fragment_root(il, interner, kids[0]) => Some(true),
        NodeKind::ExprStmt if exact_expr_statement_fragment_root(il, kids[0]) => Some(true),
        NodeKind::If if exact_conditional_fragment_root(il, interner, kids[0]) => Some(true),
        NodeKind::Loop if exact_loop_effect_fragment_root(il, interner, kids[0]) => Some(true),
        _ => None,
    }
}

fn exact_ordered_loop_effect_sequence_block(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && kids
            .iter()
            .all(|&kid| exact_loop_effect_fragment_root(il, interner, kid))
}

fn exact_ordered_mixed_effect_sequence_block(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let loop_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count();
    let direct_effect_count = kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count();
    if loop_count != 1 || direct_effect_count != 1 {
        return false;
    }
    kids.iter()
        .all(|&kid| exact_ordered_mixed_effect_sequence_item(il, interner, kid))
}

fn exact_ordered_mixed_effect_sequence_item(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Loop => exact_loop_effect_fragment_root(il, interner, node),
        NodeKind::ExprStmt | NodeKind::Assign => {
            exact_direct_effect_statement_root(il, interner, node)
        }
        _ => false,
    }
}

fn exact_ordered_conditional_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && kids.iter().all(|&kid| il.kind(kid) == NodeKind::If)
        && kids
            .iter()
            .all(|&kid| exact_conditional_direct_effect_fragment_root(il, interner, kid))
}

fn exact_conditional_direct_effect_fragment_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    let kids = il.children(node);
    if il.kind(node) != NodeKind::If || !(kids.len() == 2 || kids.len() == 3) {
        return false;
    }
    let mut has_effect = false;
    for &branch in kids.iter().skip(1) {
        let Some(branch_has_effect) =
            empty_or_single_direct_exact_effect_block(il, interner, branch)
        else {
            return false;
        };
        has_effect |= branch_has_effect;
    }
    has_effect
}

fn exact_ordered_conditional_mixed_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let conditional_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count();
    let direct_effect_count = kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count();
    if conditional_count != 1 || direct_effect_count != 1 {
        return false;
    }
    kids.iter().all(|&kid| match il.kind(kid) {
        NodeKind::If => exact_conditional_direct_effect_fragment_root(il, interner, kid),
        NodeKind::ExprStmt | NodeKind::Assign => {
            exact_direct_effect_statement_root(il, interner, kid)
        }
        _ => false,
    })
}

fn exact_ordered_loop_conditional_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let loop_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count();
    let conditional_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count();
    if loop_count != 1 || conditional_count != 1 {
        return false;
    }
    kids.iter().all(|&kid| match il.kind(kid) {
        NodeKind::Loop => exact_loop_effect_fragment_root(il, interner, kid),
        NodeKind::If => exact_conditional_direct_effect_fragment_root(il, interner, kid),
        _ => false,
    })
}

fn exact_ordered_loop_conditional_mixed_effect_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 3 {
        return false;
    }
    let loop_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::Loop)
        .count();
    let conditional_count = kids
        .iter()
        .filter(|&&kid| il.kind(kid) == NodeKind::If)
        .count();
    let direct_effect_count = kids
        .iter()
        .filter(|&&kid| matches!(il.kind(kid), NodeKind::ExprStmt | NodeKind::Assign))
        .count();
    if loop_count != 1 || conditional_count != 1 || direct_effect_count != 1 {
        return false;
    }
    kids.iter().all(|&kid| match il.kind(kid) {
        NodeKind::Loop => exact_loop_effect_fragment_root(il, interner, kid),
        NodeKind::If => exact_conditional_direct_effect_fragment_root(il, interner, kid),
        NodeKind::ExprStmt | NodeKind::Assign => {
            exact_direct_effect_statement_root(il, interner, kid)
        }
        _ => false,
    })
}

fn empty_or_single_direct_exact_effect_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<bool> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(false);
    }
    if kids.len() != 1 {
        return None;
    }
    exact_direct_effect_statement_root(il, interner, kids[0]).then_some(true)
}

fn exact_direct_effect_statement_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => exact_append_effect_statement_root(il, interner, node),
        NodeKind::Assign => exact_index_assignment_fragment_root(il, node),
        _ => false,
    }
}

fn exact_ordered_append_effect_sequence_block(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if !(2..=5).contains(&kids.len()) {
        return false;
    }
    if !kids
        .iter()
        .all(|&kid| matches!(il.kind(kid), NodeKind::Assign | NodeKind::ExprStmt))
    {
        return false;
    }
    let expected_effects = match kids
        .iter()
        .filter(|&&kid| exact_append_effect_statement_root(il, interner, kid))
        .count()
    {
        2 if kids.len() <= 4 => 2,
        3 => 3,
        _ => return false,
    };
    let mut idx = 0;
    let mut effects = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && exact_temp_chain_consumed_by_append_effect(
                il,
                interner,
                kids[idx],
                kids[idx + 1],
                kids[idx + 2],
            )
        {
            effects += 1;
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && exact_temp_assignment_consumed_by_append_effect(
                il,
                interner,
                kids[idx],
                kids[idx + 1],
            )
        {
            effects += 1;
            idx += 2;
            continue;
        }
        if exact_append_effect_statement_root(il, interner, kids[idx]) {
            effects += 1;
            idx += 1;
            continue;
        }
        return false;
    }
    effects == expected_effects
}

fn exact_append_effect_statement_root(il: &Il, interner: &Interner, stmt: NodeId) -> bool {
    if il.kind(stmt) != NodeKind::ExprStmt {
        return false;
    }
    let kids = il.children(stmt);
    kids.len() == 1 && exact_single_item_append_call(il, interner, kids[0])
}

fn exact_single_item_append_call(il: &Il, interner: &Interner, call: NodeId) -> bool {
    append_call_args(il, interner, call).is_some()
}

fn exact_temp_assignment_consumed_by_append_effect(
    il: &Il,
    interner: &Interner,
    assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp_cid);
    let empty = FxHashSet::default();
    let kids = il.children(effect);
    il.kind(effect) == NodeKind::ExprStmt
        && kids.len() == 1
        && exact_single_item_append_call(il, interner, kids[0])
        && append_effect_consumes_temp(il, interner, kids[0], &empty, &temp_cids)
}

fn exact_temp_chain_consumed_by_append_effect(
    il: &Il,
    interner: &Interner,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some(chain) = local_nontrivial_assignment_chain(il, first_assign, second_assign) else {
        return false;
    };
    let empty = FxHashSet::default();
    let kids = il.children(effect);
    il.kind(effect) == NodeKind::ExprStmt
        && kids.len() == 1
        && exact_single_item_append_call(il, interner, kids[0])
        && append_effect_consumes_chained_temp(
            il,
            interner,
            kids[0],
            &empty,
            &chain.all,
            &chain.second,
            &chain.first,
        )
}

fn exact_ordered_index_assignment_effect_sequence_block(il: &Il, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    if !(2..=5).contains(&kids.len()) {
        return false;
    }
    if !kids.iter().all(|&kid| il.kind(kid) == NodeKind::Assign) {
        return false;
    }
    let expected_effects = match kids
        .iter()
        .filter(|&&kid| exact_index_assignment_fragment_root(il, kid))
        .count()
    {
        2 if kids.len() <= 4 => 2,
        3 => 3,
        _ => return false,
    };
    let mut idx = 0;
    let mut effects = 0;
    while idx < kids.len() {
        if idx + 2 < kids.len()
            && exact_temp_chain_consumed_by_index_assignment_effect(
                il,
                kids[idx],
                kids[idx + 1],
                kids[idx + 2],
            )
        {
            effects += 1;
            idx += 3;
            continue;
        }
        if idx + 1 < kids.len()
            && exact_temp_assignment_consumed_by_index_assignment_effect(
                il,
                kids[idx],
                kids[idx + 1],
            )
        {
            effects += 1;
            idx += 2;
            continue;
        }
        if exact_index_assignment_fragment_root(il, kids[idx]) {
            effects += 1;
            idx += 1;
            continue;
        }
        return false;
    }
    effects == expected_effects
}

fn exact_ordered_self_field_assignment_sequence_block(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let kids = il.children(node);
    (2..=3).contains(&kids.len())
        && kids
            .iter()
            .all(|&kid| exact_self_field_assignment_fragment_root(il, interner, kid))
}

fn exact_temp_assignment_consumed_by_index_assignment_effect(
    il: &Il,
    assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    exact_index_assignment_consumes_temp(il, effect, temp_cid, None)
}

fn exact_temp_chain_consumed_by_index_assignment_effect(
    il: &Il,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
) -> bool {
    let Some(chain) = local_nontrivial_assignment_chain(il, first_assign, second_assign) else {
        return false;
    };
    exact_index_assignment_consumes_temp(il, effect, chain.second_cid, Some(&chain.first))
}

fn exact_temp_assignment_consumed_by_statement(il: &Il, assign: NodeId, stmt: NodeId) -> bool {
    let Some((temp_cid, _)) = local_nontrivial_assignment(il, assign) else {
        return false;
    };
    exact_statement_consumes_temp(il, stmt, temp_cid, None)
}

fn exact_temp_chain_consumed_by_statement(
    il: &Il,
    first_assign: NodeId,
    second_assign: NodeId,
    stmt: NodeId,
) -> bool {
    let Some((first_cid, _)) = local_nontrivial_assignment(il, first_assign) else {
        return false;
    };
    let Some((second_cid, second_rhs)) = local_nontrivial_assignment(il, second_assign) else {
        return false;
    };
    if first_cid == second_cid {
        return false;
    }
    let mut first = FxHashSet::default();
    first.insert(first_cid);
    if !node_mentions_any_cid(il, second_rhs, &first) {
        return false;
    }
    exact_statement_consumes_temp(il, stmt, second_cid, Some(&first))
}

fn exact_statement_consumes_temp(
    il: &Il,
    stmt: NodeId,
    temp_cid: u32,
    forbidden_cids: Option<&FxHashSet<u32>>,
) -> bool {
    match il.kind(stmt) {
        NodeKind::Return | NodeKind::Throw => {
            let kids = il.children(stmt);
            if kids.len() != 1 {
                return false;
            }
            let mut temp = FxHashSet::default();
            temp.insert(temp_cid);
            (il.kind(kids[0]) == NodeKind::Var
                && matches!(il.node(kids[0]).payload, Payload::Cid(cid) if cid == temp_cid))
                || (!matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit)
                    && node_mentions_any_cid(il, kids[0], &temp)
                    && match forbidden_cids {
                        Some(cids) => !node_mentions_any_cid(il, kids[0], cids),
                        None => true,
                    })
        }
        NodeKind::ExprStmt if exact_expr_statement_fragment_root(il, stmt) => {
            let mut temp = FxHashSet::default();
            temp.insert(temp_cid);
            node_mentions_any_cid(il, stmt, &temp)
                && match forbidden_cids {
                    Some(cids) => !node_mentions_any_cid(il, stmt, cids),
                    None => true,
                }
        }
        NodeKind::Assign => {
            exact_index_assignment_consumes_temp(il, stmt, temp_cid, forbidden_cids)
        }
        _ => false,
    }
}

fn exact_index_assignment_consumes_temp(
    il: &Il,
    stmt: NodeId,
    temp_cid: u32,
    forbidden_cids: Option<&FxHashSet<u32>>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, stmt)
    else {
        return false;
    };

    let mut temp = FxHashSet::default();
    temp.insert(temp_cid);
    if node_mentions_any_cid(il, receiver, &temp)
        || forbidden_cids.is_some_and(|cids| node_mentions_any_cid(il, receiver, cids))
    {
        return false;
    }

    let key_uses_temp = key.is_some_and(|key| node_mentions_any_cid(il, key, &temp));
    let value_uses_temp = node_mentions_any_cid(il, value, &temp);
    if !(key_uses_temp || value_uses_temp) {
        return false;
    }
    match forbidden_cids {
        Some(cids) => {
            !key.is_some_and(|key| node_mentions_any_cid(il, key, cids))
                && !node_mentions_any_cid(il, value, cids)
        }
        None => true,
    }
}

fn exact_assignment_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_assignment_fragment_kind(il, interner, node).is_some()
}

/// Classify an assignment fragment as an index-assignment effect or a Java self-field
/// write — the two exact assignment shapes. Index assignment is checked first so the
/// classification is deterministic (the two shapes are structurally disjoint regardless).
fn exact_assignment_fragment_kind(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<FragmentKind> {
    if exact_index_assignment_fragment_root(il, node) {
        Some(FragmentKind::IndexAssignEffect)
    } else if exact_self_field_assignment_fragment_root(il, interner, node) {
        Some(FragmentKind::SelfFieldAssign)
    } else {
        None
    }
}

fn exact_index_assignment_fragment_root(il: &Il, node: NodeId) -> bool {
    exact_non_overloadable_index_assignment(il, node)
}

// Field-write fingerprints model final receiver+field state. Expose only Java's fixed
// `this.field = ...`; arbitrary receivers such as `other.field = ...` need a
// receiver-place proof fact before they can be exact fragments.
fn exact_self_field_assignment_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_self_field_write_assignment(il, interner, node)
}

fn exact_java_return_this_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    exact_java_return_this(il, interner, node)
}

fn exact_function_body_self_field_fragment_root(
    il: &Il,
    interner: &Interner,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Block {
        return false;
    }
    let Some(func) = parent_of(parents, node) else {
        return false;
    };
    if il.kind(func) != NodeKind::Func {
        return false;
    }
    let kids = il.children(node);
    if kids.len() < 2 {
        return false;
    }
    let mut has_field_effect = false;
    for (idx, &child) in kids.iter().enumerate() {
        match exact_self_field_body_statement_root(il, interner, child) {
            Some(SelfFieldBodyStatement::FieldEffect) => {
                has_field_effect = true;
            }
            Some(SelfFieldBodyStatement::ReturnThis) if idx + 1 == kids.len() => {}
            _ => return false,
        }
    }
    has_field_effect
}

#[derive(Clone, Copy)]
enum SelfFieldBodyStatement {
    FieldEffect,
    ReturnThis,
}

fn exact_self_field_body_statement_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<SelfFieldBodyStatement> {
    if exact_java_return_this_fragment_root(il, interner, node) {
        return Some(SelfFieldBodyStatement::ReturnThis);
    }
    exact_self_field_statement_fragment_root(il, interner, node)
        .then_some(SelfFieldBodyStatement::FieldEffect)
}

fn exact_self_field_statement_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Assign => exact_self_field_assignment_fragment_root(il, interner, node),
        NodeKind::If => {
            let kids = il.children(node);
            if !(kids.len() == 2 || kids.len() == 3) {
                return false;
            }
            let mut has_field_assignment = false;
            for &branch in kids.iter().skip(1) {
                let Some(branch_has_field_assignment) =
                    exact_self_field_statement_branch_root(il, interner, branch)
                else {
                    return false;
                };
                has_field_assignment |= branch_has_field_assignment;
            }
            has_field_assignment
        }
        _ => false,
    }
}

fn exact_self_field_statement_branch_root(
    il: &Il,
    interner: &Interner,
    node: NodeId,
) -> Option<bool> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(false);
    }
    if kids.len() != 1 {
        return None;
    }
    Some(exact_self_field_statement_fragment_root(
        il, interner, kids[0],
    ))
}

fn strict_exact_self_field_fragment_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Block => {
            let kids = il.children(node);
            exact_function_body_self_field_fragment_root(il, interner, parents, node)
                && kids.iter().all(|&child| {
                    strict_exact_self_field_body_statement_safe(il, interner, facts, parents, child)
                })
        }
        _ => strict_exact_self_field_effect_safe(il, interner, facts, parents, node),
    }
}

fn strict_exact_self_field_body_statement_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    exact_java_return_this_fragment_root(il, interner, node)
        || strict_exact_self_field_effect_safe(il, interner, facts, parents, node)
}

fn strict_exact_self_field_effect_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Assign => {
            let kids = il.children(node);
            kids.len() == 2
                && exact_self_field_assignment_fragment_root(il, interner, node)
                && strict_exact_safe_tree(il, interner, facts, kids[1])
        }
        NodeKind::If => {
            let kids = il.children(node);
            if !(kids.len() == 2 || kids.len() == 3) {
                return false;
            }
            if !strict_exact_safe_tree(il, interner, facts, kids[0]) {
                return false;
            }
            let mut has_field_assignment = false;
            for &branch in kids.iter().skip(1) {
                let Some(branch_has_field_assignment) =
                    strict_exact_self_field_branch_safe(il, interner, facts, parents, branch)
                else {
                    return false;
                };
                has_field_assignment |= branch_has_field_assignment;
            }
            has_field_assignment
        }
        _ => false,
    }
}

fn strict_exact_self_field_branch_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    parents: &[Option<NodeId>],
    node: NodeId,
) -> Option<bool> {
    if il.kind(node) != NodeKind::Block {
        return None;
    }
    let kids = il.children(node);
    if kids.is_empty() {
        return Some(false);
    }
    if (2..=3).contains(&kids.len()) && kids.iter().all(|&kid| il.kind(kid) == NodeKind::Assign) {
        return Some(
            kids.iter()
                .all(|&kid| strict_exact_self_field_effect_safe(il, interner, facts, parents, kid)),
        );
    }
    if kids.len() != 1 {
        return None;
    }
    Some(strict_exact_self_field_effect_safe(
        il, interner, facts, parents, kids[0],
    ))
}

fn exact_loop_effect_fragment_root(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if !matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 3 {
        return false;
    }
    let mut iter_cids = FxHashSet::default();
    collect_cids(il, kids[0], &mut iter_cids);
    if iter_cids.is_empty() {
        return false;
    }
    foreach_effect_body_depends_on_iter(il, interner, kids[2], &iter_cids).unwrap_or(false)
}

fn foreach_effect_body_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<bool> {
    match il.kind(node) {
        NodeKind::Block => {
            let mut has_effect = false;
            let kids = il.children(node);
            let mut idx = 0;
            while idx < kids.len() {
                if idx + 2 < kids.len()
                    && loop_temp_chain_consumed_by_effect(
                        il,
                        interner,
                        kids[idx],
                        kids[idx + 1],
                        kids[idx + 2],
                        iter_cids,
                    )
                {
                    has_effect = true;
                    idx += 3;
                    continue;
                }
                if idx + 1 < kids.len()
                    && loop_temp_assignment_consumed_by_effect(
                        il,
                        interner,
                        kids[idx],
                        kids[idx + 1],
                        iter_cids,
                    )
                {
                    has_effect = true;
                    idx += 2;
                    continue;
                }
                has_effect |=
                    foreach_effect_body_depends_on_iter(il, interner, kids[idx], iter_cids)?;
                idx += 1;
            }
            Some(has_effect)
        }
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            (kids.len() == 1 && append_effect_depends_on_iter(il, interner, kids[0], iter_cids))
                .then_some(true)
        }
        NodeKind::Assign => {
            index_assignment_effect_depends_on_iter(il, interner, node, iter_cids).then_some(true)
        }
        NodeKind::If => {
            let kids = il.children(node);
            if !(kids.len() == 2 || kids.len() == 3) {
                return None;
            }
            let mut has_append = false;
            for &branch in kids.iter().skip(1) {
                if il.kind(branch) != NodeKind::Block {
                    return None;
                }
                has_append |= foreach_effect_body_depends_on_iter(il, interner, branch, iter_cids)?;
            }
            Some(has_append)
        }
        _ => None,
    }
}

fn loop_temp_assignment_consumed_by_effect(
    il: &Il,
    interner: &Interner,
    assign: NodeId,
    effect: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some(temp_cid) = loop_local_iter_temp_assignment(il, assign, iter_cids) else {
        return false;
    };
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp_cid);
    loop_effect_consumes_temp(il, interner, effect, iter_cids, &temp_cids)
}

fn loop_temp_chain_consumed_by_effect(
    il: &Il,
    interner: &Interner,
    first_assign: NodeId,
    second_assign: NodeId,
    effect: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some((first_cid, first_rhs)) = local_nontrivial_assignment(il, first_assign) else {
        return false;
    };
    if iter_cids.contains(&first_cid) || !node_mentions_any_cid(il, first_rhs, iter_cids) {
        return false;
    }
    let Some((second_cid, second_rhs)) = local_nontrivial_assignment(il, second_assign) else {
        return false;
    };
    if iter_cids.contains(&second_cid) || first_cid == second_cid {
        return false;
    }

    let mut first_temp = FxHashSet::default();
    first_temp.insert(first_cid);
    if !node_mentions_any_cid(il, second_rhs, &first_temp) {
        return false;
    }

    let mut all_temps = first_temp.clone();
    all_temps.insert(second_cid);
    let mut final_temp = FxHashSet::default();
    final_temp.insert(second_cid);
    loop_effect_consumes_chained_temp(
        il,
        interner,
        effect,
        iter_cids,
        &all_temps,
        &final_temp,
        &first_temp,
    )
}

fn loop_local_iter_temp_assignment(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> Option<u32> {
    let (lhs, rhs) = il.assignment_var_parts(node)?;
    if matches!(il.kind(rhs), NodeKind::Var | NodeKind::Lit) {
        return None;
    }
    let temp_cid = il.var_cid(lhs)?;
    if iter_cids.contains(&temp_cid) {
        return None;
    }
    let mut temp_cids = FxHashSet::default();
    temp_cids.insert(temp_cid);
    if node_mentions_any_cid(il, rhs, &temp_cids) || !node_mentions_any_cid(il, rhs, iter_cids) {
        return None;
    }
    Some(temp_cid)
}

fn loop_effect_consumes_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            kids.len() == 1
                && append_effect_consumes_temp(il, interner, kids[0], iter_cids, temp_cids)
        }
        NodeKind::Assign => index_assignment_effect_consumes_temp(il, node, iter_cids, temp_cids),
        _ => false,
    }
}

fn loop_effect_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    match il.kind(node) {
        NodeKind::ExprStmt => {
            let kids = il.children(node);
            kids.len() == 1
                && append_effect_consumes_chained_temp(
                    il,
                    interner,
                    kids[0],
                    iter_cids,
                    all_temp_cids,
                    final_temp_cids,
                    prior_temp_cids,
                )
        }
        NodeKind::Assign => index_assignment_effect_consumes_chained_temp(
            il,
            node,
            iter_cids,
            all_temp_cids,
            final_temp_cids,
            prior_temp_cids,
        ),
        _ => false,
    }
}

fn append_effect_consumes_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !node_mentions_any_cid(il, receiver, iter_cids)
        && !node_mentions_any_cid(il, receiver, temp_cids)
        && node_mentions_any_cid(il, value, temp_cids)
}

fn append_effect_consumes_chained_temp(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !node_mentions_any_cid(il, receiver, iter_cids)
        && !node_mentions_any_cid(il, receiver, all_temp_cids)
        && node_mentions_any_cid(il, value, final_temp_cids)
        && !node_mentions_any_cid(il, value, prior_temp_cids)
}

fn index_assignment_effect_consumes_temp(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, node)
    else {
        return false;
    };
    if node_mentions_any_cid(il, receiver, iter_cids)
        || node_mentions_any_cid(il, receiver, temp_cids)
    {
        return false;
    }
    key.is_some_and(|key| node_mentions_any_cid(il, key, temp_cids))
        || node_mentions_any_cid(il, value, temp_cids)
}

fn index_assignment_effect_consumes_chained_temp(
    il: &Il,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
    all_temp_cids: &FxHashSet<u32>,
    final_temp_cids: &FxHashSet<u32>,
    prior_temp_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, node)
    else {
        return false;
    };
    if node_mentions_any_cid(il, receiver, iter_cids)
        || node_mentions_any_cid(il, receiver, all_temp_cids)
    {
        return false;
    }
    let key_uses_final = key.is_some_and(|key| node_mentions_any_cid(il, key, final_temp_cids));
    let key_uses_prior = key.is_some_and(|key| node_mentions_any_cid(il, key, prior_temp_cids));
    let value_uses_final = node_mentions_any_cid(il, value, final_temp_cids);
    let value_uses_prior = node_mentions_any_cid(il, value, prior_temp_cids);
    (key_uses_final || value_uses_final) && !key_uses_prior && !value_uses_prior
}

fn append_effect_depends_on_iter(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, value)) = append_call_args(il, interner, node) else {
        return false;
    };
    !node_mentions_any_cid(il, receiver, iter_cids) && node_mentions_any_cid(il, value, iter_cids)
}

fn append_call_args(il: &Il, interner: &Interner, node: NodeId) -> Option<(NodeId, NodeId)> {
    builder_append_call_args(il, interner, node)
}

fn index_assignment_effect_depends_on_iter(
    il: &Il,
    _interner: &Interner,
    node: NodeId,
    iter_cids: &FxHashSet<u32>,
) -> bool {
    let Some((receiver, key, value)) = exact_non_overloadable_index_assignment_parts(il, node)
    else {
        return false;
    };
    if node_mentions_any_cid(il, receiver, iter_cids) {
        return false;
    }
    key.is_some_and(|key| node_mentions_any_cid(il, key, iter_cids))
        || node_mentions_any_cid(il, value, iter_cids)
}

pub(crate) fn top_level_statement_fragment_context_safe(
    il: &Il,
    node: NodeId,
    parents: &[Option<NodeId>],
    interner: &Interner,
) -> bool {
    let Some(body) = parent_of(parents, node) else {
        return false;
    };
    if il.kind(body) != NodeKind::Block {
        return false;
    }
    let Some(func) = parent_of(parents, body) else {
        return false;
    };
    if il.kind(func) != NodeKind::Func {
        return false;
    }

    let mut used = FxHashSet::default();
    collect_cids(il, node, &mut used);
    if used.is_empty() {
        return true;
    }

    let mut blocked = used;
    for &stmt in il.children(body) {
        if stmt == node {
            return true;
        }
        if previous_statement_invalidates_fragment_inputs(il, interner, stmt, &mut blocked) {
            return false;
        }
    }
    false
}

fn previous_statement_invalidates_fragment_inputs(
    il: &Il,
    interner: &Interner,
    stmt: NodeId,
    blocked: &mut FxHashSet<u32>,
) -> bool {
    if assignment_aliases_or_mutates_blocked_cid(il, stmt, blocked) {
        return true;
    }
    if call_may_mutate_blocked_cid(il, interner, stmt, blocked) {
        return true;
    }
    for &child in il.children(stmt) {
        if previous_statement_invalidates_fragment_inputs(il, interner, child, blocked) {
            return true;
        }
    }
    false
}

fn assignment_aliases_or_mutates_blocked_cid(
    il: &Il,
    node: NodeId,
    blocked: &mut FxHashSet<u32>,
) -> bool {
    if il.kind(node) != NodeKind::Assign {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    if node_mentions_any_cid(il, kids[0], blocked) {
        return true;
    }
    if !node_mentions_any_cid(il, kids[1], blocked) {
        return false;
    }
    if let (NodeKind::Var, Payload::Cid(cid)) = (il.kind(kids[0]), il.node(kids[0]).payload) {
        blocked.insert(cid);
    }
    false
}

fn call_may_mutate_blocked_cid(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    blocked: &FxHashSet<u32>,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    if let Some(receiver) = receiver_mutation_call_receiver(il, interner, node) {
        return node_mentions_any_cid(il, receiver, blocked);
    }
    if let Payload::Builtin(builtin) = il.node(node).payload {
        if admitted_builtin_semantics_at_call(il, node, builtin) {
            return false;
        }
        return il
            .children(node)
            .iter()
            .any(|&arg| node_mentions_any_cid(il, arg, blocked));
    }
    opaque_argument_escape_args(il, node).is_some_and(|args| {
        args.iter()
            .any(|&arg| node_mentions_any_cid(il, arg, blocked))
    })
}

fn collect_cids(il: &Il, node: NodeId, out: &mut FxHashSet<u32>) {
    if let (NodeKind::Var, Payload::Cid(cid)) = (il.kind(node), il.node(node).payload) {
        out.insert(cid);
    }
    for &child in il.children(node) {
        collect_cids(il, child, out);
    }
}

fn parent_of(parents: &[Option<NodeId>], node: NodeId) -> Option<NodeId> {
    parents.get(node.0 as usize).copied().flatten()
}

pub(crate) fn build_parent_index(il: &Il) -> Vec<Option<NodeId>> {
    let mut parents = vec![None; il.nodes.len()];
    for idx in 0..il.nodes.len() {
        let parent = NodeId(idx as u32);
        for &child in il.children(parent) {
            if let Some(slot) = parents.get_mut(child.0 as usize) {
                *slot = Some(parent);
            }
        }
    }
    parents
}

pub(crate) fn subtree_spans_within(il: &Il, node: NodeId, span: Span) -> bool {
    let node_span = il.node(node).span;
    if node_span.file != span.file
        || node_span.start_line < span.start_line
        || node_span.end_line > span.end_line
        || node_span.start_byte < span.start_byte
        || node_span.end_byte > span.end_byte
    {
        return false;
    }
    il.children(node)
        .iter()
        .all(|&child| subtree_spans_within(il, child, span))
}

/// Pre-order DFS collecting all descendant node ids of `root` (inclusive).
fn collect_pre(il: &Il, root: NodeId, out: &mut Vec<NodeId>) {
    out.push(root);
    for &c in il.children(root) {
        collect_pre(il, c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        stable_symbol_hash, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
        EvidenceProvenance, EvidenceRecord, EvidenceStatus, FileId, FileMeta, IlBuilder,
        ImportEvidenceKind, LibraryApiEvidenceKind, SequenceSurfaceKind, SourceCallKind,
        SourceFactKind, SourceOperatorKind, Span, SymbolEvidenceKind, UnitKind,
    };
    use nose_semantics::{
        library_api_callee_contract_hash, library_api_contract_id_hash,
        library_free_function_builtin_contract, library_java_collection_factory_contract,
        library_js_like_map_constructor_contract, library_js_like_set_constructor_contract,
        library_method_call_contract, FIRST_PARTY_PACK_ID,
    };

    fn sp(line: u32) -> Span {
        Span::new(FileId(0), line, line, line, line)
    }

    fn evidence(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor,
            kind,
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("test")),
            },
            dependencies,
            status: EvidenceStatus::Asserted,
        }
    }

    fn library_api_contract_evidence(
        id: u32,
        call_span: Span,
        contract_id: nose_semantics::LibraryApiContractId,
        callee: LibraryApiCalleeContract,
        arity: u16,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        evidence(
            id,
            EvidenceAnchor::node(call_span, NodeKind::Call),
            EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                contract_hash: library_api_contract_id_hash(contract_id),
                callee_hash: library_api_callee_contract_hash(callee),
                arity,
            }),
            dependencies,
        )
    }

    fn method_call_library_api_evidence(
        id: u32,
        lang: Lang,
        method: &str,
        call_span: Span,
        arity: usize,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        let contract =
            library_method_call_contract(lang, method, arity).expect("method call contract");
        library_api_contract_evidence(
            id,
            call_span,
            contract.id,
            contract.callee,
            arity as u16,
            dependencies,
        )
    }

    /// Push the `List.of(…)`-shaped factory contract plus the dependent `contains`
    /// method-call evidence used by the Java collection-factory tests.
    fn push_java_factory_contract_evidence(
        il: &mut Il,
        contract_id: nose_semantics::LibraryApiContractId,
        callee: LibraryApiCalleeContract,
    ) {
        il.evidence.push(library_api_contract_evidence(
            2,
            sp(25),
            contract_id,
            callee,
            2,
            vec![EvidenceId(1)],
        ));
        il.evidence.push(method_call_library_api_evidence(
            3,
            Lang::Java,
            "contains",
            sp(28),
            1,
            vec![EvidenceId(2)],
        ));
    }

    fn js_new_set_il(interner: &Interner) -> (Il, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let set = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Set")),
            sp(10),
            &[],
        );
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(11), &[]);
        let array = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(12),
            &[one],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(13), &[set, array]);
        let root = b.add(NodeKind::Block, Payload::None, sp(13), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.js".into(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::source_span(sp(13)),
            EvidenceKind::Source(SourceFactKind::Call(SourceCallKind::Construct)),
            Vec::new(),
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(sp(10), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("Set"),
            }),
            Vec::new(),
        ));
        il.evidence.push(evidence(
            2,
            EvidenceAnchor::sequence(sp(12)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            Vec::new(),
        ));
        (il, call)
    }

    fn js_typeof_call_il(interner: &Interner) -> (Il, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("typeof")),
            sp(42),
            &[],
        );
        let arg = b.add(NodeKind::Lit, Payload::LitInt(1), sp(43), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(44), &[callee, arg]);
        let root = b.add(NodeKind::Block, Payload::None, sp(44), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, call)
    }

    fn raw_array_seq_il(interner: &Interner) -> (Il, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(60), &[]);
        let seq = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(61),
            &[one],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(59), &[seq]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t.js".into(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, seq)
    }

    fn ts_contains_call_il(interner: &Interner) -> (Il, NodeId, Span) {
        let mut b = IlBuilder::new(FileId(0));
        let receiver_span = sp(50);
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("xs")),
            receiver_span,
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("includes")),
            sp(51),
            &[receiver],
        );
        let item = b.add(NodeKind::Lit, Payload::LitInt(7), sp(52), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(53), &[callee, item]);
        let root = b.add(NodeKind::Block, Payload::None, sp(49), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, call, receiver_span)
    }

    fn canonical_python_abs_il() -> (Il, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let arg = b.add(NodeKind::Lit, Payload::LitInt(-1), sp(71), &[]);
        let call = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Abs),
            sp(72),
            &[arg],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(70), &[call]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        (il, call)
    }

    #[test]
    fn strict_exact_sequence_surfaces_require_evidence() {
        let interner = Interner::new();
        let (mut il, seq) = raw_array_seq_il(&interner);
        let facts = StrictFacts::collect(&il, &interner);

        assert!(!strict_exact_safe_tree(&il, &interner, &facts, seq));
        assert!(!strict_exact_membership_collection_safe(
            &il, &interner, &facts, seq
        ));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(61)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);

        assert!(strict_exact_safe_tree(&il, &interner, &facts, seq));
        assert!(strict_exact_membership_collection_safe(
            &il, &interner, &facts, seq
        ));
    }

    #[test]
    fn strict_exact_typeof_requires_source_operator_evidence() {
        let interner = Interner::new();
        let (mut il, call) = js_typeof_call_il(&interner);
        let facts = StrictFacts::collect(&il, &interner);

        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "Call(Var(\"typeof\"), arg) must not be exact-safe by spelling alone"
        );

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::source_span(sp(44)),
            EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::Typeof)),
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);

        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn strict_exact_raw_builtin_payload_requires_admission() {
        let interner = Interner::new();
        let (mut il, call) = canonical_python_abs_il();
        let facts = StrictFacts::collect(&il, &interner);

        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "canonical Abs payload alone must not make a call strict-exact safe"
        );

        let contract = library_free_function_builtin_contract(Lang::Python, "abs", 1)
            .expect("Python abs contract");
        il.evidence.push(library_api_contract_evidence(
            0,
            sp(72),
            contract.id,
            contract.callee,
            1,
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));
    }

    #[test]
    fn function_binding_safe_raw_builtin_payload_requires_admission() {
        let interner = Interner::new();
        let (mut il, call) = canonical_python_abs_il();
        let facts = StrictFacts::collect(&il, &interner);

        assert!(
            !function_binding_safe(&il, &interner, &facts, call, call),
            "function binding safety must not trust a raw canonical Abs payload"
        );

        let contract = library_free_function_builtin_contract(Lang::Python, "abs", 1)
            .expect("Python abs contract");
        il.evidence.push(library_api_contract_evidence(
            0,
            sp(72),
            contract.id,
            contract.callee,
            1,
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(function_binding_safe(&il, &interner, &facts, call, call));
    }

    #[test]
    fn raw_append_payload_without_effect_does_not_bypass_mutation_blocking() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Cid(7), sp(80), &[]);
        let appended = b.add(NodeKind::Lit, Payload::LitInt(1), sp(81), &[]);
        let append = b.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp(82),
            &[receiver, appended],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(79), &[append]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        let mut blocked = FxHashSet::default();
        blocked.insert(7);

        assert!(
            call_may_mutate_blocked_cid(&il, &interner, append, &blocked),
            "raw Append payload must not be treated as a proven non-mutating builtin"
        );
    }

    #[test]
    fn strict_exact_contains_consumes_receiver_domain_evidence() {
        let interner = Interner::new();
        let (mut il, call, receiver_span) = ts_contains_call_il(&interner);
        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_safe_tree(&il, &interner, &facts, call));

        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(receiver_span, NodeKind::Var),
            EvidenceKind::Domain(nose_semantics::DomainEvidence::Collection),
            Vec::new(),
        ));
        il.evidence.push(method_call_library_api_evidence(
            1,
            Lang::TypeScript,
            "includes",
            sp(53),
            1,
            vec![EvidenceId(0)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_safe_tree(&il, &interner, &facts, call));

        il.evidence.push(evidence(
            2,
            EvidenceAnchor::node(receiver_span, NodeKind::Var),
            EvidenceKind::Domain(nose_semantics::DomainEvidence::Map),
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "conflicting receiver-domain evidence must close strict exact fallback"
        );
    }

    #[test]
    fn strict_exact_contains_consumes_binding_domain_evidence() {
        let interner = Interner::new();
        let xs = interner.intern("xs");
        let mut b = IlBuilder::new(FileId(0));
        let lhs = b.add(NodeKind::Var, Payload::Cid(0), sp(30), &[]);
        let seq = b.add(NodeKind::Seq, Payload::None, sp(31), &[]);
        let assign = b.add(NodeKind::Assign, Payload::None, sp(30), &[lhs, seq]);
        let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(32), &[]);
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("includes")),
            sp(33),
            &[receiver],
        );
        let item = b.add(NodeKind::Lit, Payload::LitInt(7), sp(34), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(35), &[callee, item]);
        let root = b.add(NodeKind::Block, Payload::None, sp(29), &[assign, call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            vec![xs],
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(30), stable_symbol_hash("xs")),
            EvidenceKind::Domain(nose_semantics::DomainEvidence::Collection),
            Vec::new(),
        ));
        il.evidence.push(method_call_library_api_evidence(
            1,
            Lang::TypeScript,
            "includes",
            sp(35),
            1,
            vec![EvidenceId(0)],
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_collection_contains_call_safe(
            &il, &interner, &facts, call, callee, "includes"
        ));

        il.evidence.push(evidence(
            2,
            EvidenceAnchor::binding(sp(30), stable_symbol_hash("xs")),
            EvidenceKind::Domain(nose_semantics::DomainEvidence::Map),
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_collection_contains_call_safe(
                &il, &interner, &facts, call, callee, "includes"
            ),
            "conflicting binding-domain evidence must close strict exact receiver proof"
        );
    }

    #[test]
    fn strict_exact_contains_does_not_use_result_domain_as_exact_tree_proof() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let factory_callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("Set")),
            sp(40),
            &[],
        );
        let seed = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(41),
            &[],
        );
        let receiver = b.add(
            NodeKind::Call,
            Payload::None,
            sp(42),
            &[factory_callee, seed],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("includes")),
            sp(43),
            &[receiver],
        );
        let item = b.add(NodeKind::Lit, Payload::LitInt(7), sp(44), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(45), &[callee, item]);
        let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.ts".into(),
                lang: Lang::TypeScript,
            },
            Vec::new(),
            Vec::new(),
        );
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "call-result receiver must not be collection-like without domain evidence"
        );

        let api = library_js_like_set_constructor_contract(Lang::TypeScript, "Set").unwrap();
        il.evidence.push(library_api_contract_evidence(
            0,
            sp(42),
            api.id,
            api.callee,
            1,
            Vec::new(),
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(sp(42), NodeKind::Call),
            EvidenceKind::Domain(nose_semantics::DomainEvidence::Set),
            vec![EvidenceId(0)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "result-domain evidence proves the call result's receiver domain, not the exact-safety of the receiver expression"
        );

        il.evidence[0].status = EvidenceStatus::Ambiguous;
        let facts = StrictFacts::collect(&il, &interner);
        assert!(
            !strict_exact_safe_tree(&il, &interner, &facts, call),
            "ambiguous LibraryApi dependency must close strict exact receiver proof"
        );
    }

    #[test]
    fn strict_exact_js_constructor_requires_library_api_evidence() {
        let interner = Interner::new();
        let (mut il, call) = js_new_set_il(&interner);
        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_set_constructor_collection_safe(
            &il, &interner, &facts, call
        ));

        let wrong = library_js_like_map_constructor_contract(Lang::JavaScript, "Map").unwrap();
        il.evidence.push(library_api_contract_evidence(
            3,
            sp(13),
            wrong.id,
            wrong.callee,
            1,
            vec![EvidenceId(0), EvidenceId(1)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_set_constructor_collection_safe(
            &il, &interner, &facts, call
        ));

        let (mut il, call) = js_new_set_il(&interner);
        let set = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
        il.evidence.push(library_api_contract_evidence(
            3,
            sp(13),
            set.id,
            set.callee,
            1,
            vec![EvidenceId(0), EvidenceId(1)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_set_constructor_collection_safe(
            &il, &interner, &facts, call
        ));
    }

    #[test]
    fn strict_exact_python_builtin_factory_requires_library_api_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("list")),
            sp(40),
            &[],
        );
        let item = b.add(NodeKind::Lit, Payload::LitInt(1), sp(41), &[]);
        let seq = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(42),
            &[item],
        );
        let call = b.add(NodeKind::Call, Payload::None, sp(43), &[callee, seq]);
        let root = b.add(NodeKind::Block, Payload::None, sp(39), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.py".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::sequence(sp(42)),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            Vec::new(),
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_python_collection_factory_safe(
            &il, &interner, &facts, call
        ));

        let contract = library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(sp(40), NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash("list"),
            }),
            Vec::new(),
        ));
        il.evidence.push(library_api_contract_evidence(
            2,
            sp(43),
            contract.id,
            contract.callee,
            1,
            vec![EvidenceId(1)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_python_collection_factory_safe(
            &il, &interner, &facts, call
        ));
    }

    #[test]
    fn strict_exact_java_collection_factory_uses_library_api_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let list = interner.intern("List");
        let lhs = b.add(NodeKind::Var, Payload::Name(list), sp(20), &[]);
        let rhs = b.add(NodeKind::Seq, Payload::None, sp(20), &[]);
        let import = b.add(NodeKind::Assign, Payload::None, sp(20), &[lhs, rhs]);
        let receiver = b.add(NodeKind::Var, Payload::Name(list), sp(21), &[]);
        let factory_callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp(22),
            &[receiver],
        );
        let left = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("red")),
            sp(23),
            &[],
        );
        let right = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("blue")),
            sp(24),
            &[],
        );
        let factory = b.add(
            NodeKind::Call,
            Payload::None,
            sp(25),
            &[factory_callee, left, right],
        );
        let contains_callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("contains")),
            sp(26),
            &[factory],
        );
        let value = b.add(NodeKind::Var, Payload::Cid(0), sp(27), &[]);
        let contains = b.add(
            NodeKind::Call,
            Payload::None,
            sp(28),
            &[contains_callee, value],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(20), &[import, contains]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
            .expect("List.of contract");
        let binding_symbol = EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash("java.util"),
            exported_hash: stable_symbol_hash("List"),
        });
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::binding(sp(20), stable_symbol_hash("List")),
            binding_symbol,
            Vec::new(),
        ));
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(sp(21), NodeKind::Var),
            binding_symbol,
            vec![EvidenceId(0)],
        ));
        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_java_collection_factory_safe(
            &il, &interner, &facts, factory
        ));
        assert!(!strict_exact_safe_tree(&il, &interner, &facts, contains));

        push_java_factory_contract_evidence(&mut il, contract.id, contract.callee);
        let facts = StrictFacts::collect(&il, &interner);
        assert!(strict_exact_java_collection_factory_safe(
            &il, &interner, &facts, factory
        ));
        assert!(strict_exact_safe_tree(&il, &interner, &facts, contains));

        let wrong = library_js_like_set_constructor_contract(Lang::JavaScript, "Set").unwrap();
        il.evidence.pop();
        il.evidence.pop();
        push_java_factory_contract_evidence(&mut il, wrong.id, wrong.callee);
        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_java_collection_factory_safe(
            &il, &interner, &facts, factory
        ));
        assert!(!strict_exact_safe_tree(&il, &interner, &facts, contains));
    }

    #[test]
    fn strict_exact_java_map_provider_proof_does_not_replace_receiver_identity() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("FakeMap")),
            sp(30),
            &[],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp(31),
            &[receiver],
        );
        let key = b.add(
            NodeKind::Lit,
            Payload::LitStr(stable_symbol_hash("k")),
            sp(32),
            &[],
        );
        let value = b.add(NodeKind::Lit, Payload::LitInt(1), sp(33), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(34), &[callee, key, value]);
        let root = b.add(NodeKind::Block, Payload::None, sp(34), &[call]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.java".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(evidence(
            0,
            EvidenceAnchor::node(sp(34), NodeKind::Call),
            EvidenceKind::Import(ImportEvidenceKind::ImmutableLiteralExport {
                module_hash: stable_symbol_hash("t"),
                exported_hash: stable_symbol_hash("VALUES"),
                root_kind: NodeKind::Call,
            }),
            Vec::new(),
        ));

        let facts = StrictFacts::collect(&il, &interner);
        assert!(!strict_exact_java_map_factory_safe(
            &il, &interner, &facts, call
        ));
    }

    fn lowered_java_unit_with_features(
        src: &str,
        interner: &Interner,
        kind: UnitKind,
        name: &str,
        shape_features: bool,
        abstraction_witnesses: bool,
    ) -> UnitFeat {
        let raw =
            nose_frontend::lower_source(FileId(0), "T.java", src.as_bytes(), Lang::Java, interner)
                .expect("lower Java source");
        let il =
            nose_normalize::normalize(&raw, interner, &nose_normalize::NormalizeOptions::default());
        let seeds = crate::minhash::seeds(64);
        let units = extract(
            &il,
            interner,
            &seeds,
            1,
            1,
            true,
            ExtractFeatures {
                shape_features,
                abstraction_witnesses,
            },
        );
        units
            .into_iter()
            .find(|unit| unit.kind == kind && unit.name.as_deref() == Some(name))
            .expect("requested Java unit")
    }

    fn lowered_java_unit(src: &str, interner: &Interner, kind: UnitKind, name: &str) -> UnitFeat {
        lowered_java_unit_with_features(src, interner, kind, name, false, false)
    }

    fn lowered_java_method_unit(src: &str, interner: &Interner) -> UnitFeat {
        lowered_java_unit(src, interner, UnitKind::Method, "f")
    }

    fn lowered_fragment_units(src: &str, lang: Lang, interner: &Interner) -> Vec<UnitFeat> {
        let raw =
            nose_frontend::lower_source(FileId(0), "fragment", src.as_bytes(), lang, interner)
                .expect("lower source");
        let il =
            nose_normalize::normalize(&raw, interner, &nose_normalize::NormalizeOptions::default());
        let seeds = crate::minhash::seeds(64);
        extract(
            &il,
            interner,
            &seeds,
            99,
            999,
            true,
            ExtractFeatures {
                shape_features: false,
                abstraction_witnesses: false,
            },
        )
        .into_iter()
        .filter(|unit| unit.fragment_kind.is_some())
        .collect()
    }

    #[test]
    fn exact_fragment_collector_produces_contract_recognized_direct_return() {
        let interner = Interner::new();
        let fragments = lowered_fragment_units(
            "function f(x) { console.log(x); return (x + 1) * (x + 2); }\n",
            Lang::JavaScript,
            &interner,
        );

        assert!(
            fragments
                .iter()
                .any(|unit| unit.fragment_kind == Some(FragmentKind::DirectReturn)),
            "contract-first collector should still produce the exact direct-return fragment"
        );
    }

    #[test]
    fn abstraction_tokens_do_not_depend_on_shape_features() {
        let interner = Interner::new();
        let left = lowered_java_unit_with_features(
            "class Left { static int f() { return 1; } }\n",
            &interner,
            UnitKind::Method,
            "f",
            false,
            true,
        );
        let right = lowered_java_unit_with_features(
            "class Right { static int f() { return 2; } }\n",
            &interner,
            UnitKind::Method,
            "f",
            false,
            true,
        );

        assert!(
            left.shapes.is_empty(),
            "shape features should stay disabled"
        );
        assert!(
            left.linear.is_empty(),
            "linear shape features should stay disabled"
        );
        assert!(
            !left.abstraction_tokens.is_empty() && !right.abstraction_tokens.is_empty(),
            "abstraction witnesses need their own tokens even when shape features are off"
        );
        let witness = abstraction_family_witness([&left, &right])
            .expect("one changed integer literal should produce an abstraction witness");
        assert_eq!(witness.basis, "family");
        assert_eq!(witness.members_checked, 2);
        assert_eq!(witness.reason_code, "literal-abstracted");
        assert_eq!(witness.holes[0].left, "int-literal");
        assert_eq!(witness.holes[0].right, "int-literal");
    }

    #[test]
    fn abstraction_family_witness_requires_one_shared_hole_position() {
        let interner = Interner::new();
        let base = lowered_java_unit_with_features(
            "class Base { static int f(int x) { int a = 1; int b = 2; return x + a + b; } }\n",
            &interner,
            UnitKind::Method,
            "f",
            false,
            true,
        );
        let same_hole = lowered_java_unit_with_features(
            "class SameHole { static int f(int x) { int a = 3; int b = 2; return x + a + b; } }\n",
            &interner,
            UnitKind::Method,
            "f",
            false,
            true,
        );
        let also_same_hole = lowered_java_unit_with_features(
            "class AlsoSameHole { static int f(int x) { int a = 4; int b = 2; return x + a + b; } }\n",
            &interner,
            UnitKind::Method,
            "f",
            false,
            true,
        );
        let witness = abstraction_family_witness([&base, &same_hole, &also_same_hole])
            .expect("same literal position across the family should produce a witness");
        assert_eq!(witness.basis, "family");
        assert_eq!(witness.members_checked, 3);
        assert_eq!(witness.reason_code, "literal-abstracted");
        assert_eq!(witness.holes[0].observed, vec!["int-literal"]);
    }

    #[test]
    fn lowered_java_static_collection_factories_share_exact_fingerprint() {
        let interner = Interner::new();
        let list = lowered_java_method_unit(
            "import java.util.List;\n\nclass JavaListOf { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\n",
            &interner,
        );
        let set = lowered_java_method_unit(
            "import java.util.Set;\n\nclass JavaSetOf { static boolean f(String value, String other) { return Set.of(\"red\", \"blue\").contains(value); } }\n",
            &interner,
        );
        let arrays = lowered_java_method_unit(
            "import java.util.Arrays;\n\nclass JavaArraysAsList { static boolean f(String value, String other) { return Arrays.asList(\"red\", \"blue\").contains(value); } }\n",
            &interner,
        );
        let module_method = lowered_java_unit(
            "import java.util.List;\n\nclass ModuleList {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleList(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n",
            &interner,
            UnitKind::Method,
            "moduleList",
        );
        assert!(list.exact_safe, "List.of method must stay exact-safe");
        assert!(set.exact_safe, "Set.of method must stay exact-safe");
        assert!(
            arrays.exact_safe,
            "Arrays.asList method must stay exact-safe"
        );
        assert!(
            module_method.exact_safe,
            "class-level List.of binding must stay exact-safe"
        );
        assert!(
            list.value.len() >= EXACT_VALUE_MIN,
            "List.of method should produce a dense semantic fingerprint"
        );
        assert_eq!(list.value, set.value);
        assert_eq!(list.value, arrays.value);
        assert_eq!(list.value, module_method.value);
    }
}
