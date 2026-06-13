//! Exported value DAG for a single unit (#315).
//!
//! The fingerprint ([`api`](super::api)) is a *multiset of node hashes* — it answers
//! "do these two units compute the same thing?" but discards the graph shape. The
//! near channel needs the shape back: to say "these two units are equal **except at
//! these k spots**", a consumer must align the two graphs node-by-node. This module
//! exports the same hash-consed value graph the fingerprint is built from — nodes,
//! their argument edges, the behavior sinks, and the **resolved referents** of every
//! name the unit consumes — so `nose-detect`'s witness can anti-unify a pair.
//!
//! Referent resolution is the soundness-relevant part: two near units can be
//! node-for-node identical yet call same-named-but-different functions (`equals` on
//! two unrelated classes, a locale table by the same name in two files). Each
//! consumed name is resolved to a content-based identity — a file-local definition by
//! the content hash of its body, an imported name by its `(module, exported)`
//! coordinate, a self-call by a stable self marker. Names that cannot be resolved
//! are reported with `referent: None` so the witness can scope its claim instead of
//! over-claiming (the same-name-different-referent class).

use super::*;
use nose_il::{CallTargetEvidenceKind, EvidenceKind, ImportEvidenceKind};

/// Stable self-referent marker: a unit's call to itself (recursion) resolves here, so
/// two recursive clones pair as "self ↔ self" rather than content-comparing two
/// near-identical bodies (which would falsely differ).
const SELF_REFERENT: u64 = 0x5e1f_5e1f_5e1f_5e1f;

/// The operator family of a [`VgNode`]. Mirrors the private `ValOp`; the paired `key`
/// disambiguates within a family (the operator code for `Bin`/`Un`, the builtin
/// discriminant for `Call`, the field-name hash for `Field`, …).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VgOp {
    Input,
    Const,
    Bin,
    Un,
    Field,
    Index,
    Call,
    KwArg,
    Hof,
    Clamp,
    Seq,
    ImportNamespace,
    ImportBinding,
    CollectionParam,
    ArrayParam,
    StringParam,
    Phi,
    Lambda,
    Loop,
    Elem,
    Idx,
    Reduce,
    Formula,
    Recurrence,
    Opaque,
}

/// One value-graph node: its `(op, key)` identity (the same payload the structural
/// `hash` keys on), the argument edges (indices into [`ValueDag::nodes`]), the
/// structural hash, and the source line range of the IL subtree that produced it
/// (`(0, 0)` when unknown).
#[derive(Clone)]
pub struct VgNode {
    pub op: VgOp,
    pub key: u64,
    pub args: Vec<u32>,
    pub hash: u64,
    pub line_start: u32,
    pub line_end: u32,
}

/// What a behavior sink observes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VgSinkKind {
    Return,
    Cond,
    Effect,
    Break,
    Throw,
}

/// One behavior sink: the kind, the sunk value (an index into [`ValueDag::nodes`]),
/// and the ordered-effect slot when the sink is a sequenced effect.
#[derive(Clone)]
pub struct VgSink {
    pub kind: VgSinkKind,
    pub value: u32,
    pub effect_ord: Option<u32>,
}

/// A name the unit consumed, resolved to a content-based referent identity. `referent`
/// is `None` when the name could not be resolved (dynamic dispatch, an unmodeled global)
/// — the residual the witness reports as a scoped caveat rather than silently trusting.
#[derive(Clone)]
pub struct VgReferent {
    pub name: String,
    /// Hash of the name text (interner-independent), so two units' referents for the
    /// same name align across per-file interners.
    pub name_key: u64,
    pub referent: Option<u64>,
}

/// A unit's exported value DAG. `nodes` is the hash-consed graph (args reference
/// earlier indices), `sinks` is what the unit observes, `referents` is the resolved
/// identity of every consumed name.
#[derive(Clone)]
pub struct ValueDag {
    pub nodes: Vec<VgNode>,
    pub sinks: Vec<VgSink>,
    pub referents: Vec<VgReferent>,
}

/// Whether a `Bin` node with this `key` is a commutative operator whose value-graph
/// operands were canonically reordered — so a witness aligning two graphs may match
/// such a node's operands as a multiset rather than positionally.
pub fn bin_is_commutative(key: u64) -> bool {
    u32::try_from(key).is_ok_and(is_commutative)
}

fn vg_op_and_key(op: &ValOp) -> (VgOp, u64) {
    match op {
        ValOp::Input(k) => (VgOp::Input, u64::from(*k)),
        // `kind` and `bits` jointly identify the constant (kind distinguishes
        // `1: Int` from a sentinel/bool sharing the bit pattern); mix both into the key.
        ValOp::Const { kind, bits } => (VgOp::Const, fxh(&[*bits, *kind as u64])),
        ValOp::Bin(o) => (VgOp::Bin, u64::from(*o)),
        ValOp::Un(o) => (VgOp::Un, u64::from(*o)),
        ValOp::Field(h) => (VgOp::Field, *h),
        ValOp::Index => (VgOp::Index, 0),
        ValOp::Call(c) => (VgOp::Call, u64::from(*c)),
        ValOp::KwArg(h) => (VgOp::KwArg, *h),
        ValOp::Hof(k) => (VgOp::Hof, u64::from(*k)),
        ValOp::Clamp => (VgOp::Clamp, 0),
        ValOp::Seq(k) => (VgOp::Seq, *k),
        ValOp::ImportNamespace { module_hash } => (VgOp::ImportNamespace, *module_hash),
        ValOp::ImportBinding {
            module_hash,
            exported_hash,
        } => (
            VgOp::ImportBinding,
            module_hash ^ exported_hash.rotate_left(17),
        ),
        ValOp::CollectionParam => (VgOp::CollectionParam, 0),
        ValOp::ArrayParam => (VgOp::ArrayParam, 0),
        ValOp::StringParam => (VgOp::StringParam, 0),
        ValOp::Phi => (VgOp::Phi, 0),
        ValOp::Lambda(h) => (VgOp::Lambda, *h),
        ValOp::Loop(k) => (VgOp::Loop, u64::from(*k)),
        ValOp::Elem(h) => (VgOp::Elem, *h),
        ValOp::Idx(h) => (VgOp::Idx, *h),
        ValOp::Reduce(k) => (VgOp::Reduce, u64::from(*k)),
        ValOp::Formula(h) => (VgOp::Formula, *h),
        ValOp::Recurrence(h) => (VgOp::Recurrence, *h),
        ValOp::Opaque(h) => (VgOp::Opaque, *h),
    }
}

fn sink_kind(k: SinkKind) -> VgSinkKind {
    match k {
        SinkKind::Return => VgSinkKind::Return,
        SinkKind::Cond => VgSinkKind::Cond,
        SinkKind::Effect => VgSinkKind::Effect,
        SinkKind::Break => VgSinkKind::Break,
        SinkKind::Throw => VgSinkKind::Throw,
    }
}

fn fxh(parts: &[u64]) -> u64 {
    // Small deterministic mix (FxHash-style) — stable across runs and thread counts.
    let mut h: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    for &p in parts {
        h = (h.rotate_left(5) ^ p).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    h
}

fn str_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcb_f2_9c_e4_84_22_23_25;
    for b in s.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x1_00_00_00_01_b3);
    }
    h
}

/// Payload identity of a node, content-based: literal payloads by value, names by
/// their resolved TEXT (not the interner id, which is per-file), module-level `Cid`
/// numbers by their original name (the number depends on file binding order, so two
/// byte-identical definitions would otherwise split).
fn payload_key(il: &Il, node: NodeId, interner: &Interner) -> u64 {
    match il.node(node).payload {
        Payload::None => 0,
        Payload::Op(o) => fxh(&[1, o as u64]),
        Payload::Lit(c) => fxh(&[2, c as u64]),
        Payload::LitInt(v) => fxh(&[3, v as u64]),
        Payload::LitBool(b) => fxh(&[4, u64::from(b)]),
        Payload::LitStr(h) => fxh(&[5, h]),
        Payload::LitFloat(h) => fxh(&[6, h]),
        Payload::Name(s) => fxh(&[7, str_hash(interner.resolve(s))]),
        Payload::Cid(c) => match il.cid_names.get(c as usize) {
            Some(&s) => fxh(&[8, str_hash(interner.resolve(s))]),
            None => fxh(&[8]),
        },
        Payload::Builtin(b) => fxh(&[9, b as u64]),
        Payload::HoF(k) => fxh(&[10, k as u64]),
        Payload::Loop(k) => fxh(&[11, k as u64]),
    }
}

/// Content hash of an IL subtree: `(kind, payload, children)`. Two textually-identical
/// definitions hash equal; any structural or literal difference splits them. Iterative
/// (explicit post-order stack) so a deeply-nested generated/minified AST cannot
/// overflow the native stack.
fn content_hash(il: &Il, node: NodeId, interner: &Interner, memo: &mut FxHashMap<u32, u64>) -> u64 {
    if let Some(&h) = memo.get(&node.0) {
        return h;
    }
    enum Step {
        Enter(NodeId),
        Exit(NodeId),
    }
    let mut stack = vec![Step::Enter(node)];
    while let Some(step) = stack.pop() {
        match step {
            Step::Enter(n) => {
                if memo.contains_key(&n.0) {
                    continue;
                }
                stack.push(Step::Exit(n));
                for &c in il.children(n) {
                    stack.push(Step::Enter(c));
                }
            }
            Step::Exit(n) => {
                let mut parts = vec![il.kind(n) as u64, payload_key(il, n, interner)];
                for &c in il.children(n) {
                    parts.push(memo.get(&c.0).copied().unwrap_or(0));
                }
                memo.insert(n.0, fxh(&parts));
            }
        }
    }
    memo.get(&node.0).copied().unwrap_or(0)
}

fn span_within(outer: Span, inner: Span) -> bool {
    outer.file == inner.file
        && inner.start_byte >= outer.start_byte
        && inner.end_byte <= outer.end_byte
}

/// Per-file referent-resolution tables, built once and reused across the file's units
/// (the maps are O(file), so rebuilding them per unit is what made huge generated
/// files pathological). Construct with [`FileReferents::new`], then call
/// [`FileReferents::of`] per unit root.
pub struct FileReferents<'a> {
    il: &'a Il,
    interner: &'a Interner,
    def_by_span: FxHashMap<(u32, u32), NodeId>,
    def_by_name: FxHashMap<Symbol, Vec<NodeId>>,
    /// Unit name keyed by definition start-byte, for `def_name` — avoids an O(units)
    /// scan per call-target.
    name_by_start: FxHashMap<u32, Symbol>,
    /// `CallTarget` evidence (anchor span + kind) sorted by anchor start-byte, so the
    /// per-unit lookup is a range scan, not a full `il.evidence` walk (the latter was
    /// O(units × evidence) — the §BQ "index the hot lookups" rule).
    call_evidence: Vec<(Span, CallTargetEvidenceKind)>,
    import_evidence: Vec<(Span, ImportEvidenceKind)>,
    /// Content-hash memo shared across the file's units (definitions are re-referenced).
    memo: std::cell::RefCell<FxHashMap<u32, u64>>,
}

impl<'a> FileReferents<'a> {
    pub fn new(il: &'a Il, interner: &'a Interner) -> Self {
        let mut def_by_span: FxHashMap<(u32, u32), NodeId> = FxHashMap::default();
        let mut def_by_name: FxHashMap<Symbol, Vec<NodeId>> = FxHashMap::default();
        let mut name_by_start: FxHashMap<u32, Symbol> = FxHashMap::default();
        for u in &il.units {
            let s = il.node(u.root).span;
            def_by_span
                .entry((s.start_byte, s.end_byte))
                .or_insert(u.root);
            if let Some(n) = u.name {
                def_by_name.entry(n).or_default().push(u.root);
                name_by_start.entry(s.start_byte).or_insert(n);
            }
        }
        for stmt in top_level_statements_for(il) {
            if let Some(n) = crate::module_facts::assignment_name_in(il, stmt) {
                def_by_name.entry(n).or_default().push(stmt);
            }
        }
        let import_evidence = il
            .evidence
            .iter()
            .filter_map(|ev| match ev.kind {
                EvidenceKind::Import(ik) => Some((ev.anchor.span(), ik)),
                _ => None,
            })
            .collect();
        let mut call_evidence: Vec<(Span, CallTargetEvidenceKind)> = il
            .evidence
            .iter()
            .filter_map(|ev| match ev.kind {
                EvidenceKind::CallTarget(k) => Some((ev.anchor.span(), k)),
                _ => None,
            })
            .collect();
        call_evidence.sort_by_key(|(s, _)| s.start_byte);
        FileReferents {
            il,
            interner,
            def_by_span,
            def_by_name,
            name_by_start,
            call_evidence,
            import_evidence,
            memo: std::cell::RefCell::new(FxHashMap::default()),
        }
    }

    fn content(&self, def: NodeId) -> u64 {
        content_hash(self.il, def, self.interner, &mut self.memo.borrow_mut())
    }

    /// An import-bound name's referent is its module COORDINATE — not the content hash
    /// of the import statement, whose module hash embeds the file path and would
    /// falsely split identical definitions across files.
    fn import_referent(&self, stmt_span: Span, local: &str) -> Option<u64> {
        self.import_evidence
            .iter()
            .filter(|(s, _)| span_within(stmt_span, *s))
            .find_map(|(_, ik)| match *ik {
                ImportEvidenceKind::Binding {
                    module_hash,
                    exported_hash,
                } => Some(fxh(&[12, module_hash, exported_hash])),
                ImportEvidenceKind::Namespace { .. }
                | ImportEvidenceKind::Wildcard { .. }
                | ImportEvidenceKind::Require { .. } => Some(fxh(&[14, str_hash(local)])),
                ImportEvidenceKind::CQuoteInclude { include_hash } => {
                    Some(fxh(&[15, include_hash]))
                }
                // Literal-export/snapshot evidence anchors at a LOCAL definition, so its
                // referent is the def's content, not a path-embedding module coordinate.
                ImportEvidenceKind::ImmutableLiteralExport { .. }
                | ImportEvidenceKind::ImportedLiteralSnapshot { .. } => None,
            })
    }

    fn def_name(&self, span: Span) -> Option<String> {
        self.name_by_start
            .get(&span.start_byte)
            .map(|&s| self.interner.resolve(s).to_string())
    }

    /// The names the unit rooted at `root` consumes, each with a resolved referent.
    /// Sources: `CallTarget` evidence anchored inside the unit, and bare free `Var`
    /// names resolved against the file's units and module-level assignments.
    pub fn of(&self, root: NodeId) -> Vec<VgReferent> {
        let mut out = self.call_target_referents(root);
        self.free_name_referents(root, &mut out);
        out.sort_by_key(|r| (r.name_key, r.referent));
        out.dedup_by(|a, b| a.name_key == b.name_key && a.referent == b.referent);
        out
    }

    /// Referents from `CallTarget` evidence anchored inside the unit. `call_evidence` is
    /// sorted by anchor start-byte, so we binary-search to the unit's span and scan only
    /// that window instead of walking all of `il.evidence` per unit.
    fn call_target_referents(&self, root: NodeId) -> Vec<VgReferent> {
        let il = self.il;
        let unit_span = il.node(root).span;
        let mut out: Vec<VgReferent> = Vec::new();
        let lo = self
            .call_evidence
            .partition_point(|(s, _)| s.start_byte < unit_span.start_byte);
        for (span, k) in self.call_evidence[lo..]
            .iter()
            .take_while(|(s, _)| s.start_byte <= unit_span.end_byte)
        {
            if !span_within(unit_span, *span) {
                continue;
            }
            let k = *k;
            let (name_key, referent, name) = match k {
                CallTargetEvidenceKind::DirectFunction {
                    target_span,
                    name_hash,
                } => {
                    let r = if target_span == unit_span {
                        Some(SELF_REFERENT)
                    } else {
                        self.def_by_span
                            .get(&(target_span.start_byte, target_span.end_byte))
                            .map(|&n| self.content(n))
                    };
                    (name_hash, r, self.def_name(target_span))
                }
                CallTargetEvidenceKind::DirectMethod {
                    target_span,
                    receiver_type_hash,
                    method_hash,
                } => {
                    let r = if target_span == unit_span {
                        Some(SELF_REFERENT)
                    } else {
                        self.def_by_span
                            .get(&(target_span.start_byte, target_span.end_byte))
                            .map(|&n| fxh(&[self.content(n), receiver_type_hash]))
                    };
                    (method_hash, r, self.def_name(target_span))
                }
                CallTargetEvidenceKind::ImportedFunction {
                    module_hash,
                    exported_hash,
                    local_hash,
                } => (
                    local_hash,
                    Some(fxh(&[12, module_hash, exported_hash])),
                    None,
                ),
                CallTargetEvidenceKind::ImportedMember {
                    module_hash,
                    exported_hash,
                    member_hash,
                } => (
                    member_hash,
                    Some(fxh(&[13, module_hash, exported_hash])),
                    None,
                ),
                CallTargetEvidenceKind::DynamicDispatch { method_hash, .. } => {
                    (method_hash, None, None)
                }
            };
            out.push(VgReferent {
                name: name.unwrap_or_else(|| format!("call#{name_key:x}")),
                name_key,
                referent,
            });
        }
        out
    }

    /// Referents from bare free `Var` names inside the unit (post-alpha, locals are
    /// `Cid`s; surviving `Var` name nodes are free/global references — field names are
    /// excluded by the kind gate), appended to `out`.
    fn free_name_referents(&self, root: NodeId, out: &mut Vec<VgReferent>) {
        let il = self.il;
        let own_name = il
            .units
            .iter()
            .find(|u| u.root == root)
            .and_then(|u| u.name);
        let mut free: Vec<Symbol> = Vec::new();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if il.kind(n) == NodeKind::Var {
                if let Payload::Name(s) = il.node(n).payload {
                    free.push(s);
                }
            }
            stack.extend(il.children(n).iter().copied());
        }
        free.sort_unstable_by_key(|s| self.interner.resolve(*s).to_string());
        free.dedup();
        for sym in free {
            let name = self.interner.resolve(sym).to_string();
            // Receiver keywords are scope-bound, not referent-bearing names.
            if matches!(name.as_str(), "this" | "self" | "super") {
                continue;
            }
            let name_key = str_hash(&name);
            if own_name == Some(sym) {
                out.push(VgReferent {
                    name,
                    name_key,
                    referent: Some(SELF_REFERENT),
                });
                continue;
            }
            match self.def_by_name.get(&sym) {
                Some(defs) => {
                    let defs = defs.clone();
                    for def in defs {
                        let r = self
                            .import_referent(il.node(def).span, &name)
                            .unwrap_or_else(|| self.content(def));
                        out.push(VgReferent {
                            name: name.clone(),
                            name_key,
                            referent: Some(r),
                        });
                    }
                }
                None => out.push(VgReferent {
                    name,
                    name_key,
                    referent: None,
                }),
            }
        }
    }
}

/// Export the value DAG of the unit rooted at `root`, built exactly the way the
/// fingerprint is (including the shared per-file inline/global `context` when
/// supplied). `referents` is the file's resolution context — build it once per file
/// with [`FileReferents::new`] and reuse it across the file's units.
pub fn value_dag(
    il: &Il,
    root: NodeId,
    interner: &Interner,
    context: Option<&ValueFingerprintContext>,
    referents: &FileReferents<'_>,
) -> ValueDag {
    let mut b = Builder::new(il, interner);
    if let Some(ctx) = context {
        b = b.with_context(ctx);
        b.build_unit_with_context(root, Some(ctx));
    } else {
        b.build_unit(root);
    }
    let nodes = b
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (op, key) = vg_op_and_key(&n.op);
            let (line_start, line_end) = b.node_span[i]
                .map(|s| (s.start_line, s.end_line))
                .unwrap_or((0, 0));
            VgNode {
                op,
                key,
                args: n.args.clone(),
                hash: b.vhash[i],
                line_start,
                line_end,
            }
        })
        .collect();
    let sinks = b
        .sinks
        .iter()
        .map(|s| VgSink {
            kind: sink_kind(s.kind),
            value: s.value,
            effect_ord: s.effect_ord,
        })
        .collect();
    ValueDag {
        nodes,
        sinks,
        referents: referents.of(root),
    }
}
