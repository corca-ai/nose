//! Builder construction and initial mutable state.

use super::*;

impl<'a> Builder<'a> {
    pub(super) fn new(il: &'a Il, interner: &'a Interner) -> Self {
        Self::new_with_local_scope_nodes(il, interner, Cow::Owned(local_scope_nodes(il)))
    }

    pub(super) fn new_with_local_scope_nodes(
        il: &'a Il,
        interner: &'a Interner,
        local_scope_nodes: Cow<'a, [bool]>,
    ) -> Self {
        Builder {
            il,
            interner,
            nodes: Vec::new(),
            vhash: Vec::new(),
            node_span: Vec::new(),
            cur_span: None,
            source_salt_used: false,
            review_source_ids: None,
            cur_il_kind: None,
            await_transparent: true,
            async_protocol_depth: 0,
            opaque_census: None,
            intern: FxHashMap::default(),
            sinks: Vec::new(),
            opaque_ctr: 0,
            field_env: FxHashMap::default(),
            index_env: FxHashMap::default(),
            subtree_hash: None,
            shared_subtree_hashes: None,
            shared_valued_subtree_hashes: None,
            valued_subtree_hash: None,
            vty: Vec::new(),
            reorder_safe_cache: FxHashMap::default(),
            non_concat_cache: FxHashMap::default(),
            possibly_float_cache: FxHashMap::default(),
            param_ty: Vec::new(),
            param_domain: FxHashMap::default(),
            path: Vec::new(),
            bound_order_facts: Vec::new(),
            effect_slot: 0,
            building: FxHashMap::default(),
            building_kind: FxHashMap::default(),
            global_env: FxHashMap::default(),
            inline_candidates: None,
            inline_exclude_root: None,
            inline_env_keys: FxHashSet::default(),
            inline_stack: Vec::new(),
            inline_capture: Vec::new(),
            loop_depth: 0,
            local_scope_nodes,
            loop_recurrence: None,
            next_loop_key_base: 0,
            contracts: Vec::new(),
            value_laws: Vec::new(),
            clamp_candidate_count: 0,
            clamp_proof_backed_candidate_count: 0,
        }
    }
}
