//! Shared lowering context and helpers used by every per-language frontend.
//! Language-specific walks build IL through this, so the arena/span/intern
//! mechanics live in one place.

use nose_il::{
    FileId, FileMeta, Il, IlBuilder, Interner, Lang, LoopKind, NodeId, NodeKind, Op, ParamSemantic,
    ParamTypeFact, Payload, Span, Symbol, Unit, UnitKind,
};
use nose_semantics::{import_fact_tag, ImportFactKind};
use tree_sitter::Node as TsNode;

/// Mutable state threaded through a single file's lowering.
pub(crate) struct Lowering<'a> {
    pub b: IlBuilder,
    pub src: &'a [u8],
    pub lang: Lang,
    pub interner: &'a Interner,
    pub units: Vec<Unit>,
    pub param_type_facts: Vec<ParamTypeFact>,
    pub param_semantic_aliases: Vec<(String, ParamSemantic)>,
    pub unsigned_32_aliases: Vec<String>,
}

impl<'a> Lowering<'a> {
    pub(crate) fn new(file: FileId, src: &'a [u8], lang: Lang, interner: &'a Interner) -> Self {
        Lowering {
            b: IlBuilder::new(file),
            src,
            lang,
            interner,
            units: Vec::new(),
            param_type_facts: Vec::new(),
            param_semantic_aliases: Vec::new(),
            unsigned_32_aliases: Vec::new(),
        }
    }

    /// Source text covered by a CST node.
    pub(crate) fn text(&self, n: TsNode) -> &'a str {
        n.utf8_text(self.src).unwrap_or("")
    }

    pub(crate) fn sym(&self, s: &str) -> Symbol {
        self.interner.intern(s)
    }

    /// Build a [`Span`] from a CST node (1-based inclusive lines).
    pub(crate) fn span(&self, n: TsNode) -> Span {
        Span::new(
            self.b.file(),
            n.start_byte() as u32,
            n.end_byte() as u32,
            n.start_position().row as u32 + 1,
            n.end_position().row as u32 + 1,
        )
    }

    pub(crate) fn add(
        &mut self,
        kind: NodeKind,
        payload: Payload,
        span: Span,
        children: &[NodeId],
    ) -> NodeId {
        self.b.add(kind, payload, span, children)
    }

    pub(crate) fn record_param_semantic(&mut self, span: Span, semantic: ParamSemantic) {
        self.param_type_facts.push(ParamTypeFact { span, semantic });
    }

    pub(crate) fn record_param_semantic_alias(&mut self, local: &str, semantic: ParamSemantic) {
        let alias = normalize_type_text(local);
        if alias.is_empty() {
            return;
        }
        if let Some((_, existing)) = self
            .param_semantic_aliases
            .iter_mut()
            .find(|(known, _)| known == &alias)
        {
            *existing = semantic;
            return;
        }
        self.param_semantic_aliases.push((alias, semantic));
    }

    pub(crate) fn clear_param_semantic_alias(&mut self, local: &str) {
        let alias = normalize_type_text(local);
        if alias.is_empty() {
            return;
        }
        self.param_semantic_aliases
            .retain(|(known, _)| known != &alias);
    }

    pub(crate) fn record_unsigned_32_alias(&mut self, local: &str) {
        let alias = normalize_type_text(local);
        if alias.is_empty() || self.unsigned_32_aliases.iter().any(|known| known == &alias) {
            return;
        }
        self.unsigned_32_aliases.push(alias);
    }

    pub(crate) fn param_semantic_from_text(&self, text: &str) -> Option<ParamSemantic> {
        param_semantic_from_text(text).or_else(|| {
            let t = normalize_type_text(text);
            self.param_semantic_aliases
                .iter()
                .find_map(|(alias, semantic)| {
                    (t.contains(&format!(":{alias}[")) || t.contains(&format!(":{alias}<")))
                        .then_some(*semantic)
                })
        })
    }

    /// An empty `Block` (used for absent loop init/update slots, empty bodies).
    pub(crate) fn empty_block(&mut self, span: Span) -> NodeId {
        self.b.add(NodeKind::Block, Payload::None, span, &[])
    }

    /// Wrap a single lowered statement in a one-child `Block`, or yield an empty block when
    /// the statement lowered to nothing. This is the shared tail of every frontend's
    /// `stmt_as_block` helper (which differ only in their language's block-node kind and
    /// `lower_stmt`); centralizing it keeps the absent-statement fallback uniform.
    pub(crate) fn block_of_stmt(&mut self, span: Span, stmt: Option<NodeId>) -> NodeId {
        match stmt {
            Some(s) => self.b.add(NodeKind::Block, Payload::None, span, &[s]),
            None => self.empty_block(span),
        }
    }

    /// A `Var` carrying the raw identifier name (canonicalized later).
    pub(crate) fn var(&mut self, name: &str, span: Span) -> NodeId {
        let sym = self.sym(name);
        self.b.add(NodeKind::Var, Payload::Name(sym), span, &[])
    }

    /// Lower an integer literal, retaining its **value** as [`Payload::LitInt`] so the
    /// value-graph (the behavioral fingerprint) keeps behavior-defining constants
    /// distinct — `x % 7` ≢ `x % 11`, `return 100` ≢ `return 200` — rather than
    /// collapsing them to one abstract `Int` (a latent false merge: different behavior,
    /// identical fingerprint). This is the §AH/§AT *behavioral* axis being sound.
    ///
    /// The *candidate* axis stays fuzzy without help here: `node_tag` folds `LitInt`
    /// back to the abstract `Int` class for the structural-shape channel, and candidate
    /// mode is shape-dominant — so clones differing only in an incidental magnitude
    /// (buffer sizes, timeouts) still cluster for refactoring. Non-parseable / oversized
    /// integers fall back to the abstract class.
    pub(crate) fn int_lit(&mut self, text: &str, span: Span) -> NodeId {
        // Strip digit-group underscores (`1_000_000`, common in Rust/Python/etc.).
        let t = text.trim().replace('_', "");
        match t.parse::<i64>() {
            Ok(v) => self.b.add(NodeKind::Lit, Payload::LitInt(v), span, &[]),
            // A float-shaped numeric (`.`/`e` exponent) keeps a value hash so `3.14` ≠
            // `2.71` (JS has one `number` kind, so its floats arrive here). Hex/binary/
            // suffixed integers that don't parse stay the abstract `Int` class (unchanged).
            _ if t.contains(['.', 'e', 'E']) && !t.starts_with("0x") => self.float_lit(text, span),
            _ => self.b.add(
                NodeKind::Lit,
                Payload::Lit(nose_il::LitClass::Int),
                span,
                &[],
            ),
        }
    }

    /// Lower a float literal, retaining a hash of its source text so float constants are
    /// behavior-DISTINCT in the value graph (`3.14` ≠ `2.71`). The structural tag stays the
    /// abstract `Float` class (see `node_tag`), so shape similarity is unaffected.
    pub(crate) fn float_lit(&mut self, text: &str, span: Span) -> NodeId {
        let h = nose_il::stable_symbol_hash(text.trim().trim_end_matches(['f', 'F', 'd', 'D']));
        self.b.add(NodeKind::Lit, Payload::LitFloat(h), span, &[])
    }

    /// Lower a string literal, retaining a content hash so behavior-defining string
    /// constants (`"OPTIONS"`/`"HEAD"`, locale messages, schema-format keys) are
    /// distinct in the value-graph. The structural tag stays the abstract `Str`
    /// class (see `node_tag`), so shape similarity is unaffected.
    pub(crate) fn str_lit(&mut self, text: &str, span: Span) -> NodeId {
        let content = text.trim_matches(|c| c == '"' || c == '\'' || c == '`');
        let h = nose_il::stable_symbol_hash(content);
        self.b.add(NodeKind::Lit, Payload::LitStr(h), span, &[])
    }

    /// An opaque `Raw` node wrapping `children`, tagged with the original surface
    /// kind for debugging. Used for constructs a frontend does not lower.
    pub(crate) fn raw(&mut self, surface_kind: &str, span: Span, children: &[NodeId]) -> NodeId {
        let sym = self.sym(surface_kind);
        self.b
            .add(NodeKind::Raw, Payload::Name(sym), span, children)
    }

    /// Tag a detection unit.
    pub(crate) fn push_unit(&mut self, root: NodeId, kind: UnitKind, name: Option<Symbol>) {
        self.units.push(Unit { root, kind, name });
    }

    /// Collect a CST node's named children into a `Vec` (decouples from the
    /// tree cursor so the borrow checker stays happy during recursion). Comments
    /// are skipped everywhere — they are never semantic and would otherwise land
    /// as `Raw` noise.
    pub(crate) fn named_children(n: TsNode<'a>) -> Vec<TsNode<'a>> {
        let mut cur = n.walk();
        n.named_children(&mut cur)
            .filter(|c| !is_trivia(c.kind()))
            .collect()
    }
}

/// Does `node` have a *direct* child token of the given `kind`? Used to read an
/// operator token (`--`, `++`) off the node it belongs to without being fooled by a
/// nested occurrence in the operand (e.g. the inner `i--` of `a[i--]++`), which a
/// substring scan over the node's whole text would wrongly match.
pub(crate) fn has_direct_token(node: TsNode, kind: &str) -> bool {
    let mut cur = node.walk();
    let found = node.children(&mut cur).any(|c| c.kind() == kind);
    found
}

/// Lower an import / `#include` / `use` statement to a `Seq` of its identifier and
/// string leaves. Imports carry no behavior, but a *duplicated import block* is real
/// copy-paste (jscpd flags it); emitting its tokens lets the contiguous copy-paste
/// channel — nose's Type-1/2 floor — cover it. These form no unit (the structural and
/// behavioral channels ignore them) and rank near-zero, so users never see import
/// noise; only the copy-paste floor does.
pub(crate) fn import_tokens(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    collect_leaf_tokens(lo, node, &mut kids);
    lo.add(NodeKind::Seq, Payload::None, span, &kids)
}

/// A strict semantic proof fact for a static import binding:
/// local name → `(module coordinate, exported symbol)`.
///
/// Frontends only call this for import forms whose module/export identity is fully static.
/// Ambiguous forms fall back to [`import_tokens`], remaining visible to syntax/near but
/// unavailable to strict exact semantic mode.
pub(crate) fn import_binding(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    module: &str,
    exported: &str,
) -> NodeId {
    import_fact(
        lo,
        span,
        local,
        ImportFactKind::Binding,
        &[module, exported],
    )
}

/// A strict semantic proof fact for a static namespace import:
/// local namespace → module coordinate.
pub(crate) fn import_namespace(lo: &mut Lowering, span: Span, local: &str, module: &str) -> NodeId {
    import_fact(lo, span, local, ImportFactKind::Namespace, &[module])
}

/// Shared shape of the static-import proof facts: `local = Seq[tag](str_lit(c) for c in
/// coords)`. Both `import_binding` and `import_namespace` build this exact form, differing
/// only in the Seq tag and the coordinate count. The node-allocation order — lhs var, then
/// each coordinate string, then the Seq, then the Assign — is preserved identically to the
/// original two functions.
fn import_fact(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    kind: ImportFactKind,
    coords: &[&str],
) -> NodeId {
    let lhs = lo.var(local, span);
    let strs: Vec<NodeId> = coords.iter().map(|c| lo.str_lit(c, span)).collect();
    let tag = lo.sym(import_fact_tag(kind));
    let rhs = lo.add(NodeKind::Seq, Payload::Name(tag), span, &strs);
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

/// Emit a `Var` token for every named leaf (identifier, string fragment, path
/// component) in `node`'s subtree — the textual identity of an import.
fn collect_leaf_tokens(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    let named = Lowering::named_children(node);
    if named.is_empty() {
        let t = lo.text(node);
        if !t.is_empty() {
            let span = lo.span(node);
            out.push(lo.var(t, span));
        }
    } else {
        for c in named {
            collect_leaf_tokens(lo, c, out);
        }
    }
}

/// The shared parse → lower-root → finish pipeline every frontend's `lower` entry
/// point repeats. The frontend supplies only what is language-specific: the grammar
/// (`key` + `lang_fn`), its [`Lang`] tag, and `lower_root`, which turns the parsed
/// CST root into the file's `Module` node.
// The arguments are irreducible: the four file-context values (which mirror every
// frontend's `lower` signature) plus the three grammar/lang specifics and the root
// lowering. Bundling them into a struct used by this one function would add
// indirection without clarifying anything.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_file(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
    key: u16,
    lang_fn: impl FnOnce() -> tree_sitter::Language,
    lang: Lang,
    lower_root: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> anyhow::Result<Il> {
    lower_file_with_setup(
        file,
        path,
        src,
        interner,
        key,
        lang_fn,
        lang,
        |_| {},
        lower_root,
    )
}

/// Like [`lower_file`], but lets a frontend seed file-local proof facts after
/// parsing and before walking the root. This keeps language-specific facts in the
/// frontend while preserving the shared IL construction path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_file_with_setup(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
    key: u16,
    lang_fn: impl FnOnce() -> tree_sitter::Language,
    lang: Lang,
    setup: impl FnOnce(&mut Lowering),
    lower_root: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> anyhow::Result<Il> {
    let tree = parse(key, lang_fn, src)?;
    let mut lo = Lowering::new(file, src, lang, interner);
    setup(&mut lo);
    let module = lower_root(&mut lo, tree.root_node());
    let meta = FileMeta {
        path: path.to_string(),
        lang,
    };
    let units = std::mem::take(&mut lo.units);
    let param_type_facts = std::mem::take(&mut lo.param_type_facts);
    let mut il = lo.b.finish(module, meta, units, Vec::new());
    il.param_type_facts = param_type_facts;
    drop_suppressed_units(&mut il, src);
    Ok(il)
}

fn normalize_type_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn param_semantic_from_text(text: &str) -> Option<ParamSemantic> {
    let t = normalize_type_text(text);
    if t.contains("hashmap<")
        || t.contains("btreemap<")
        || t.contains("map<")
        || t.contains("dict[")
        || t.contains("dictionary[")
        || t.contains("mapping[")
        || t.contains("mapping<")
        || t.contains("map[")
    {
        return Some(ParamSemantic::Map);
    }
    if t.contains("option<") || t.contains("optional<") {
        return Some(ParamSemantic::Option);
    }
    if t.contains("set[") || t.contains("set<") || t.contains("hashset<") || t.contains("btreeset<")
    {
        return Some(ParamSemantic::Set);
    }
    if t.contains("[]")
        || t.contains(":&[")
        || t.contains("&[")
        || t.contains("list[")
        || t.contains("list<")
        || t.contains("tuple[")
        || t.contains("container[")
        || t.contains("container<")
        || t.contains("collection<")
        || t.contains("queue<")
        || t.contains("deque<")
        || t.contains("iterable<")
        || t.contains("iterable[")
        || t.contains("sequence[")
        || t.contains("array<")
        || t.contains("readonlyarray<")
        || t.contains("vec<")
        || t.contains("vecdeque<")
        || t.contains("slice<")
    {
        return Some(ParamSemantic::Collection);
    }
    if t.contains("string")
        || t == "str"
        || t == "&str"
        || t.contains(":str")
        || t.contains(":&str")
    {
        return Some(ParamSemantic::String);
    }
    if is_integer_semantic_text(&t) {
        return Some(ParamSemantic::Integer);
    }
    if is_float_semantic_text(&t) || t.contains(":number") || t == "number" {
        return Some(ParamSemantic::Number);
    }
    None
}

fn is_integer_semantic_text(t: &str) -> bool {
    matches!(
        t,
        "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "long"
            | "short"
            | "byte"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
    ) || t.contains(":int")
        || t.contains(":long")
        || t.contains(":short")
        || t.contains(":byte")
        || t.starts_with("int")
        || t.starts_with("long")
        || t.starts_with("short")
        || t.starts_with("byte")
        || t.contains(":i8")
        || t.contains(":i16")
        || t.contains(":i32")
        || t.contains(":i64")
        || t.contains(":i128")
        || t.contains(":isize")
        || t.contains(":u8")
        || t.contains(":u16")
        || t.contains(":u32")
        || t.contains(":u64")
        || t.contains(":u128")
        || t.contains(":usize")
}

fn is_float_semantic_text(t: &str) -> bool {
    matches!(
        t,
        "float" | "float32" | "float64" | "double" | "f32" | "f64"
    ) || t.contains(":float")
        || t.contains(":double")
        || t.contains(":f32")
        || t.contains(":f64")
        || t.starts_with("float")
        || t.starts_with("double")
}

pub(crate) fn stdlib_type_semantic(module: &str, exported: &str) -> Option<ParamSemantic> {
    let module = module.trim();
    let exported = exported.trim();
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "Dict" | "Mapping" | "MutableMapping")
    {
        return Some(ParamSemantic::Map);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "FrozenSet" | "MutableSet" | "Set")
    {
        return Some(ParamSemantic::Set);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(
            exported,
            "Collection"
                | "Container"
                | "Deque"
                | "List"
                | "MutableSequence"
                | "Sequence"
                | "Tuple"
        )
    {
        return Some(ParamSemantic::Collection);
    }
    None
}

/// Inline suppression: drop any unit whose source carries a `nose-ignore` marker
/// on its first line or the line just above it (in a comment, any language). Lets a
/// maintainer mark a clone as intentionally-kept so it never shows up as a candidate.
fn drop_suppressed_units(il: &mut Il, src: &[u8]) {
    if il.units.is_empty() || !contains_marker(src) {
        return; // fast path: nothing to suppress
    }
    let keep: Vec<bool> = il
        .units
        .iter()
        .map(|u| !unit_suppressed(src, il.node(u.root).span.start_byte as usize))
        .collect();
    // Record suppressed units' byte spans so the contiguous channel excludes them too.
    for (u, &kept) in il.units.iter().zip(&keep) {
        if !kept {
            let sp = il.node(u.root).span;
            il.suppressed.push((sp.start_byte, sp.end_byte));
        }
    }
    let mut it = keep.iter();
    il.units.retain(|_| *it.next().unwrap());
}

const SUPPRESS_MARKER: &str = "nose-ignore";

fn contains_marker(src: &[u8]) -> bool {
    // cheap whole-file prescreen so the per-unit work only runs when relevant
    src.windows(SUPPRESS_MARKER.len())
        .any(|w| w.eq_ignore_ascii_case(SUPPRESS_MARKER.as_bytes()))
}

/// Is the unit starting at `start_byte` suppressed — i.e. does its first line or the
/// line immediately above contain the marker (typically in a trailing/preceding
/// comment)?
fn unit_suppressed(src: &[u8], start_byte: usize) -> bool {
    let start = start_byte.min(src.len());
    let cur_begin = src[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    let prev_begin = if cur_begin == 0 {
        0
    } else {
        src[..cur_begin - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |p| p + 1)
    };
    let cur_end = src[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(src.len(), |p| start + p);
    let window = String::from_utf8_lossy(&src[prev_begin..cur_end]);
    window.contains(SUPPRESS_MARKER)
}

/// Lower each named child of `node` with `lower_one`, keeping the `Some` results,
/// and wrap them in a `kind` node (`Module` for a file root, `Block` for a body).
/// Every frontend's module/block builders are this same iterate-lower-collect loop
/// differing only in the node kind and per-language statement lowering.
pub(crate) fn collect_into(
    lo: &mut Lowering,
    node: TsNode,
    kind: NodeKind,
    mut lower_one: impl FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    for child in Lowering::named_children(node) {
        if let Some(id) = lower_one(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(kind, Payload::None, span, &stmts)
}

/// Lower a C-family `switch` (scrutinee in the `condition` field, case groups in
/// `body`) to an `if`/else-if chain. Case labels become `scrutinee == label`
/// conditions; a default label becomes the final `else`. Frontends supply only
/// which child nodes are case groups (`is_case`) and how to lower expressions and
/// statements.
pub(crate) fn switch_to_if_chain(
    lo: &mut Lowering,
    node: TsNode,
    is_case: impl Fn(&str) -> bool,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode) -> NodeId,
    mut lower_stmt: impl FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let cases: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter(|c| is_case(c.kind()))
                .collect()
        })
        .unwrap_or_default();

    let mut branches = Vec::new();
    let mut default_block = None;
    for case in cases {
        let (labels, block) = lower_switch_case(lo, case, span, &mut lower_expr, &mut lower_stmt);
        match fold_switch_case_labels(lo, span, scrutinee, labels) {
            Some(cond) => branches.push((cond, block)),
            None => default_block = Some(block),
        }
    }

    let mut acc = default_block.unwrap_or_else(|| lo.empty_block(span));
    for (cond, block) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, block, acc]);
    }
    acc
}

fn lower_switch_case<E, S>(
    lo: &mut Lowering,
    case: TsNode,
    span: Span,
    lower_expr: &mut E,
    lower_stmt: &mut S,
) -> (Vec<NodeId>, NodeId)
where
    E: FnMut(&mut Lowering, TsNode) -> NodeId,
    S: FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
{
    let mut labels = Vec::new();
    let mut stmts = Vec::new();
    let mut label_phase = true;
    let mut saw_explicit_label = false;

    for child in Lowering::named_children(case) {
        if label_phase && child.kind() == "switch_label" {
            saw_explicit_label = true;
            for label in Lowering::named_children(child) {
                labels.push(lower_expr(lo, label));
            }
            continue;
        }
        if label_phase && !saw_explicit_label && !is_switch_body_child(child.kind()) {
            labels.push(lower_expr(lo, child));
            continue;
        }

        label_phase = false;
        if let Some(id) = lower_stmt(lo, child) {
            stmts.push(id);
        }
    }

    let block = lo.add(NodeKind::Block, Payload::None, span, &stmts);
    (labels, block)
}

fn fold_switch_case_labels(
    lo: &mut Lowering,
    span: Span,
    scrutinee: NodeId,
    labels: Vec<NodeId>,
) -> Option<NodeId> {
    let mut acc = None;
    for label in labels {
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[scrutinee, label],
        );
        acc = Some(match acc {
            None => cond,
            Some(prev) => lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[prev, cond]),
        });
    }
    acc
}

fn is_switch_body_child(kind: &str) -> bool {
    matches!(
        kind,
        "assert_statement"
            | "block"
            | "break_statement"
            | "compound_statement"
            | "continue_statement"
            | "declaration"
            | "do_statement"
            | "expression_statement"
            | "for_statement"
            | "if_statement"
            | "labeled_statement"
            | "local_variable_declaration"
            | "return_statement"
            | "switch_statement"
            | "synchronized_statement"
            | "throw_statement"
            | "try_statement"
            | "try_with_resources_statement"
            | "while_statement"
            | "yield_statement"
    )
}

/// Build a `Func` unit from a `name`/`parameters`/`body`-shaped node and register
/// it for detection (a `Method` when `method`, else a `Function`). Every frontend
/// shares this skeleton — extract the name, lower the parameters, lower the body,
/// push the unit; `lower_params` and `lower_body` are the only language-specific
/// pieces (param node shapes and body/return conventions differ per grammar).
pub(crate) fn function_unit(
    lo: &mut Lowering,
    node: TsNode,
    method: bool,
    lower_params: impl FnOnce(&mut Lowering, TsNode, &mut Vec<NodeId>),
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_params(lo, params, &mut kids);
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    let kind = if method {
        UnitKind::Method
    } else {
        UnitKind::Function
    };
    lo.push_unit(func, kind, name);
    func
}

/// Lower a `left`/`operator`/`right` binary-expression node into a `BinOp`. Every
/// supported grammar names those fields identically; each frontend supplies its
/// dialect's operator resolution and its expression lowering. An operator the
/// dialect doesn't recognise (or a missing operand) becomes a `Raw` node that
/// preserves the children — never a silently-wrong default operator.
pub(crate) fn binary(
    lo: &mut Lowering,
    node: TsNode,
    op_of: impl FnOnce(&str) -> Option<Op>,
    mut lower_operand: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let l = node
        .child_by_field_name("left")
        .map(|x| lower_operand(lo, x));
    let r = node
        .child_by_field_name("right")
        .map(|x| lower_operand(lo, x));
    let op = node
        .child_by_field_name("operator")
        .and_then(|o| op_of(lo.text(o)));
    match (l, r, op) {
        (Some(l), Some(r), Some(op)) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l, r]),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_operand(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

/// Lower a `left`/`right` assignment-expression node into an `Assign`.
/// JS/TS and Rust grammars use the same field names for simple assignment; compound
/// assignment remains frontend-specific because operator spelling and rewrites differ.
pub(crate) fn assignment(
    lo: &mut Lowering,
    node: TsNode,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let lhs = node
        .child_by_field_name("left")
        .map(|l| lower_expr(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = node
        .child_by_field_name("right")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

/// Lower a `condition`/`body`-shaped CST node into a canonical `While` [`Loop`].
/// Every C-family `while` lowers identically apart from *how* its condition and
/// body sub-nodes are lowered, so each frontend supplies those two as closures
/// and shares the field-extraction, empty-fallback, and node-construction here.
pub(crate) fn while_loop(
    lo: &mut Lowering,
    node: TsNode,
    lower_cond: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_cond(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

thread_local! {
    /// Per-thread, per-grammar parser cache. `tree_sitter::Parser::new` allocates
    /// the parser's internal scan stack and lexer caches; recreating one for every
    /// file (corpora run thousands) is pure overhead. Rayon hands each worker its
    /// own thread, so a thread-local pool needs no locking and a grammar's parser
    /// is built at most once per worker.
    static PARSERS: std::cell::RefCell<std::collections::HashMap<u16, tree_sitter::Parser>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Parse `src` with a thread-local parser cached under `key` (which must uniquely
/// identify the grammar — JS/TS/TSX share a crate but need distinct slots).
/// `lang` is only evaluated the first time a thread sees `key`.
pub(crate) fn parse(
    key: u16,
    lang: impl FnOnce() -> tree_sitter::Language,
    src: &[u8],
) -> anyhow::Result<tree_sitter::Tree> {
    PARSERS.with(|cell| {
        let mut pool = cell.borrow_mut();
        let parser = match pool.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let mut p = tree_sitter::Parser::new();
                p.set_language(&lang())?;
                e.insert(p)
            }
        };
        parser
            .parse(src, None)
            .ok_or_else(|| anyhow::anyhow!("parse failed"))
    })
}

/// Stable grammar keys for the thread-local parser pool. JS/TS/TSX are distinct.
pub(crate) mod grammar {
    pub(crate) const PYTHON: u16 = 0;
    pub(crate) const JAVASCRIPT: u16 = 1;
    pub(crate) const TYPESCRIPT: u16 = 2;
    pub(crate) const TSX: u16 = 3;
    pub(crate) const GO: u16 = 4;
    pub(crate) const RUST: u16 = 5;
    pub(crate) const JAVA: u16 = 6;
    pub(crate) const C: u16 = 7;
    pub(crate) const RUBY: u16 = 8;
}

/// Comment / trivia node kinds across the supported grammars.
pub(crate) fn is_trivia(kind: &str) -> bool {
    matches!(
        kind,
        "comment" | "line_comment" | "block_comment" | "hash_bang_line"
    )
}

/// Binary-operator tokens shared by ~every C-family language. Per-language
/// frontends delegate here and then handle their own extras (JS `===`/`**`/`??`,
/// Go `&^`, …) — so the universal operator table lives in one place.
pub(crate) fn common_bin_op(text: &str) -> Option<Op> {
    Some(match text {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" => Op::Mul,
        "/" => Op::Div,
        "%" => Op::Mod,
        "==" => Op::Eq,
        "!=" => Op::Ne,
        "<" => Op::Lt,
        "<=" => Op::Le,
        ">" => Op::Gt,
        ">=" => Op::Ge,
        "&&" => Op::And,
        "||" => Op::Or,
        "&" => Op::BitAnd,
        "|" => Op::BitOr,
        "^" => Op::BitXor,
        "<<" => Op::Shl,
        ">>" => Op::Shr,
        _ => return None,
    })
}
