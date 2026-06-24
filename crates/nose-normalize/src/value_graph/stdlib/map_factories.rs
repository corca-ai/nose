use super::super::*;

impl<'a> Builder<'a> {
    /// `factory([<entry>, …])` where `factory` is a free name that builds a map from a sequence of
    /// 2-element key/value entries. Data-driven by first-party map contracts in `nose-semantics`;
    /// the matched row's `Seq` tag says how each entry is shaped (JS array vs Rust tuple).
    fn proven_free_name_map_factory(&mut self, value: ValueId) -> Option<ValueId> {
        let (callee, seq) = self.collection_call_callee_seq(value)?;
        let admitted = admitted_free_name_map_factory_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(value, callee, None, 1),
            |name| self.is_free_name_value(callee, name),
        )?;
        let LibraryMapFactoryResult::EntrySequence { entry_seq_tag } = admitted.contract.result
        else {
            return None;
        };
        self.map_factory_from_seq(seq, entry_seq_tag)
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
    fn proven_java_map_factory_entries(&mut self, value: ValueId) -> Option<ValueId> {
        if !semantics(self.il.meta.lang).stdlib().java_map_factories() {
            return None;
        }
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.is_empty() {
            return None;
        }
        let args = node.args.clone();
        let callee_value = args[0];
        let callee = &self.nodes[callee_value as usize];
        let ValOp::Field(method) = callee.op else {
            return None;
        };
        if callee.args.len() != 1 {
            return None;
        }
        let admitted = admitted_java_map_factory_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(
                value,
                callee_value,
                Some(callee.args[0]),
                args.len().saturating_sub(1),
            ),
            method,
        )?;
        let LibraryMapFactoryResult::JavaFactory { kind } = admitted.contract.result else {
            return None;
        };
        if matches!(
            kind,
            JavaMapFactoryKind::Of | JavaMapFactoryKind::GuavaImmutableMapOf
        ) {
            let entries = &args[1..];
            if !java_map_factory_positional_arg_count_supported(kind, entries.len())
                || !self.java_map_positional_entries_value_safe(kind, entries)
            {
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
    pub(in crate::value_graph) fn eval_java_map_factory_expr(
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
            JavaMapFactoryKind::Of | JavaMapFactoryKind::GuavaImmutableMapOf => {
                let entries = &kids[1..];
                if !java_map_factory_positional_arg_count_supported(kind, entries.len())
                    || !self.java_map_positional_entries_expr_safe(kind, entries)
                {
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
    fn java_map_positional_entries_value_safe(
        &self,
        kind: JavaMapFactoryKind,
        entries: &[ValueId],
    ) -> bool {
        if kind != JavaMapFactoryKind::GuavaImmutableMapOf {
            return true;
        }
        if entries.iter().any(|&arg| self.value_is_null_const(arg)) {
            return false;
        }
        let mut seen = FxHashSet::default();
        for key in entries.iter().step_by(2).copied() {
            let Some(key) = self.value_static_literal_key(key) else {
                continue;
            };
            if !seen.insert(key) {
                return false;
            }
        }
        true
    }
    fn java_map_positional_entries_expr_safe(
        &self,
        kind: JavaMapFactoryKind,
        entries: &[NodeId],
    ) -> bool {
        if kind != JavaMapFactoryKind::GuavaImmutableMapOf {
            return true;
        }
        !nodes_contain_static_null_literal(self.il, entries.iter().copied())
            && !nodes_contain_duplicate_static_literal_keys(
                self.il,
                entries.iter().step_by(2).copied(),
            )
    }
    fn value_static_literal_key(&self, value: ValueId) -> Option<(ConstKind, u64)> {
        match self.nodes[value as usize].op {
            ValOp::Const { kind, bits } if kind != ConstKind::Sentinel => Some((kind, bits)),
            _ => None,
        }
    }
    fn eval_java_map_entry_pair_expr(
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
    fn proven_java_map_entry_pair(&self, value: ValueId) -> Option<Vec<ValueId>> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 3 {
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
        admitted_java_map_entry_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(value, args[0], Some(callee.args[0]), 2),
            method,
        )?;
        Some(args[1..].to_vec())
    }
    pub(in crate::value_graph) fn eval_js_like_constructed_collection_or_map(
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
    pub(in crate::value_graph) fn proven_go_literal_zero_map_value(
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
    pub(in crate::value_graph) fn proven_go_literal_zero_map_seq(
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
    fn go_literal_zero_default_value(&mut self, kind: GoZeroMapDefaultKind) -> ValueId {
        match kind {
            GoZeroMapDefaultKind::Int => self.int_const(0),
            GoZeroMapDefaultKind::String => {
                self.mk_const(ConstKind::Str, stable_string_const_bits(""))
            }
            GoZeroMapDefaultKind::Bool => self.bool_const_value(false),
            GoZeroMapDefaultKind::Float => {
                self.mk_const(ConstKind::Float, stable_float_const_bits("0.0"))
            }
            GoZeroMapDefaultKind::Null => self.null_const(),
        }
    }
    pub(in crate::value_graph) fn proven_map_value(&mut self, value: ValueId) -> Option<ValueId> {
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
}
