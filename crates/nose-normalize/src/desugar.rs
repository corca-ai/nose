//! The structural rebuild pass: walks the raw IL and emits a new arena with
//!
//! - C-style loops rewritten to `while` (init hoisted before, update appended to
//!   the body) so they converge with hand-written `while` loops;
//! - cross-language builtins canonicalized (see [`crate::idioms`]);
//! - exact-safe `length` field reads folded to the `Len` builtin;
//! - statement-position blocks flattened, empty blocks dropped;
//! - (when `cfg_norm`) `if c { …; return } else { B }` flattened to
//!   `if c { …; return } ; B`.
//!
//! Unit roots (`Func`/`Method`/class `Block`) are stable node kinds across this
//! pass, so we remap their ids as we go.

use crate::idioms::{canon_call_with_domains, CallCanon};
use crate::NormalizeOptions;
use nose_il::{Il, IlBuilder, Interner, LoopKind, NodeId, NodeKind, Payload};
use nose_semantics::{
    library_api_contract_evidence_for_node, library_property_builtin_contract,
    seq_surface_contract_for_node, DomainRequirement, LibraryApiEvidenceStatus,
    ReceiverDomainEvidenceIndex,
};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn run(old: &Il, interner: &Interner, opts: &NormalizeOptions) -> Il {
    let unit_root_set: FxHashSet<u32> = old.units.iter().map(|u| u.root.0).collect();
    let mut rb = Rebuilder {
        old,
        b: IlBuilder::with_capacity(old.file, old.nodes.len(), old.edges.len()),
        interner,
        opts,
        remap: FxHashMap::default(),
        unit_root_set,
        receiver_domains: ReceiverDomainEvidenceIndex::new(old, interner),
    };
    let new_root = rb.go(old.root);

    // Remap unit roots; drop units whose root somehow vanished.
    crate::finalize_rebuild(old, &rb.remap, rb.b, new_root, Vec::new())
}

struct Rebuilder<'a> {
    old: &'a Il,
    b: IlBuilder,
    interner: &'a Interner,
    opts: &'a NormalizeOptions,
    remap: FxHashMap<u32, NodeId>,
    unit_root_set: FxHashSet<u32>,
    receiver_domains: ReceiverDomainEvidenceIndex<'a>,
}

impl Rebuilder<'_> {
    /// Generic rebuild of any node. Records the unit-root remap.
    fn go(&mut self, old_id: NodeId) -> NodeId {
        let kind = self.old.kind(old_id);
        let new_id = match kind {
            NodeKind::Block => self.block(old_id),
            NodeKind::Call => self.call(old_id),
            NodeKind::Field => self.field(old_id),
            NodeKind::Loop => self.loop_expr(old_id),
            _ => self.generic(old_id),
        };
        if self.unit_root_set.contains(&old_id.0) {
            self.remap.insert(old_id.0, new_id);
        }
        new_id
    }

    // Rebuild children verbatim, copying kind/payload/span (via the shared macro).
    crate::rebuild_generic!();

    fn block(&mut self, old_id: NodeId) -> NodeId {
        let span = self.old.node(old_id).span;
        let child_count = self.old.children(old_id).len();
        let mut out = Vec::with_capacity(child_count);
        for idx in 0..child_count {
            let c = self.old.children(old_id)[idx];
            self.emit_stmt(c, &mut out);
        }
        self.b.add(NodeKind::Block, Payload::None, span, &out)
    }

    /// Emit a statement into `out`, applying block flattening and loop / control
    /// desugaring that needs to insert sibling statements.
    fn emit_stmt(&mut self, old_id: NodeId, out: &mut Vec<NodeId>) {
        let kind = self.old.kind(old_id);
        match kind {
            // Flatten statement-position blocks; drop empties.
            NodeKind::Block => {
                let child_count = self.old.children(old_id).len();
                for idx in 0..child_count {
                    let c = self.old.children(old_id)[idx];
                    self.emit_stmt(c, out);
                }
            }
            NodeKind::Loop => self.emit_loop(old_id, out),
            NodeKind::If if self.opts.cfg_norm => self.emit_if(old_id, out),
            // Canonicalize `ExprStmt(Return|Throw)` to the bare statement. Languages whose
            // `return`/`throw` are expressions (Rust, …) lower them wrapped in an `ExprStmt`;
            // others (Python) emit the bare statement. The value graph already treats the two
            // as equal, but the syntactic recognizers (e.g. recursion::recognize) match on a
            // bare `Return`, so the wrapper silently disabled them for the wrapping languages.
            // Unwrapping here makes return/throw representation language-uniform at the source.
            NodeKind::ExprStmt
                if matches!(
                    self.old.children(old_id),
                    [inner] if matches!(self.old.kind(*inner), NodeKind::Return | NodeKind::Throw)
                ) =>
            {
                let inner = self.old.children(old_id)[0];
                self.emit_stmt(inner, out);
            }
            _ => out.push(self.go(old_id)),
        }
    }

    fn emit_loop(&mut self, old_id: NodeId, out: &mut Vec<NodeId>) {
        let span = self.old.node(old_id).span;
        let kind = match self.old.node(old_id).payload {
            Payload::Loop(k) => k,
            _ => LoopKind::While,
        };
        let kids = self.old.children(old_id).to_vec();
        match kind {
            LoopKind::CStyle if kids.len() == 4 => {
                // [init, cond, update, body] -> init; while(cond) { body; update }
                self.emit_stmt(kids[0], out); // hoist init (flattened, empties dropped)
                let cond = self.go(kids[1]);
                let mut body_stmts = Vec::new();
                self.emit_stmt(kids[3], &mut body_stmts); // body (flattened)
                self.emit_stmt(kids[2], &mut body_stmts); // append update
                let body = self
                    .b
                    .add(NodeKind::Block, Payload::None, span, &body_stmts);
                let wl = self.b.add(
                    NodeKind::Loop,
                    Payload::Loop(LoopKind::While),
                    span,
                    &[cond, body],
                );
                out.push(wl);
            }
            _ => out.push(self.go(old_id)),
        }
    }

    /// `if c { …; return } else { B }`  →  `if c { …; return }` followed by `B`.
    fn emit_if(&mut self, old_id: NodeId, out: &mut Vec<NodeId>) {
        let kids = self.old.children(old_id).to_vec();
        if kids.len() == 3 && self.then_terminates(kids[1]) {
            let cond = self.go(kids[0]);
            let then = self.go(kids[1]);
            let if_span = self
                .old
                .node(kids[0])
                .span
                .merge(self.old.node(kids[1]).span);
            let if_node = self
                .b
                .add(NodeKind::If, Payload::None, if_span, &[cond, then]);
            out.push(if_node);
            // splice the else branch's statements into the enclosing block
            self.emit_stmt(kids[2], out);
        } else {
            out.push(self.go(old_id));
        }
    }

    /// True if the (then-)block's last statement is a control-flow terminator.
    fn then_terminates(&self, block_id: NodeId) -> bool {
        if self.old.kind(block_id) != NodeKind::Block {
            return crate::is_terminator(self.old.kind(block_id));
        }
        match self.old.children(block_id).last() {
            Some(&last) => crate::is_terminator(self.old.kind(last)),
            None => false,
        }
    }

    /// A `Loop` reached as a non-statement (rare). Convert CStyle to a while with
    /// init/update folded into the body; rebuild others faithfully.
    fn loop_expr(&mut self, old_id: NodeId) -> NodeId {
        let span = self.old.node(old_id).span;
        let kind = match self.old.node(old_id).payload {
            Payload::Loop(k) => k,
            _ => LoopKind::While,
        };
        let kids = self.old.children(old_id).to_vec();
        if kind == LoopKind::CStyle && kids.len() == 4 {
            let init = self.go(kids[0]);
            let cond = self.go(kids[1]);
            let update = self.go(kids[2]);
            let body_inner = self.go(kids[3]);
            let body = self
                .b
                .add(NodeKind::Block, Payload::None, span, &[body_inner, update]);
            let wl = self.b.add(
                NodeKind::Loop,
                Payload::Loop(LoopKind::While),
                span,
                &[cond, body],
            );
            return self
                .b
                .add(NodeKind::Block, Payload::None, span, &[init, wl]);
        }
        let new_kids: Vec<NodeId> = kids.iter().map(|&c| self.go(c)).collect();
        self.b.add(
            NodeKind::Loop,
            self.old.node(old_id).payload,
            span,
            &new_kids,
        )
    }

    fn call(&mut self, old_id: NodeId) -> NodeId {
        let span = self.old.node(old_id).span;
        match canon_call_with_domains(self.old, self.interner, &self.receiver_domains, old_id) {
            CallCanon::Builtin { op, arg_olds } => {
                let kids: Vec<NodeId> = arg_olds.iter().map(|&a| self.go(a)).collect();
                self.b
                    .add(NodeKind::Call, Payload::Builtin(op), span, &kids)
            }
            CallCanon::HoF {
                kind,
                collection_old,
                fn_old,
            } => {
                let coll = self.go(collection_old);
                let f = self.go(fn_old);
                self.b
                    .add(NodeKind::HoF, Payload::HoF(kind), span, &[coll, f])
            }
            CallCanon::None => self.generic(old_id),
        }
    }

    /// `x.length` → `Len(x)` when the receiver has exact collection proof;
    /// others pass through.
    fn field(&mut self, old_id: NodeId) -> NodeId {
        let n = *self.old.node(old_id);
        if let Payload::Name(s) = n.payload {
            let name = self.interner.resolve(s);
            if let Some(contract) = library_property_builtin_contract(self.old.meta.lang, name) {
                if !matches!(
                    library_api_contract_evidence_for_node(
                        self.old,
                        self.interner,
                        old_id,
                        contract.id,
                        contract.callee,
                        0,
                    ),
                    LibraryApiEvidenceStatus::Admitted
                ) {
                    return self.generic(old_id);
                }
                if let Some(&base) = self.old.children(old_id).first() {
                    if !property_receiver_exact_safe(
                        self.old,
                        self.interner,
                        &self.receiver_domains,
                        base,
                    ) {
                        return self.generic(old_id);
                    }
                    let new_base = self.go(base);
                    return self.b.add(
                        NodeKind::Call,
                        Payload::Builtin(contract.result),
                        n.span,
                        &[new_base],
                    );
                }
            }
        }
        self.generic(old_id)
    }
}

fn property_receiver_exact_safe(
    il: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    seq_receiver_exact_collection_safe(il, interner, node)
        || domains.receiver_satisfies_domain(node, DomainRequirement::ArrayOrCollection)
        || property_receiver_exact_hof_node(il, interner, domains, node)
        || property_receiver_exact_hof_call(il, interner, domains, node)
}

fn seq_receiver_exact_collection_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::Seq {
        return false;
    }
    seq_surface_contract_for_node(il, interner, node)
        .is_some_and(|contract| contract.membership_collection)
}

fn property_receiver_exact_hof_call(
    il: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::Call {
        return false;
    }
    let kids = il.children(node);
    let Some(&callee) = kids.first() else {
        return false;
    };
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    if kids.len() < 2 {
        return false;
    }
    let Some(&receiver) = il.children(callee).first() else {
        return false;
    };
    let method_text = interner.resolve(method);
    let arg_count = kids.len() - 1;
    let Some(kind) = nose_semantics::method_hof_contract(il.meta.lang, method_text) else {
        return false;
    };
    let Some(contract) =
        nose_semantics::library_method_call_contract(il.meta.lang, method_text, arg_count)
    else {
        return false;
    };
    if contract.result.semantic != nose_semantics::MethodSemanticContract::HoF(kind) {
        return false;
    }
    matches!(
        nose_semantics::library_api_contract_evidence_for_call(
            il,
            interner,
            node,
            contract.id,
            contract.callee,
            arg_count,
        ),
        LibraryApiEvidenceStatus::Admitted
    ) && property_receiver_exact_safe(il, interner, domains, receiver)
}

fn property_receiver_exact_hof_node(
    il: &Il,
    interner: &Interner,
    domains: &ReceiverDomainEvidenceIndex<'_>,
    node: NodeId,
) -> bool {
    if il.kind(node) != NodeKind::HoF {
        return false;
    }
    if !matches!(il.node(node).payload, Payload::HoF(_)) {
        return false;
    }
    il.children(node)
        .first()
        .is_some_and(|&receiver| property_receiver_exact_safe(il, interner, domains, receiver))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        EvidenceAnchor, EvidenceKind, FileId, FileMeta, Lang, SequenceSurfaceKind, Span,
    };
    use nose_semantics::FIRST_PARTY_PACK_ID;

    fn sp() -> Span {
        Span::new(FileId(0), 1, 1, 1, 1)
    }

    #[test]
    fn async_like_field_names_are_not_rewritten_without_protocol_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("reader")),
            sp(),
            &[],
        );
        let field = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("aread")),
            sp(),
            &[receiver],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(), &[field]);
        let il = b.finish(
            root,
            FileMeta {
                path: "t.py".to_string(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );

        let out = run(&il, &interner, &NormalizeOptions::default());
        let lowered_field = out
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Field)
            .expect("field should remain a field");
        assert!(matches!(
            lowered_field.payload,
            Payload::Name(name) if interner.resolve(name) == "aread"
        ));
    }

    #[test]
    fn hof_length_field_requires_hof_occurrence_evidence() {
        let interner = Interner::new();
        let mut b = IlBuilder::new(FileId(0));
        let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(), &[]);
        let receiver = b.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp(),
            &[one],
        );
        let callee = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("filter")),
            sp(),
            &[receiver],
        );
        let lambda = b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("predicate")),
            sp(),
            &[],
        );
        let filter_call = b.add(NodeKind::Call, Payload::None, sp(), &[callee, lambda]);
        let length = b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("length")),
            sp(),
            &[filter_call],
        );
        let root = b.add(NodeKind::Block, Payload::None, sp(), &[length]);
        let mut il = b.finish(
            root,
            FileMeta {
                path: "t.js".to_string(),
                lang: Lang::JavaScript,
            },
            Vec::new(),
            Vec::new(),
        );
        il.find_or_push_first_party_evidence(
            EvidenceAnchor::sequence(sp()),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            FIRST_PARTY_PACK_ID,
            "test_sequence_surface",
            Vec::new(),
        );

        let out = run(&il, &interner, &NormalizeOptions::default());
        assert!(
            !out.nodes
                .iter()
                .any(|node| matches!(node.payload, Payload::Builtin(nose_il::Builtin::Len))),
            "raw HOF selector plus receiver evidence must not prove length semantics"
        );
    }
}
