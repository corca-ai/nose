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

use crate::idioms::{canon_call, CallCanon};
use crate::NormalizeOptions;
use nose_il::{Il, IlBuilder, Interner, LoopKind, NodeId, NodeKind, ParamSemantic, Payload};
use rustc_hash::{FxHashMap, FxHashSet};

pub(crate) fn run(old: &Il, interner: &Interner, opts: &NormalizeOptions) -> Il {
    let unit_root_set: FxHashSet<u32> = old.units.iter().map(|u| u.root.0).collect();
    let mut rb = Rebuilder {
        old,
        b: IlBuilder::new(old.file),
        interner,
        opts,
        remap: FxHashMap::default(),
        unit_root_set,
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
        let children = self.old.children(old_id).to_vec();
        let mut out = Vec::with_capacity(children.len());
        for c in children {
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
                for c in self.old.children(old_id).to_vec() {
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
        match canon_call(self.old, self.interner, old_id) {
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

    /// `x.length` → `Len(x)`; async method/type names → their sync counterpart
    /// (`__aexit__` → `__exit__`, `AsyncIterable` → `Iterable`); others pass through.
    fn field(&mut self, old_id: NodeId) -> NodeId {
        let n = *self.old.node(old_id);
        if let Payload::Name(s) = n.payload {
            let name = self.interner.resolve(s);
            if let Some(builtin) =
                nose_semantics::property_builtin_contract(self.old.meta.lang, name)
            {
                if let Some(&base) = self.old.children(old_id).first() {
                    if !property_receiver_exact_safe(self.old, self.interner, base) {
                        return self.generic(old_id);
                    }
                    let new_base = self.go(base);
                    return self.b.add(
                        NodeKind::Call,
                        Payload::Builtin(builtin),
                        n.span,
                        &[new_base],
                    );
                }
            }
            if let Some(sync) = crate::idioms::async_to_sync(self.old.meta.lang, name) {
                let sym = self.interner.intern(sync);
                let kids: Vec<NodeId> = self
                    .old
                    .children(old_id)
                    .to_vec()
                    .iter()
                    .map(|&c| self.go(c))
                    .collect();
                return self
                    .b
                    .add(NodeKind::Field, Payload::Name(sym), n.span, &kids);
            }
        }
        self.generic(old_id)
    }
}

fn property_receiver_exact_safe(il: &Il, interner: &Interner, node: NodeId) -> bool {
    il.kind(node) == NodeKind::Seq
        || matches!(
            param_semantic_for_var(il, node),
            Some(ParamSemantic::Array | ParamSemantic::Collection)
        )
        || property_receiver_exact_hof_node(il, interner, node)
        || property_receiver_exact_hof_call(il, interner, node)
}

fn param_semantic_for_var(il: &Il, node: NodeId) -> Option<ParamSemantic> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Cid(cid) = il.node(node).payload else {
        return None;
    };
    let span = il.nodes.iter().find_map(|candidate| {
        (candidate.kind == NodeKind::Param && candidate.payload == Payload::Cid(cid))
            .then_some(candidate.span)
    })?;
    il.param_type_facts
        .iter()
        .find(|fact| fact.span == span)
        .map(|fact| fact.semantic)
}

fn property_receiver_exact_hof_call(il: &Il, interner: &Interner, node: NodeId) -> bool {
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
    nose_semantics::method_hof_contract(il.meta.lang, method_text).is_some()
        && property_receiver_exact_safe(il, interner, receiver)
}

fn property_receiver_exact_hof_node(il: &Il, interner: &Interner, node: NodeId) -> bool {
    if il.kind(node) != NodeKind::HoF {
        return false;
    }
    if !matches!(il.node(node).payload, Payload::HoF(_)) {
        return false;
    }
    il.children(node)
        .first()
        .is_some_and(|&receiver| property_receiver_exact_safe(il, interner, receiver))
}
