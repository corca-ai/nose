use super::super::*;

pub(super) struct JavaStaticMemberValueCall {
    pub(super) args: Vec<ValueId>,
    pub(super) callee: ValueId,
    pub(super) receiver: ValueId,
    pub(super) method: u64,
}

impl<'a> Builder<'a> {
    pub(super) fn java_static_member_value_call(
        &self,
        value: ValueId,
    ) -> Option<JavaStaticMemberValueCall> {
        let node = &self.nodes[value as usize];
        if !matches!(node.op, ValOp::Call(0)) || node.args.is_empty() {
            return None;
        }
        let args = node.args.clone();
        let callee = args[0];
        let callee_node = &self.nodes[callee as usize];
        let ValOp::Field(method) = callee_node.op else {
            return None;
        };
        if callee_node.args.len() != 1 {
            return None;
        }
        Some(JavaStaticMemberValueCall {
            args,
            callee,
            receiver: callee_node.args[0],
            method,
        })
    }
}
