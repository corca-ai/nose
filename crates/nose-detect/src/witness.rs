//! Graded equivalence witness (#315): anti-unification over two units' value DAGs.
//!
//! The exact channel proves "these compute the same thing" (equal fingerprint). The
//! near channel only scores similarity. This module bridges them: given two near
//! units' value DAGs, it computes their *least general generalization* — aligns the
//! two graphs node-by-node and reports the spots where they differ as **holes**. The
//! result grades the near family's witness from a bare score to "equal **except at
//! these k holes**", with each hole's value class and a soundness-relevant referent
//! check.
//!
//! It is **fail-closed**: a name both units consume that resolves to different
//! referents demotes the claim (`referent-mismatch`); a name that cannot be resolved
//! is reported as a scoped caveat; a pair too large or too deep to align soundly
//! yields no witness at all (`None`) rather than a guessed one. Recognized divergence
//! shapes (reordered effects, one-sided supersets, fragment containment) are reported
//! as patterns instead of noisy positional holes.
//!
//! proof-obligation: detect.graded_witness

use nose_normalize::{bin_is_commutative, ValueDag, VgOp, VgSinkKind};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

/// Per-pair guards. A witness is best-effort enrichment, so a pathological pair
/// (huge generated file, a degenerately deep expression) fails closed to *no*
/// witness rather than burning time or risking the worker stack.
const MAX_NODES: usize = 6_000;
const MAX_NODE_PRODUCT: u64 = 16_000_000;
const MAX_DEPTH: u32 = 1_000;
/// Holes are itemized up to this many; the count `holes` is always exact.
const MAX_SPOTS: usize = 24;

/// The graded witness attached to a near family: how equal its two representative
/// copies really are, beyond the similarity score.
#[derive(Clone, Serialize)]
pub struct GradedWitness {
    /// `k` — the number of spots where the two value DAGs differ. `0` means equal in
    /// the modeled fraction; small `k` means "equal except these few parameters".
    pub holes: usize,
    /// Per-hole detail (capped at `MAX_SPOTS`); source text is filled by the
    /// presentation layer, which has file access.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spots: Vec<WitnessHole>,
    /// Recognized divergence shapes: `effects-reordered`, `sink-superset-a/b`,
    /// `fragment-containment`, `low-substance`, `referent-mismatch`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<&'static str>,
    /// Names BOTH units consume that resolve to different referents (same-named but
    /// behaviorally distinct — e.g. `equals` on two classes). Non-empty ⇒ the witness
    /// is demoted: the copies are NOT equal-modulo-holes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub referent_mismatches: Vec<String>,
    /// Names unresolved on at least one side — the claim is scoped past these (a
    /// reviewer should confirm they denote the same thing).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub caveat_names: Vec<String>,
    /// The two copies' **value graphs** are equal except at the listed holes, every
    /// hole is a small value-leaf, the behavior sinks aligned, and no referent
    /// mismatched — the strongest grade this channel makes. Scope: this is a claim
    /// about the unit body the value graph models, NOT its definition-site decorators,
    /// annotations, or signature (a `@deco(x)` vs `@deco(y)` difference outside the
    /// compared body is not seen — see `docs/graded-witness.md`). Near-channel
    /// evidence, not an exact-channel proof.
    pub equal_modulo_holes: bool,
    /// Either unit passed lossy lowering, so "equal" means equal in the *modeled*
    /// fraction; identically-keyed unmodeled constructs may still differ.
    pub modeled_caveat: bool,
}

/// One differing spot between the two value DAGs — the hole an extracted helper would
/// parameterize, classified by what kind of value differs.
#[derive(Clone, Serialize)]
pub struct WitnessHole {
    /// `literal` / `input` / `field` / `call` / `lambda` / `operator` / `expr` =
    /// value-leaf differences (clean parameters); `arity` / `shape` / `unmodeled` /
    /// `extra-sink` = structural divergence (not a clean parameter).
    pub class: &'static str,
    /// Source line range of the spot in the first copy, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_lines: Option<(u32, u32)>,
    /// Source line range in the second copy, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b_lines: Option<(u32, u32)>,
    /// The spot's value feeds an effect/throw/break (ordered behavior), so swapping it
    /// is observable — a hole here is not freely parameterizable.
    pub effect: bool,
    /// Trimmed, length-capped source text of the spot in the first copy (filled by the
    /// presentation layer).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub a_text: String,
    /// Same, second copy.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub b_text: String,
}

/// A value DAG with the two derived per-node facts the alignment needs: tree weight
/// (subtree size, for ranking a hole's mass) and whether a node feeds an ordered
/// effect sink (so a hole there is behaviorally load-bearing).
struct Dag<'a> {
    dag: &'a ValueDag,
    weight: Vec<u32>,
    effectish: Vec<bool>,
}

impl<'a> Dag<'a> {
    fn new(dag: &'a ValueDag) -> Self {
        let n = dag.nodes.len();
        // Args reference earlier indices (hash-consed), so one forward pass suffices.
        let mut weight = vec![0u32; n];
        for i in 0..n {
            let mut w: u64 = 1;
            for &a in &dag.nodes[i].args {
                w += u64::from(weight[a as usize]);
            }
            weight[i] = u32::try_from(w).unwrap_or(u32::MAX);
        }
        let mut effectish = vec![false; n];
        let mut stack: Vec<u32> = dag
            .sinks
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    VgSinkKind::Effect | VgSinkKind::Break | VgSinkKind::Throw
                )
            })
            .map(|s| s.value)
            .collect();
        while let Some(v) = stack.pop() {
            if effectish[v as usize] {
                continue;
            }
            effectish[v as usize] = true;
            stack.extend(dag.nodes[v as usize].args.iter().copied());
        }
        Dag {
            dag,
            weight,
            effectish,
        }
    }
}

/// `(node, on_a)`; `node == NONE` marks an absent side (a one-sided hole).
const NONE: u32 = u32::MAX;

struct Au<'a> {
    a: &'a Dag<'a>,
    b: &'a Dag<'a>,
    visited: FxHashSet<(u32, u32)>,
    /// `(a_node, b_node)`; either may be [`NONE`] for a one-sided hole.
    holes: Vec<(u32, u32)>,
    hole_seen: FxHashSet<(u32, u32)>,
    matched_a: FxHashSet<u32>,
    matched_b: FxHashSet<u32>,
    /// Set when recursion hit [`MAX_DEPTH`] — the witness fails closed.
    truncated: bool,
}

impl<'a> Au<'a> {
    fn new(a: &'a Dag<'a>, b: &'a Dag<'a>) -> Self {
        Au {
            a,
            b,
            visited: FxHashSet::default(),
            holes: Vec::new(),
            hole_seen: FxHashSet::default(),
            matched_a: FxHashSet::default(),
            matched_b: FxHashSet::default(),
            truncated: false,
        }
    }

    fn mark(matched: &mut FxHashSet<u32>, nodes: &[nose_normalize::VgNode], root: u32) {
        let mut stack = vec![root];
        while let Some(v) = stack.pop() {
            if !matched.insert(v) {
                continue;
            }
            stack.extend(nodes[v as usize].args.iter().copied());
        }
    }

    fn mark_subtree(&mut self, x: u32, y: u32) {
        Self::mark(&mut self.matched_a, &self.a.dag.nodes, x);
        Self::mark(&mut self.matched_b, &self.b.dag.nodes, y);
    }

    fn hole(&mut self, x: u32, y: u32) {
        if self.hole_seen.insert((x, y)) {
            self.holes.push((x, y));
        }
    }

    fn one_sided(&mut self, node: u32, on_a: bool) {
        let key = if on_a { (node, NONE) } else { (NONE, node) };
        if self.hole_seen.insert(key) {
            self.holes.push(key);
        }
    }

    /// Flatten a commutative-operator chain rooted at `root` (same `key`) into its leaf
    /// operands, so two chains can be matched as multisets rather than positionally.
    fn flatten(dag: &Dag<'_>, root: u32, key: u64, out: &mut Vec<u32>) {
        let mut stack = vec![root];
        while let Some(v) = stack.pop() {
            let n = &dag.dag.nodes[v as usize];
            if n.op == VgOp::Bin && n.key == key {
                for &a in n.args.iter().rev() {
                    stack.push(a);
                }
            } else {
                out.push(v);
            }
        }
    }

    fn unify(&mut self, x: u32, y: u32, depth: u32) {
        if depth > MAX_DEPTH {
            self.truncated = true;
            return;
        }
        if !self.visited.insert((x, y)) {
            return;
        }
        let nx = &self.a.dag.nodes[x as usize];
        let ny = &self.b.dag.nodes[y as usize];
        if nx.hash == ny.hash {
            self.mark_subtree(x, y);
            return;
        }
        if nx.op != ny.op || nx.key != ny.key {
            self.hole(x, y);
            return;
        }
        // Same op and key.
        if nx.op == VgOp::Bin && bin_is_commutative(nx.key) {
            self.unify_commutative(x, y, depth);
            return;
        }
        if nx.args.len() != ny.args.len() {
            self.hole(x, y);
            return;
        }
        self.matched_a.insert(x);
        self.matched_b.insert(y);
        // Recurse on each aligned argument pair. Index rather than clone the two arg
        // vecs: this is the per-node hot path, and the immutable borrow of each node is
        // released before the `&mut self` recursive call.
        let arity = self.a.dag.nodes[x as usize].args.len();
        for i in 0..arity {
            let cx = self.a.dag.nodes[x as usize].args[i];
            let cy = self.b.dag.nodes[y as usize].args[i];
            self.unify(cx, cy, depth + 1);
        }
    }

    /// Align a commutative chain: pair identical leaves by hash multiset, recurse on
    /// the hash-sorted leftovers, and count arity gaps as one-sided holes.
    fn unify_commutative(&mut self, x: u32, y: u32, depth: u32) {
        let key = self.a.dag.nodes[x as usize].key;
        let (mut la, mut lb) = (Vec::new(), Vec::new());
        Self::flatten(self.a, x, key, &mut la);
        Self::flatten(self.b, y, key, &mut lb);
        self.matched_a.insert(x);
        self.matched_b.insert(y);
        let mut by_hash: FxHashMap<u64, Vec<u32>> = FxHashMap::default();
        for &l in &lb {
            by_hash
                .entry(self.b.dag.nodes[l as usize].hash)
                .or_default()
                .push(l);
        }
        let mut rest_a: Vec<u32> = Vec::new();
        for &l in &la {
            let h = self.a.dag.nodes[l as usize].hash;
            if let Some(m) = by_hash.get_mut(&h).and_then(Vec::pop) {
                self.mark_subtree(l, m);
            } else {
                rest_a.push(l);
            }
        }
        let mut rest_b: Vec<u32> = by_hash.into_values().flatten().collect();
        rest_a.sort_unstable_by_key(|&l| self.a.dag.nodes[l as usize].hash);
        rest_b.sort_unstable_by_key(|&l| self.b.dag.nodes[l as usize].hash);
        let common = rest_a.len().min(rest_b.len());
        for i in 0..common {
            self.unify(rest_a[i], rest_b[i], depth + 1);
        }
        for &l in &rest_a[common..] {
            self.one_sided(l, true);
        }
        for &l in &rest_b[common..] {
            self.one_sided(l, false);
        }
    }
}

/// Pair the two DAGs' sinks of one `kind`. Effects align by an order-preserving LCS
/// over their hash sequences (so identical effects align without crossings); leftover
/// hash-equal effects at different positions signal a reorder; the rest pair by
/// position so a genuinely-differing sink is still unified to find its hole. Returns
/// the paired `(a_value, b_value)` roots plus the unpaired extras on each side.
struct SinkPairing {
    pairs: Vec<(u32, u32)>,
    extra_a: Vec<u32>,
    extra_b: Vec<u32>,
    reordered: bool,
}

fn pair_sinks(a: &Dag<'_>, b: &Dag<'_>, kind: VgSinkKind) -> SinkPairing {
    let want = |k: VgSinkKind| k as u8 == kind as u8;
    let mut sa: Vec<_> = a.dag.sinks.iter().filter(|s| want(s.kind)).collect();
    let mut sb: Vec<_> = b.dag.sinks.iter().filter(|s| want(s.kind)).collect();
    let mut reordered = false;
    if matches!(kind, VgSinkKind::Effect) {
        sa.sort_by_key(|s| (s.effect_ord, a.dag.nodes[s.value as usize].hash));
        sb.sort_by_key(|s| (s.effect_ord, b.dag.nodes[s.value as usize].hash));
        let ha: Vec<u64> = sa
            .iter()
            .map(|s| a.dag.nodes[s.value as usize].hash)
            .collect();
        let hb: Vec<u64> = sb
            .iter()
            .map(|s| b.dag.nodes[s.value as usize].hash)
            .collect();
        let (n, m) = (ha.len(), hb.len());
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        let mut in_lcs_a = vec![false; n];
        let mut in_lcs_b = vec![false; m];
        if n.saturating_mul(m) <= 1_000_000 {
            let mut dp = vec![0u32; (n + 1) * (m + 1)];
            for i in (0..n).rev() {
                for j in (0..m).rev() {
                    dp[i * (m + 1) + j] = if ha[i] == hb[j] {
                        dp[(i + 1) * (m + 1) + j + 1] + 1
                    } else {
                        dp[(i + 1) * (m + 1) + j].max(dp[i * (m + 1) + j + 1])
                    };
                }
            }
            let (mut i, mut j) = (0, 0);
            while i < n && j < m {
                if ha[i] == hb[j] {
                    pairs.push((i, j));
                    in_lcs_a[i] = true;
                    in_lcs_b[j] = true;
                    i += 1;
                    j += 1;
                } else if dp[(i + 1) * (m + 1) + j] >= dp[i * (m + 1) + j + 1] {
                    i += 1;
                } else {
                    j += 1;
                }
            }
        }
        let mut by_hash: FxHashMap<u64, Vec<usize>> = FxHashMap::default();
        for j in (0..m).rev() {
            if !in_lcs_b[j] {
                by_hash.entry(hb[j]).or_default().push(j);
            }
        }
        let mut rest_a: Vec<usize> = Vec::new();
        let mut used_b: FxHashSet<usize> = FxHashSet::default();
        for i in 0..n {
            if in_lcs_a[i] {
                continue;
            }
            if let Some(j) = by_hash.get_mut(&ha[i]).and_then(Vec::pop) {
                pairs.push((i, j));
                used_b.insert(j);
                reordered = true;
            } else {
                rest_a.push(i);
            }
        }
        let rest_b: Vec<usize> = (0..m)
            .filter(|j| !in_lcs_b[*j] && !used_b.contains(j))
            .collect();
        let common = rest_a.len().min(rest_b.len());
        for i in 0..common {
            pairs.push((rest_a[i], rest_b[i]));
        }
        return SinkPairing {
            pairs: pairs
                .into_iter()
                .map(|(i, j)| (sa[i].value, sb[j].value))
                .collect(),
            extra_a: rest_a[common..].iter().map(|&i| sa[i].value).collect(),
            extra_b: rest_b[common..].iter().map(|&j| sb[j].value).collect(),
            reordered,
        };
    }
    if matches!(kind, VgSinkKind::Cond) {
        sa.sort_by_key(|s| a.dag.nodes[s.value as usize].hash);
        sb.sort_by_key(|s| b.dag.nodes[s.value as usize].hash);
    }
    let common = sa.len().min(sb.len());
    SinkPairing {
        pairs: sa[..common]
            .iter()
            .zip(&sb[..common])
            .map(|(x, y)| (x.value, y.value))
            .collect(),
        extra_a: sa[common..].iter().map(|s| s.value).collect(),
        extra_b: sb[common..].iter().map(|s| s.value).collect(),
        reordered,
    }
}

fn classify(a: &Dag<'_>, b: &Dag<'_>, x: u32, y: u32) -> &'static str {
    if x == NONE || y == NONE {
        return "arity";
    }
    let (ta, tb) = (a.dag.nodes[x as usize].op, b.dag.nodes[y as usize].op);
    if ta != tb {
        return "shape";
    }
    match ta {
        VgOp::Const => "literal",
        VgOp::Input => "input",
        VgOp::Field => "field",
        VgOp::Call => "call",
        VgOp::Lambda => "lambda",
        VgOp::Bin | VgOp::Un => "operator",
        VgOp::Opaque | VgOp::Formula | VgOp::Recurrence | VgOp::Loop => "unmodeled",
        _ => "expr",
    }
}

fn lines(d: &Dag<'_>, v: u32) -> Option<(u32, u32)> {
    if v == NONE {
        return None;
    }
    let n = &d.dag.nodes[v as usize];
    (n.line_start != 0).then_some((n.line_start, n.line_end))
}

/// Compare the two units' resolved referents. A name both consume that resolves to
/// disjoint referent sets is a mismatch (fail-closed); a name unresolved on either
/// side is a scoped caveat.
fn compare_referents(a: &ValueDag, b: &ValueDag) -> (Vec<String>, Vec<String>) {
    type Acc = FxHashMap<u64, (String, FxHashSet<u64>, bool)>;
    let collect = |dag: &ValueDag| -> Acc {
        let mut m: Acc = FxHashMap::default();
        for r in &dag.referents {
            let e = m
                .entry(r.name_key)
                .or_insert_with(|| (r.name.clone(), FxHashSet::default(), false));
            match r.referent {
                Some(id) => {
                    e.1.insert(id);
                }
                None => e.2 = true,
            }
        }
        m
    };
    let (ra, rb) = (collect(a), collect(b));
    let (mut mism, mut caveat) = (Vec::new(), Vec::new());
    for (key, (name, set_a, unres_a)) in &ra {
        let Some((_, set_b, unres_b)) = rb.get(key) else {
            continue;
        };
        if !set_a.is_empty() && !set_b.is_empty() && set_a.is_disjoint(set_b) {
            mism.push(name.clone());
        } else if *unres_a || *unres_b {
            caveat.push(name.clone());
        }
    }
    mism.sort();
    mism.dedup();
    mism.truncate(16);
    caveat.sort();
    caveat.dedup();
    caveat.truncate(16);
    (mism, caveat)
}

/// Anti-unify two units' value DAGs into a graded witness. `a_lossy`/`b_lossy` mark
/// whether each unit passed lossy lowering (so the claim is scoped to the modeled
/// fraction). Returns `None` when the pair is too large or too deep to align soundly
/// (the witness fails closed to absent rather than guessed).
/// The aligned shape of a pair: the differing nodes (`holes`), the unmatched sinks
/// (`extra`, with the side each is on), and whether any effects were reordered.
struct Alignment {
    holes: Vec<(u32, u32)>,
    extra: Vec<(u32, bool)>,
    reordered: bool,
}

/// Pair every sink kind and anti-unify the matched roots. `None` if the alignment hit
/// the depth guard (fail-closed).
fn align(da: &Dag<'_>, db: &Dag<'_>) -> Option<Alignment> {
    let mut au = Au::new(da, db);
    let mut extra: Vec<(u32, bool)> = Vec::new();
    let mut reordered = false;
    for kind in [
        VgSinkKind::Return,
        VgSinkKind::Cond,
        VgSinkKind::Effect,
        VgSinkKind::Break,
        VgSinkKind::Throw,
    ] {
        let sp = pair_sinks(da, db, kind);
        reordered |= sp.reordered;
        extra.extend(sp.extra_a.into_iter().map(|v| (v, true)));
        extra.extend(sp.extra_b.into_iter().map(|v| (v, false)));
        for (x, y) in sp.pairs {
            au.unify(x, y, 0);
        }
    }
    if au.truncated {
        return None;
    }
    Some(Alignment {
        holes: std::mem::take(&mut au.holes),
        extra,
        reordered,
    })
}

/// Itemize the holes and unmatched sinks into [`WitnessHole`]s (text filled later),
/// returning the spots and whether every hole is a small value leaf.
fn build_spots(
    da: &Dag<'_>,
    db: &Dag<'_>,
    holes: &[(u32, u32)],
    extra: &[(u32, bool)],
) -> (Vec<WitnessHole>, bool) {
    let mut spots: Vec<WitnessHole> = Vec::new();
    let mut all_leafy = true;
    for &(x, y) in holes {
        let class = classify(da, db, x, y);
        let wa = if x == NONE { 0 } else { da.weight[x as usize] };
        let wb = if y == NONE { 0 } else { db.weight[y as usize] };
        let effect =
            (x != NONE && da.effectish[x as usize]) || (y != NONE && db.effectish[y as usize]);
        if wa > 4 || wb > 4 || matches!(class, "unmodeled" | "shape" | "arity") {
            all_leafy = false;
        }
        if spots.len() < MAX_SPOTS {
            spots.push(WitnessHole {
                class,
                a_lines: lines(da, x),
                b_lines: lines(db, y),
                effect,
                a_text: String::new(),
                b_text: String::new(),
            });
        }
    }
    for &(v, on_a) in extra {
        let line = lines(if on_a { da } else { db }, v);
        if spots.len() < MAX_SPOTS {
            spots.push(WitnessHole {
                class: "extra-sink",
                a_lines: if on_a { line } else { None },
                b_lines: if on_a { None } else { line },
                effect: true,
                a_text: String::new(),
                b_text: String::new(),
            });
        }
    }
    (spots, all_leafy)
}

/// Derive the divergence-pattern labels from the alignment and sizes.
fn derive_patterns(al: &Alignment, na: usize, nb: usize, low_substance: bool) -> Vec<&'static str> {
    let mut patterns: Vec<&'static str> = Vec::new();
    if al.reordered {
        patterns.push("effects-reordered");
    }
    if !al.extra.is_empty() && al.holes.is_empty() {
        let a_extra = al.extra.iter().any(|&(_, on_a)| on_a);
        let b_extra = al.extra.iter().any(|&(_, on_a)| !on_a);
        if a_extra && !b_extra {
            patterns.push("sink-superset-a");
        } else if b_extra && !a_extra {
            patterns.push("sink-superset-b");
        }
    }
    let skew = na.min(nb) as f64 / na.max(nb).max(1) as f64;
    if !al.extra.is_empty() && skew < 0.3 {
        patterns.push("fragment-containment");
    }
    if low_substance {
        patterns.push("low-substance");
    }
    patterns
}

/// Anti-unify two units' value DAGs into a graded witness. `a_lossy`/`b_lossy` mark
/// whether each unit passed lossy lowering (so the claim is scoped to the modeled
/// fraction). Returns `None` when the pair is too large or too deep to align soundly
/// (the witness fails closed to absent rather than guessed).
pub fn graded_witness(
    a: &ValueDag,
    b: &ValueDag,
    a_lossy: bool,
    b_lossy: bool,
) -> Option<GradedWitness> {
    let (na, nb) = (a.nodes.len(), b.nodes.len());
    if na == 0 || nb == 0 || na > MAX_NODES || nb > MAX_NODES {
        return None;
    }
    if (na as u64).saturating_mul(nb as u64) > MAX_NODE_PRODUCT {
        return None;
    }
    let (da, db) = (Dag::new(a), Dag::new(b));
    let al = align(&da, &db)?;
    let (spots, all_leafy) = build_spots(&da, &db, &al.holes, &al.extra);

    let k = al.holes.len() + al.extra.len();
    let low_substance = k > 0 && na.min(nb) < 10;
    let mut patterns = derive_patterns(&al, na, nb, low_substance);

    let (referent_mismatches, caveat_names) = compare_referents(a, b);
    if !referent_mismatches.is_empty() {
        patterns.push("referent-mismatch");
    }

    let equal_modulo_holes = al.extra.is_empty()
        && !al.reordered
        && all_leafy
        && !low_substance
        && referent_mismatches.is_empty();

    Some(GradedWitness {
        holes: k,
        spots,
        patterns,
        referent_mismatches,
        caveat_names,
        equal_modulo_holes,
        modeled_caveat: a_lossy || b_lossy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_normalize::{ValueDag, VgNode, VgReferent, VgSink};

    fn node(op: VgOp, key: u64, args: &[u32], hash: u64) -> VgNode {
        VgNode {
            op,
            key,
            args: args.to_vec(),
            hash,
            line_start: 0,
            line_end: 0,
        }
    }

    fn name_key(name: &str) -> u64 {
        name.bytes().fold(1469598103934665603u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(1099511628211)
        })
    }

    fn referent(name: &str, id: Option<u64>) -> VgReferent {
        VgReferent {
            name: name.to_string(),
            name_key: name_key(name),
            referent: id,
        }
    }

    /// A `Return` over a chain of `len` non-commutative `Un` ops wrapping `Const(key)`,
    /// so two such DAGs differ only in the const while staying above the low-substance
    /// floor. Node 0 = Const, 1..len = the Un chain, top is the return value.
    fn chain(len: u32, const_key: u64, salt: u64) -> ValueDag {
        let mut nodes = vec![node(VgOp::Const, const_key, &[], 1000 + const_key + salt)];
        for i in 1..=len {
            nodes.push(node(VgOp::Un, 7, &[i - 1], 2000 + u64::from(i) + salt));
        }
        ValueDag {
            sinks: vec![VgSink {
                kind: VgSinkKind::Return,
                value: len,
                effect_ord: None,
            }],
            referents: vec![],
            nodes,
        }
    }

    #[test]
    fn identical_dags_are_equal_modulo_zero_holes() {
        let w = graded_witness(&chain(12, 5, 0), &chain(12, 5, 0), false, false).unwrap();
        assert_eq!(w.holes, 0);
        assert!(w.equal_modulo_holes);
        assert!(w.referent_mismatches.is_empty());
        assert!(!w.modeled_caveat);
    }

    #[test]
    fn single_differing_literal_is_one_leaf_hole() {
        // Same chain, different const at the bottom: one literal hole, still clean.
        let w = graded_witness(&chain(12, 5, 0), &chain(12, 9, 1), false, false).unwrap();
        assert_eq!(w.holes, 1);
        assert_eq!(w.spots.len(), 1);
        assert_eq!(w.spots[0].class, "literal");
        assert!(w.equal_modulo_holes);
    }

    #[test]
    fn lossy_lowering_marks_modeled_caveat() {
        let w = graded_witness(&chain(12, 5, 0), &chain(12, 5, 0), true, false).unwrap();
        assert!(w.modeled_caveat);
    }

    #[test]
    fn tiny_units_are_low_substance_not_clean() {
        // A 2-node difference below the substance floor is not an equal-modulo claim.
        let mut a = chain(2, 5, 0);
        let mut b = chain(2, 9, 1);
        a.referents.clear();
        b.referents.clear();
        let w = graded_witness(&a, &b, false, false).unwrap();
        assert!(w.patterns.contains(&"low-substance"));
        assert!(!w.equal_modulo_holes);
    }

    #[test]
    fn disjoint_referents_demote_the_witness() {
        // Identical graphs, but a shared name resolves to different definitions.
        let mut a = chain(12, 5, 0);
        let mut b = chain(12, 5, 0);
        a.referents.push(referent("equals", Some(111)));
        b.referents.push(referent("equals", Some(222)));
        let w = graded_witness(&a, &b, false, false).unwrap();
        assert_eq!(w.referent_mismatches, vec!["equals".to_string()]);
        assert!(w.patterns.contains(&"referent-mismatch"));
        assert!(!w.equal_modulo_holes);
    }

    #[test]
    fn unresolved_shared_name_is_a_scoped_caveat() {
        let mut a = chain(12, 5, 0);
        let mut b = chain(12, 5, 0);
        a.referents.push(referent("globalThing", None));
        b.referents.push(referent("globalThing", None));
        let w = graded_witness(&a, &b, false, false).unwrap();
        assert_eq!(w.caveat_names, vec!["globalThing".to_string()]);
        assert!(w.referent_mismatches.is_empty());
    }

    #[test]
    fn oversized_pair_fails_closed_to_no_witness() {
        let big = chain(MAX_NODES as u32 + 5, 5, 0);
        assert!(graded_witness(&big, &chain(12, 5, 0), false, false).is_none());
    }

    #[test]
    fn extra_return_sink_is_a_superset_pattern() {
        let mut a = chain(12, 5, 0);
        let b = chain(12, 5, 0);
        // Give `a` a second return sink with no counterpart in `b`.
        a.sinks.push(VgSink {
            kind: VgSinkKind::Return,
            value: 0,
            effect_ord: None,
        });
        let w = graded_witness(&a, &b, false, false).unwrap();
        assert!(w.holes >= 1);
        assert!(w.patterns.contains(&"sink-superset-a"));
        assert!(!w.equal_modulo_holes);
    }
}
