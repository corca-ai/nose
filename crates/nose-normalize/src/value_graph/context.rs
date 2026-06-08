//! File-level value-graph context and immutable module seeding.

use super::*;
use nose_il::UnitKind;

/// File-level facts that are independent of the unit currently being fingerprinted.
///
/// `units::extract` may fingerprint hundreds of block units from the same large file.
/// Function binding proofs require scanning every function and building a
/// literal-sensitive subtree hash for the whole IL, and opaque raw/lambda values need a
/// structural subtree hash for the same file. Doing either once per unit turns a
/// file-level proof into the dominant cost. This context keeps the reusable proof result
/// and lazily shares structural subtree hashes. Each per-unit builder still interns
/// the corresponding lambda values into its own value arena, so value ids never cross
/// builder boundaries.
pub struct ValueFingerprintContext {
    module: ModuleSeedContext,
    function_bindings: Vec<(Symbol, u64)>,
    subtree_hashes: OnceLock<Vec<u64>>,
}

impl ValueFingerprintContext {
    pub fn new(il: &Il, interner: &Interner) -> Self {
        let module = ModuleSeedContext::new(il, interner);
        let subtree_hashes = OnceLock::new();
        let function_bindings = {
            let mut b = Builder::new(il, interner)
                .with_shared_subtree_hashes(&subtree_hashes)
                .with_local_scope_nodes(&module.local_scope);
            b.seed_module_value_bindings_from_context(&module, None);
            b.collect_function_binding_hashes()
        };
        Self {
            module,
            function_bindings,
            subtree_hashes,
        }
    }
}

struct ModuleSeedContext {
    local_scope: Vec<bool>,
    top_level: Vec<NodeId>,
    assignment_counts: FxHashMap<Symbol, usize>,
    assignment_deps: FxHashMap<Symbol, FxHashSet<Symbol>>,
    mutated_bindings: FxHashSet<Symbol>,
    unit_symbols: FxHashSet<Symbol>,
}

impl ModuleSeedContext {
    fn new(il: &Il, interner: &Interner) -> Self {
        let local_scope = local_scope_nodes(il);
        let top_level = top_level_statements_for(il);
        let mut is_top_level = vec![false; il.nodes.len()];
        for &stmt in &top_level {
            if let Some(slot) = is_top_level.get_mut(stmt.0 as usize) {
                *slot = true;
            }
        }

        let mut assignment_counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for &stmt in &top_level {
            if let Some(name) = module_seed_assignment_name(il, stmt, &local_scope) {
                *assignment_counts.entry(name).or_insert(0) += 1;
            }
        }
        let mut assignment_deps: FxHashMap<Symbol, FxHashSet<Symbol>> = FxHashMap::default();
        for &stmt in &top_level {
            let Some(name) = module_seed_assignment_name(il, stmt, &local_scope) else {
                continue;
            };
            if let Some(&rhs) = il.children(stmt).get(1) {
                let mut deps = FxHashSet::default();
                collect_all_node_symbols_in_scope(il, rhs, &local_scope, &mut deps);
                assignment_deps.insert(name, deps);
            }
        }

        let unit_symbols: FxHashSet<Symbol> =
            il.units.iter().filter_map(|unit| unit.name).collect();
        let candidate_names: FxHashSet<Symbol> = assignment_counts
            .iter()
            .filter_map(|(&name, &count)| {
                (count == 1 && !unit_symbols.contains(&name)).then_some(name)
            })
            .collect();
        let direct_definitions: FxHashSet<NodeId> = top_level
            .iter()
            .copied()
            .filter(|&stmt| module_seed_assignment_name(il, stmt, &local_scope).is_some())
            .collect();
        let mutated_bindings = collect_module_mutations_in_scope_with_direct_definitions(
            il,
            interner,
            &candidate_names,
            &is_top_level,
            &local_scope,
            &direct_definitions,
        );

        Self {
            local_scope,
            top_level,
            assignment_counts,
            assignment_deps,
            mutated_bindings,
            unit_symbols,
        }
    }

    fn required_bindings_for(&self, il: &Il, root: NodeId) -> FxHashSet<Symbol> {
        let mut required = FxHashSet::default();
        collect_all_node_symbols_in_scope(il, root, &self.local_scope, &mut required);
        let mut stack: Vec<Symbol> = required.iter().copied().collect();
        while let Some(name) = stack.pop() {
            let Some(deps) = self.assignment_deps.get(&name) else {
                continue;
            };
            for &dep in deps {
                if self.assignment_counts.contains_key(&dep) && required.insert(dep) {
                    stack.push(dep);
                }
            }
        }
        required
    }
}

fn module_seed_assignment_name(il: &Il, stmt: NodeId, local_scope: &[bool]) -> Option<Symbol> {
    assignment_name_in_scope(il, stmt, local_scope)
        .or_else(|| evidence_backed_raw_assignment_name(il, stmt))
}

fn evidence_backed_raw_assignment_name(il: &Il, stmt: NodeId) -> Option<Symbol> {
    let (lhs, rhs) = il.assignment_var_parts(stmt)?;
    let symbol = il.var_name(lhs)?;
    if import_fact_evidence_rhs(il, rhs).is_some()
        || imported_literal_producer_evidence_for_node(il, rhs)
    {
        Some(symbol)
    } else {
        None
    }
}

impl<'a> Builder<'a> {
    pub(super) fn with_shared_subtree_hashes(mut self, hashes: &'a OnceLock<Vec<u64>>) -> Self {
        self.shared_subtree_hashes = Some(hashes);
        self
    }

    pub(super) fn with_local_scope_nodes(mut self, local_scope_nodes: &'a [bool]) -> Self {
        self.local_scope_nodes = Cow::Borrowed(local_scope_nodes);
        self
    }

    pub(super) fn with_context(self, context: &'a ValueFingerprintContext) -> Self {
        self.with_shared_subtree_hashes(&context.subtree_hashes)
            .with_local_scope_nodes(&context.module.local_scope)
    }
}

impl<'a> Builder<'a> {
    pub(super) fn seed_immutable_bindings(
        &mut self,
        root: NodeId,
        context: Option<&ValueFingerprintContext>,
    ) {
        if let Some(context) = context {
            let required = context.module.required_bindings_for(self.il, root);
            self.seed_module_value_bindings_from_context(&context.module, Some(&required));
        } else {
            self.seed_module_value_bindings();
        }
        if let Some(context) = context {
            self.seed_function_binding_hashes(&context.function_bindings);
        } else {
            self.seed_function_bindings();
        }
    }

    pub(super) fn seed_module_value_bindings(&mut self) {
        let mut counts: FxHashMap<Symbol, usize> = FxHashMap::default();
        for stmt in self.top_level_statements() {
            let Some(name) = self.assignment_name(stmt) else {
                continue;
            };
            *counts.entry(name).or_insert(0) += 1;
        }

        let top_level = self.top_level_statements();
        self.seed_module_value_bindings_from_parts(&top_level, &counts, None, None, None);
    }

    fn seed_module_value_bindings_from_context(
        &mut self,
        context: &ModuleSeedContext,
        required_bindings: Option<&FxHashSet<Symbol>>,
    ) {
        self.seed_module_value_bindings_from_parts(
            &context.top_level,
            &context.assignment_counts,
            Some(&context.mutated_bindings),
            Some(&context.unit_symbols),
            required_bindings,
        );
    }

    fn seed_module_value_bindings_from_parts(
        &mut self,
        top_level: &[NodeId],
        counts: &FxHashMap<Symbol, usize>,
        mutated_bindings: Option<&FxHashSet<Symbol>>,
        unit_symbols: Option<&FxHashSet<Symbol>>,
        required_bindings: Option<&FxHashSet<Symbol>>,
    ) {
        let mut env: FxHashMap<u32, ValueId> = FxHashMap::default();
        for &stmt in top_level {
            let kids = self.il.children(stmt);
            if kids.len() != 2 {
                continue;
            }
            let Some(name) = self.assignment_name(stmt) else {
                continue;
            };
            if required_bindings.is_some_and(|required| !required.contains(&name)) {
                continue;
            }
            let unit_defines_symbol = unit_symbols
                .map(|symbols| symbols.contains(&name))
                .unwrap_or_else(|| self.unit_defines_symbol(name));
            if unit_defines_symbol {
                continue;
            }
            if counts.get(&name).copied().unwrap_or(0) != 1 {
                continue;
            }
            let mutated = mutated_bindings
                .map(|bindings| bindings.contains(&name))
                .unwrap_or_else(|| self.module_binding_mutated(name));
            if mutated {
                continue;
            }
            let value = self.eval(kids[1], &env);
            let binding_domain =
                nose_semantics::domain_evidence_for_binding_lhs(self.il, self.interner, kids[0]);
            let value = if self.immutable_binding_safe(kids[1], &env) {
                value
            } else if binding_domain.is_some_and(|domain| domain.is_map()) {
                let Some(proven) = self.proven_map_value(value) else {
                    continue;
                };
                proven
            } else if binding_domain.is_some_and(|domain| domain.is_collection_or_set()) {
                let Some(proven) = self.proven_collection_value(value) else {
                    continue;
                };
                proven
            } else {
                continue;
            };
            if let Payload::Cid(cid) = self.il.node(kids[0]).payload {
                env.insert(cid, value);
            }
            self.global_env.insert(name, value);
        }
    }

    fn top_level_statements(&self) -> Vec<NodeId> {
        top_level_statements_for(self.il)
    }

    fn seed_function_bindings(&mut self) {
        let bindings = self.collect_function_binding_hashes();
        self.seed_function_binding_hashes(&bindings);
    }

    fn collect_function_binding_hashes(&mut self) -> Vec<(Symbol, u64)> {
        let mut bindings = Vec::new();
        for unit in self.il.units.clone() {
            if !matches!(unit.kind, UnitKind::Function | UnitKind::Method) {
                continue;
            }
            let Some(name) = unit.name else {
                continue;
            };
            if self.function_binding_safe(unit.root, unit.root) {
                let hash = self.valued_subtree_hash(unit.root);
                bindings.push((name, hash));
            }
        }
        bindings
    }

    fn seed_function_binding_hashes(&mut self, bindings: &[(Symbol, u64)]) {
        for &(name, hash) in bindings {
            let value = self.mk(ValOp::Lambda(hash), vec![]);
            self.global_env.insert(name, value);
        }
    }

    fn assignment_name(&self, stmt: NodeId) -> Option<Symbol> {
        module_seed_assignment_name(self.il, stmt, &self.local_scope_nodes)
    }

    pub(super) fn unit_defines_symbol(&self, symbol: Symbol) -> bool {
        self.il
            .units
            .iter()
            .any(|unit| unit.name.is_some_and(|name| name == symbol))
    }

    pub(super) fn module_binding_mutated(&self, name: Symbol) -> bool {
        let top_level = self.top_level_statements();
        let shadowed = shadowed_js_like_module_binding_nodes_for_symbol_in_scope(
            self.il,
            name,
            &self.local_scope_nodes,
        );
        self.il.nodes.iter().enumerate().any(|(idx, node)| {
            let node_id = NodeId(idx as u32);
            if shadowed.contains(&node_id) {
                return false;
            }
            match node.kind {
                NodeKind::Call => self.call_mutates_binding(node_id, name).unwrap_or(false),
                NodeKind::Assign if !top_level.contains(&node_id) => self
                    .assignment_mutates_binding(node_id, name)
                    .unwrap_or(false),
                _ => false,
            }
        })
    }

    fn assignment_mutates_binding(&self, assign: NodeId, name: Symbol) -> Option<bool> {
        let lhs = binding_write_target(self.il, assign)?;
        Some(self.node_contains_symbol(lhs, name))
    }

    fn call_mutates_binding(&self, call: NodeId, name: Symbol) -> Option<bool> {
        if let Some(receiver) = receiver_mutation_call_receiver(self.il, self.interner, call) {
            return Some(self.node_refers_to_symbol(receiver, name));
        }
        if let Some(args) = opaque_argument_escape_args(self.il, call) {
            return Some(args.iter().any(|&arg| self.node_contains_symbol(arg, name)));
        }
        Some(false)
    }

    fn node_refers_to_symbol(&self, node: NodeId, name: Symbol) -> bool {
        self.node_symbol(node).is_some_and(|symbol| symbol == name)
    }

    fn node_symbol(&self, node: NodeId) -> Option<Symbol> {
        node_symbol_in_scope(self.il, node, &self.local_scope_nodes)
    }

    fn node_contains_symbol(&self, node: NodeId, name: Symbol) -> bool {
        self.node_refers_to_symbol(node, name)
            || self
                .il
                .children(node)
                .iter()
                .any(|&child| self.node_contains_symbol(child, name))
    }

    pub(super) fn node_refers_to_cid(&self, node: NodeId, cid: u32) -> bool {
        matches!(self.il.node(node).payload, Payload::Cid(current) if current == cid)
    }

    pub(super) fn node_contains_cid(&self, node: NodeId, cid: u32) -> bool {
        self.node_refers_to_cid(node, cid)
            || self
                .il
                .children(node)
                .iter()
                .any(|&child| self.node_contains_cid(child, cid))
    }

    fn immutable_binding_safe(&self, node: NodeId, env: &FxHashMap<u32, ValueId>) -> bool {
        match self.il.kind(node) {
            NodeKind::Raw
            | NodeKind::Call
            | NodeKind::HoF
            | NodeKind::Func
            | NodeKind::Lambda
            | NodeKind::Loop
            | NodeKind::Try
            | NodeKind::Throw
            | NodeKind::Assign => false,
            NodeKind::Var => match self.il.node(node).payload {
                Payload::Cid(c) => env.contains_key(&c),
                Payload::Name(s) => self.global_env.contains_key(&s),
                _ => false,
            },
            NodeKind::Lit => matches!(
                self.il.node(node).payload,
                Payload::LitInt(_)
                    | Payload::LitBool(_)
                    | Payload::LitStr(_)
                    | Payload::LitFloat(_)
                    | Payload::Lit(nose_il::LitClass::Null)
            ),
            _ => self
                .il
                .children(node)
                .iter()
                .all(|&c| self.immutable_binding_safe(c, env)),
        }
    }
}
