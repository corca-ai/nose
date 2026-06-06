//! Cross-language idiom canonicalization: collapse equivalent builtins from
//! different languages to one [`Builtin`] op so that, e.g., Python `len(xs)`,
//! JS `xs.length`, and Go `len(xs)` all converge. Detection on `xs.length` (a
//! field access) is handled by the caller; this module only inspects `Call`s.
//!
//! proof-obligation: normalize.value_graph.bool_reduce
//! proof-obligation: normalize.value_graph.functor
//! proof-obligation: normalize.value_graph.min_max

use nose_il::{stable_symbol_hash, Builtin, HoFKind, Il, Interner, NodeId, NodeKind, Payload};

/// The result of inspecting a `Call`: it canonicalizes to a builtin, to a
/// higher-order op (`HoF`), or stays an ordinary call. The carried `NodeId`s are
/// *old* ids to be rebuilt as the new node's children.
pub(crate) enum CallCanon {
    Builtin {
        op: Builtin,
        arg_olds: Vec<NodeId>,
    },
    /// `xs.map(f)` / `.filter(f)` / `.reduce(f)` → `HoF[xs, f]`, converging with
    /// Python comprehensions.
    HoF {
        kind: HoFKind,
        collection_old: NodeId,
        fn_old: NodeId,
    },
    None,
}

pub(crate) fn canon_call(old: &Il, interner: &Interner, call_id: NodeId) -> CallCanon {
    let kids = old.children(call_id);
    if kids.is_empty() {
        return CallCanon::None;
    }
    let callee = kids[0];
    let args = &kids[1..];
    // Every plain builtin recognition returns the same shape — a `CallCanon::Builtin`
    // carrying either the single first argument or all arguments. `builtin!(Op, first)` and
    // `builtin!(Op, all)` name those two cases so the recognition arms stay one line each.
    macro_rules! builtin {
        ($op:expr, first) => {
            return CallCanon::Builtin {
                op: $op,
                arg_olds: vec![args[0]],
            }
        };
        ($op:expr, all) => {
            return CallCanon::Builtin {
                op: $op,
                arg_olds: args.to_vec(),
            }
        };
    }
    let cn = old.node(callee);
    match cn.kind {
        NodeKind::Var => {
            if let Payload::Name(s) = cn.payload {
                match interner.resolve(s) {
                    "len" if args.len() == 1 => builtin!(Builtin::Len, first),
                    "print" => builtin!(Builtin::Print, all),
                    // Go's builtin `append(xs, x...)`
                    "append" => builtin!(Builtin::Append, all),
                    "range" => builtin!(Builtin::Range, all),
                    // `sum(xs)` / `sum(x for x in xs)` — additive reduction.
                    "sum" if args.len() == 1 => builtin!(Builtin::Sum, first),
                    // `functools.reduce(f, xs[, init])` — explicit fold.
                    "reduce" if args.len() >= 2 => builtin!(Builtin::Reduce, all),
                    // `min(iterable)` / `max(iterable)` — selection reduction.
                    // `min(a, b)` / `max(a, b)` — scalar 2-way choice.
                    "min" if args.len() == 1 || args.len() == 2 => builtin!(Builtin::Min, all),
                    "max" if args.len() == 1 || args.len() == 2 => builtin!(Builtin::Max, all),
                    "fmin" | "fminf" | "fminl" if args.len() == 2 => builtin!(Builtin::Min, all),
                    "fmax" | "fmaxf" | "fmaxl" if args.len() == 2 => builtin!(Builtin::Max, all),
                    "abs" | "fabs" if args.len() == 1 => builtin!(Builtin::Abs, first),
                    "zip" if args.len() == 2 => builtin!(Builtin::Zip, all),
                    "enumerate" if args.len() == 1 => builtin!(Builtin::Enumerate, first),
                    // `any(gen)` / `all(gen)` — Python existential/universal reduction over
                    // a generator (which lowers to a `Map`). One canonical arg (the Map).
                    "any" if args.len() == 1 => builtin!(Builtin::Any, first),
                    "all" if args.len() == 1 => builtin!(Builtin::All, first),
                    _ => {}
                }
            }
        }
        NodeKind::Field => {
            if let Payload::Name(s) = cn.payload {
                let fname = interner.resolve(s);
                let base = old.children(callee).first().copied();
                let base_name = base.and_then(|b| name_of(old, interner, b));
                match fname {
                    // xs.append(x) / xs.push(x)  →  Append[xs, args...]
                    "append" | "push" => {
                        let mut a = Vec::with_capacity(args.len() + 1);
                        if let Some(b) = base {
                            a.push(b);
                        }
                        a.extend_from_slice(args);
                        return CallCanon::Builtin {
                            op: Builtin::Append,
                            arg_olds: a,
                        };
                    }
                    "log" | "info" | "debug" if base_name == Some("console") => {
                        return CallCanon::Builtin {
                            op: Builtin::Print,
                            arg_olds: args.to_vec(),
                        }
                    }
                    "Println" | "Printf" | "Print" if base_name == Some("fmt") => {
                        return CallCanon::Builtin {
                            op: Builtin::Print,
                            arg_olds: args.to_vec(),
                        }
                    }
                    "Abs" if base_name == Some("math") && args.len() == 1 => {
                        return CallCanon::Builtin {
                            op: Builtin::Abs,
                            arg_olds: vec![args[0]],
                        }
                    }
                    "HasPrefix" | "HasSuffix"
                        if base_name == Some("strings") && args.len() == 2 =>
                    {
                        return CallCanon::Builtin {
                            op: if fname == "HasPrefix" {
                                Builtin::StartsWith
                            } else {
                                Builtin::EndsWith
                            },
                            arg_olds: args.to_vec(),
                        }
                    }
                    "Contains"
                        if args.len() == 2
                            && base.is_some_and(|b| {
                                import_namespace_expr(old, interner, b, "slices")
                            }) =>
                    {
                        return CallCanon::Builtin {
                            op: Builtin::Contains,
                            arg_olds: vec![args[1], args[0]],
                        }
                    }
                    "len" | "length" | "size" if base.is_some() && args.is_empty() => {
                        return CallCanon::Builtin {
                            op: Builtin::Len,
                            arg_olds: vec![base.unwrap()],
                        }
                    }
                    "is_empty" | "isEmpty" | "empty?" if base.is_some() && args.is_empty() => {
                        return CallCanon::Builtin {
                            op: Builtin::IsEmpty,
                            arg_olds: vec![base.unwrap()],
                        }
                    }
                    "nil?" | "is_none" if base.is_some() && args.is_empty() => {
                        return CallCanon::Builtin {
                            op: Builtin::IsNull,
                            arg_olds: vec![base.unwrap()],
                        }
                    }
                    "is_some" if base.is_some() && args.is_empty() => {
                        if let Some((map, key)) = map_get_call_parts(old, interner, base.unwrap()) {
                            return CallCanon::Builtin {
                                op: Builtin::Contains,
                                arg_olds: vec![key, map],
                            };
                        }
                        return CallCanon::Builtin {
                            op: Builtin::IsNotNull,
                            arg_olds: vec![base.unwrap()],
                        };
                    }
                    "startsWith" | "startswith" | "starts_with" | "start_with?"
                        if base.is_some() && args.len() == 1 =>
                    {
                        return CallCanon::Builtin {
                            op: Builtin::StartsWith,
                            arg_olds: vec![base.unwrap(), args[0]],
                        }
                    }
                    "endsWith" | "endswith" | "ends_with" | "end_with?"
                        if base.is_some() && args.len() == 1 =>
                    {
                        return CallCanon::Builtin {
                            op: Builtin::EndsWith,
                            arg_olds: vec![base.unwrap(), args[0]],
                        }
                    }
                    "containsKey" | "contains_key" | "key?" | "has_key?" | "__contains__"
                        if base.is_some() && args.len() == 1 =>
                    {
                        return CallCanon::Builtin {
                            op: Builtin::Contains,
                            arg_olds: vec![args[0], base.unwrap()],
                        }
                    }
                    "includes" | "include?" | "contains" | "__contains__"
                        if base.is_some()
                            && args.len() == 1
                            && (old.kind(base.unwrap()) == NodeKind::Seq
                                || (fname == "contains"
                                    && key_set_receiver(old, interner, base.unwrap())
                                        .is_some())) =>
                    {
                        let collection = if fname == "contains" {
                            key_set_receiver(old, interner, base.unwrap()).unwrap_or(base.unwrap())
                        } else {
                            base.unwrap()
                        };
                        return CallCanon::Builtin {
                            op: Builtin::Contains,
                            arg_olds: vec![args[0], collection],
                        };
                    }
                    // Python `sep.join(xs)`: ordered string-builder fold. Keep this
                    // restricted to a literal separator receiver; JavaScript/Ruby
                    // `xs.join(sep)` have the collection as the receiver and are not this
                    // surface shape.
                    "join"
                        if base.is_some()
                            && args.len() == 1
                            && matches!(
                                old.node(base.unwrap()).payload,
                                Payload::LitStr(_) | Payload::Lit(nose_il::LitClass::Str)
                            ) =>
                    {
                        return CallCanon::Builtin {
                            op: Builtin::Join,
                            arg_olds: vec![base.unwrap(), args[0]],
                        };
                    }
                    "get" | "fetch"
                        if base.is_some()
                            && args.len() == 2
                            && map_like_literal(old, interner, base.unwrap()) =>
                    {
                        let fallback = if fname == "fetch" && old.kind(args[1]) == NodeKind::Lambda
                        {
                            let Some(fallback) = zero_arg_lambda_body_value(old, args[1]) else {
                                return CallCanon::None;
                            };
                            fallback
                        } else {
                            args[1]
                        };
                        return CallCanon::Builtin {
                            op: Builtin::GetOrDefault,
                            arg_olds: vec![base.unwrap(), args[0], fallback],
                        };
                    }
                    "getOrDefault" if base.is_some() && args.len() == 2 => {
                        return CallCanon::Builtin {
                            op: Builtin::GetOrDefault,
                            arg_olds: vec![base.unwrap(), args[0], args[1]],
                        }
                    }
                    "unwrap_or" if base.is_some() && args.len() == 1 => {
                        if let Some((map, key)) = map_get_call_parts(old, interner, base.unwrap()) {
                            return CallCanon::Builtin {
                                op: Builtin::GetOrDefault,
                                arg_olds: vec![map, key, args[0]],
                            };
                        }
                        return CallCanon::Builtin {
                            op: Builtin::ValueOrDefault,
                            arg_olds: vec![base.unwrap(), args[0]],
                        };
                    }
                    "unwrap_or_else" if base.is_some() && args.len() == 1 => {
                        if let Some(fallback) = zero_arg_lambda_body(old, args[0]) {
                            return CallCanon::Builtin {
                                op: Builtin::ValueOrDefault,
                                arg_olds: vec![base.unwrap(), fallback],
                            };
                        }
                    }
                    "map_or" if base.is_some() && args.len() == 2 => {
                        if identity_lambda(old, args[1]) {
                            return CallCanon::Builtin {
                                op: Builtin::ValueOrDefault,
                                arg_olds: vec![base.unwrap(), args[0]],
                            };
                        }
                    }
                    // `functools.reduce(f, xs[, init])` — here the *base* is the module
                    // `functools`, not the collection, so it is an explicit fold over
                    // `xs` (the same `Builtin::Reduce` as a bare `reduce(f, xs, init)`),
                    // NOT a `xs.reduce(f)` method HoF. Without this it was read as a HoF
                    // over `functools`, and a `functools.reduce` sum never converged with
                    // its loop form.
                    "reduce" if base_name == Some("functools") && args.len() >= 2 => {
                        return CallCanon::Builtin {
                            op: Builtin::Reduce,
                            arg_olds: args.to_vec(),
                        }
                    }
                    "Min" | "Max" if base_name == Some("math") && args.len() == 2 => {
                        let op = if fname == "Min" {
                            Builtin::Min
                        } else {
                            Builtin::Max
                        };
                        return CallCanon::Builtin {
                            op,
                            arg_olds: args.to_vec(),
                        };
                    }
                    // Rust/iterator-style `a.iter().zip(b.iter())` is the same aligned
                    // pair stream as Python `zip(a, b)`. Canonicalize it before outer
                    // `.fold`/`.map` reasoning so tuple element bindings are shared.
                    "zip" if base.is_some() && args.len() == 1 => {
                        return CallCanon::Builtin {
                            op: Builtin::Zip,
                            arg_olds: vec![
                                unwrap_iter(old, interner, base.unwrap()),
                                unwrap_iter(old, interner, args[0]),
                            ],
                        }
                    }
                    // Folds across languages → one canonical `Reduce[fn, coll, init]`,
                    // so a `.reduce`/`.inject`/`.fold` converges with an accumulator loop
                    // (and with `functools.reduce`). The lambda is detected regardless of
                    // arg order — JS `xs.reduce(fn, init)`, Ruby `xs.inject(init){fn}` /
                    // `xs.reduce(init){fn}`, Rust `it.fold(init, fn)`. A Rust `.iter()` /
                    // `.into_iter()` base is unwrapped to the underlying collection.
                    "reduce" | "inject" | "fold" if base.is_some() && !args.is_empty() => {
                        let (fn_old, init_old) = fold_fn_init(old, args);
                        if let Some(fn_old) = fn_old {
                            let coll = unwrap_iter(old, interner, base.unwrap());
                            let mut a = vec![fn_old, coll];
                            if let Some(init) = init_old {
                                a.push(init);
                            }
                            return CallCanon::Builtin {
                                op: Builtin::Reduce,
                                arg_olds: a,
                            };
                        }
                        // no lambda arg → not a recognizable fold; leave as a plain call.
                    }
                    // Existential/universal predicate reductions across languages → one
                    // canonical `Any`/`All[coll, λ]`: JS `xs.some/every(p)`, Rust
                    // `xs.iter().any/all(p)`, Java `stream.anyMatch/allMatch(p)`. The
                    // iteration adapter base is unwrapped to the collection, so it
                    // converges with Python `any(p(x) for x in xs)`.
                    "some" | "any" | "any?" | "anyMatch" if base.is_some() && !args.is_empty() => {
                        let coll = unwrap_iter(old, interner, base.unwrap());
                        return CallCanon::Builtin {
                            op: Builtin::Any,
                            arg_olds: vec![coll, args[0]],
                        };
                    }
                    "every" | "all" | "all?" | "allMatch" if base.is_some() && !args.is_empty() => {
                        let coll = unwrap_iter(old, interner, base.unwrap());
                        return CallCanon::Builtin {
                            op: Builtin::All,
                            arg_olds: vec![coll, args[0]],
                        };
                    }
                    "map" | "collect" | "filter" | "select"
                        if base.is_some() && !args.is_empty() =>
                    {
                        let kind = if matches!(fname, "map" | "collect") {
                            HoFKind::Map
                        } else {
                            HoFKind::Filter
                        };
                        return CallCanon::HoF {
                            kind,
                            collection_old: unwrap_iter(old, interner, base.unwrap()),
                            fn_old: args[0],
                        };
                    }
                    "abs" if base.is_some() && args.is_empty() => {
                        return CallCanon::Builtin {
                            op: Builtin::Abs,
                            arg_olds: vec![base.unwrap()],
                        };
                    }
                    // Method-form iterator reductions (Rust `it.sum()/min()/max()/count()`,
                    // taking no value args — the receiver IS the collection). Canonicalize to
                    // the same builtin as the function form, unwrapping a `.iter()` base, so
                    // `xs.iter().filter(p).sum()` converges with Python `sum(x for x in xs if p)`
                    // and `.count()` with `len([… if p])` / `sum(1 for …)` (both via `Len`).
                    "sum" | "min" | "max" | "count" if base.is_some() && args.is_empty() => {
                        let op = match fname {
                            "sum" => Builtin::Sum,
                            "min" => Builtin::Min,
                            "max" => Builtin::Max,
                            _ => Builtin::Len, // count
                        };
                        let base_id = base.unwrap();
                        if matches!(fname, "min" | "max") && old.kind(base_id) == NodeKind::Seq {
                            let items = old.children(base_id);
                            if items.len() == 2 {
                                return CallCanon::Builtin {
                                    op,
                                    arg_olds: items.to_vec(),
                                };
                            }
                        }
                        let coll = unwrap_iter(old, interner, base_id);
                        return CallCanon::Builtin {
                            op,
                            arg_olds: vec![coll],
                        };
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    CallCanon::None
}

/// Classify a fold's args into `(fn, init)`: the lambda is the function (whatever its
/// position — JS puts it first, Ruby/Rust last), the other arg (if any) is the seed.
fn fold_fn_init(old: &Il, args: &[NodeId]) -> (Option<NodeId>, Option<NodeId>) {
    let mut fn_old = None;
    let mut init = None;
    for &a in args {
        if old.kind(a) == NodeKind::Lambda {
            fn_old = Some(a);
        } else {
            init = Some(a);
        }
    }
    (fn_old, init)
}

/// Peel a Rust iteration adapter — `xs.iter()` / `xs.into_iter()` / `xs.iter_mut()` —
/// to the underlying collection, so a `xs.iter().fold(..)` iterates over `xs` (matching
/// `for v in xs`). NOT `.keys()`/`.values()`, which change *what* is iterated.
fn unwrap_iter(old: &Il, interner: &Interner, node: NodeId) -> NodeId {
    if old.kind(node) == NodeKind::Call {
        if let Some(&callee) = old.children(node).first() {
            if old.kind(callee) == NodeKind::Field {
                if let Payload::Name(s) = old.node(callee).payload {
                    let method = interner.resolve(s);
                    if method == "stream" {
                        if let Some(&base) = old.children(callee).first() {
                            if name_of(old, interner, base) == Some("Arrays") {
                                if let Some(&arg) = old.children(node).get(1) {
                                    return arg;
                                }
                            } else {
                                return base;
                            }
                        }
                    }
                    if matches!(method, "iter" | "into_iter" | "iter_mut") {
                        if let Some(&base) = old.children(callee).first() {
                            return base;
                        }
                    }
                }
            }
        }
    }
    node
}

/// Canonical sync name for a known async counterpart, so behaviorally-equivalent
/// sync/async twins (a frequent real Type-4 pattern, e.g. `__exit__`/`__aexit__`,
/// `read`/`aread`) converge. Curated to high-confidence pairs only — generic
/// `a`-prefixed names (`add`, `append`, `get`) are deliberately excluded.
pub(crate) fn async_to_sync(name: &str) -> Option<&'static str> {
    Some(match name {
        // async dunder protocol methods
        "__aenter__" => "__enter__",
        "__aexit__" => "__exit__",
        "__anext__" => "__next__",
        "__aiter__" => "__iter__",
        // async I/O / lifecycle method twins
        "aread" => "read",
        "areadline" => "readline",
        "areadlines" => "readlines",
        "awrite" => "write",
        "aclose" => "close",
        "asend" => "send",
        "areceive" => "receive",
        "aconnect" => "connect",
        "adrain" => "drain",
        "aflush" => "flush",
        // async typing constructs
        "AsyncIterable" => "Iterable",
        "AsyncIterator" => "Iterator",
        "AsyncGenerator" => "Generator",
        "AsyncContextManager" => "ContextManager",
        _ => return None,
    })
}

fn name_of<'a>(old: &Il, interner: &'a Interner, id: NodeId) -> Option<&'a str> {
    let n = old.node(id);
    if n.kind == NodeKind::Var {
        if let Payload::Name(s) = n.payload {
            return Some(interner.resolve(s));
        }
    }
    None
}

fn import_namespace_expr(old: &Il, interner: &Interner, id: NodeId, module: &str) -> bool {
    let Some(alias) = name_of(old, interner, id) else {
        return false;
    };
    old.children(old.root).iter().any(|&stmt| {
        import_namespace_assignment(old, interner, stmt, alias, module)
            || (old.kind(stmt) == NodeKind::Block
                && old
                    .children(stmt)
                    .iter()
                    .any(|&child| import_namespace_assignment(old, interner, child, alias, module)))
    })
}

fn import_namespace_assignment(
    old: &Il,
    interner: &Interner,
    stmt: NodeId,
    alias: &str,
    module: &str,
) -> bool {
    if old.kind(stmt) != NodeKind::Assign {
        return false;
    }
    let kids = old.children(stmt);
    if kids.len() != 2 || assignment_alias_name(old, interner, kids[0]) != Some(alias) {
        return false;
    }
    if old.kind(kids[1]) != NodeKind::Seq {
        return false;
    }
    let Payload::Name(seq_name) = old.node(kids[1]).payload else {
        return false;
    };
    if interner.resolve(seq_name) != "import_namespace" {
        return false;
    }
    let Some(&module_node) = old.children(kids[1]).first() else {
        return false;
    };
    matches!(
        old.node(module_node).payload,
        Payload::LitStr(hash) if hash == stable_symbol_hash(module)
    )
}

fn assignment_alias_name<'a>(old: &Il, interner: &'a Interner, id: NodeId) -> Option<&'a str> {
    if old.kind(id) != NodeKind::Var {
        return None;
    }
    match old.node(id).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        Payload::Cid(cid) => old
            .cid_names
            .get(cid as usize)
            .map(|&symbol| interner.resolve(symbol)),
        _ => None,
    }
}

fn map_like_literal(old: &Il, interner: &Interner, id: NodeId) -> bool {
    if old.kind(id) != NodeKind::Seq {
        return false;
    }
    match old.node(id).payload {
        Payload::Name(s) => matches!(interner.resolve(s), "dictionary" | "hash" | "object"),
        _ => false,
    }
}

fn map_get_call_parts(old: &Il, interner: &Interner, id: NodeId) -> Option<(NodeId, NodeId)> {
    if old.kind(id) != NodeKind::Call {
        return None;
    }
    let kids = old.children(id);
    if kids.len() != 2 || old.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = old.node(kids[0]).payload else {
        return None;
    };
    if interner.resolve(method) != "get" {
        return None;
    }
    let receiver = *old.children(kids[0]).first()?;
    Some((receiver, kids[1]))
}

fn key_set_receiver(old: &Il, interner: &Interner, id: NodeId) -> Option<NodeId> {
    if old.kind(id) != NodeKind::Call {
        return None;
    }
    let kids = old.children(id);
    if kids.len() != 1 || old.kind(kids[0]) != NodeKind::Field {
        return None;
    }
    let Payload::Name(method) = old.node(kids[0]).payload else {
        return None;
    };
    if interner.resolve(method) != "keySet" {
        return None;
    }
    old.children(kids[0]).first().copied()
}

fn zero_arg_lambda_body(old: &Il, lambda: NodeId) -> Option<NodeId> {
    if old.kind(lambda) != NodeKind::Lambda {
        return None;
    }
    let kids = old.children(lambda);
    if kids.len() == 1 {
        Some(kids[0])
    } else {
        None
    }
}

fn zero_arg_lambda_body_value(old: &Il, lambda: NodeId) -> Option<NodeId> {
    let body = zero_arg_lambda_body(old, lambda)?;
    implicit_block_value(old, body).or(Some(body))
}

fn implicit_block_value(old: &Il, block: NodeId) -> Option<NodeId> {
    if old.kind(block) != NodeKind::Block {
        return None;
    }
    let kids = old.children(block);
    let &[stmt] = kids else {
        return None;
    };
    match old.kind(stmt) {
        NodeKind::ExprStmt | NodeKind::Return => {
            let stmt_kids = old.children(stmt);
            let &[expr] = stmt_kids else {
                return None;
            };
            Some(expr)
        }
        _ => None,
    }
}

fn identity_lambda(old: &Il, lambda: NodeId) -> bool {
    if old.kind(lambda) != NodeKind::Lambda {
        return false;
    }
    let kids = old.children(lambda);
    if kids.len() != 2 || old.kind(kids[0]) != NodeKind::Param || old.kind(kids[1]) != NodeKind::Var
    {
        return false;
    }
    match (old.node(kids[0]).payload, old.node(kids[1]).payload) {
        (Payload::Cid(a), Payload::Cid(b)) => a == b,
        (Payload::Name(a), Payload::Name(b)) => a == b,
        _ => false,
    }
}
