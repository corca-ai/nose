use super::*;

impl<'a> Lowering<'a> {
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

    /// A `Var` proven by the frontend to denote a language-defined unshadowed
    /// global symbol at this source occurrence.
    pub(crate) fn unshadowed_global_var(&mut self, name: &str, span: Span) -> NodeId {
        let var = self.var(name, span);
        self.record_evidence(
            EvidenceAnchor::node(span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash(name),
            }),
            "symbol_unshadowed_global",
        );
        var
    }

    /// Record that a source node denotes an exact language-defined qualified
    /// global path, such as `Array.from` or `Object.hasOwn`.
    pub(crate) fn record_qualified_global_symbol(
        &mut self,
        span: Span,
        kind: NodeKind,
        path: &str,
    ) -> EvidenceId {
        let dependencies = self.qualified_global_root_dependencies(span, path);
        self.record_evidence_with_dependencies(
            EvidenceAnchor::node(span, kind),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash(path),
            }),
            "symbol_qualified_global",
            dependencies,
        )
    }

    /// Record a qualified global API proof for a source-level semantic contract
    /// that is not represented by a preserved IL node.
    pub(crate) fn record_qualified_global_source_symbol(
        &mut self,
        span: Span,
        path: &str,
        rule: &str,
    ) -> EvidenceId {
        let dependencies = self.qualified_global_root_dependencies(span, path);
        self.record_evidence_with_dependencies(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash(path),
            }),
            rule,
            dependencies,
        )
    }

    fn qualified_global_root_dependencies(&mut self, span: Span, path: &str) -> Vec<EvidenceId> {
        let Some(contract) = qualified_global_symbol_contract(self.lang, path) else {
            return Vec::new();
        };
        if !contract.requires_unshadowed_root {
            return Vec::new();
        }
        vec![self.record_evidence(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash(contract.root),
            }),
            "symbol_qualified_global_root",
        )]
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
            _ => self
                .b
                .add(NodeKind::Lit, Payload::Lit(LitClass::Int), span, &[]),
        }
    }

    /// Lower a float literal, retaining a hash of its source text so float constants are
    /// behavior-DISTINCT in the value graph (`3.14` ≠ `2.71`). The structural tag stays the
    /// abstract `Float` class (see `node_tag`), so shape similarity is unaffected.
    pub(crate) fn float_lit(&mut self, text: &str, span: Span) -> NodeId {
        let h = stable_symbol_hash(text.trim().trim_end_matches(['f', 'F', 'd', 'D']));
        self.b.add(NodeKind::Lit, Payload::LitFloat(h), span, &[])
    }

    /// Lower a string literal, retaining a content hash so behavior-defining string
    /// constants (`"OPTIONS"`/`"HEAD"`, locale messages, schema-format keys) are
    /// distinct in the value-graph. The structural tag stays the abstract `Str`
    /// class (see `node_tag`), so shape similarity is unaffected.
    pub(crate) fn str_lit(&mut self, text: &str, span: Span) -> NodeId {
        // Strip exactly the outer delimiter, never quote characters belonging
        // to the value. CST string fragments must use str_content instead.
        let content = ["\"\"\"", "'''", "\"", "'", "`"]
            .into_iter()
            .find_map(|delimiter| text.strip_prefix(delimiter)?.strip_suffix(delimiter));
        match content {
            Some(content) if !content.contains('\\') => self.str_content(content, span),
            // Prefixes and escapes require language-specific decoding. Preserve
            // exact source and fail closed rather than treating token spelling
            // as a runtime string value (which can alias a quoted token).
            _ => self.raw(&format!("string_literal:{:?}:{text}", self.lang), span, &[]),
        }
    }

    /// Already delimited content (template fragments, symbols, generated keys).
    pub(crate) fn str_content(&mut self, content: &str, span: Span) -> NodeId {
        let h = stable_symbol_hash(content);
        self.b.add(NodeKind::Lit, Payload::LitStr(h), span, &[])
    }

    /// An opaque `Raw` node wrapping `children`, tagged with the original surface
    /// kind for debugging. Used for constructs a frontend does not lower.
    pub(crate) fn raw(&mut self, surface_kind: &str, span: Span, children: &[NodeId]) -> NodeId {
        let sym = self.sym(surface_kind);
        self.b
            .add(NodeKind::Raw, Payload::Name(sym), span, children)
    }

    /// Preserve an async `await` boundary until a protocol/demand contract proves
    /// it can be erased safely.
    pub(crate) fn await_boundary(&mut self, span: Span, value: NodeId) -> NodeId {
        self.protocol_boundary(span, SourceProtocolKind::Await, "await", &[value])
    }

    /// Preserve a generator `yield` boundary until a protocol/demand contract
    /// proves it can be interpreted safely.
    pub(crate) fn yield_boundary(&mut self, span: Span, value: Option<NodeId>) -> NodeId {
        let children: Vec<NodeId> = value.into_iter().collect();
        self.protocol_boundary(span, SourceProtocolKind::Yield, "yield", &children)
    }

    /// Preserve a language protocol boundary until a contract proves it can be
    /// interpreted as a shared semantic operation.
    pub(crate) fn protocol_boundary(
        &mut self,
        span: Span,
        protocol: SourceProtocolKind,
        tag: &str,
        children: &[NodeId],
    ) -> NodeId {
        debug_assert!(
            is_protocol_boundary_tag(tag),
            "protocol boundary tag `{tag}` missing from PROTOCOL_BOUNDARY_TAGS — coverage \
             reporting would misclassify it as a lowering gap",
        );
        self.record_source_fact(span, SourceFactKind::Protocol(protocol));
        self.raw(tag, span, children)
    }

    /// Tag a detection unit.
    pub(crate) fn push_unit(&mut self, root: NodeId, kind: UnitKind, name: Option<Symbol>) {
        self.push_unit_with_origin(root, kind, name, UnitOrigin::unknown());
    }

    /// Tag a detection unit with language-neutral source-origin facets.
    pub(crate) fn push_unit_with_origin(
        &mut self,
        root: NodeId,
        kind: UnitKind,
        name: Option<Symbol>,
        origin: UnitOrigin,
    ) {
        self.units.push(Unit {
            root,
            kind,
            name,
            origin,
        });
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
