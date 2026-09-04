//! The fragment behavior oracle: lower a [`FragmentContract`] into a runnable wrapper and
//! run it through the existing unit interpreter.
//!
//! Issue #33 decision: fragments go through the *same* independent behavior check as
//! whole functions, via **wrapper synthesis** rather than a new `run_fragment` interpreter
//! path. A contract is lowered into a synthetic `Func` — its free inputs become parameters,
//! its body is a deep copy of the fragment subtree — and handed to
//! [`nose_normalize::run_unit`]. We reuse `run_unit`, its [`Behavior`] (return value +
//! ordered effects + final field state), and the caller's input battery unchanged.
//!
//! The forcing function: a contract that cannot be lowered into a runnable wrapper is
//! *underspecified*. [`synthesize_wrapper`] returning `None` is therefore a signal that the
//! recognizer described a fragment the oracle cannot vouch for — fail closed.
//!
//! proof-obligation: detect.fragment.free_inputs
//! proof-obligation: detect.fragment.wrapper_synthesis
//! proof-obligation: il.arena.deep_copy

use super::contract::{Effect, FragmentContract};
use nose_il::{
    Builtin, FileMeta, Il, IlBuilder, Interner, LoopKind, NodeId, NodeKind, Payload, Span, Unit,
    UnitKind,
};
use nose_normalize::{run_unit, Behavior, Value};
use nose_semantics::admitted_builder_append_call_args;
use std::collections::HashMap;

/// How the offline oracle is allowed to represent one input.
///
/// `Declared` retains the whole source domain. `Cardinality` is a narrower quotient: the
/// fragment was structurally proven to observe that input only through an admitted `Len`
/// builtin, so bounded representative lists can execute the fragment without inventing erased
/// element values. `UnusedTrailing` applies only to whole functions and is proved by the
/// Soundness Lab collector.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OracleInputProjection {
    Declared,
    Cardinality,
    /// The parameter is part of a whole-function declaration but a trailing suffix proof found
    /// no read of it. It is excluded from the oracle's effective input contract.
    UnusedTrailing,
}

impl OracleInputProjection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Cardinality => "cardinality",
            Self::UnusedTrailing => "unused-trailing",
        }
    }
}

/// Prove the executable input projection for every free input in `contract`.
///
/// A cardinality proof requires at least one occurrence of the input and requires *every*
/// occurrence in the fragment subtree to be the sole argument of an already-admitted
/// `Builtin::Len` node. Indexing, iteration, element comparison, mutation, passing the value to
/// another call, or any unrecognized use falls back to the full declared domain.
pub fn fragment_input_projections(
    il: &Il,
    contract: &FragmentContract,
) -> Vec<OracleInputProjection> {
    contract
        .inputs
        .iter()
        .map(|&cid| {
            let mut seen = false;
            if input_is_cardinality_only(il, contract.root, None, cid, &mut seen) && seen {
                OracleInputProjection::Cardinality
            } else {
                OracleInputProjection::Declared
            }
        })
        .collect()
}

fn input_is_cardinality_only(
    il: &Il,
    node: NodeId,
    parent: Option<NodeId>,
    cid: u32,
    seen: &mut bool,
) -> bool {
    if il.kind(node) == NodeKind::Var && il.node(node).payload == Payload::Cid(cid) {
        *seen = true;
        let Some(parent) = parent else {
            return false;
        };
        if il.kind(parent) != NodeKind::Call
            || il.node(parent).payload != Payload::Builtin(Builtin::Len)
            || il.children(parent) != [node]
        {
            return false;
        }
    }
    il.children(node)
        .iter()
        .all(|&child| input_is_cardinality_only(il, child, Some(node), cid, seen))
}

/// Run the fragment described by `contract` on `args` (bound to its inputs in order) and
/// return its observable [`Behavior`], or `None` if the wrapper cannot be synthesized or
/// the interpreter cannot model the fragment.
pub fn fragment_behavior(
    il: &Il,
    interner: &Interner,
    contract: &FragmentContract,
    args: &[Value],
) -> Option<Behavior> {
    let (synth, func) = synthesize_wrapper(il, interner, contract)?;
    run_unit(&synth, interner, func, args)
}

/// Lower `contract` into a fresh single-`Func` [`Il`] and return that IL plus the func id.
///
/// Layout of the synthesized function: `Func[ Param(input₀) … Param(inputₙ) , Block[ <copy
/// of fragment subtree> ] ]`. Parameters carry the fragment's free canonical ids so the
/// deep-copied `Var` references resolve against them; the interpreter binds them
/// positionally from `args`.
pub fn synthesize_wrapper(
    il: &Il,
    interner: &Interner,
    contract: &FragmentContract,
) -> Option<(Il, NodeId)> {
    synthesize_wrapper_with_module_strings(il, interner, contract, true)
}

/// Soundness Lab replay variant that can reproduce the pre-Swift tranche from the same binary.
/// Product callers always use [`synthesize_wrapper`], which keeps the proven bindings enabled.
pub fn synthesize_wrapper_with_module_strings(
    il: &Il,
    interner: &Interner,
    contract: &FragmentContract,
    include_module_strings: bool,
) -> Option<(Il, NodeId)> {
    let mut b = IlBuilder::new(il.file);
    let syn = Span::synthetic(il.file);
    let policy = CopyPolicy {
        canonicalize_append_effects: contract
            .effects
            .iter()
            .any(|site| site.effect == Effect::Append),
    };
    let input_spans = enclosing_parameter_spans(il, contract.root);
    let referenced_names = referenced_name_symbols(il, contract.root);
    let module_strings = if include_module_strings {
        nose_normalize::module_facts::immutable_module_string_bindings(il, interner)
    } else {
        Vec::new()
    };
    let module_statements: Vec<NodeId> = module_strings
        .into_iter()
        .filter(|binding| referenced_names.contains(&binding.name))
        .map(|binding| binding.statement)
        .map(|statement| copy_subtree(il, interner, statement, &mut b, policy))
        .collect::<Option<Vec<_>>>()?;

    // Parameters: one per free input, in the contract's canonical order.
    let mut children: Vec<NodeId> = contract
        .inputs
        .iter()
        .map(|&cid| {
            // Parameter-domain evidence is anchored to the original declaration span. Preserve
            // that span in the wrapper so typed battery coercion and hard-lane domain comparison
            // see the same source contract as the enclosing function. A free local with no
            // enclosing parameter deliberately stays synthetic/unknown.
            let span = input_spans.get(&cid).copied().unwrap_or(syn);
            b.add(NodeKind::Param, Payload::Cid(cid), span, &[])
        })
        .collect();

    // Body: deep-copy the fragment into the wrapper's body block. A block-rooted fragment
    // (a conditional branch, a loop or ordered-effect body) is spliced statement-by-statement
    // so the wrapper body stays flat rather than a `Block` nested in a `Block`; a single
    // statement becomes the lone body statement. Either way the interpreter executes the same
    // statements in the same order.
    let body_stmts: Vec<NodeId> = if il.kind(contract.root) == NodeKind::Block {
        il.children(contract.root)
            .to_vec()
            .iter()
            .map(|&s| copy_subtree(il, interner, s, &mut b, policy))
            .collect::<Option<Vec<_>>>()?
    } else {
        vec![copy_subtree(il, interner, contract.root, &mut b, policy)?]
    };
    let body = b.add(NodeKind::Block, Payload::None, syn, &body_stmts);
    children.push(body);

    let func = b.add(NodeKind::Func, Payload::None, syn, &children);
    let meta = FileMeta {
        path: il.meta.path.clone(),
        lang: il.meta.lang,
    };
    let units = vec![Unit {
        root: func,
        kind: UnitKind::Function,
        name: None,
        origin: Default::default(),
    }];
    let root = if module_statements.is_empty() {
        func
    } else {
        let mut children = module_statements;
        children.push(func);
        b.add(NodeKind::Module, Payload::None, syn, &children)
    };
    let mut synth = b.finish(root, meta, units, il.cid_names.clone());
    // Copied fragment nodes keep their original spans, so their semantic evidence
    // remains valid for interpreter admission in the wrapper.
    synth.edit().evidence = il.evidence.clone();
    debug_assert!(
        synth.validate().is_ok(),
        "synthesized fragment wrapper must be a valid arena"
    );
    Some((synth, func))
}

fn referenced_name_symbols(il: &Il, root: NodeId) -> std::collections::HashSet<nose_il::Symbol> {
    let mut names = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Payload::Name(name) = il.node(node).payload {
            names.insert(name);
        }
        stack.extend(il.children(node));
    }
    names
}

fn enclosing_parameter_spans(il: &Il, root: NodeId) -> HashMap<u32, Span> {
    let mut parents = vec![None; il.nodes.len()];
    let mut stack = vec![il.root];
    while let Some(parent) = stack.pop() {
        for &child in il.children(parent) {
            if parents[child.0 as usize].is_none() {
                parents[child.0 as usize] = Some(parent);
                stack.push(child);
            }
        }
    }
    let mut cursor = Some(root);
    while let Some(node) = cursor {
        if il.kind(node) == NodeKind::Func {
            let mut spans = HashMap::new();
            let mut duplicates = std::collections::HashSet::new();
            for &child in il.children(node) {
                if il.kind(child) != NodeKind::Param {
                    continue;
                }
                let Payload::Cid(cid) = il.node(child).payload else {
                    continue;
                };
                if spans.insert(cid, il.node(child).span).is_some() {
                    duplicates.insert(cid);
                }
            }
            for cid in duplicates {
                // Duplicate canonical ids do not identify one declaration domain. Leave the
                // wrapper input unknown instead of choosing one by source order.
                spans.remove(&cid);
            }
            return spans;
        }
        cursor = parents[node.0 as usize];
    }
    HashMap::new()
}

#[derive(Clone, Copy)]
struct CopyPolicy {
    canonicalize_append_effects: bool,
}

/// Deep-copy the subtree rooted at `node` from `src` into `b`, preserving kind, payload, and
/// span unless the accepted contract needs an append effect surface made executable. That
/// rewrite is deliberately local to wrapper synthesis: normal semantic normalization remains
/// proof-gated and does not infer collection semantics from a method name alone.
fn copy_subtree(
    src: &Il,
    interner: &Interner,
    node: NodeId,
    b: &mut IlBuilder,
    policy: CopyPolicy,
) -> Option<NodeId> {
    if policy.canonicalize_append_effects {
        if let Some((receiver, value)) = append_surface_parts(src, interner, node) {
            let receiver_tag = append_receiver_tag(src, receiver)?;
            let target = copy_subtree(src, interner, receiver, b, policy)?;
            let mut kids = Vec::with_capacity(2);
            kids.push(target);
            let value = copy_subtree(src, interner, value, b, policy)?;
            let tag = b.add(
                NodeKind::Lit,
                Payload::LitInt(receiver_tag),
                src.node(node).span,
                &[],
            );
            let tagged_value = b.add(
                NodeKind::Seq,
                Payload::None,
                src.node(node).span,
                &[tag, value],
            );
            kids.push(tagged_value);
            return Some(b.add(
                NodeKind::Call,
                Payload::Builtin(Builtin::Append),
                src.node(node).span,
                &kids,
            ));
        }
    }

    let kids: Vec<NodeId> = src
        .children(node)
        .to_vec()
        .iter()
        .map(|&c| copy_subtree(src, interner, c, b, policy))
        .collect::<Option<Vec<_>>>()?;
    let n = src.node(node);
    Some(b.add(n.kind, n.payload, n.span, &kids))
}

fn append_surface_parts(src: &Il, interner: &Interner, node: NodeId) -> Option<(NodeId, NodeId)> {
    admitted_builder_append_call_args(src, interner, node)
}

fn append_receiver_tag(src: &Il, receiver: NodeId) -> Option<i64> {
    match (src.kind(receiver), src.node(receiver).payload) {
        (NodeKind::Var, Payload::Cid(cid)) => Some(i64::from(cid)),
        _ => None,
    }
}

/// Collect the free canonical ids read in the subtree rooted at `node`, in ascending
/// (canonical) order — the cids the fragment reads from its enclosing scope. These become the
/// synthesized wrapper's parameters.
///
/// "Free" excludes cids *bound within* the fragment: a local assigned before use, a `for-each`
/// loop variable, a nested lambda parameter. The interpreter binds those as the wrapper runs
/// (assignment targets and loop patterns enter `env`), so making them parameters would inflate
/// the arity and feed a battery value the fragment immediately overwrites — the loop-variable
/// hazard that previously made loop/temp shapes unmodelable. The binding model mirrors the one
/// alpha-renaming uses (see `nose_normalize::alpha`): assignment targets and `for-each`
/// patterns — a `Var`, or each `Var` in a destructuring `Seq` — plus nested `Param`s.
///
/// Soundness: omitting a *genuine* outer input can only under-report, and an unbound `Var`
/// read makes the wrapper uninterpretable (`run_unit` returns `None`) — fail-closed, never a
/// false merge. Index/field stores mutate an existing receiver (which stays a free input) and
/// bind nothing, so they are deliberately not treated as bindings.
pub fn free_input_cids(il: &Il, node: NodeId) -> Vec<u32> {
    let mut reads = Vec::new();
    collect_var_cids(il, node, &mut reads);
    reads.sort_unstable();
    reads.dedup();

    let mut bound = Vec::new();
    collect_bound_cids(il, node, &mut bound);
    bound.sort_unstable();
    bound.dedup();

    reads.retain(|c| bound.binary_search(c).is_err());
    reads
}

fn collect_var_cids(il: &Il, node: NodeId, out: &mut Vec<u32>) {
    if il.kind(node) == NodeKind::Var {
        if let Payload::Cid(c) = il.node(node).payload {
            out.push(c);
        }
    }
    for &k in il.children(node) {
        collect_var_cids(il, k, out);
    }
}

/// Collect cids *bound within* the subtree: assignment targets, `for-each` loop patterns, and
/// nested `Param`s. Mirrors the binding model alpha-renaming uses, so "free" here means the
/// same thing it does after renaming.
fn collect_bound_cids(il: &Il, node: NodeId, out: &mut Vec<u32>) {
    match il.kind(node) {
        NodeKind::Param => {
            if let Payload::Cid(c) = il.node(node).payload {
                out.push(c);
            }
        }
        NodeKind::Assign => {
            if let Some(&lhs) = il.children(node).first() {
                collect_binding_targets(il, lhs, out);
            }
        }
        NodeKind::Loop if matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) => {
            if let Some(&pat) = il.children(node).first() {
                collect_binding_targets(il, pat, out);
            }
        }
        _ => {}
    }
    for &k in il.children(node) {
        collect_bound_cids(il, k, out);
    }
}

/// Assignment / `for`-pattern binding targets: a `Var` cid, or each `Var` in a destructuring
/// `Seq`. Only plain `Var`/`Seq` targets bind a fresh cid; an `Index`/`Field` store target
/// mutates an existing receiver and binds nothing.
fn collect_binding_targets(il: &Il, node: NodeId, out: &mut Vec<u32>) {
    match il.kind(node) {
        NodeKind::Var => {
            if let Payload::Cid(c) = il.node(node).payload {
                out.push(c);
            }
        }
        NodeKind::Seq => {
            for &c in il.children(node) {
                collect_binding_targets(il, c, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
