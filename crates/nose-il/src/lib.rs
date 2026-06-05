//! `nose-il` — the normalized Intermediate Language (IL) at the heart of nose.
//!
//! The IL is a compact, arena-backed tree (see [`Node`]). One [`Il`] holds one
//! lowered source file; a whole codebase is a [`Corpus`] of them sharing a single
//! string [`Interner`]. Every node carries a [`Span`] for sourcemap-style
//! traceback. The crate defines the data model and (de)serialization only — the
//! frontends build raw IL, and `nose-normalize` rewrites it into canonical form.

pub mod intern;
pub mod node;
pub mod span;

mod sexpr;

pub use intern::{symbol_index, Interner, Symbol};
pub use node::{
    Builtin, HoFKind, LitClass, LoopKind, Node, NodeId, NodeKind, Op, ParamSemantic, ParamTypeFact,
    Payload,
};
pub use span::{FileId, FileMeta, Lang, Span};

use serde::{Deserialize, Serialize};

/// A detection unit: a syntactic region (function/method/class/block) tagged by a
/// frontend. Its span comes from `root`'s node. Boundaries are real syntactic
/// boundaries, which gives the detector accurate report spans for free.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub root: NodeId,
    pub kind: UnitKind,
    pub name: Option<Symbol>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    Function,
    Method,
    Class,
    Block,
}

/// One lowered source file. `nodes` is the arena; child links live out-of-line in
/// `edges` so each [`Node`] stays small. `file == ` this file's index in the
/// owning [`Corpus`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Il {
    pub nodes: Vec<Node>,
    pub edges: Vec<NodeId>,
    pub root: NodeId,
    pub file: FileId,
    pub meta: FileMeta,
    pub units: Vec<Unit>,
    /// `cid_names[cid]` is the original identifier name a canonical id came from,
    /// for human-readable reports. Empty before alpha-renaming.
    pub cid_names: Vec<Symbol>,
    /// Byte ranges (start, end) of units dropped by inline `// nose-ignore`. Set at
    /// lowering; consulted by the contiguous channel so suppressed code is excluded
    /// there too (the structural channel excludes it by dropping the unit). Not
    /// preserved across normalization (which rebuilds the arena) — only the raw IL
    /// carries it, which is what the contiguous channel reads.
    #[serde(default)]
    pub suppressed: Vec<(u32, u32)>,
    /// Explicit source-level parameter semantic facts, keyed by the original parameter
    /// node span. Normalization preserves spans; after alpha-renaming the value graph can
    /// connect these facts to canonical parameter ids for strict proof-gated rewrites.
    #[serde(default)]
    pub param_type_facts: Vec<ParamTypeFact>,
}

impl Il {
    #[inline]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    #[inline]
    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.nodes[id.0 as usize].kind
    }

    /// The child node ids of `id`, in order.
    #[inline]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let n = &self.nodes[id.0 as usize];
        let s = n.child_start as usize;
        &self.edges[s..s + n.child_len as usize]
    }

    /// Render a subtree as an s-expression (used by `nose il --format sexpr`).
    pub fn to_sexpr(&self, root: NodeId, interner: &Interner) -> String {
        sexpr::to_sexpr(self, root, interner)
    }

    /// Structural verifier for the arena — the IL analogue of a compiler's IR
    /// validator (LLVM's `verify`, rustc's MIR validator). Every frontend and
    /// normalization pass must produce IL satisfying these invariants; a violation
    /// means an arena-construction bug, not bad input. Cheap and O(n), so it runs
    /// under `debug_assert!` after normalization and in the test harness.
    ///
    /// Returns `Err` with the first violation found.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.nodes.len();
        let edges = self.edges.len();
        if (self.root.0 as usize) >= n {
            return Err(format!("root {} out of bounds (nodes: {n})", self.root.0));
        }
        for (i, node) in self.nodes.iter().enumerate() {
            let start = node.child_start as usize;
            let end = start + node.child_len as usize;
            if end > edges {
                return Err(format!(
                    "node {i} child range {start}..{end} exceeds edge arena (len {edges})"
                ));
            }
            let s = node.span;
            if s.start_byte > s.end_byte || s.start_line > s.end_line {
                return Err(format!("node {i} has a reversed span ({s:?})"));
            }
        }
        for (e, child) in self.edges.iter().enumerate() {
            if (child.0 as usize) >= n {
                return Err(format!("edge {e} references node {} >= {n}", child.0));
            }
        }
        for (u, unit) in self.units.iter().enumerate() {
            if (unit.root.0 as usize) >= n {
                return Err(format!("unit {u} root {} out of bounds", unit.root.0));
            }
        }
        Ok(())
    }
}

/// Incrementally builds an [`Il`] arena via post-order construction: lower the
/// children first, collect their ids, then [`add`](IlBuilder::add) the parent.
pub struct IlBuilder {
    nodes: Vec<Node>,
    edges: Vec<NodeId>,
    file: FileId,
}

impl IlBuilder {
    pub fn new(file: FileId) -> Self {
        IlBuilder {
            nodes: Vec::new(),
            edges: Vec::new(),
            file,
        }
    }

    pub fn file(&self) -> FileId {
        self.file
    }

    /// Allocate a node with the given children (copied into the edge arena).
    pub fn add(
        &mut self,
        kind: NodeKind,
        payload: Payload,
        span: Span,
        children: &[NodeId],
    ) -> NodeId {
        let child_start = self.edges.len() as u32;
        self.edges.extend_from_slice(children);
        let child_len = children.len() as u32;
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node {
            kind,
            payload,
            span,
            child_start,
            child_len,
        });
        id
    }

    /// Number of nodes allocated so far.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn finish(
        self,
        root: NodeId,
        meta: FileMeta,
        units: Vec<Unit>,
        cid_names: Vec<Symbol>,
    ) -> Il {
        Il {
            nodes: self.nodes,
            edges: self.edges,
            root,
            file: self.file,
            meta,
            units,
            cid_names,
            suppressed: Vec::new(),
            param_type_facts: Vec::new(),
        }
    }
}

/// A whole codebase: many lowered files sharing one interner. `files[i].file ==
/// FileId(i)`.
#[derive(Clone)]
pub struct Corpus {
    pub interner: Interner,
    pub files: Vec<Il>,
}

impl Corpus {
    pub fn new(interner: Interner, files: Vec<Il>) -> Self {
        Corpus { interner, files }
    }

    /// Total node count across all files (handy for diagnostics).
    pub fn node_count(&self) -> usize {
        self.files.iter().map(|f| f.nodes.len()).sum()
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;

    fn leaf_il() -> Il {
        let mut b = IlBuilder::new(FileId(0));
        let span = Span::new(FileId(0), 0, 1, 1, 1);
        let root = b.add(NodeKind::Module, Payload::None, span, &[]);
        b.finish(
            root,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn well_formed_il_validates() {
        assert!(leaf_il().validate().is_ok());
    }

    #[test]
    fn dangling_child_is_caught() {
        let mut il = leaf_il();
        il.edges.push(NodeId(999)); // child id past the arena
        il.nodes[0].child_len = 1;
        assert!(il.validate().is_err(), "a dangling child id must fail");
    }

    #[test]
    fn out_of_bounds_root_is_caught() {
        let mut il = leaf_il();
        il.root = NodeId(42);
        assert!(il.validate().is_err(), "an invalid root must fail");
    }

    #[test]
    fn child_range_past_edges_is_caught() {
        let mut il = leaf_il();
        il.nodes[0].child_len = 5; // claims children that don't exist
        assert!(
            il.validate().is_err(),
            "an out-of-range child span must fail"
        );
    }
}
