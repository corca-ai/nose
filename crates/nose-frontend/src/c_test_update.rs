#[cfg(test)]
mod tests {
    use crate::c;
    use nose_il::intern::Interner;
    use nose_il::node::*;

    fn binop_ops(src: &str) -> Vec<Op> {
        let interner = Interner::new();
        let il = c::lower(
            nose_il::file::FileId(0),
            "t.c",
            src.as_bytes(),
            &interner,
        )
        .expect("lower");
        il.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::BinOp)
            .filter_map(|n| match n.payload {
                Payload::Op(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn update_expression_increment() {
        let ops = binop_ops("int f() { int i = 0; i++; return 0; }");
        assert!(ops.contains(&Op::Add), "i++ should have BinOp with Op::Add, got {:?}", ops);
    }

    #[test]
    fn update_expression_decrement() {
        let ops = binop_ops("int f() { int i = 0; i--; return 0; }");
        assert!(ops.contains(&Op::Sub), "i-- should have BinOp with Op::Sub, got {:?}", ops);
    }

    #[test]
    fn update_expression_nested_decrement() {
        // This is the key test case that reveals the bug
        let ops = binop_ops("int f() { int arr[10]; int i=0; arr[i--]++; return 0; }");
        
        // There should be two BinOps:
        // 1. The inner i-- gives Op::Sub
        // 2. The outer arr[i--]++ should give Op::Add (CURRENTLY FAILS)
        
        // Count the ops
        let sub_count = ops.iter().filter(|op| **op == Op::Sub).count();
        let add_count = ops.iter().filter(|op| **op == Op::Add).count();
        
        println!("ops: {:?}", ops);
        println!("sub_count: {}, add_count: {}", sub_count, add_count);
        
        // If the bug exists, both will be Sub
        // If fixed, we should have 1 Sub (inner i--) and 1 Add (outer ++)
        assert_eq!(sub_count, 1, "Should have exactly 1 Sub (from i--), got {:?}", ops);
        assert_eq!(add_count, 1, "Should have exactly 1 Add (from ++), got {:?}", ops);
    }
}
