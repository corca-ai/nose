//! Compact s-expression rendering of an IL subtree, for eyeballing the output of
//! `nose il`. Not parsed back; purely for human inspection.

use crate::intern::Interner;
use crate::node::{NodeId, NodeKind, Payload};
use crate::Il;

pub(crate) fn to_sexpr(il: &Il, root: NodeId, interner: &Interner) -> String {
    let mut out = String::new();
    write_node(il, root, interner, 0, &mut out);
    out
}

fn write_node(il: &Il, id: NodeId, interner: &Interner, depth: usize, out: &mut String) {
    let node = il.node(id);
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push('(');
    out.push_str(kind_name(node.kind));
    if let Some(p) = payload_str(node.payload, interner) {
        out.push(' ');
        out.push_str(&p);
    }
    let children = il.children(id);
    if children.is_empty() {
        out.push(')');
    } else {
        out.push('\n');
        for (i, &c) in children.iter().enumerate() {
            write_node(il, c, interner, depth + 1, out);
            if i + 1 < children.len() {
                out.push('\n');
            }
        }
        out.push(')');
    }
}

fn payload_str(p: Payload, interner: &Interner) -> Option<String> {
    Some(match p {
        Payload::None => return None,
        Payload::Op(op) => format!("{op:?}"),
        Payload::Lit(c) => format!("{c:?}"),
        Payload::LitInt(v) => format!("int={v}"),
        Payload::LitBool(b) => format!("bool={b}"),
        Payload::LitStr(h) => format!("str#{h:x}"),
        Payload::LitFloat(h) => format!("float#{h:x}"),
        Payload::Name(s) => format!("\"{}\"", interner.resolve(s)),
        Payload::Cid(c) => format!("v{c}"),
        Payload::Builtin(b) => format!("@{b:?}"),
        Payload::HoF(k) => format!("{k:?}"),
        Payload::Loop(k) => format!("{k:?}"),
    })
}

fn kind_name(k: NodeKind) -> &'static str {
    use NodeKind::*;
    match k {
        Module => "module",
        Func => "func",
        Param => "param",
        Block => "block",
        Assign => "assign",
        ExprStmt => "expr-stmt",
        Return => "return",
        If => "if",
        Loop => "loop",
        Break => "break",
        Continue => "continue",
        Throw => "throw",
        Try => "try",
        Var => "var",
        Lit => "lit",
        Call => "call",
        BinOp => "binop",
        UnOp => "unop",
        Index => "index",
        Field => "field",
        Lambda => "lambda",
        Seq => "seq",
        HoF => "hof",
        KwArg => "kwarg",
        Splat => "splat",
        Raw => "raw",
    }
}
