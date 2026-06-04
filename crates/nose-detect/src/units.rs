//! Extract detection units from a normalized file and compute their structural
//! features: a multiset of local **subtree-shape** hashes (tree 2-grams: a node
//! tag combined with its children's tags), a pre-order **linearization** of node
//! tags for alignment, and a **MinHash** signature for candidate generation.

use nose_il::{
    Builtin, Il, Interner, Lang, LitClass, NodeId, NodeKind, Op, ParamSemantic, Payload, Symbol,
    UnitKind,
};
use nose_normalize::node_tag;
use rustc_hash::{FxHashMap, FxHashSet};

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
    /// Sorted multiset of literal (`Const`) value hashes. A high `lits/value`
    /// ratio marks a "data-table" unit (constant-dominated, e.g. a locale map),
    /// where the constants must match for a clone.
    pub lits: Vec<u64>,
    /// Sorted multiset of RETURN-sink value hashes — what the unit returns. True
    /// clones return the same computed values; used to demote near-identical units
    /// that differ only in their result (`<` vs `<=`, an extra effect).
    pub returns: Vec<u64>,
    /// Whether the value fingerprint is safe to use as a strict semantic proof.
    ///
    /// `semantic` mode must not report units whose fingerprint passed through lossy
    /// lowering (`Raw`, abstract literals such as JS regex, or opaque calls). Those
    /// units can still participate in `near` via structural scoring.
    #[serde(default)]
    pub exact_safe: bool,
}

const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Upper bound (pre-order node count) for a *block* unit. ~10× the typical
/// fragment clone; bounds the cost of extracting features for every nested block in
/// a very large function/class (which the enclosing unit already covers).
const MAX_BLOCK_TOKENS: usize = 400;
const EXACT_VALUE_MIN: usize = 4;

#[inline]
fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(7) ^ b).wrapping_mul(SEED)
}

/// Extract all units of `il` passing the size gates, with features computed.
pub(crate) fn extract(
    il: &Il,
    interner: &Interner,
    seeds: &[u64],
    min_lines: u32,
    min_tokens: usize,
    block_units: bool,
) -> Vec<UnitFeat> {
    // Frontend-tagged functions/methods/classes, and (when enabled) substantial
    // sub-function blocks (loops / ifs / try). The ceiling funnel showed ~56% of
    // gold pairs have a region that is a sub-function block, undetectable unless
    // extracted as its own unit — but block units only pay off once candidate
    // generation can surface them, so they are opt-in.
    let mut roots: Vec<(NodeId, UnitKind, Option<Symbol>)> =
        il.units.iter().map(|u| (u.root, u.kind, u.name)).collect();
    if block_units {
        collect_block_units(il, il.root, &mut roots);
    }

    let facts = StrictFacts::collect(il, interner);
    let mut out = Vec::new();
    for (root, kind, uname) in roots {
        let span = il.node(root).span;
        let lines = span.line_count();

        let mut pre = Vec::new();
        collect_pre(il, root, &mut pre);
        let exact_safe = strict_exact_safe_tree(il, interner, &facts, root);
        // The value graph is the semantic fingerprint (already sorted), with the
        // literal-only multiset for data-table detection. Computed before the size
        // gate so the gate can consult semantic richness (below).
        let (value, lits, returns) = nose_normalize::value_fingerprint_lits(il, root, interner);

        // Size gate. A short unit normally isn't a meaningful clone — EXCEPT a
        // frontend-tagged function whose body is behaviorally *dense*: a functional
        // one-liner like `return sum(v for v in xs if v>0)` is a real Type-4 clone of a
        // multi-line loop (the value graph converges them to an *identical* fingerprint),
        // just compressed below the line/token gate. Admit such a function when its value
        // fingerprint is rich enough to be matched by the oracle-certified exact-match
        // path (`value.len() >= 4`, the same floor that path uses) — this recovers the
        // compressed functional Type-4 forms without lowering the gate for trivial units
        // (`return x` has 1–2 atoms) or for blocks (kept strict; they are the noisy ones).
        // Blocks share the function gate: measurement showed the real sub-function
        // clones are small (24–40 tokens), so a stricter block gate drops signal
        // (pool-precision 0.106→0.074, AUC 0.42→0.17) faster than noise.
        let dense_fn =
            matches!(kind, UnitKind::Function | UnitKind::Method) && value.len() >= EXACT_VALUE_MIN;
        if (lines < min_lines || pre.len() < min_tokens) && !dense_fn {
            continue;
        }
        // …but a *huge* block (a whole big method/class body) is not a "fragment"
        // clone — it's covered by its enclosing function/class unit — and extracting
        // features for every nested block of a 700-line class is quadratic. Cap block
        // units well above real fragment clones (≈40 tokens) so only the pathological
        // giants are skipped; functions/methods/classes are never capped.
        if kind == UnitKind::Block && pre.len() > MAX_BLOCK_TOKENS {
            continue;
        }

        let mut shapes = Vec::with_capacity(pre.len());
        let mut linear = Vec::with_capacity(pre.len());
        for &nid in &pre {
            let n = il.node(nid);
            let tag = node_tag(n.kind, n.payload, interner);
            linear.push(tag);
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
        let shape_minhash = crate::minhash::sign(&distinct_shapes, seeds);

        // Candidate generation keys on the value graph when present (so clones
        // that converge only semantically still become candidates).
        let mut distinct = if value.is_empty() {
            shapes.clone()
        } else {
            value.clone()
        };
        distinct.dedup();
        let minhash = crate::minhash::sign(&distinct, seeds);

        out.push(UnitFeat {
            path: il.meta.path.clone(),
            lang: il.meta.lang,
            kind,
            name: uname.map(|s| interner.resolve(s).to_string()),
            start_line: span.start_line,
            end_line: span.end_line,
            token_count: pre.len(),
            shapes,
            shape_minhash,
            value,
            minhash,
            linear,
            lits,
            returns,
            exact_safe,
        });
    }
    out
}

#[derive(Default)]
struct StrictFacts {
    immutable_names: FxHashSet<Symbol>,
    function_names: FxHashSet<Symbol>,
}

impl StrictFacts {
    fn collect(il: &Il, interner: &Interner) -> Self {
        let mut facts = StrictFacts::default();
        facts.collect_immutable_bindings(il, interner);
        facts.collect_function_bindings(il, interner);
        facts
    }

    fn proven_name(&self, name: Symbol) -> bool {
        self.immutable_names.contains(&name) || self.function_names.contains(&name)
    }

    fn collect_immutable_bindings(&mut self, il: &Il, interner: &Interner) {
        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for stmt in top_level_statements(il) {
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }

        let mut env: FxHashSet<u32> = FxHashSet::default();
        for stmt in top_level_statements(il) {
            let kids = il.children(stmt);
            if kids.len() != 2 {
                continue;
            }
            let Some(name) = assignment_name(il, stmt) else {
                continue;
            };
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            let safe_literal = immutable_binding_safe(il, &env, &self.immutable_names, kids[1]);
            let safe_proven_container = !module_binding_mutated(il, interner, name)
                && strict_exact_module_container_binding_safe(il, interner, self, kids[1]);
            if safe_literal || safe_proven_container {
                self.immutable_names.insert(name);
                if let Payload::Cid(cid) = il.node(kids[0]).payload {
                    env.insert(cid);
                }
            }
        }
    }

    fn collect_function_bindings(&mut self, il: &Il, interner: &Interner) {
        for unit in &il.units {
            let Some(name) = unit.name else {
                continue;
            };
            if function_binding_safe(il, interner, self, unit.root, unit.root) {
                self.function_names.insert(name);
            }
        }
    }
}

fn top_level_statements(il: &Il) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &stmt in il.children(il.root) {
        if il.kind(stmt) == NodeKind::Block {
            out.extend(il.children(stmt).iter().copied());
        } else {
            out.push(stmt);
        }
    }
    out
}

fn assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    if il.kind(stmt) != NodeKind::Assign {
        return None;
    }
    let kids = il.children(stmt);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return None;
    }
    let Payload::Cid(cid) = il.node(kids[0]).payload else {
        return None;
    };
    il.cid_names.get(cid as usize).copied()
}

fn module_binding_mutated(il: &Il, interner: &Interner, name: Symbol) -> bool {
    let top_level = top_level_statements(il);
    il.nodes.iter().enumerate().any(|(idx, node)| {
        let node_id = NodeId(idx as u32);
        match il.kind(node_id) {
            NodeKind::Field => {
                field_mutates_binding(il, interner, node_id, name).unwrap_or(false)
                    && matches!(node.payload, Payload::Name(_))
            }
            NodeKind::Assign if !top_level.contains(&node_id) => {
                assignment_mutates_binding(il, node_id, name).unwrap_or(false)
            }
            _ => false,
        }
    })
}

fn assignment_mutates_binding(il: &Il, assign: NodeId, name: Symbol) -> Option<bool> {
    let lhs = il.children(assign).first().copied()?;
    Some(node_contains_symbol(il, lhs, name))
}

fn field_mutates_binding(
    il: &Il,
    interner: &Interner,
    field: NodeId,
    name: Symbol,
) -> Option<bool> {
    let Payload::Name(method) = il.node(field).payload else {
        return Some(false);
    };
    if !matches!(
        interner.resolve(method),
        "add"
            | "addAll"
            | "append"
            | "delete"
            | "clear"
            | "compute"
            | "computeIfAbsent"
            | "computeIfPresent"
            | "merge"
            | "pop"
            | "push"
            | "put"
            | "putAll"
            | "remove"
            | "removeAll"
            | "removeIf"
            | "replace"
            | "replaceAll"
            | "retainAll"
            | "shift"
            | "sort"
            | "splice"
            | "unshift"
            | "set"
    ) {
        return Some(false);
    }
    let receiver = il.children(field).first().copied()?;
    Some(node_refers_to_symbol(il, receiver, name))
}

fn node_refers_to_symbol(il: &Il, node: NodeId, name: Symbol) -> bool {
    match il.node(node).payload {
        Payload::Name(symbol) => symbol == name,
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .is_some_and(|&symbol| symbol == name),
        _ => false,
    }
}

fn node_contains_symbol(il: &Il, node: NodeId, name: Symbol) -> bool {
    node_refers_to_symbol(il, node, name)
        || il
            .children(node)
            .iter()
            .any(|&child| node_contains_symbol(il, child, name))
}

fn strict_exact_module_container_binding_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    strict_exact_map_constructor_entries_safe(il, interner, facts, node)
        || strict_exact_java_map_factory_safe(il, interner, facts, node)
        || strict_exact_rust_std_map_factory_safe(il, interner, facts, node)
        || strict_exact_python_collection_factory_safe(il, interner, facts, node)
        || strict_exact_ruby_set_factory_safe(il, interner, facts, node)
        || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node)
        || strict_exact_rust_std_collection_factory_safe(il, interner, facts, node)
        || strict_exact_set_constructor_collection_safe(il, interner, facts, node)
        || strict_exact_java_collection_factory_safe(il, interner, facts, node)
}

fn immutable_binding_safe(
    il: &Il,
    env: &FxHashSet<u32>,
    immutable_names: &FxHashSet<Symbol>,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::Call
        | NodeKind::HoF
        | NodeKind::Func
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw
        | NodeKind::Assign => false,
        NodeKind::Var => match il.node(node).payload {
            Payload::Cid(c) => env.contains(&c),
            Payload::Name(s) => immutable_names.contains(&s),
            _ => false,
        },
        NodeKind::Lit => exact_literal_safe(il, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| immutable_binding_safe(il, env, immutable_names, c)),
    }
}

fn function_binding_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    root: NodeId,
    node: NodeId,
) -> bool {
    match il.kind(node) {
        NodeKind::Raw
        | NodeKind::HoF
        | NodeKind::Lambda
        | NodeKind::Loop
        | NodeKind::Try
        | NodeKind::Throw => false,
        NodeKind::Func if node != root => false,
        NodeKind::Call => matches!(il.node(node).payload, Payload::Builtin(_)),
        NodeKind::Seq => strict_exact_safe_seq(il, interner, node),
        NodeKind::Lit => exact_literal_safe(il, node),
        NodeKind::Var => strict_exact_safe_var(il, facts, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| function_binding_safe(il, interner, facts, root, c)),
    }
}

fn strict_exact_safe_tree(il: &Il, interner: &Interner, facts: &StrictFacts, node: NodeId) -> bool {
    match il.kind(node) {
        NodeKind::Raw => false,
        NodeKind::Seq => {
            strict_exact_safe_seq(il, interner, node)
                && il
                    .children(node)
                    .iter()
                    .all(|&c| strict_exact_safe_tree(il, interner, facts, c))
        }
        NodeKind::Call => strict_exact_safe_call(il, interner, facts, node),
        NodeKind::Index
            if strict_exact_go_literal_zero_map_index_safe(il, interner, facts, node) =>
        {
            true
        }
        NodeKind::BinOp if strict_exact_static_index_membership_safe(il, interner, facts, node) => {
            true
        }
        NodeKind::Lit => exact_literal_safe(il, node),
        NodeKind::Var => strict_exact_safe_var(il, facts, node),
        _ => il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c)),
    }
}

fn exact_literal_safe(il: &Il, node: NodeId) -> bool {
    matches!(
        il.node(node).payload,
        Payload::LitInt(_)
            | Payload::LitBool(_)
            | Payload::LitStr(_)
            | Payload::LitFloat(_)
            | Payload::Lit(LitClass::Null)
    )
}

fn strict_exact_static_index_membership_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    let Payload::Op(op) = il.node(node).payload else {
        return false;
    };
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    if strict_exact_index_membership_threshold(il, op, false, kids[1]) {
        if let Some((element, collection)) =
            strict_exact_static_index_membership_parts(il, interner, facts, kids[0])
        {
            return strict_exact_safe_tree(il, interner, facts, element)
                && strict_exact_static_non_float_collection(il, collection);
        }
    }
    if strict_exact_index_membership_threshold(il, op, true, kids[0]) {
        if let Some((element, collection)) =
            strict_exact_static_index_membership_parts(il, interner, facts, kids[1])
        {
            return strict_exact_safe_tree(il, interner, facts, element)
                && strict_exact_static_non_float_collection(il, collection);
        }
    }
    false
}

fn strict_exact_static_index_membership_parts(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> Option<(NodeId, NodeId)> {
    if il.kind(node) != NodeKind::Call {
        return None;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = il.node(kids[0]).payload else {
        return None;
    };
    let method = interner.resolve(method);
    let receiver = *il.children(kids[0]).first()?;
    if !strict_exact_static_non_float_collection(il, receiver) {
        return None;
    }
    if method == "indexOf" {
        return Some((kids[1], receiver));
    }
    if method == "findIndex" {
        let element = strict_exact_lambda_eq_param_element(il, interner, facts, kids[1])?;
        return Some((element, receiver));
    }
    None
}

fn strict_exact_index_membership_threshold(
    il: &Il,
    op: Op,
    index_call_on_right: bool,
    threshold: NodeId,
) -> bool {
    if strict_exact_minus_one_literal(il, threshold) {
        return op == Op::Ne
            || (!index_call_on_right && op == Op::Gt)
            || (index_call_on_right && op == Op::Lt);
    }
    if matches!(il.node(threshold).payload, Payload::LitInt(0)) {
        return (!index_call_on_right && op == Op::Ge) || (index_call_on_right && op == Op::Le);
    }
    false
}

fn strict_exact_lambda_eq_param_element(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    lambda: NodeId,
) -> Option<NodeId> {
    if il.kind(lambda) != NodeKind::Lambda {
        return None;
    }
    let kids = il.children(lambda);
    let param = kids.iter().find_map(|&kid| {
        if il.kind(kid) != NodeKind::Param {
            return None;
        }
        if let Payload::Cid(cid) = il.node(kid).payload {
            Some(cid)
        } else {
            None
        }
    })?;
    let ret = strict_exact_first_return_expr(il, *kids.last()?)?;
    if il.kind(ret) != NodeKind::BinOp || !matches!(il.node(ret).payload, Payload::Op(Op::Eq)) {
        return None;
    }
    let ret_kids = il.children(ret);
    if ret_kids.len() != 2 {
        return None;
    }
    if strict_exact_lambda_param_var(il, ret_kids[0], param) {
        return strict_exact_safe_tree(il, interner, facts, ret_kids[1]).then_some(ret_kids[1]);
    }
    if strict_exact_lambda_param_var(il, ret_kids[1], param) {
        return strict_exact_safe_tree(il, interner, facts, ret_kids[0]).then_some(ret_kids[0]);
    }
    None
}

fn strict_exact_first_return_expr(il: &Il, node: NodeId) -> Option<NodeId> {
    if il.kind(node) == NodeKind::Return {
        return il.children(node).first().copied();
    }
    if il.kind(node) == NodeKind::Block {
        return il
            .children(node)
            .iter()
            .find_map(|&child| strict_exact_first_return_expr(il, child));
    }
    None
}

fn strict_exact_lambda_param_var(il: &Il, node: NodeId, param: u32) -> bool {
    il.kind(node) == NodeKind::Var
        && matches!(il.node(node).payload, Payload::Cid(cid) if cid == param)
}

fn strict_exact_minus_one_literal(il: &Il, node: NodeId) -> bool {
    if matches!(il.node(node).payload, Payload::LitInt(-1)) {
        return true;
    }
    if il.kind(node) != NodeKind::UnOp || !matches!(il.node(node).payload, Payload::Op(Op::Neg)) {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 1 && matches!(il.node(kids[0]).payload, Payload::LitInt(1))
}

fn strict_exact_static_non_float_collection(il: &Il, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let kids = il.children(node);
    !kids.is_empty()
        && kids.iter().all(|&kid| {
            matches!(
                il.node(kid).payload,
                Payload::LitInt(_)
                    | Payload::LitBool(_)
                    | Payload::LitStr(_)
                    | Payload::Lit(LitClass::Null)
            )
        })
}

fn strict_exact_safe_var(il: &Il, facts: &StrictFacts, node: NodeId) -> bool {
    match il.node(node).payload {
        Payload::Cid(_) => true,
        Payload::Name(name) => facts.proven_name(name),
        _ => false,
    }
}

fn strict_exact_safe_seq(il: &Il, interner: &Interner, node: NodeId) -> bool {
    match il.node(node).payload {
        Payload::None => true,
        Payload::Name(name) => matches!(
            interner.resolve(name),
            "array"
                | "list"
                | "tuple"
                | "dictionary"
                | "hash"
                | "array_expression"
                | "tuple_expression"
                | "object"
                | "pair"
                | "import_binding"
                | "import_namespace"
                | "own_property_guard"
                | "record_guard"
        ),
        _ => false,
    }
}

fn strict_exact_safe_call(il: &Il, interner: &Interner, facts: &StrictFacts, node: NodeId) -> bool {
    if matches!(il.node(node).payload, Payload::Builtin(Builtin::Contains)) {
        let kids = il.children(node);
        return kids.len() == 2
            && strict_exact_safe_tree(il, interner, facts, kids[0])
            && strict_exact_membership_collection_safe(il, interner, facts, kids[1]);
    }
    if matches!(il.node(node).payload, Payload::Builtin(_)) {
        return il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c));
    }
    if strict_exact_set_constructor_collection_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_python_collection_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_ruby_set_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_std_collection_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_java_collection_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_java_map_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_rust_std_map_factory_safe(il, interner, facts, node) {
        return true;
    }
    if strict_exact_map_constructor_entries_safe(il, interner, facts, node) {
        return true;
    }
    let Some(&callee) = il.children(node).first() else {
        return false;
    };
    if strict_exact_callee_name(il, interner, callee, "typeof") {
        return strict_exact_call_args_safe(il, interner, facts, node);
    }
    if il.kind(callee) != NodeKind::Field {
        return strict_exact_callee_identity(il, facts, callee)
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(name);
    if method == "test" {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        return matches!(il.node(receiver).payload, Payload::LitStr(_))
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    if method == "isArray" {
        return strict_exact_field_receiver_name(il, interner, callee, "Array")
            && strict_exact_call_args_safe(il, interner, facts, node);
    }
    if matches!(
        method,
        "contains" | "__contains__" | "include?" | "member?" | "includes"
    ) {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        if strict_exact_literal_collection_receiver_safe(il, interner, facts, receiver)
            || strict_exact_python_collection_factory_safe(il, interner, facts, receiver)
            || strict_exact_ruby_set_factory_safe(il, interner, facts, receiver)
            || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, receiver)
            || strict_exact_rust_std_collection_factory_safe(il, interner, facts, receiver)
            || strict_exact_java_collection_factory_safe(il, interner, facts, receiver)
        {
            return strict_exact_call_args_safe(il, interner, facts, node);
        }
    }
    if matches!(method, "get" | "has" | "getOrDefault") {
        let Some(&receiver) = il.children(callee).first() else {
            return false;
        };
        if method == "get"
            && strict_exact_typed_map_param_receiver_safe(il, receiver)
            && il.children(node).len() == 3
        {
            return strict_exact_call_args_safe(il, interner, facts, node);
        }
        if strict_exact_set_constructor_collection_safe(il, interner, facts, receiver) {
            return strict_exact_call_args_safe(il, interner, facts, node);
        }
        if strict_exact_java_map_factory_safe(il, interner, facts, receiver) {
            return strict_exact_call_args_safe(il, interner, facts, node);
        }
        if strict_exact_map_constructor_entries_safe(il, interner, facts, receiver) {
            return strict_exact_call_args_safe(il, interner, facts, node);
        }
    }
    if matches!(
        method,
        "iter" | "into_iter" | "iter_mut" | "collect" | "to_vec" | "copied" | "cloned"
    ) {
        return strict_exact_call_args_safe(il, interner, facts, node);
    }
    strict_exact_callee_identity(il, facts, callee)
        && strict_exact_call_args_safe(il, interner, facts, node)
}

fn strict_exact_typed_map_param_receiver_safe(il: &Il, receiver: NodeId) -> bool {
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Payload::Cid(receiver_cid) = il.node(receiver).payload else {
        return false;
    };
    il.nodes.iter().any(|node| {
        node.kind == NodeKind::Param
            && matches!(node.payload, Payload::Cid(param_cid) if param_cid == receiver_cid)
            && il
                .param_type_facts
                .iter()
                .any(|fact| fact.span == node.span && matches!(fact.semantic, ParamSemantic::Map))
    })
}

fn strict_exact_literal_collection_receiver_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.kind(node) == NodeKind::Seq
        && strict_exact_membership_collection_safe(il, interner, facts, node)
}

fn strict_exact_membership_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        if il.kind(node) == NodeKind::Call {
            return strict_exact_set_constructor_collection_safe(il, interner, facts, node)
                || strict_exact_python_collection_factory_safe(il, interner, facts, node)
                || strict_exact_ruby_set_factory_safe(il, interner, facts, node)
                || strict_exact_rust_vec_macro_collection_safe(il, interner, facts, node)
                || strict_exact_rust_std_collection_factory_safe(il, interner, facts, node)
                || strict_exact_java_collection_factory_safe(il, interner, facts, node);
        }
        return strict_exact_safe_tree(il, interner, facts, node);
    }
    let tag_safe = match il.node(node).payload {
        Payload::None => true,
        Payload::Name(name) => matches!(
            interner.resolve(name),
            "array" | "list" | "array_expression" | "composite_literal"
        ),
        _ => false,
    };
    tag_safe
        && il
            .children(node)
            .iter()
            .all(|&c| strict_exact_safe_tree(il, interner, facts, c))
}

fn strict_exact_set_constructor_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || !strict_exact_callee_name(il, interner, kids[0], "Set") {
        return false;
    }
    strict_exact_membership_collection_safe(il, interner, facts, kids[1])
}

fn strict_exact_python_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Python || il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 {
        return false;
    }
    let builtin = if il.kind(kids[0]) == NodeKind::Var {
        let Payload::Name(name) = il.node(kids[0]).payload else {
            return false;
        };
        let name = interner.resolve(name);
        matches!(name, "list" | "set" | "frozenset" | "tuple")
            && !file_defines_name(il, interner, name)
    } else {
        false
    };
    let imported_stdlib_factory =
        strict_exact_python_imported_factory_name(il, interner, kids[0], "collections", "deque");
    (builtin || imported_stdlib_factory)
        && strict_exact_membership_collection_safe(il, interner, facts, kids[1])
}

fn strict_exact_python_imported_factory_name(
    il: &Il,
    interner: &Interner,
    callee: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    match il.kind(callee) {
        NodeKind::Var => {
            let Payload::Name(local) = il.node(callee).payload else {
                return false;
            };
            !unit_defines_symbol(il, local)
                && top_level_assignment_count(il, local) == 1
                && top_level_assignment_rhs(il, local).is_some_and(|rhs| {
                    import_binding_rhs_matches(il, interner, rhs, module, exported)
                })
        }
        NodeKind::Field => {
            let Payload::Name(method) = il.node(callee).payload else {
                return false;
            };
            if interner.resolve(method) != exported {
                return false;
            }
            let Some(&receiver) = il.children(callee).first() else {
                return false;
            };
            if il.kind(receiver) != NodeKind::Var {
                return false;
            }
            let Payload::Name(namespace) = il.node(receiver).payload else {
                return false;
            };
            !unit_defines_symbol(il, namespace)
                && top_level_assignment_count(il, namespace) == 1
                && top_level_assignment_rhs(il, namespace)
                    .is_some_and(|rhs| import_namespace_rhs_matches(il, interner, rhs, module))
        }
        _ => false,
    }
}

fn unit_defines_symbol(il: &Il, symbol: Symbol) -> bool {
    il.units
        .iter()
        .any(|unit| unit.name.is_some_and(|name| name == symbol))
}

fn top_level_assignment_count(il: &Il, symbol: Symbol) -> usize {
    top_level_statements(il)
        .iter()
        .filter(|&&stmt| assignment_name(il, stmt).is_some_and(|name| name == symbol))
        .count()
}

fn top_level_assignment_rhs(il: &Il, symbol: Symbol) -> Option<NodeId> {
    top_level_statements(il).into_iter().find_map(|stmt| {
        if !assignment_name(il, stmt).is_some_and(|name| name == symbol) {
            return None;
        }
        let kids = il.children(stmt);
        (kids.len() == 2).then_some(kids[1])
    })
}

fn import_binding_rhs_matches(
    il: &Il,
    interner: &Interner,
    rhs: NodeId,
    module: &str,
    exported: &str,
) -> bool {
    let kids = il.children(rhs);
    il.kind(rhs) == NodeKind::Seq
        && matches!(
            il.node(rhs).payload,
            Payload::Name(seq_name) if interner.resolve(seq_name) == "import_binding"
        )
        && kids.len() == 2
        && matches!(il.node(kids[0]).payload, Payload::LitStr(hash) if hash == stable_symbol_hash(module))
        && matches!(il.node(kids[1]).payload, Payload::LitStr(hash) if hash == stable_symbol_hash(exported))
}

fn import_namespace_rhs_matches(il: &Il, interner: &Interner, rhs: NodeId, module: &str) -> bool {
    let kids = il.children(rhs);
    il.kind(rhs) == NodeKind::Seq
        && matches!(
            il.node(rhs).payload,
            Payload::Name(seq_name) if interner.resolve(seq_name) == "import_namespace"
        )
        && kids.len() == 1
        && matches!(il.node(kids[0]).payload, Payload::LitStr(hash) if hash == stable_symbol_hash(module))
}

fn strict_exact_ruby_set_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Ruby
        || il.kind(node) != NodeKind::Call
        || !ruby_file_requires_module(il, interner, "set")
        || file_defines_name(il, interner, "Set")
    {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(kids[0]).payload else {
        return false;
    };
    if interner.resolve(method) != "new" {
        return false;
    }
    let Some(&receiver) = il.children(kids[0]).first() else {
        return false;
    };
    strict_exact_callee_name(il, interner, receiver, "Set")
        && strict_exact_membership_collection_safe(il, interner, facts, kids[1])
}

fn ruby_file_requires_module(il: &Il, interner: &Interner, module: &str) -> bool {
    if il.meta.lang != Lang::Ruby {
        return false;
    }
    let expected = stable_symbol_hash(module);
    top_level_statements(il).iter().any(|&stmt| {
        let expr = if il.kind(stmt) == NodeKind::ExprStmt {
            il.children(stmt).first().copied()
        } else {
            Some(stmt)
        };
        let Some(call) = expr else {
            return false;
        };
        if il.kind(call) != NodeKind::Call {
            return false;
        }
        let kids = il.children(call);
        if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
            return false;
        }
        let Payload::Name(callee) = il.node(kids[0]).payload else {
            return false;
        };
        if interner.resolve(callee) != "require" {
            return false;
        }
        matches!(il.node(kids[1]).payload, Payload::LitStr(hash) if hash == expected)
    })
}

fn strict_exact_rust_vec_macro_collection_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Rust || il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    strict_exact_callee_name(il, interner, callee, "vec")
        && !file_defines_name(il, interner, "vec")
        && kids
            .iter()
            .skip(1)
            .all(|&kid| strict_exact_safe_tree(il, interner, facts, kid))
}

fn strict_exact_rust_std_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Rust || il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(kids[0]).payload else {
        return false;
    };
    if !matches!(
        interner.resolve(name),
        "std::collections::HashSet::from"
            | "std::collections::BTreeSet::from"
            | "std::collections::VecDeque::from"
    ) {
        return false;
    }
    strict_exact_membership_collection_safe(il, interner, facts, kids[1])
}

fn strict_exact_java_collection_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Java || il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() < 2 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(kids[0]).payload else {
        return false;
    };
    let Some(&receiver) = il.children(kids[0]).first() else {
        return false;
    };
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Payload::Name(receiver_name) = il.node(receiver).payload else {
        return false;
    };
    let method = interner.resolve(method);
    let receiver_name = interner.resolve(receiver_name);
    let standard_factory = match (receiver_name, method) {
        ("List" | "Set", "of") => true,
        ("Arrays", "asList") => true,
        _ => false,
    };
    standard_factory
        && !java_file_defines_type_name(il, interner, receiver_name)
        && kids
            .iter()
            .skip(1)
            .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

fn java_file_defines_type_name(il: &Il, interner: &Interner, name: &str) -> bool {
    if il.meta.lang != Lang::Java {
        return false;
    }
    il.units.iter().any(|unit| {
        unit.kind == UnitKind::Class
            && unit
                .name
                .is_some_and(|symbol| interner.resolve(symbol) == name)
    })
}

fn file_defines_name(il: &Il, interner: &Interner, name: &str) -> bool {
    top_level_statements(il).iter().any(|&stmt| {
        assignment_name(il, stmt).is_some_and(|symbol| interner.resolve(symbol) == name)
    }) || il.units.iter().any(|unit| {
        unit.name
            .is_some_and(|symbol| interner.resolve(symbol) == name)
    })
}

fn strict_exact_java_std_var_name(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: &str,
) -> bool {
    if il.kind(node) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(node).payload else {
        return false;
    };
    let name = interner.resolve(name);
    name == expected && !java_file_defines_type_name(il, interner, name)
}

fn strict_exact_java_map_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Java || il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.is_empty() || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(kids[0]).payload else {
        return false;
    };
    let Some(&receiver) = il.children(kids[0]).first() else {
        return false;
    };
    if !strict_exact_java_std_var_name(il, interner, receiver, "Map") {
        return false;
    }
    match interner.resolve(method) {
        "of" => {
            let entries = &kids[1..];
            entries.len() % 2 == 0
                && entries
                    .iter()
                    .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
        }
        "ofEntries" => kids
            .iter()
            .skip(1)
            .all(|&entry| strict_exact_java_map_entry_safe(il, interner, facts, entry)),
        _ => false,
    }
}

fn strict_exact_java_map_entry_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 3 || il.kind(kids[0]) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(kids[0]).payload else {
        return false;
    };
    if interner.resolve(method) != "entry" {
        return false;
    }
    let Some(&receiver) = il.children(kids[0]).first() else {
        return false;
    };
    strict_exact_java_std_var_name(il, interner, receiver, "Map")
        && kids
            .iter()
            .skip(1)
            .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

fn strict_exact_map_constructor_entries_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || !strict_exact_callee_name(il, interner, kids[0], "Map") {
        return false;
    }
    strict_exact_map_entries_safe(il, interner, facts, kids[1])
}

fn strict_exact_rust_std_map_factory_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Rust || il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    if kids.len() != 2 || il.kind(kids[0]) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(kids[0]).payload else {
        return false;
    };
    if !matches!(
        interner.resolve(name),
        "std::collections::HashMap::from" | "std::collections::BTreeMap::from"
    ) {
        return false;
    }
    strict_exact_map_entries_safe(il, interner, facts, kids[1])
}

fn strict_exact_map_entries_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(name) = il.node(node).payload else {
        return false;
    };
    if !matches!(
        interner.resolve(name),
        "array" | "list" | "array_expression"
    ) {
        return false;
    }
    il.children(node).iter().all(|&entry| {
        il.kind(entry) == NodeKind::Seq
            && il.children(entry).len() == 2
            && strict_exact_safe_tree(il, interner, facts, entry)
    })
}

fn strict_exact_go_literal_zero_map_index_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.meta.lang != Lang::Go || il.kind(node) != NodeKind::Index {
        return false;
    }
    let kids = il.children(node);
    kids.len() == 2
        && strict_exact_go_literal_zero_map_safe(il, interner, facts, kids[0])
        && strict_exact_safe_tree(il, interner, facts, kids[1])
}

fn strict_exact_go_literal_zero_map_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(name) = il.node(node).payload else {
        return false;
    };
    if interner.resolve(name) != "composite_literal" || il.children(node).is_empty() {
        return false;
    }
    let mut value_kind = None;
    il.children(node).iter().all(|&entry| {
        if il.kind(entry) != NodeKind::Seq {
            return false;
        }
        let Payload::Name(entry_name) = il.node(entry).payload else {
            return false;
        };
        let kv = il.children(entry);
        if interner.resolve(entry_name) != "keyed_element"
            || kv.len() != 2
            || !matches!(il.node(kv[0]).payload, Payload::LitStr(_))
            || !strict_exact_safe_tree(il, interner, facts, kv[0])
        {
            return false;
        }
        let Some(kind) = strict_exact_go_zero_value_kind(il.node(kv[1]).payload) else {
            return false;
        };
        match value_kind {
            Some(current) if current != kind => false,
            Some(_) => true,
            None => {
                value_kind = Some(kind);
                true
            }
        }
    })
}

fn strict_exact_go_zero_value_kind(payload: Payload) -> Option<u8> {
    match payload {
        Payload::LitInt(_) => Some(1),
        Payload::LitStr(_) => Some(2),
        Payload::LitBool(_) => Some(3),
        Payload::LitFloat(_) => Some(4),
        Payload::Lit(LitClass::Null) => Some(5),
        _ => None,
    }
}

fn strict_exact_call_args_safe(
    il: &Il,
    interner: &Interner,
    facts: &StrictFacts,
    node: NodeId,
) -> bool {
    il.children(node)
        .iter()
        .skip(1)
        .all(|&arg| strict_exact_safe_tree(il, interner, facts, arg))
}

fn strict_exact_field_receiver_name(
    il: &Il,
    interner: &Interner,
    field: NodeId,
    expected: &str,
) -> bool {
    let Some(&receiver) = il.children(field).first() else {
        return false;
    };
    if il.kind(receiver) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(receiver).payload else {
        return false;
    };
    interner.resolve(name) == expected
}

fn strict_exact_callee_name(il: &Il, interner: &Interner, callee: NodeId, expected: &str) -> bool {
    if il.kind(callee) != NodeKind::Var {
        return false;
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return false;
    };
    interner.resolve(name) == expected
}

fn stable_symbol_hash(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn strict_exact_callee_identity(il: &Il, facts: &StrictFacts, callee: NodeId) -> bool {
    match il.kind(callee) {
        NodeKind::Var => strict_exact_safe_var(il, facts, callee),
        NodeKind::Field => {
            matches!(il.node(callee).payload, Payload::Name(_))
                && il
                    .children(callee)
                    .first()
                    .is_some_and(|&receiver| strict_exact_callee_identity(il, facts, receiver))
        }
        _ => false,
    }
}

/// Collect sub-function block roots (loops / ifs / try) as extra unit candidates.
fn collect_block_units(il: &Il, node: NodeId, out: &mut Vec<(NodeId, UnitKind, Option<Symbol>)>) {
    let is_statement_if = il.kind(node) == NodeKind::If
        && il
            .children(node)
            .iter()
            .skip(1)
            .any(|&child| il.kind(child) == NodeKind::Block);
    if matches!(il.kind(node), NodeKind::Loop | NodeKind::Try) || is_statement_if {
        out.push((node, UnitKind::Block, None));
    }
    for &c in il.children(node) {
        collect_block_units(il, c, out);
    }
}

/// Pre-order DFS collecting all descendant node ids of `root` (inclusive).
fn collect_pre(il: &Il, root: NodeId, out: &mut Vec<NodeId>) {
    out.push(root);
    for &c in il.children(root) {
        collect_pre(il, c, out);
    }
}
