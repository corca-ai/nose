use super::super::*;

const PYTHON_TUPLE_ITERATOR_MATERIALIZER: u64 =
    stable_symbol_hash("python.iterator.materializer.tuple");
const PYTHON_SET_ITERATOR_MATERIALIZER: u64 =
    stable_symbol_hash("python.iterator.materializer.set");
const PYTHON_FROZENSET_ITERATOR_MATERIALIZER: u64 =
    stable_symbol_hash("python.iterator.materializer.frozenset");

impl<'a> Builder<'a> {
    /// A collection sequence-literal call `Call(0, [callee, Seq(collection)])` → its
    /// `(callee, seq)`. The shared guard under every collection/map factory recognizer; `None`
    /// when `value` is not that shape.
    pub(super) fn collection_call_callee_seq(&self, value: ValueId) -> Option<(ValueId, ValueId)> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.len() != 2 {
            return None;
        }
        let (callee, seq) = (node.args[0], node.args[1]);
        matches!(
            self.nodes[seq as usize].op,
            ValOp::Seq(SEQ_VALUE_COLLECTION)
        )
        .then_some((callee, seq))
    }
    /// Shared skeleton of the collection-factory recognizers: a collection sequence literal
    /// call `Call(0, [callee, Seq(collection)])`
    /// whose `callee` passes `is_factory` wraps the sequence literal `args[1]`; return it. The
    /// per-language recognizers differ ONLY in their callee predicate, so this collapses the
    /// identical skeletons that nose's own duplication gate flagged across them.
    fn collection_factory_seq(
        &self,
        value: ValueId,
        is_factory: impl FnOnce(&Self, ValueId) -> bool,
    ) -> Option<ValueId> {
        let (callee, seq) = self.collection_call_callee_seq(value)?;
        is_factory(self, callee).then_some(seq)
    }
    /// `factory(<seq>)` where `factory` is a free function/path name that constructs a collection
    /// from a single sequence literal. Data-driven by the first-party collection contracts in
    /// `nose-semantics`; each row carries the names and whether a same-named local definition
    /// shadows the builtin.
    fn proven_free_name_collection_factory(&self, value: ValueId) -> Option<ValueId> {
        self.collection_factory_seq(value, |s, callee| {
            admitted_free_name_collection_factory_at_call_span(
                s.il,
                s.interner,
                s.library_api_span_call(value, callee, None, 1),
                |name| s.is_free_name_value(callee, name),
            )
            .is_some()
        })
    }
    /// Python `from collections import deque; deque(<seq>)` — the imported-stdlib collection
    /// factory (the non-free-name part of the former python recognizer).
    fn proven_python_deque_collection_value(&self, value: ValueId) -> Option<ValueId> {
        self.collection_factory_seq(value, |s, callee| {
            let receiver = match s.nodes[callee as usize].op {
                ValOp::Field(_) => s.nodes[callee as usize].args.first().copied(),
                _ => None,
            };
            admitted_imported_collection_factory_at_call_span(
                s.il,
                s.interner,
                s.library_api_span_call(value, callee, receiver, 1),
            )
            .is_some()
        })
    }
    fn proven_java_collection_factory_value(&mut self, value: ValueId) -> Option<ValueId> {
        if !semantics(self.il.meta.lang)
            .stdlib()
            .java_collection_factories()
        {
            return None;
        }
        let call = self.java_static_member_value_call(value)?;
        let admitted = admitted_java_collection_factory_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(
                value,
                call.callee,
                Some(call.receiver),
                call.args.len().saturating_sub(1),
            ),
            call.method,
        )?;
        if matches!(
            admitted.contract.id,
            LibraryApiContractId::JavaCollectionFactory(kind)
                if java_collection_factory_rejects_null_literal(kind)
        ) && call.args[1..]
            .iter()
            .any(|&arg| self.value_is_null_const(arg))
        {
            return None;
        }
        match admitted.contract.result {
            LibraryCollectionFactoryResult::EmptySequence => {
                (call.args.len() == 1).then(|| self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), vec![]))
            }
            LibraryCollectionFactoryResult::ElementArguments => {
                if call.args.len() != 2 {
                    return None;
                }
                Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), call.args[1..].to_vec()))
            }
            LibraryCollectionFactoryResult::VariadicElements {
                single_arg_spreads_array,
            } => {
                // A single argument to a varargs collection factory (`Arrays.asList(x)`,
                // `List.of(x)`, `Set.of(x)`) is ambiguous: when `x` is an array it may
                // be spread into the element list or treated as the sole element,
                // depending on static typing and overload resolution. The two readings
                // have different membership semantics, so only proven array params can
                // use the spread interpretation here.
                if call.args.len() == 2 {
                    if single_arg_spreads_array && self.is_array_param_value(call.args[1]) {
                        return Some(self.mk(ValOp::ArrayParam, vec![call.args[1]]));
                    }
                    return None;
                }
                Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), call.args[1..].to_vec()))
            }
            _ => None,
        }
    }

    pub(in crate::value_graph) fn value_is_null_const(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const {
                kind: ConstKind::Null,
                ..
            }
        )
    }
    pub(in crate::value_graph) fn eval_java_collection_constructor_expr(
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

    pub(in crate::value_graph) fn eval_python_iterator_materializer_expr(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if self.il.meta.lang != Lang::Python || kids.len() != 2 {
            return None;
        }
        let occurrence =
            admitted_free_name_collection_factory_at_call(self.il, self.interner, expr)?;
        if !matches!(
            occurrence.contract.id,
            LibraryApiContractId::PythonBuiltinCollectionFactory
        ) {
            return None;
        }
        let source = kids[1];
        if !self.proven_python_lazy_iterator_source(source) {
            return None;
        }
        let materialized = self.eval(source, env);
        match self.python_iterator_materializer_tag(occurrence.callee) {
            Some(tag) => Some(self.mk(ValOp::Seq(tag), vec![materialized])),
            None => Some(materialized),
        }
    }

    fn python_iterator_materializer_tag(&self, callee: NodeId) -> Option<u64> {
        let Payload::Name(name) = self.il.node(callee).payload else {
            return None;
        };
        match self.interner.resolve(name) {
            "list" => None,
            "tuple" => Some(PYTHON_TUPLE_ITERATOR_MATERIALIZER),
            "set" => Some(PYTHON_SET_ITERATOR_MATERIALIZER),
            "frozenset" => Some(PYTHON_FROZENSET_ITERATOR_MATERIALIZER),
            _ => None,
        }
    }

    fn proven_python_lazy_iterator_source(&self, source: NodeId) -> bool {
        match (self.il.kind(source), self.il.node(source).payload) {
            (NodeKind::HoF, Payload::HoF(kind)) => {
                admitted_hof_demand_effect_profile_at_node_with_interner(
                    self.il,
                    Some(self.interner),
                    source,
                    kind,
                )
                .is_some_and(|profile| profile.callback_effects_delayed_until_pull())
            }
            (NodeKind::Call, Payload::Builtin(Builtin::Zip)) => {
                self.admitted_builtin_call(source, Builtin::Zip)
            }
            (NodeKind::Call, Payload::Builtin(Builtin::Enumerate)) => {
                self.admitted_builtin_call(source, Builtin::Enumerate)
            }
            _ => false,
        }
    }

    pub(in crate::value_graph) fn eval_swift_collection_factory_expr(
        &mut self,
        expr: NodeId,
        kids: &[NodeId],
        env: &FxHashMap<u32, ValueId>,
    ) -> Option<ValueId> {
        if !semantics(self.il.meta.lang)
            .stdlib()
            .swift_collection_factories()
        {
            return None;
        }
        let occurrence = admitted_swift_collection_factory_at_call(self.il, self.interner, expr)?;
        if occurrence.arg_count != 1 {
            return None;
        }
        if !matches!(
            occurrence.contract.id,
            LibraryApiContractId::SwiftCollectionFactory(SwiftCollectionFactoryKind::Set)
        ) {
            return None;
        }
        let [_, collection] = kids else {
            return None;
        };
        Some(self.eval_membership_collection(*collection, env))
    }

    fn proven_ruby_set_factory_value(&self, value: ValueId) -> Option<ValueId> {
        self.collection_factory_seq(value, |s, callee_value| {
            let callee = &s.nodes[callee_value as usize];
            let ValOp::Field(method) = callee.op else {
                return false;
            };
            if callee.args.len() != 1 {
                return false;
            }
            let receiver_value = callee.args[0];
            let Some(admitted) = admitted_ruby_set_factory_at_call_span(
                s.il,
                s.interner,
                s.library_api_span_call(value, callee_value, Some(receiver_value), 1),
                method,
            ) else {
                return false;
            };
            let LibraryApiCalleeContract::RubyRequireStaticMember { receiver, .. } =
                admitted.contract.callee
            else {
                return false;
            };
            s.is_free_name_value(receiver_value, receiver)
        })
    }
    fn proven_rust_vec_macro_collection_value(&mut self, value: ValueId) -> Option<ValueId> {
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
        let admitted = admitted_rust_vec_macro_factory_at_call_span(
            self.il,
            self.interner,
            self.library_api_span_call(value, args[0], None, args.len().saturating_sub(1)),
        )?;
        let LibraryApiCalleeContract::RustMacro { name, .. } = admitted.contract.callee else {
            return None;
        };
        if !self.is_free_name_value(args[0], name) {
            return None;
        }
        Some(self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), args[1..].to_vec()))
    }
    pub(in crate::value_graph) fn proven_collection_value(
        &mut self,
        value: ValueId,
    ) -> Option<ValueId> {
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
}
