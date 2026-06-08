//! Evidence-gated standard-library and library-API value recognizers.

use super::*;

impl<'a> Builder<'a> {
    /// Shared skeleton of the collection-factory recognizers: a collection sequence literal
    /// call `Call(0, [callee, Seq(collection)])`
    /// whose `callee` passes `is_factory` wraps the sequence literal `args[1]`; return it. The
    /// per-language recognizers differ ONLY in their callee predicate, so this collapses the
    /// identical skeletons that nose's own duplication gate flagged across them.
    pub(super) fn collection_factory_seq(
        &self,
        value: ValueId,
        is_factory: impl FnOnce(&Self, ValueId) -> bool,
    ) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let (callee, seq) = (node.args[0], node.args[1]);
        if !matches!(
            self.nodes[seq as usize].op,
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        ) {
            return None;
        }
        is_factory(self, callee).then_some(seq)
    }

    /// `factory(<seq>)` where `factory` is a free function/path name that constructs a collection
    /// from a single sequence literal. Data-driven by the first-party collection contracts in
    /// `nose-semantics`; each row carries the names and whether a same-named local definition
    /// shadows the builtin.
    pub(super) fn proven_free_name_collection_factory(&self, value: ValueId) -> Option<ValueId> {
        self.collection_factory_seq(value, |s, callee| {
            let node = &s.nodes[callee as usize];
            let ValOp::Input(key) = node.op else {
                return false;
            };
            let Some(contract) = library_free_name_collection_factory_contracts(s.il.meta.lang)
                .find(|contract| {
                    let LibraryApiCalleeContract::FreeName { name, .. } = contract.callee else {
                        return false;
                    };
                    key == s.free_name_input_key(name)
                })
            else {
                return false;
            };
            let LibraryApiCalleeContract::FreeName { name, .. } = contract.callee else {
                return false;
            };
            s.is_free_name_value(callee, name)
                && matches!(
                    s.library_api_evidence_for_value_call(
                        value,
                        callee,
                        None,
                        contract.id,
                        contract.callee,
                        1,
                    ),
                    LibraryApiEvidenceStatus::Admitted
                )
        })
    }

    /// Python `from collections import deque; deque(<seq>)` — the imported-stdlib collection
    /// factory (the non-free-name part of the former python recognizer).
    pub(super) fn proven_python_deque_collection_value(&self, value: ValueId) -> Option<ValueId> {
        self.collection_factory_seq(value, |s, callee| {
            library_imported_collection_factory_contracts(s.il.meta.lang).any(|contract| {
                let LibraryApiCalleeContract::ImportedBinding { .. } = contract.callee else {
                    return false;
                };
                let receiver = match s.nodes[callee as usize].op {
                    ValOp::Field(_) => s.nodes[callee as usize].args.first().copied(),
                    _ => None,
                };
                match s.library_api_evidence_for_value_call(
                    value,
                    callee,
                    receiver,
                    contract.id,
                    contract.callee,
                    1,
                ) {
                    LibraryApiEvidenceStatus::Admitted => true,
                    LibraryApiEvidenceStatus::Rejected => false,
                    LibraryApiEvidenceStatus::Missing => false,
                }
            })
        })
    }

    pub(super) fn proven_java_collection_factory_value(
        &mut self,
        value: ValueId,
    ) -> Option<ValueId> {
        if !semantics(self.il.meta.lang)
            .stdlib()
            .java_collection_factories()
        {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() < 2 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        if callee.args.len() != 1 {
            return None;
        }
        let receiver = callee.args[0];
        let contract = ["List", "Set", "Arrays"]
            .into_iter()
            .find_map(|receiver_name| {
                let contract = library_java_collection_factory_contract_by_hash(
                    self.il.meta.lang,
                    receiver_name,
                    method,
                )?;
                let LibraryApiCalleeContract::JavaUtilStaticMember { .. } = contract.callee else {
                    return None;
                };
                match self.library_api_evidence_for_value_call(
                    value,
                    args[0],
                    Some(receiver),
                    contract.id,
                    contract.callee,
                    args.len().saturating_sub(1),
                ) {
                    LibraryApiEvidenceStatus::Admitted => Some(contract),
                    LibraryApiEvidenceStatus::Rejected => None,
                    LibraryApiEvidenceStatus::Missing => None,
                }
            })?;
        // A single argument to a varargs collection factory (`Arrays.asList(x)`,
        // `List.of(x)`, `Set.of(x)`) is ambiguous: when `x` is an array it is spread
        // into the element list, but when `x` is a single object it is the sole
        // element. The two readings have different membership semantics
        // (`value` in the array elements vs `value.equals(x)`), so a single argument
        // can only be canonicalized when the receiver is a proven array. Otherwise we
        // must refuse, or an array-typed field and a list-typed field of the same name
        // would false-merge. Multi-argument factories are always a literal element list.
        if args.len() == 2 {
            let single_arg_spreads_array = match contract.result {
                LibraryCollectionFactoryResult::VariadicElements {
                    single_arg_spreads_array,
                } => single_arg_spreads_array,
                _ => false,
            };
            if single_arg_spreads_array && self.is_array_param_value(args[1]) {
                return Some(self.mk(ValOp::ArrayParam, vec![args[1]]));
            }
            return None;
        }
        Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), args[1..].to_vec()))
    }

    pub(super) fn eval_java_collection_constructor_expr(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
    ) -> Option<ValueId> {
        if kids.len() != 1 {
            return None;
        }
        let occurrence =
            admitted_java_collection_constructor_at_call(self.il, self.interner, expr)?;
        if occurrence.arg_count != 0 {
            return None;
        }
        match occurrence.contract.result {
            LibraryCollectionFactoryResult::EmptySequence => {
                Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]))
            }
            _ => None,
        }
    }

    pub(super) fn proven_ruby_set_factory_value(&self, value: ValueId) -> Option<ValueId> {
        self.collection_factory_seq(value, |s, callee_value| {
            let callee = &s.nodes[callee_value as usize];
            let ValOp::Field(method) = callee.op else {
                return false;
            };
            let Some(contract) =
                library_ruby_set_factory_contract_by_hash(s.il.meta.lang, "Set", method, 1)
            else {
                return false;
            };
            let LibraryApiCalleeContract::RubyRequireStaticMember { receiver, .. } =
                contract.callee
            else {
                return false;
            };
            callee.args.len() == 1
                && s.is_free_name_value(callee.args[0], receiver)
                && matches!(
                    s.library_api_evidence_for_value_call(
                        value,
                        callee_value,
                        Some(callee.args[0]),
                        contract.id,
                        contract.callee,
                        1,
                    ),
                    LibraryApiEvidenceStatus::Admitted
                )
        })
    }

    pub(super) fn proven_rust_vec_macro_collection_value(
        &mut self,
        value: ValueId,
    ) -> Option<ValueId> {
        if !semantics(self.il.meta.lang)
            .stdlib()
            .rust_vec_macro_factory()
        {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.is_empty() {
            return None;
        }
        let args = node.args.clone();
        let contract = library_rust_vec_macro_factory_contract(self.il.meta.lang, "vec")?;
        let LibraryApiCalleeContract::RustMacro { name, .. } = contract.callee else {
            return None;
        };
        if !self.is_free_name_value(args[0], name)
            || !matches!(
                self.library_api_evidence_for_value_call(
                    value,
                    args[0],
                    None,
                    contract.id,
                    contract.callee,
                    args.len().saturating_sub(1),
                ),
                LibraryApiEvidenceStatus::Admitted
            )
        {
            return None;
        }
        Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), args[1..].to_vec()))
    }

    pub(super) fn is_import_namespace_expr(
        &mut self,
        expr: NodeId,
        module: &str,
        env: &FxHashMap<u32, ValueId>,
    ) -> bool {
        let value = self.eval(expr, env);
        self.is_import_namespace_value(value, module)
    }

    pub(super) fn is_import_namespace_value(&self, value: ValueId, module: &str) -> bool {
        let node = &self.nodes[value as usize];
        matches!(
            node.op,
            ValOp::ImportNamespace { module_hash }
                if module_hash == stable_symbol_hash(module)
        )
    }

    #[cfg(test)]
    pub(super) fn is_import_binding_value(
        &self,
        value: ValueId,
        module: &str,
        exported: &str,
    ) -> bool {
        let node = &self.nodes[value as usize];
        matches!(
            node.op,
            ValOp::ImportBinding {
                module_hash,
                exported_hash,
            } if module_hash == stable_symbol_hash(module)
                && exported_hash == stable_symbol_hash(exported)
        )
    }

    pub(super) fn import_fact_value(&mut self, expr: NodeId) -> Option<ValueId> {
        let fact = import_fact_evidence_rhs(self.il, expr)?;
        match fact.kind {
            ImportFactKind::Namespace => Some(self.mk(
                ValOp::ImportNamespace {
                    module_hash: fact.module_hash,
                },
                vec![],
            )),
            ImportFactKind::Binding => Some(self.mk(
                ValOp::ImportBinding {
                    module_hash: fact.module_hash,
                    exported_hash: fact.exported_hash?,
                },
                vec![],
            )),
        }
    }

    pub(super) fn file_imports_namespace(&self, expr: NodeId, module: &str) -> bool {
        semantics(self.il.meta.lang)
            .modules()
            .go_import_namespace_facts()
            && imported_namespace_symbol(self.il, self.interner, expr, module)
    }

    pub(super) fn proven_collection_value(&mut self, value: ValueId) -> Option<ValueId> {
        if matches!(
            self.nodes[value as usize].op,
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        ) {
            return Some(value);
        }
        self.proven_free_name_collection_factory(value)
            .or_else(|| self.proven_java_collection_factory_value(value))
            .or_else(|| self.proven_python_deque_collection_value(value))
            .or_else(|| self.proven_ruby_set_factory_value(value))
            .or_else(|| self.proven_rust_vec_macro_collection_value(value))
    }

    pub(super) fn proven_collection_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        self.proven_collection_value(value)
            .or_else(|| self.proven_local_collection_binding_value(expr, env))
    }

    pub(super) fn proven_local_collection_binding_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.proven_local_binding_initializer_value(expr, env, |domain| {
            domain.is_collection_or_set()
        })?;
        self.proven_collection_value(value)
    }

    pub(super) fn proven_local_map_binding_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value =
            self.proven_local_binding_initializer_value(expr, env, |domain| domain.is_map())?;
        self.proven_map_value(value)
    }

    pub(super) fn proven_local_binding_initializer_value(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
        accepts_domain: impl FnOnce(DomainEvidence) -> bool,
    ) -> Option<ValueId> {
        if self.il.kind(expr) != NodeKind::Var {
            return None;
        }
        let Payload::Cid(cid) = self.il.node(expr).payload else {
            return None;
        };
        let (lhs, rhs) = self.local_binding_initializer(cid, expr)?;
        if !nose_semantics::domain_evidence_for_binding_lhs(self.il, self.interner, lhs)
            .is_some_and(accepts_domain)
        {
            return None;
        }
        if self.node_contains_cid(rhs, cid) {
            return None;
        }
        Some(self.eval(rhs, env))
    }

    pub(super) fn local_binding_initializer(
        &self,
        cid: u32,
        use_node: NodeId,
    ) -> Option<(NodeId, NodeId)> {
        let mut rhs = None;
        for (idx, node) in self.il.nodes.iter().enumerate() {
            if node.kind != NodeKind::Assign {
                continue;
            }
            let assign = NodeId(idx as u32);
            let kids = self.il.children(assign);
            if kids.len() != 2 {
                continue;
            }
            if self.node_refers_to_cid(kids[0], cid) {
                if node.span.end_byte > self.il.node(use_node).span.start_byte {
                    continue;
                }
                if rhs.is_some() {
                    return None;
                }
                rhs = Some((kids[0], kids[1]));
            } else if self.node_contains_cid(kids[0], cid) {
                return None;
            }
        }
        rhs
    }

    /// `factory([<entry>, …])` where `factory` is a free name that builds a map from a sequence of
    /// 2-element key/value entries. Data-driven by first-party map contracts in `nose-semantics`;
    /// the matched row's `Seq` tag says how each entry is shaped (JS array vs Rust tuple).
    pub(super) fn proven_free_name_map_factory(&mut self, value: ValueId) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let (callee, seq) = (node.args[0], node.args[1]);
        if !matches!(
            self.nodes[seq as usize].op,
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        ) {
            return None;
        }
        let entry_tag =
            library_free_name_map_factory_contracts(self.il.meta.lang).find_map(|contract| {
                let LibraryApiCalleeContract::FreeName { name, .. } = contract.callee else {
                    return None;
                };
                if !self.is_free_name_value(callee, name)
                    || !matches!(
                        self.library_api_evidence_for_value_call(
                            value,
                            callee,
                            None,
                            contract.id,
                            contract.callee,
                            1,
                        ),
                        LibraryApiEvidenceStatus::Admitted
                    )
                {
                    return None;
                }
                match contract.result {
                    LibraryMapFactoryResult::EntrySequence { entry_seq_tag } => Some(entry_seq_tag),
                    LibraryMapFactoryResult::JavaFactory { .. } => None,
                }
            })?;
        self.map_factory_from_seq(seq, entry_tag)
    }

    /// Canonicalize a collection sequence of 2-element entries to the canonical map shape.
    /// Shared by the free-name and (entry-wise) other map factories.
    pub(super) fn map_factory_from_seq(&mut self, seq: ValueId, entry_tag: u64) -> Option<ValueId> {
        let entries = self.nodes[seq as usize].args.clone();
        let mut canonical_entries = Vec::with_capacity(entries.len());
        for entry in entries {
            let entry_node = &self.nodes[entry as usize];
            if !matches!(entry_node.op, ValOp::Seq(t) if t == entry_tag)
                || entry_node.args.len() != 2
            {
                return None;
            }
            let kv = entry_node.args.clone();
            canonical_entries.push(self.mk(ValOp::Seq(SEQ_VALUE_PAIR), kv));
        }
        Some(self.mk(ValOp::Seq(SEQ_VALUE_MAP), canonical_entries))
    }

    pub(super) fn proven_java_map_factory_entries(&mut self, value: ValueId) -> Option<ValueId> {
        if !semantics(self.il.meta.lang).stdlib().java_map_factories() {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.is_empty() {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        if callee.args.len() != 1 {
            return None;
        }
        let contract = library_java_map_factory_contract_by_hash(self.il.meta.lang, "Map", method)?;
        let LibraryApiCalleeContract::JavaUtilStaticMember { .. } = contract.callee else {
            return None;
        };
        let api_status = self.library_api_evidence_for_value_call(
            value,
            args[0],
            Some(callee.args[0]),
            contract.id,
            contract.callee,
            args.len().saturating_sub(1),
        );
        match api_status {
            LibraryApiEvidenceStatus::Admitted => {}
            LibraryApiEvidenceStatus::Rejected => return None,
            LibraryApiEvidenceStatus::Missing => return None,
        }
        let LibraryMapFactoryResult::JavaFactory { kind } = contract.result else {
            return None;
        };
        if kind == JavaMapFactoryKind::Of {
            let entries = &args[1..];
            if entries.len() % 2 != 0 {
                return None;
            }
            let mut canonical_entries = Vec::with_capacity(entries.len() / 2);
            for kv in entries.chunks(2) {
                canonical_entries.push(self.mk(ValOp::Seq(SEQ_VALUE_PAIR), kv.to_vec()));
            }
            return Some(self.mk(ValOp::Seq(SEQ_VALUE_MAP), canonical_entries));
        }
        if kind == JavaMapFactoryKind::OfEntries {
            let mut canonical_entries = Vec::with_capacity(args.len().saturating_sub(1));
            for entry in args.iter().skip(1).copied() {
                let kv = self.proven_java_map_entry_pair(entry)?;
                canonical_entries.push(self.mk(ValOp::Seq(SEQ_VALUE_PAIR), kv));
            }
            return Some(self.mk(ValOp::Seq(SEQ_VALUE_MAP), canonical_entries));
        }
        None
    }

    pub(super) fn eval_java_map_factory_expr(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if !semantics(self.il.meta.lang).stdlib().java_map_factories() || kids.is_empty() {
            return None;
        }
        let occurrence = admitted_java_map_factory_at_call(self.il, self.interner, expr)?;
        let LibraryMapFactoryResult::JavaFactory { kind } = occurrence.contract.result else {
            return None;
        };
        match kind {
            JavaMapFactoryKind::Of => {
                let entries = &kids[1..];
                if entries.len() % 2 != 0 {
                    return None;
                }
                let values: Vec<ValueId> = entries.iter().map(|&kid| self.eval(kid, env)).collect();
                let mut canonical_entries = Vec::with_capacity(values.len() / 2);
                for kv in values.chunks(2) {
                    canonical_entries.push(self.mk(ValOp::Seq(SEQ_VALUE_PAIR), kv.to_vec()));
                }
                Some(self.mk(ValOp::Seq(SEQ_VALUE_MAP), canonical_entries))
            }
            JavaMapFactoryKind::OfEntries => {
                let mut canonical_entries = Vec::with_capacity(kids.len().saturating_sub(1));
                for &entry in &kids[1..] {
                    let kv = self.eval_java_map_entry_pair_expr(entry, env)?;
                    canonical_entries.push(self.mk(ValOp::Seq(SEQ_VALUE_PAIR), kv));
                }
                Some(self.mk(ValOp::Seq(SEQ_VALUE_MAP), canonical_entries))
            }
        }
    }

    pub(super) fn eval_java_map_entry_pair_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<Vec<ValueId>> {
        let kids = self.il.children(expr);
        if kids.len() != 3 {
            return None;
        }
        let occurrence = admitted_java_map_entry_at_call(self.il, self.interner, expr)?;
        (occurrence.arg_count == 2).then_some(())?;
        Some(vec![self.eval(kids[1], env), self.eval(kids[2], env)])
    }

    pub(super) fn proven_java_map_entry_pair(&self, value: ValueId) -> Option<Vec<ValueId>> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 3 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        let contract = library_java_map_entry_contract_by_hash(self.il.meta.lang, "Map", method)?;
        let LibraryApiCalleeContract::JavaUtilStaticMember { .. } = contract.callee else {
            return None;
        };
        if callee.args.len() != 1 {
            return None;
        }
        match self.library_api_evidence_for_value_call(
            value,
            args[0],
            Some(callee.args[0]),
            contract.id,
            contract.callee,
            2,
        ) {
            LibraryApiEvidenceStatus::Admitted => {}
            LibraryApiEvidenceStatus::Rejected => return None,
            LibraryApiEvidenceStatus::Missing => return None,
        }
        Some(args[1..].to_vec())
    }

    pub(super) fn library_api_evidence_for_value_call(
        &self,
        value: ValueId,
        callee: ValueId,
        receiver: Option<ValueId>,
        id: nose_semantics::LibraryApiContractId,
        callee_contract: LibraryApiCalleeContract,
        arg_count: usize,
    ) -> LibraryApiEvidenceStatus {
        library_api_contract_evidence_at_call_span(
            self.il,
            self.interner,
            LibraryApiSpanEvidenceQuery {
                call_span: self.node_span[value as usize],
                callee_span: self.library_api_value_span(callee),
                receiver_span: self.library_api_receiver_query_span(value, callee, receiver),
                id,
                callee: callee_contract,
                arg_count,
            },
        )
    }

    pub(super) fn library_api_evidence_admitted(
        &self,
        call: NodeId,
        id: nose_semantics::LibraryApiContractId,
        callee: LibraryApiCalleeContract,
        arg_count: usize,
    ) -> bool {
        matches!(
            library_api_contract_evidence_for_call(
                self.il,
                self.interner,
                call,
                id,
                callee,
                arg_count,
            ),
            LibraryApiEvidenceStatus::Admitted
        )
    }

    pub(super) fn library_api_value_span(&self, value: ValueId) -> Option<Span> {
        match self.nodes[value as usize].op {
            ValOp::ImportBinding { .. } | ValOp::ImportNamespace { .. } => None,
            _ => self.node_span[value as usize],
        }
    }

    pub(super) fn library_api_receiver_query_span(
        &self,
        value: ValueId,
        callee: ValueId,
        receiver: Option<ValueId>,
    ) -> Option<Span> {
        let receiver_span = receiver.and_then(|receiver| self.library_api_value_span(receiver))?;
        let Some(call_span) = self.node_span[value as usize] else {
            return Some(receiver_span);
        };
        let Some(callee_span) = self.library_api_value_span(callee) else {
            return Some(receiver_span);
        };
        if self
            .source_call_receiver_span(call_span, callee_span)
            .is_some_and(|source_receiver_span| source_receiver_span != receiver_span)
        {
            None
        } else {
            Some(receiver_span)
        }
    }

    pub(super) fn source_call_receiver_span(
        &self,
        call_span: Span,
        callee_span: Span,
    ) -> Option<Span> {
        self.il.nodes.iter().enumerate().find_map(|(idx, node)| {
            if node.kind != NodeKind::Call || node.span != call_span {
                return None;
            }
            let call = NodeId(idx as u32);
            let callee = self.il.children(call).first().copied()?;
            if self.il.node(callee).span != callee_span {
                return None;
            }
            self.il
                .children(callee)
                .first()
                .map(|&receiver| self.il.node(receiver).span)
        })
    }

    pub(super) fn eval_js_like_constructed_collection_or_map(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if !construct_syntax_proof(self.il, expr) || kids.len() != 2 {
            return None;
        }
        if let Some(occurrence) =
            admitted_js_like_set_constructor_at_call(self.il, self.interner, expr)
        {
            if occurrence.arg_count != 1 {
                return None;
            }
            if !self.is_static_non_float_collection_expr(kids[1]) {
                return None;
            }
            return Some(self.eval_membership_collection(kids[1], env));
        }
        let occurrence = admitted_js_like_map_constructor_at_call(self.il, self.interner, expr)?;
        if occurrence.arg_count != 1 {
            return None;
        }
        let LibraryMapFactoryResult::EntrySequence { entry_seq_tag } = occurrence.contract.result
        else {
            return None;
        };
        let entries = self.eval(kids[1], env);
        self.map_factory_from_seq(entries, entry_seq_tag)
    }

    pub(super) fn proven_go_literal_zero_map_value(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, ValueId)> {
        let contract = go_zero_map_lookup_contract(self.il.meta.lang)?;
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Seq(tag) if tag == stable_symbol_hash(contract.canonical_value_tag))
            || node.args.len() != 2
        {
            return None;
        }
        Some((node.args[1], node.args[0]))
    }

    pub(super) fn proven_go_literal_zero_map_seq(
        &mut self,
        expr: NodeId,
        args: &[ValueId],
    ) -> Option<ValueId> {
        let contract = go_zero_map_literal_contract_for_node(self.il, self.interner, expr)?;
        if args.is_empty() {
            return None;
        }
        let entry_nodes = self.il.children(expr).to_vec();
        if entry_nodes.len() != args.len() {
            return None;
        }
        let mut canonical_entries = Vec::with_capacity(args.len());
        let mut value_kind = None;
        let mut default = None;
        for (&entry_node_id, &entry_value) in entry_nodes.iter().zip(args.iter()) {
            go_zero_map_entry_contract_for_node(self.il, self.interner, entry_node_id)?;
            let kv_nodes = self.il.children(entry_node_id);
            if kv_nodes.len() != 2
                || !matches!(self.il.node(kv_nodes[0]).payload, Payload::LitStr(_))
            {
                return None;
            }
            let kind =
                go_zero_map_default_kind(self.il.meta.lang, self.il.node(kv_nodes[1]).payload)?;
            let value_default = self.go_literal_zero_default_value(kind);
            match value_kind {
                Some(current_kind) if current_kind != kind => return None,
                Some(_) => {}
                None => {
                    value_kind = Some(kind);
                    default = Some(value_default);
                }
            }
            let entry_value_node = &self.nodes[entry_value as usize];
            if !matches!(entry_value_node.op, ValOp::Seq(tag) if tag == stable_symbol_hash(contract.entry_tag))
                || entry_value_node.args.len() != 2
            {
                return None;
            }
            canonical_entries
                .push(self.mk(ValOp::Seq(SEQ_VALUE_PAIR), entry_value_node.args.clone()));
        }
        let map = self.mk(ValOp::Seq(SEQ_VALUE_MAP), canonical_entries);
        Some(self.mk(
            ValOp::Seq(stable_symbol_hash(contract.canonical_value_tag)),
            vec![default?, map],
        ))
    }

    pub(super) fn go_literal_zero_default_value(&mut self, kind: GoZeroMapDefaultKind) -> ValueId {
        match kind {
            GoZeroMapDefaultKind::Int => self.int_const(0),
            GoZeroMapDefaultKind::String => {
                self.mk(ValOp::Const(stable_string_const_key("")), vec![])
            }
            GoZeroMapDefaultKind::Bool => self.mk(ValOp::Const(0x3000_0001), vec![]),
            GoZeroMapDefaultKind::Float => {
                self.mk(ValOp::Const(stable_float_const_key("0.0")), vec![])
            }
            GoZeroMapDefaultKind::Null => self.null_const(),
        }
    }

    pub(super) fn proven_map_value(&mut self, value: ValueId) -> Option<ValueId> {
        if matches!(self.nodes[value as usize].op, ValOp::Seq(SEQ_VALUE_MAP)) {
            return Some(value);
        }
        self.proven_free_name_map_factory(value)
            .or_else(|| self.proven_java_map_factory_entries(value))
            .or_else(|| {
                self.proven_go_literal_zero_map_value(value)
                    .map(|(map, _)| map)
            })
    }

    pub(super) fn proven_map_get_value(&mut self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let args = node.args.clone();
        let callee = &self.nodes[node.args[0] as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        let contract = library_map_get_contract_by_hash(
            self.il.meta.lang,
            method,
            args.len().saturating_sub(1),
        )?;
        if callee.args.len() != 1 {
            return None;
        }
        match self.library_api_evidence_for_value_call(
            value,
            args[0],
            Some(callee.args[0]),
            contract.id,
            contract.callee,
            args.len().saturating_sub(1),
        ) {
            LibraryApiEvidenceStatus::Admitted => {}
            LibraryApiEvidenceStatus::Rejected => return None,
            LibraryApiEvidenceStatus::Missing => return None,
        }
        let map = callee.args[0];
        let map = if self.is_param_value(map, DomainEvidence::Map) {
            map
        } else {
            self.proven_map_value(map)?
        };
        Some((map, args[1]))
    }

    pub(super) fn proven_map_key_view_value(&mut self, value: ValueId) -> Option<ValueId> {
        self.proven_map_key_view_value_matching(value, MapKeyViewKind::Collection)
    }

    pub(super) fn proven_map_key_view_value_matching(
        &mut self,
        value: ValueId,
        accepted: MapKeyViewKind,
    ) -> Option<ValueId> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) {
            return None;
        }
        let args = node.args.clone();
        if args.len() == 1 {
            let callee = &self.nodes[args[0] as usize];
            let ValOp::Field(method) = callee.op else {
                return None;
            };
            let contract = library_map_key_view_contract_by_hash(self.il.meta.lang, method, 0)?;
            if contract.result.kind != accepted || callee.args.len() != 1 {
                return None;
            }
            match self.library_api_evidence_for_value_call(
                value,
                args[0],
                Some(callee.args[0]),
                contract.id,
                contract.callee,
                0,
            ) {
                LibraryApiEvidenceStatus::Admitted => {}
                LibraryApiEvidenceStatus::Rejected => return None,
                LibraryApiEvidenceStatus::Missing => return None,
            }
            let map = callee.args[0];
            return if self.is_param_value(map, DomainEvidence::Map) {
                Some(map)
            } else {
                self.proven_map_value(map)
            };
        }
        if args.len() == 2 {
            let callee = &self.nodes[args[0] as usize];
            let ValOp::Field(method) = callee.op else {
                return None;
            };
            let contract = library_map_key_view_wrapper_contract_by_hash(
                self.il.meta.lang,
                "Array",
                method,
                1,
            )?;
            if accepted != MapKeyViewKind::Collection || callee.args.len() != 1 {
                return None;
            }
            let receiver_span = callee
                .args
                .first()
                .and_then(|&receiver| self.node_span[receiver as usize]);
            match library_api_contract_evidence_at_call_span(
                self.il,
                self.interner,
                LibraryApiSpanEvidenceQuery {
                    call_span: self.node_span[value as usize],
                    callee_span: self.node_span[args[0] as usize],
                    receiver_span,
                    id: contract.id,
                    callee: contract.callee,
                    arg_count: 1,
                },
            ) {
                LibraryApiEvidenceStatus::Admitted => {}
                LibraryApiEvidenceStatus::Rejected => return None,
                LibraryApiEvidenceStatus::Missing => return None,
            }
            return self.proven_map_key_view_value_matching(args[1], MapKeyViewKind::Iterator);
        }
        None
    }

    pub(super) fn proven_map_key_view_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        self.proven_map_key_view_value(value)
    }

    pub(super) fn eval_proven_collection_membership_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let &callee = kids.first()?;
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let callee_kids = self.il.children(callee);
        let receiver = callee_kids.first().copied();

        let contract =
            library_method_call_contract(self.il.meta.lang, method, kids.len().saturating_sub(1))?;
        if !self.library_api_evidence_admitted(
            expr,
            contract.id,
            contract.callee,
            kids.len().saturating_sub(1),
        ) {
            return None;
        }
        let result = contract.result;
        if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains) {
            return None;
        }

        if result.args == MethodBuiltinArgs::FirstThenReceiver
            && matches!(
                result.receiver,
                MethodReceiverContract::ExactCollection
                    | MethodReceiverContract::ExactCollectionOrMap
                    | MethodReceiverContract::ExactCollectionOrJavaKeySet
                    | MethodReceiverContract::ExactSetOrMap
            )
            && kids.len() == 2
        {
            let receiver = receiver?;
            let element = self.eval(kids[1], env);
            if matches!(
                result.receiver,
                MethodReceiverContract::ExactCollectionOrMap
                    | MethodReceiverContract::ExactCollectionOrJavaKeySet
            ) {
                if let Some(map) = self.proven_map_key_view_expr(receiver, env) {
                    return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, map]));
                }
            }
            let receiver_value = self.eval(receiver, env);
            if let Some(collection) = self
                .proven_collection_value(receiver_value)
                .or_else(|| self.proven_local_collection_binding_value(receiver, env))
            {
                let collection = self.canonical_membership_collection_value(collection);
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            let receiver_param_safe = match result.receiver {
                MethodReceiverContract::ExactSetOrMap => self.is_set_param_expr(receiver),
                _ => self.is_collection_param_expr(receiver),
            };
            if receiver_param_safe {
                let collection = self.eval_membership_collection(receiver, env);
                return Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]));
            }
            None
        } else if result.args == MethodBuiltinArgs::GoSliceContains && kids.len() == 3 {
            let receiver = receiver?;
            let MethodReceiverContract::ImportedNamespace(module) = result.receiver else {
                return None;
            };
            if !self.is_import_namespace_expr(receiver, module, env)
                && !self.file_imports_namespace(receiver, module)
            {
                return None;
            }
            let element = self.eval(kids[2], env);
            let collection = if self.is_collection_param_expr(kids[1]) {
                self.eval_membership_collection(kids[1], env)
            } else {
                self.proven_collection_expr(kids[1], env)?
            };
            let collection = self.canonical_membership_collection_value(collection);
            Some(self.mk(ValOp::Bin(Op::In as u32), vec![element, collection]))
        } else {
            None
        }
    }

    pub(super) fn eval_proven_map_key_membership_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 2 {
            return None;
        }
        let callee = kids[0];
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        let contract = library_method_call_contract(self.il.meta.lang, method, 1)?;
        if !self.library_api_evidence_admitted(expr, contract.id, contract.callee, 1) {
            return None;
        }
        let result = contract.result;
        if result.semantic != MethodSemanticContract::Builtin(Builtin::Contains)
            || result.args != MethodBuiltinArgs::FirstThenReceiver
            || !matches!(
                result.receiver,
                MethodReceiverContract::ExactMap
                    | MethodReceiverContract::ExactCollectionOrMap
                    | MethodReceiverContract::ExactSetOrMap
            )
        {
            return None;
        }
        let receiver = self.il.children(callee).first().copied()?;
        let key = self.eval(kids[1], env);
        let receiver_value = self.eval(receiver, env);
        let map = if self.is_map_param_expr(receiver) {
            receiver_value
        } else {
            self.proven_map_value(receiver_value)
                .or_else(|| self.proven_local_map_binding_value(receiver, env))?
        };
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![key, map]))
    }

    pub(super) fn eval_proven_map_get_default_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 3 {
            return None;
        }
        let callee = kids[0];
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let contract = library_method_call_contract(
            self.il.meta.lang,
            self.interner.resolve(name),
            kids.len().saturating_sub(1),
        )?;
        if !self.library_api_evidence_admitted(
            expr,
            contract.id,
            contract.callee,
            kids.len().saturating_sub(1),
        ) {
            return None;
        }
        let result = contract.result;
        if result.semantic != MethodSemanticContract::Builtin(Builtin::GetOrDefault)
            || result.receiver != MethodReceiverContract::ExactMap
            || !matches!(
                result.args,
                MethodBuiltinArgs::MapGetDefault | MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda
            )
        {
            return None;
        }
        let receiver = self.il.children(callee).first().copied()?;
        let receiver_value = self.eval(receiver, env);
        let map = if self.is_map_param_expr(receiver) {
            receiver_value
        } else {
            self.proven_map_value(receiver_value)
                .or_else(|| self.proven_local_map_binding_value(receiver, env))?
        };
        let key = self.eval(kids[1], env);
        let default = self.eval_map_get_default_arg(result.args, kids[2], env)?;
        Some(self.mk(
            ValOp::Call(builtin_tag(Builtin::GetOrDefault)),
            vec![map, key, default],
        ))
    }

    pub(super) fn eval_map_get_default_arg(
        &mut self,
        contract: MethodBuiltinArgs,
        default: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        match contract {
            MethodBuiltinArgs::MapGetDefault => Some(self.eval(default, env)),
            MethodBuiltinArgs::MapGetDefaultOrZeroArgLambda => {
                if self.il.kind(default) == NodeKind::Lambda {
                    return self.eval_zero_arg_lambda_body(default, env);
                }
                Some(self.eval(default, env))
            }
            _ => None,
        }
    }

    pub(super) fn eval_zero_arg_lambda_body(
        &mut self,
        lambda: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.kind(lambda) != NodeKind::Lambda {
            return None;
        }
        let kids = self.il.children(lambda);
        if kids.len() != 1 {
            return None;
        }
        self.eval_lambda_body(lambda, &[], env)
    }

    pub(super) fn eval_proven_integer_method_call(
        &mut self,
        call: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let &callee = kids.first()?;
        if self.il.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        let arg_count = kids.len().saturating_sub(1);
        let contract =
            library_scalar_integer_method_contract(self.il.meta.lang, method, arg_count)?;
        if !matches!(
            library_api_contract_evidence_for_call(
                self.il,
                self.interner,
                call,
                contract.id,
                contract.callee,
                arg_count,
            ),
            LibraryApiEvidenceStatus::Admitted
        ) {
            return None;
        }
        if contract.result.receiver != MethodReceiverContract::ExactInteger {
            return None;
        }
        let receiver = self.il.children(callee).first().copied()?;
        let receiver_value = self.eval_proven_integer_expr(receiver, env)?;
        match contract.result.semantic {
            ScalarIntegerMethod::Abs => Some(self.mk(ValOp::Un(ABS_CODE), vec![receiver_value])),
            ScalarIntegerMethod::Min => {
                let rhs = self.eval(*kids.get(1)?, env);
                self.eval_proven_integer_minmax_method_call(MIN_CODE, receiver_value, rhs)
            }
            ScalarIntegerMethod::Max => {
                let rhs = self.eval(*kids.get(1)?, env);
                self.eval_proven_integer_minmax_method_call(MAX_CODE, receiver_value, rhs)
            }
            ScalarIntegerMethod::Clamp => {
                let lo = self.eval(*kids.get(1)?, env);
                let hi = self.eval(*kids.get(2)?, env);
                self.eval_proven_integer_clamp_method_call(receiver_value, lo, hi)
            }
        }
    }

    pub(super) fn eval_proven_integer_minmax_method_call(
        &mut self,
        op: u32,
        receiver: ValueId,
        rhs: ValueId,
    ) -> Option<ValueId> {
        if !self.is_integer_domain_value(rhs) {
            return None;
        }
        Some(self.mk(ValOp::Bin(op), vec![receiver, rhs]))
    }

    pub(super) fn eval_proven_free_minmax_call(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if kids.len() != 3 || self.il.kind(kids[0]) != NodeKind::Var {
            return None;
        }
        let Payload::Name(name) = self.il.node(kids[0]).payload else {
            return None;
        };
        let method = self.interner.resolve(name);
        let contract = library_free_function_builtin_contract(self.il.meta.lang, method, 2)?;
        if !self.library_api_evidence_admitted(expr, contract.id, contract.callee, 2) {
            return None;
        }
        let op = match (contract.result.builtin, contract.result.args) {
            (Builtin::Min, BuiltinArgContract::All) => MIN_CODE,
            (Builtin::Max, BuiltinArgContract::All) => MAX_CODE,
            _ => return None,
        };
        let left = self.eval(kids[1], env);
        let right = self.eval(kids[2], env);
        Some(self.mk(ValOp::Bin(op), vec![left, right]))
    }

    pub(super) fn eval_proven_integer_clamp_method_call(
        &mut self,
        receiver: ValueId,
        lo: ValueId,
        hi: ValueId,
    ) -> Option<ValueId> {
        if !self.is_integer_domain_value(lo) || !self.is_integer_domain_value(hi) {
            return None;
        }
        self.proof_backed_clamp_value(receiver, lo, hi)
    }

    pub(super) fn eval_proven_integer_expr(
        &mut self,
        expr: NodeId,
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        let value = self.eval(expr, env);
        if self.il.kind(expr) == NodeKind::Var {
            return self.is_integer_param_expr(expr).then_some(value);
        }
        self.is_integer_domain_value(value).then_some(value)
    }

    pub(super) fn is_integer_domain_value(&self, value: ValueId) -> bool {
        if self.int_const_value(value).is_some()
            || self.is_param_value(value, DomainEvidence::Integer)
        {
            return true;
        }
        let node = &self.nodes[value as usize];
        match node.op {
            ValOp::Un(op) if op == ABS_CODE && node.args.len() == 1 => {
                self.is_integer_domain_value(node.args[0])
            }
            ValOp::Bin(op) if (op == MIN_CODE || op == MAX_CODE) && node.args.len() == 2 => node
                .args
                .iter()
                .copied()
                .all(|arg| self.is_integer_domain_value(arg)),
            ValOp::Clamp if node.args.len() == 3 => node
                .args
                .iter()
                .copied()
                .all(|arg| self.is_integer_domain_value(arg)),
            _ => false,
        }
    }
}
