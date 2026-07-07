use super::super::*;

impl<'a> Builder<'a> {
    pub(in crate::value_graph) fn int_const_eq(&self, value: ValueId, expected: i64) -> bool {
        self.int_const_value(value) == Some(expected)
    }
    pub(in crate::value_graph) fn int_const_value(&self, value: ValueId) -> Option<i64> {
        match self.nodes[value as usize].op {
            ValOp::Const {
                kind: ConstKind::Int,
                bits,
            } => Some(bits as i64),
            _ => None,
        }
    }
    /// An integer-literal value, keyed identically to `eval`'s `LitInt` path so a
    /// builtin's implicit init (`sum` → 0) matches a loop's explicit `acc = 0`. `v` is a
    /// synthesized non-negative count; the FULL i64 is retained so it can never collide
    /// with a literal differing by a multiple of 2^32 (coevo series 8).
    pub(in crate::value_graph) fn int_const(&mut self, v: u32) -> ValueId {
        self.mk_const(ConstKind::Int, v as u64)
    }
    pub(in crate::value_graph) fn null_const(&mut self) -> ValueId {
        self.mk_const(ConstKind::Null, 0)
    }
    pub(in crate::value_graph) fn bool_const_value(&mut self, b: bool) -> ValueId {
        self.mk_const(ConstKind::Bool, b as u64)
    }
    /// Construct a `Const` node of the given kind and full-width payload.
    pub(in crate::value_graph) fn mk_const(&mut self, kind: ConstKind, bits: u64) -> ValueId {
        self.mk(ValOp::Const { kind, bits }, vec![])
    }
    pub(in crate::value_graph) fn sentinel_const(&mut self, tag: u64) -> ValueId {
        self.mk_const(ConstKind::Sentinel, tag)
    }
    pub(in crate::value_graph) fn bool_const(&self, id: ValueId) -> Option<bool> {
        match self.nodes[id as usize].op {
            ValOp::Const {
                kind: ConstKind::Bool,
                bits,
            } => Some(bits != 0),
            _ => None,
        }
    }
    pub(super) fn literal_equality_disjunction(
        &mut self,
        left: ValueId,
        right: ValueId,
    ) -> Option<ValueId> {
        let mut element = None;
        let mut items = Vec::new();
        self.collect_literal_membership_terms(left, &mut element, &mut items)?;
        self.collect_literal_membership_terms(right, &mut element, &mut items)?;
        if items.len() < 2 {
            return None;
        }
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        let collection = self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), items);
        Some(self.mk(ValOp::Bin(Op::In as u32), vec![element?, collection]))
    }
    pub(super) fn literal_inequality_conjunction(
        &mut self,
        left: ValueId,
        right: ValueId,
    ) -> Option<ValueId> {
        let mut element = None;
        let mut items = Vec::new();
        self.collect_literal_absence_terms(left, &mut element, &mut items)?;
        self.collect_literal_absence_terms(right, &mut element, &mut items)?;
        if items.len() < 2 {
            return None;
        }
        items.sort_by_key(|&v| (self.vhash[v as usize], v));
        items.dedup();
        let collection = self.mk(ValOp::Seq(SEQ_VALUE_COLLECTION), items);
        let membership = self.mk(ValOp::Bin(Op::In as u32), vec![element?, collection]);
        Some(self.mk(ValOp::Un(Op::Not as u32), vec![membership]))
    }
    fn collect_literal_membership_terms(
        &self,
        value: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        let node = &self.nodes[value as usize];
        match node.op {
            ValOp::Bin(op) if op == Op::Or as u32 && node.args.len() == 2 => {
                self.collect_literal_membership_terms(node.args[0], element, items)?;
                self.collect_literal_membership_terms(node.args[1], element, items)
            }
            ValOp::Bin(op) if op == Op::Eq as u32 && node.args.len() == 2 => {
                let a = node.args[0];
                let b = node.args[1];
                let (candidate, literal) = if self.static_membership_literal_value(a) {
                    (b, a)
                } else if self.static_membership_literal_value(b) {
                    (a, b)
                } else {
                    return None;
                };
                self.record_literal_membership_term(candidate, literal, element, items)
            }
            ValOp::Bin(op) if op == Op::In as u32 && node.args.len() == 2 => {
                let candidate = node.args[0];
                let collection = &self.nodes[node.args[1] as usize];
                if !matches!(collection.op, ValOp::Seq(SEQ_VALUE_COLLECTION))
                    || !collection
                        .args
                        .iter()
                        .all(|&item| self.static_membership_literal_value(item))
                {
                    return None;
                }
                match *element {
                    Some(current) if current != candidate => None,
                    Some(_) => {
                        items.extend(collection.args.iter().copied());
                        Some(())
                    }
                    None => {
                        *element = Some(candidate);
                        items.extend(collection.args.iter().copied());
                        Some(())
                    }
                }
            }
            _ => None,
        }
    }
    fn collect_literal_absence_terms(
        &self,
        value: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        let node = &self.nodes[value as usize];
        match node.op {
            ValOp::Bin(op) if op == Op::And as u32 && node.args.len() == 2 => {
                self.collect_literal_absence_terms(node.args[0], element, items)?;
                self.collect_literal_absence_terms(node.args[1], element, items)
            }
            ValOp::Bin(op) if op == Op::Ne as u32 && node.args.len() == 2 => {
                let a = node.args[0];
                let b = node.args[1];
                let (candidate, literal) = if self.static_membership_literal_value(a) {
                    (b, a)
                } else if self.static_membership_literal_value(b) {
                    (a, b)
                } else {
                    return None;
                };
                self.record_literal_membership_term(candidate, literal, element, items)
            }
            _ => None,
        }
    }
    fn record_literal_membership_term(
        &self,
        candidate: ValueId,
        literal: ValueId,
        element: &mut Option<ValueId>,
        items: &mut Vec<ValueId>,
    ) -> Option<()> {
        match *element {
            Some(current) if current != candidate => None,
            Some(_) => {
                items.push(literal);
                Some(())
            }
            None => {
                *element = Some(candidate);
                items.push(literal);
                Some(())
            }
        }
    }
    fn static_membership_literal_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const { kind, bits } if !(matches!(kind, ConstKind::Sentinel) && bits == sentinel::BOTTOM)
        )
    }
    pub(in crate::value_graph) fn empty_string_value(&mut self) -> ValueId {
        self.mk_const(ConstKind::Str, stable_string_const_bits(""))
    }
    pub(in crate::value_graph) fn is_empty_string_value(&self, value: ValueId) -> bool {
        matches!(
            self.nodes[value as usize].op,
            ValOp::Const { kind: ConstKind::Str, bits } if bits == stable_string_const_bits("")
        )
    }
}
