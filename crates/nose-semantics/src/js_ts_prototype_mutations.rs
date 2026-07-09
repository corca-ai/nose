//! JavaScript/TypeScript same-file builtin prototype mutation checks.

use super::*;

pub type JsTsUnshadowedNameResolver = fn(&Il, &Interner, NodeId, &str) -> bool;

pub fn js_ts_builtin_prototype_mutated_in_file(
    il: &Il,
    interner: &Interner,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    top_level_statements: impl IntoIterator<Item = NodeId>,
    unshadowed_name: JsTsUnshadowedNameResolver,
) -> bool {
    if !matches!(il.meta.lang, Lang::JavaScript | Lang::TypeScript) {
        return false;
    }
    let Some((prototype, method)) = js_ts_mutation_sensitive_prototype_method(id, callee) else {
        return false;
    };
    top_level_statements.into_iter().any(|stmt| {
        builtin_prototype_method_mutation_in_module_scope(
            il,
            interner,
            stmt,
            prototype,
            method,
            unshadowed_name,
        )
    })
}

fn js_ts_mutation_sensitive_prototype_method(
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
) -> Option<(&'static str, &'static str)> {
    match (id, callee) {
        (
            LibraryApiContractId::MethodCall(MethodSemanticContract::Builtin(
                Builtin::StartsWith | Builtin::EndsWith,
            )),
            LibraryApiCalleeContract::Method { method, .. },
        ) => Some(("String", method)),
        (
            LibraryApiContractId::MethodCall(
                MethodSemanticContract::HoF(HoFKind::Map | HoFKind::Filter | HoFKind::FlatMap)
                | MethodSemanticContract::Builtin(Builtin::Any | Builtin::All),
            ),
            LibraryApiCalleeContract::Method {
                method,
                receiver: MethodReceiverContract::ExactArray,
            },
        ) => Some(("Array", method)),
        _ => None,
    }
}

fn builtin_prototype_method_mutation_in_module_scope(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    prototype: &str,
    expected_method: &str,
    unshadowed_name: JsTsUnshadowedNameResolver,
) -> bool {
    if matches!(il.kind(node), NodeKind::Func | NodeKind::Lambda) {
        return false;
    }
    builtin_prototype_method_write(
        il,
        interner,
        node,
        prototype,
        expected_method,
        unshadowed_name,
    ) || object_define_property_builtin_prototype_method(
        il,
        interner,
        node,
        prototype,
        expected_method,
        unshadowed_name,
    ) || il.children(node).iter().copied().any(|child| {
        builtin_prototype_method_mutation_in_module_scope(
            il,
            interner,
            child,
            prototype,
            expected_method,
            unshadowed_name,
        )
    })
}

fn builtin_prototype_method_write(
    il: &Il,
    interner: &Interner,
    stmt: NodeId,
    prototype: &str,
    expected_method: &str,
    unshadowed_name: JsTsUnshadowedNameResolver,
) -> bool {
    let assign = if il.kind(stmt) == NodeKind::ExprStmt {
        il.children(stmt).first().copied().unwrap_or(stmt)
    } else {
        stmt
    };
    if il.kind(assign) != NodeKind::Assign {
        return false;
    }
    let Some(&target) = il.children(assign).first() else {
        return false;
    };
    field_name(il, interner, target) == Some(expected_method)
        && il.children(target).first().copied().is_some_and(|object| {
            builtin_prototype_object(il, interner, object, prototype, unshadowed_name)
        })
}

fn object_define_property_builtin_prototype_method(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    prototype: &str,
    expected_method: &str,
    unshadowed_name: JsTsUnshadowedNameResolver,
) -> bool {
    let call = if il.kind(node) == NodeKind::ExprStmt {
        il.children(node).first().copied().unwrap_or(node)
    } else {
        node
    };
    if il.kind(call) != NodeKind::Call {
        return false;
    }
    let [callee, target, property, ..] = il.children(call) else {
        return false;
    };
    field_name(il, interner, *callee) == Some("defineProperty")
        && il
            .children(*callee)
            .first()
            .copied()
            .is_some_and(|base| unshadowed_name(il, interner, base, "Object"))
        && builtin_prototype_object(il, interner, *target, prototype, unshadowed_name)
        && string_literal(il, *property, expected_method)
}

fn builtin_prototype_object(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    prototype: &str,
    unshadowed_name: JsTsUnshadowedNameResolver,
) -> bool {
    field_name(il, interner, node) == Some("prototype")
        && il
            .children(node)
            .first()
            .copied()
            .is_some_and(|base| unshadowed_name(il, interner, base, prototype))
}

fn string_literal(il: &Il, node: NodeId, expected: &str) -> bool {
    matches!(il.node(node).payload, Payload::LitStr(hash) if hash == stable_symbol_hash(expected))
}

fn field_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Field {
        return None;
    }
    let Payload::Name(symbol) = il.node(node).payload else {
        return None;
    };
    Some(interner.resolve(symbol))
}
