//! First-party call-target evidence producer.
//!
//! Consumers must not resolve user calls from raw spelling. This pass is the
//! narrow boundary where the lowered file's binding shape is checked and a
//! direct in-file or imported callable target is materialized as evidence.

use nose_il::{
    CallTargetEvidenceKind, EvidenceAnchor, EvidenceEmitter, EvidenceId, EvidenceKind,
    EvidenceProvenance, EvidenceRecord, EvidenceStatus, Il, Interner, LoopKind, NodeId, NodeKind,
    Payload, Symbol, SymbolEvidenceKind, UnitKind,
};
use nose_semantics::{
    imported_occurrence_symbol_dependencies_valid_with_cache, ImportedOccurrenceValidationCache,
    FIRST_PARTY_PACK_ID,
};
use rustc_hash::{FxHashMap, FxHashSet};

const DIRECT_FUNCTION_RULE: &str = "normalize.call_target.direct_function";
const IMPORTED_FUNCTION_RULE: &str = "normalize.call_target.imported_function";
const IMPORTED_MEMBER_RULE: &str = "normalize.call_target.imported_member";
const IMPORTED_BINDING_OCCURRENCE_RULE: &str =
    "normalize.symbol_imported_binding_occurrence_for_call_target";
const IMPORTED_NAMESPACE_OCCURRENCE_RULE: &str =
    "normalize.symbol_imported_namespace_occurrence_for_call_target";

#[derive(Clone, Copy)]
struct DirectFunctionTarget {
    root: NodeId,
    name_hash: u64,
}

#[derive(Clone, Copy)]
struct ImportedFunctionTarget {
    module_hash: u64,
    exported_hash: u64,
    local_hash: u64,
    dependency: EvidenceId,
}

#[derive(Clone, Copy)]
struct ImportedMemberTarget {
    module_hash: u64,
    exported_hash: u64,
    member_hash: u64,
    dependency: EvidenceId,
}

#[derive(Clone, Copy)]
enum ImportedBindingUse {
    FunctionCallee,
    MemberReceiver,
}

pub(crate) fn run(il: &mut Il, interner: &Interner) {
    let targets = unique_direct_function_targets(il, interner);
    let mut calls = Vec::new();
    collect_call_nodes(il, il.root, &mut calls);

    let function_names = function_unit_names(il);
    let scope_bound = scope_bound_symbols(il, &function_names);
    let mut proven = Vec::new();
    let mut scope_stack = Vec::new();
    if !targets.is_empty() {
        collect_call_targets(
            il,
            interner,
            il.root,
            &mut scope_stack,
            &targets,
            &scope_bound,
            &mut proven,
        );
    }
    for (call, target) in proven {
        il.find_or_push_first_party_evidence(
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::CallTarget(CallTargetEvidenceKind::DirectFunction {
                target_span: il.node(target.root).span,
                name_hash: target.name_hash,
            }),
            FIRST_PARTY_PACK_ID,
            DIRECT_FUNCTION_RULE,
            Vec::new(),
        );
    }
    let mut imported_occurrence_cache = ImportedOccurrenceValidationCache::default();
    for call in calls {
        record_imported_call_target(il, interner, call, &mut imported_occurrence_cache);
    }
}

fn unique_direct_function_targets(
    il: &Il,
    interner: &Interner,
) -> FxHashMap<Symbol, DirectFunctionTarget> {
    let parents = parent_map(il);
    let mut targets = FxHashMap::default();
    let mut ambiguous = FxHashSet::default();
    // Names rebound at module scope from inside another function (`global name; name =
    // ...`): the runtime binding is no longer the `def` body, so the name is not a
    // DirectFunction target. Precise — a local `name = x` (no `global`) carries no fact
    // and stays a valid target (#302). Empty for non-Python and the common case.
    let rebound = nose_semantics::module_rebound_symbols(il, interner);
    for unit in &il.units {
        if unit.kind != UnitKind::Function || !is_top_level_function_root(il, &parents, unit.root) {
            continue;
        }
        let Some(name) = unit.name else { continue };
        if ambiguous.contains(&name) {
            continue;
        }
        // A decorated `def` binds `decorator(f)`, not the lowered body (coevo series 6,
        // S2-A); a `global`-reassigned name binds whatever was last assigned, not its
        // `def` body (#302). Both fail closed: no DirectFunction evidence, so the inline,
        // the content-keyed exact admission, and the behavioral oracle all stay opaque.
        if nose_semantics::decorated_definition_at_node(il, unit.root) || rebound.contains(&name) {
            continue;
        }
        let target = DirectFunctionTarget {
            root: unit.root,
            name_hash: interner.symbol_hash(name),
        };
        if targets.insert(name, target).is_some() {
            targets.remove(&name);
            ambiguous.insert(name);
        }
    }
    targets
}

fn parent_map(il: &Il) -> Vec<Option<NodeId>> {
    let mut parents = vec![None; il.nodes.len()];
    for (idx, _) in il.nodes.iter().enumerate() {
        let parent = NodeId(idx as u32);
        for &child in il.children(parent) {
            if let Some(slot) = parents.get_mut(child.0 as usize) {
                *slot = Some(parent);
            }
        }
    }
    parents
}

fn is_top_level_function_root(il: &Il, parents: &[Option<NodeId>], root: NodeId) -> bool {
    parents
        .get(root.0 as usize)
        .copied()
        .flatten()
        .is_some_and(|parent| il.kind(parent) == NodeKind::Module)
}

fn function_unit_names(il: &Il) -> FxHashMap<u32, Symbol> {
    let mut names = FxHashMap::default();
    for unit in &il.units {
        if unit.kind == UnitKind::Function {
            if let Some(name) = unit.name {
                names.insert(unit.root.0, name);
            }
        }
    }
    names
}

fn scope_bound_symbols(
    il: &Il,
    function_names: &FxHashMap<u32, Symbol>,
) -> FxHashMap<u32, FxHashSet<Symbol>> {
    let mut out = FxHashMap::default();
    collect_scope_bound_symbols(il, il.root, function_names, &mut out);
    out
}

fn collect_scope_bound_symbols(
    il: &Il,
    node: NodeId,
    function_names: &FxHashMap<u32, Symbol>,
    out: &mut FxHashMap<u32, FxHashSet<Symbol>>,
) {
    if is_scope(il.kind(node)) {
        let mut bound = FxHashSet::default();
        collect_bound_in_scope(il, node, true, il.kind(node), function_names, &mut bound);
        out.insert(node.0, bound);
    }
    for &child in il.children(node) {
        collect_scope_bound_symbols(il, child, function_names, out);
    }
}

fn collect_bound_in_scope(
    il: &Il,
    node: NodeId,
    is_root: bool,
    scope_kind: NodeKind,
    function_names: &FxHashMap<u32, Symbol>,
    out: &mut FxHashSet<Symbol>,
) {
    if !is_root && is_scope(il.kind(node)) {
        if scope_kind != NodeKind::Module {
            if let Some(&name) = function_names.get(&node.0) {
                out.insert(name);
            }
        }
        return;
    }
    match il.kind(node) {
        NodeKind::Param => {
            if let Payload::Name(name) = il.node(node).payload {
                out.insert(name);
            }
        }
        NodeKind::Assign => {
            if let Some(&lhs) = il.children(node).first() {
                collect_target_symbols(il, lhs, out);
            }
        }
        NodeKind::Loop if matches!(il.node(node).payload, Payload::Loop(LoopKind::ForEach)) => {
            if let Some(&pattern) = il.children(node).first() {
                collect_target_symbols(il, pattern, out);
            }
        }
        _ => {}
    }
    for &child in il.children(node) {
        collect_bound_in_scope(il, child, false, scope_kind, function_names, out);
    }
}

fn collect_target_symbols(il: &Il, node: NodeId, out: &mut FxHashSet<Symbol>) {
    match il.kind(node) {
        NodeKind::Var => {
            if let Payload::Name(name) = il.node(node).payload {
                out.insert(name);
            }
        }
        NodeKind::Seq => {
            for &child in il.children(node) {
                collect_target_symbols(il, child, out);
            }
        }
        _ => {}
    }
}

fn collect_call_targets(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    scope_stack: &mut Vec<NodeId>,
    targets: &FxHashMap<Symbol, DirectFunctionTarget>,
    scope_bound: &FxHashMap<u32, FxHashSet<Symbol>>,
    out: &mut Vec<(NodeId, DirectFunctionTarget)>,
) {
    let entered_scope = is_scope(il.kind(node));
    if entered_scope {
        scope_stack.push(node);
    }
    if let Some(target) = direct_call_target(il, interner, node, scope_stack, targets, scope_bound)
    {
        out.push((node, target));
    }
    for &child in il.children(node) {
        collect_call_targets(il, interner, child, scope_stack, targets, scope_bound, out);
    }
    if entered_scope {
        scope_stack.pop();
    }
}

fn direct_call_target(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    scope_stack: &[NodeId],
    targets: &FxHashMap<Symbol, DirectFunctionTarget>,
    scope_bound: &FxHashMap<u32, FxHashSet<Symbol>>,
) -> Option<DirectFunctionTarget> {
    if il.kind(node) != NodeKind::Call || !matches!(il.node(node).payload, Payload::None) {
        return None;
    }
    let callee = *il.children(node).first()?;
    if il.kind(callee) != NodeKind::Var {
        return None;
    }
    if var_has_symbol_identity_evidence(il, interner, callee) {
        return None;
    }
    let Payload::Name(name) = il.node(callee).payload else {
        return None;
    };
    if scope_stack.iter().any(|scope| {
        scope_bound
            .get(&scope.0)
            .is_some_and(|bound| bound.contains(&name))
    }) {
        return None;
    }
    targets.get(&name).copied()
}

fn collect_call_nodes(il: &Il, node: NodeId, out: &mut Vec<NodeId>) {
    if il.kind(node) == NodeKind::Call {
        out.push(node);
    }
    for &child in il.children(node) {
        collect_call_nodes(il, child, out);
    }
}

fn record_imported_call_target(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    cache: &mut ImportedOccurrenceValidationCache,
) {
    if il.kind(call) != NodeKind::Call || !matches!(il.node(call).payload, Payload::None) {
        return;
    }
    let Some(&callee) = il.children(call).first() else {
        return;
    };
    match il.kind(callee) {
        NodeKind::Var => {
            if let Some(target) = imported_function_target(il, interner, callee, cache) {
                il.find_or_push_first_party_evidence(
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedFunction {
                        module_hash: target.module_hash,
                        exported_hash: target.exported_hash,
                        local_hash: target.local_hash,
                    }),
                    FIRST_PARTY_PACK_ID,
                    IMPORTED_FUNCTION_RULE,
                    vec![target.dependency],
                );
            }
        }
        NodeKind::Field => {
            if let Some(target) = imported_member_target(il, interner, callee, cache) {
                il.find_or_push_first_party_evidence(
                    EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
                    EvidenceKind::CallTarget(CallTargetEvidenceKind::ImportedMember {
                        module_hash: target.module_hash,
                        exported_hash: target.exported_hash,
                        member_hash: target.member_hash,
                    }),
                    FIRST_PARTY_PACK_ID,
                    IMPORTED_MEMBER_RULE,
                    vec![target.dependency],
                );
            }
        }
        _ => {}
    }
}

fn imported_function_target(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<ImportedFunctionTarget> {
    let local_hash = node_name_hash(il, interner, callee)?;
    let (symbol, binding_dependency) =
        unique_binding_symbol_for_var(il, interner, callee, ImportedBindingUse::FunctionCallee)?;
    let SymbolEvidenceKind::ImportedBinding {
        module_hash,
        exported_hash,
    } = symbol
    else {
        return None;
    };
    let dependency = upsert_valid_imported_symbol_occurrence(
        il,
        interner,
        callee,
        symbol,
        binding_dependency,
        cache,
    )?;
    Some(ImportedFunctionTarget {
        module_hash,
        exported_hash,
        local_hash,
        dependency,
    })
}

fn imported_member_target(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<ImportedMemberTarget> {
    let Payload::Name(member) = il.node(callee).payload else {
        return None;
    };
    let member_hash = interner.symbol_hash(member);
    let receiver = *il.children(callee).first()?;
    if il.kind(receiver) != NodeKind::Var {
        return None;
    }
    let (symbol, binding_dependency) =
        unique_binding_symbol_for_var(il, interner, receiver, ImportedBindingUse::MemberReceiver)?;
    let dependency = upsert_valid_imported_symbol_occurrence(
        il,
        interner,
        receiver,
        symbol,
        binding_dependency,
        cache,
    )?;
    match symbol {
        SymbolEvidenceKind::ImportedBinding {
            module_hash,
            exported_hash,
        } => Some(ImportedMemberTarget {
            module_hash,
            exported_hash,
            member_hash,
            dependency,
        }),
        SymbolEvidenceKind::ImportedNamespace { module_hash } => Some(ImportedMemberTarget {
            module_hash,
            exported_hash: member_hash,
            member_hash,
            dependency,
        }),
        _ => None,
    }
}

fn unique_binding_symbol_for_var(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    imported_use: ImportedBindingUse,
) -> Option<(SymbolEvidenceKind, EvidenceId)> {
    let local_hash = node_name_hash(il, interner, node)?;
    let mut found = None;
    for record in il.evidence_binding_anchored(local_hash) {
        let EvidenceKind::Symbol(symbol) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted || !il.evidence_dependencies_asserted(record) {
            return None;
        }
        if !imported_symbol_allowed_for_use(symbol, imported_use) {
            return None;
        }
        match found {
            None => found = Some((symbol, record.id)),
            Some((existing, _)) if existing == symbol => {}
            Some(_) => return None,
        }
    }
    found
}

fn imported_symbol_allowed_for_use(
    symbol: SymbolEvidenceKind,
    imported_use: ImportedBindingUse,
) -> bool {
    match imported_use {
        ImportedBindingUse::FunctionCallee => {
            matches!(symbol, SymbolEvidenceKind::ImportedBinding { .. })
        }
        ImportedBindingUse::MemberReceiver => matches!(
            symbol,
            SymbolEvidenceKind::ImportedBinding { .. }
                | SymbolEvidenceKind::ImportedNamespace { .. }
        ),
    }
}

fn upsert_valid_imported_symbol_occurrence(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    symbol: SymbolEvidenceKind,
    binding_dependency: EvidenceId,
    cache: &mut ImportedOccurrenceValidationCache,
) -> Option<EvidenceId> {
    if !imported_symbol_occurrence_can_be_upserted(il, interner, node, symbol, cache) {
        return None;
    }
    let rule = match symbol {
        SymbolEvidenceKind::ImportedBinding { .. } => IMPORTED_BINDING_OCCURRENCE_RULE,
        SymbolEvidenceKind::ImportedNamespace { .. } => IMPORTED_NAMESPACE_OCCURRENCE_RULE,
        _ => return None,
    };
    let anchor = EvidenceAnchor::node(il.node(node).span, NodeKind::Var);
    let kind = EvidenceKind::Symbol(symbol);
    let dependencies = vec![binding_dependency];
    let candidate = EvidenceRecord {
        id: EvidenceId(u32::MAX),
        anchor,
        kind,
        provenance: EvidenceProvenance {
            emitter: EvidenceEmitter::FirstParty,
            pack_hash: None,
            rule_hash: None,
        },
        dependencies: dependencies.clone(),
        status: EvidenceStatus::Asserted,
    };
    if !imported_occurrence_symbol_dependencies_valid_with_cache(
        il, interner, &candidate, symbol, cache,
    ) {
        return None;
    }
    let id =
        il.find_or_push_first_party_evidence(anchor, kind, FIRST_PARTY_PACK_ID, rule, dependencies);
    Some(id)
}

fn imported_symbol_occurrence_can_be_upserted(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
    cache: &mut ImportedOccurrenceValidationCache,
) -> bool {
    let anchor = EvidenceAnchor::node(il.node(node).span, NodeKind::Var);
    for record in il.evidence_anchored_at(anchor.span()) {
        if record.anchor != anchor {
            continue;
        }
        let EvidenceKind::Symbol(actual) = record.kind else {
            continue;
        };
        if record.status != EvidenceStatus::Asserted
            || actual != expected
            || !il.evidence_dependencies_asserted(record)
            || !imported_occurrence_symbol_dependencies_valid_with_cache(
                il, interner, record, expected, cache,
            )
        {
            return false;
        }
    }
    true
}

fn var_has_symbol_identity_evidence(il: &Il, interner: &Interner, node: NodeId) -> bool {
    let anchor = EvidenceAnchor::node(il.node(node).span, NodeKind::Var);
    if il
        .evidence_anchored_at(anchor.span())
        .any(|record| record.anchor == anchor && matches!(record.kind, EvidenceKind::Symbol(_)))
    {
        return true;
    }
    let Some(local_hash) = node_name_hash(il, interner, node) else {
        return false;
    };
    il.evidence_binding_anchored(local_hash)
        .any(|record| matches!(record.kind, EvidenceKind::Symbol(_)))
}

fn node_name_hash(il: &Il, interner: &Interner, node: NodeId) -> Option<u64> {
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.symbol_hash(symbol)),
        Payload::Cid(cid) => il
            .cid_names
            .get(cid as usize)
            .map(|&symbol| interner.symbol_hash(symbol)),
        _ => None,
    }
}

fn is_scope(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Module | NodeKind::Func | NodeKind::Lambda)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{
        stable_symbol_hash, EvidenceEmitter, EvidenceProvenance, EvidenceRecord, FileId, FileMeta,
        IlBuilder, Lang, Span, Unit,
    };
    use nose_semantics::{
        call_target_evidence_at_call, direct_function_call_target_at_call,
        imported_function_call_target_at_call, imported_member_call_target_at_call,
    };

    fn sp(n: u32) -> Span {
        Span::new(FileId(0), n, n + 1, n, n)
    }

    fn wide_sp(start: u32, end: u32) -> Span {
        Span::new(FileId(0), start, end, start, end)
    }

    fn evidence_with_dependencies(
        id: u32,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        status: EvidenceStatus,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id),
            anchor,
            kind,
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash("test")),
            },
            dependencies,
            status,
        }
    }

    fn binding_symbol(
        id: u32,
        span: Span,
        local: &str,
        symbol: SymbolEvidenceKind,
        status: EvidenceStatus,
    ) -> EvidenceRecord {
        evidence_with_dependencies(
            id,
            EvidenceAnchor::binding(span, stable_symbol_hash(local)),
            EvidenceKind::Symbol(symbol),
            status,
            Vec::new(),
        )
    }

    fn function_with_call(
        interner: &Interner,
        func_name: &str,
        callee_name: &str,
        duplicate_unit: bool,
    ) -> (Il, NodeId, NodeId) {
        let mut b = IlBuilder::new(FileId(0));
        let func_sym = interner.intern(func_name);
        let callee_sym = interner.intern(callee_name);
        let callee = b.add(NodeKind::Var, Payload::Name(callee_sym), sp(10), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp(14), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(15), &[func]);
        let mut units = vec![Unit {
            root: func,
            kind: UnitKind::Function,
            name: Some(func_sym),
        }];
        if duplicate_unit {
            units.push(Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(func_sym),
            });
        }
        let il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            units,
            Vec::new(),
        );
        (il, func, call)
    }

    #[test]
    fn emits_direct_function_call_target_for_unique_unshadowed_function() {
        let interner = Interner::new();
        let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
        run(&mut il, &interner);
        assert!(direct_function_call_target_at_call(&il, call, func));
    }

    #[test]
    fn does_not_emit_when_local_binder_shadows_function_name() {
        let interner = Interner::new();
        let f = interner.intern("f");
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Name(f), sp(1), &[]);
        let callee = b.add(NodeKind::Var, Payload::Name(f), sp(2), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(3), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(4), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(5), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, sp(6), &[param, body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(7), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root: func,
                kind: UnitKind::Function,
                name: Some(f),
            }],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, func));
    }

    #[test]
    fn does_not_emit_for_duplicate_function_names() {
        let interner = Interner::new();
        let (mut il, func, call) = function_with_call(&interner, "f", "f", true);
        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, func));
    }

    #[test]
    fn does_not_emit_for_method_bare_call() {
        let interner = Interner::new();
        let method_sym = interner.intern("fac");
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(method_sym), sp(20), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(21), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(22), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(23), &[ret]);
        let method = b.add(NodeKind::Func, Payload::None, sp(24), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(25), &[method]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Java,
            },
            vec![Unit {
                root: method,
                kind: UnitKind::Method,
                name: Some(method_sym),
            }],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, method));
    }

    #[test]
    fn does_not_emit_for_nested_function_not_visible_as_top_level() {
        let interner = Interner::new();
        let f = interner.intern("f");
        let mut b = IlBuilder::new(FileId(0));
        let nested_body = b.add(NodeKind::Block, Payload::None, sp(1), &[]);
        let nested = b.add(NodeKind::Func, Payload::None, sp(2), &[nested_body]);
        let callee = b.add(NodeKind::Var, Payload::Name(f), sp(3), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(4), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(5), &[call]);
        let outer_body = b.add(NodeKind::Block, Payload::None, sp(6), &[nested, ret]);
        let outer = b.add(NodeKind::Func, Payload::None, sp(7), &[outer_body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(8), &[outer]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![Unit {
                root: nested,
                kind: UnitKind::Function,
                name: Some(f),
            }],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, nested));
    }

    #[test]
    fn does_not_emit_when_enclosing_scope_binds_function_name() {
        let interner = Interner::new();
        let f = interner.intern("f");
        let g = interner.intern("g");
        let mut b = IlBuilder::new(FileId(0));

        let target_body = b.add(NodeKind::Block, Payload::None, sp(1), &[]);
        let target = b.add(NodeKind::Func, Payload::None, sp(2), &[target_body]);

        let shadow_lhs = b.add(NodeKind::Var, Payload::Name(f), sp(3), &[]);
        let shadow_rhs = b.add(NodeKind::Lit, Payload::LitInt(1), sp(4), &[]);
        let shadow = b.add(
            NodeKind::Assign,
            Payload::None,
            sp(5),
            &[shadow_lhs, shadow_rhs],
        );
        let callee = b.add(NodeKind::Var, Payload::Name(f), sp(6), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(7), &[callee]);
        let inner_ret = b.add(NodeKind::Return, Payload::None, sp(8), &[call]);
        let inner_body = b.add(NodeKind::Block, Payload::None, sp(9), &[inner_ret]);
        let inner = b.add(NodeKind::Func, Payload::None, sp(10), &[inner_body]);
        let outer_body = b.add(NodeKind::Block, Payload::None, sp(11), &[shadow, inner]);
        let outer = b.add(NodeKind::Func, Payload::None, sp(12), &[outer_body]);
        let module = b.add(NodeKind::Module, Payload::None, sp(13), &[target, outer]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            vec![
                Unit {
                    root: target,
                    kind: UnitKind::Function,
                    name: Some(f),
                },
                Unit {
                    root: outer,
                    kind: UnitKind::Function,
                    name: Some(g),
                },
            ],
            Vec::new(),
        );

        run(&mut il, &interner);
        assert!(!direct_function_call_target_at_call(&il, call, target));
    }

    #[test]
    fn emits_imported_function_call_target_from_binding_symbol() {
        let interner = Interner::new();
        let p = interner.intern("p");
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(p), sp(10), &[]);
        let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(11), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(12), &[callee, arg]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(13), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(14), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "p",
            SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
            },
            EvidenceStatus::Asserted,
        ));

        run(&mut il, &interner);

        let expected = CallTargetEvidenceKind::ImportedFunction {
            module_hash: stable_symbol_hash("math"),
            exported_hash: stable_symbol_hash("prod"),
            local_hash: interner.symbol_hash(p),
        };
        assert_eq!(
            call_target_evidence_at_call(&il, &interner, call),
            Some(expected)
        );
        assert!(imported_function_call_target_at_call(&il, &interner, call));
        let target_record = il
            .evidence
            .iter()
            .find(|record| record.kind == EvidenceKind::CallTarget(expected))
            .expect("imported function call-target evidence");
        let [occurrence_dependency] = target_record.dependencies.as_slice() else {
            panic!("call-target should depend on exactly one occurrence symbol");
        };
        let occurrence = il
            .evidence_record_by_id(*occurrence_dependency)
            .expect("occurrence dependency");
        assert_eq!(
            occurrence.kind,
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
            })
        );
        assert_eq!(occurrence.dependencies, vec![EvidenceId(0)]);
    }

    #[test]
    fn emits_imported_member_call_target_from_namespace_symbol() {
        let interner = Interner::new();
        let m = interner.intern("m");
        let prod = interner.intern("prod");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Name(m), sp(10), &[]);
        let callee = b.add(NodeKind::Field, Payload::Name(prod), sp(11), &[receiver]);
        let arg = b.add(NodeKind::Lit, Payload::LitInt(3), sp(12), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(13), &[callee, arg]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(14), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(15), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "m",
            SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash("math"),
            },
            EvidenceStatus::Asserted,
        ));

        run(&mut il, &interner);

        let member_hash = interner.symbol_hash(prod);
        assert_eq!(
            call_target_evidence_at_call(&il, &interner, call),
            Some(CallTargetEvidenceKind::ImportedMember {
                module_hash: stable_symbol_hash("math"),
                exported_hash: member_hash,
                member_hash,
            })
        );
        assert!(imported_member_call_target_at_call(&il, &interner, call));
    }

    #[test]
    fn emits_imported_member_call_target_from_imported_binding_receiver() {
        let interner = Interner::new();
        let map = interner.intern("Map");
        let of = interner.intern("of");
        let mut b = IlBuilder::new(FileId(0));
        let receiver = b.add(NodeKind::Var, Payload::Name(map), sp(10), &[]);
        let callee = b.add(NodeKind::Field, Payload::Name(of), sp(11), &[receiver]);
        let call = b.add(NodeKind::Call, Payload::None, sp(12), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(13), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(14), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Java,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "Map",
            SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("java.util"),
                exported_hash: stable_symbol_hash("Map"),
            },
            EvidenceStatus::Asserted,
        ));

        run(&mut il, &interner);

        assert_eq!(
            call_target_evidence_at_call(&il, &interner, call),
            Some(CallTargetEvidenceKind::ImportedMember {
                module_hash: stable_symbol_hash("java.util"),
                exported_hash: stable_symbol_hash("Map"),
                member_hash: interner.symbol_hash(of),
            })
        );
    }

    #[test]
    fn does_not_emit_imported_function_target_for_ambiguous_binding_symbol() {
        let interner = Interner::new();
        let p = interner.intern("p");
        let mut b = IlBuilder::new(FileId(0));
        let callee = b.add(NodeKind::Var, Payload::Name(p), sp(10), &[]);
        let call = b.add(NodeKind::Call, Payload::None, sp(11), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, sp(12), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, sp(13), &[ret]);
        let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 20), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 30), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "p",
            SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
            },
            EvidenceStatus::Ambiguous,
        ));

        run(&mut il, &interner);
        assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
    }

    #[test]
    fn does_not_emit_imported_function_target_when_local_assignment_rebinds_alias() {
        let interner = Interner::new();
        let p = interner.intern("p");
        let mut b = IlBuilder::new(FileId(0));
        let lhs = b.add(NodeKind::Var, Payload::Name(p), wide_sp(10, 11), &[]);
        let rhs = b.add(NodeKind::Lit, Payload::LitInt(1), wide_sp(12, 13), &[]);
        let assign = b.add(
            NodeKind::Assign,
            Payload::None,
            wide_sp(10, 13),
            &[lhs, rhs],
        );
        let callee = b.add(NodeKind::Var, Payload::Name(p), wide_sp(30, 31), &[]);
        let call = b.add(NodeKind::Call, Payload::None, wide_sp(31, 32), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, wide_sp(31, 33), &[call]);
        let body = b.add(
            NodeKind::Block,
            Payload::None,
            wide_sp(9, 40),
            &[assign, ret],
        );
        let func = b.add(NodeKind::Func, Payload::None, wide_sp(8, 50), &[body]);
        let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 60), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "p",
            SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
            },
            EvidenceStatus::Asserted,
        ));

        run(&mut il, &interner);

        assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
        assert!(!il.evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::node(wide_sp(30, 31), NodeKind::Var)
                && matches!(record.kind, EvidenceKind::Symbol(_))
        }));
    }

    #[test]
    fn does_not_emit_imported_function_target_when_parameter_shadows_alias() {
        let interner = Interner::new();
        let p = interner.intern("p");
        let mut b = IlBuilder::new(FileId(0));
        let param = b.add(NodeKind::Param, Payload::Cid(0), wide_sp(10, 11), &[]);
        let callee = b.add(NodeKind::Var, Payload::Cid(0), wide_sp(30, 31), &[]);
        let call = b.add(NodeKind::Call, Payload::None, wide_sp(31, 32), &[callee]);
        let ret = b.add(NodeKind::Return, Payload::None, wide_sp(31, 33), &[call]);
        let body = b.add(NodeKind::Block, Payload::None, wide_sp(20, 40), &[ret]);
        let func = b.add(
            NodeKind::Func,
            Payload::None,
            wide_sp(8, 50),
            &[param, body],
        );
        let module = b.add(NodeKind::Module, Payload::None, wide_sp(0, 60), &[func]);
        let mut il = b.finish(
            module,
            FileMeta {
                path: "t".into(),
                lang: Lang::Python,
            },
            Vec::new(),
            Vec::new(),
        );
        il.cid_names = vec![p];
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "p",
            SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("math"),
                exported_hash: stable_symbol_hash("prod"),
            },
            EvidenceStatus::Asserted,
        ));

        run(&mut il, &interner);

        assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
        assert!(!il.evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::node(wide_sp(30, 31), NodeKind::Var)
                && matches!(record.kind, EvidenceKind::Symbol(_))
        }));
    }

    #[test]
    fn symbol_evidence_suppresses_direct_function_raw_name_fallback() {
        let interner = Interner::new();
        let (mut il, func, call) = function_with_call(&interner, "f", "f", false);
        il.evidence.push(binding_symbol(
            0,
            sp(1),
            "f",
            SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash("other"),
                exported_hash: stable_symbol_hash("f"),
            },
            EvidenceStatus::Asserted,
        ));

        run(&mut il, &interner);

        assert!(!direct_function_call_target_at_call(&il, call, func));
        assert_eq!(call_target_evidence_at_call(&il, &interner, call), None);
    }
}
