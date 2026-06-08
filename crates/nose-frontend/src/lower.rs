//! Shared lowering context and helpers used by every per-language frontend.
//! Language-specific walks build IL through this, so the arena/span/intern
//! mechanics live in one place.

use nose_il::{
    stable_symbol_hash, Builtin, DomainEvidence, EffectEvidenceKind, EvidenceAnchor,
    EvidenceEmitter, EvidenceId, EvidenceKind, EvidenceProvenance, EvidenceRecord, EvidenceStatus,
    FileId, FileMeta, Il, IlBuilder, ImportEvidenceKind, Interner, Lang, LibraryApiEvidenceKind,
    LitClass, LoopKind, NodeId, NodeKind, Op, ParamSemantic, Payload, PlaceEvidenceKind,
    SequenceSurfaceKind, SourceCallKind, SourceFactKind, Span, Symbol, SymbolEvidenceKind, Unit,
    UnitKind,
};
use nose_semantics::{
    library_api_callee_contract_hash, library_api_contract_id_hash,
    library_api_free_name_shadow_safe, library_api_property_dependencies_for_field_with_cache,
    library_api_receiver_dependencies_for_call_with_cache,
    library_collection_factory_result_domain_for_arity, library_free_function_builtin_contract,
    library_free_name_collection_factory_contract, library_free_name_map_factory_contract,
    library_imported_collection_factory_contracts, library_imported_namespace_function_contract,
    library_java_collection_constructor_contract, library_java_collection_factory_contract,
    library_java_map_entry_contract, library_java_map_factory_contract,
    library_js_array_is_array_contract, library_js_boolean_coercion_contract,
    library_js_like_map_constructor_contract, library_js_like_set_constructor_contract,
    library_map_factory_result_domain, library_map_key_view_wrapper_contract,
    library_map_key_view_wrapper_result_domain, library_property_builtin_contract,
    library_receiver_method_api_contract, library_regex_test_contract,
    library_ruby_set_factory_contract, library_rust_option_none_sentinel_contract,
    library_rust_option_some_constructor_contract, library_rust_vec_macro_factory_contract,
    library_rust_vec_new_factory_contract, library_static_collection_adapter_contract,
    library_static_index_membership_contract, sequence_surface_kind_for_tag, ImportFactKind,
    LibraryApiCalleeContract, LibraryApiContractId, LibraryApiDependencyCache,
    MethodReceiverContract, StaticIndexMembershipReceiverContract,
};
use tree_sitter::Node as TsNode;

struct LibraryApiEvidencePlan {
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    dependencies: Vec<EvidenceId>,
    rule: &'static str,
    result_domain: Option<DomainEvidence>,
}

/// Mutable state threaded through a single file's lowering.
pub(crate) struct Lowering<'a> {
    pub b: IlBuilder,
    pub src: &'a [u8],
    pub lang: Lang,
    pub interner: &'a Interner,
    pub units: Vec<Unit>,
    pub evidence: Vec<EvidenceRecord>,
    pub param_semantic_aliases: Vec<(String, ParamSemantic)>,
    pub unsigned_32_aliases: Vec<String>,
}

impl<'a> Lowering<'a> {
    pub(crate) fn new(file: FileId, src: &'a [u8], lang: Lang, interner: &'a Interner) -> Self {
        Lowering {
            b: IlBuilder::new(file),
            src,
            lang,
            interner,
            units: Vec::new(),
            evidence: Vec::new(),
            param_semantic_aliases: Vec::new(),
            unsigned_32_aliases: Vec::new(),
        }
    }

    /// Source text covered by a CST node.
    pub(crate) fn text(&self, n: TsNode) -> &'a str {
        n.utf8_text(self.src).unwrap_or("")
    }

    pub(crate) fn sym(&self, s: &str) -> Symbol {
        self.interner.intern(s)
    }

    /// Build a [`Span`] from a CST node (1-based inclusive lines).
    pub(crate) fn span(&self, n: TsNode) -> Span {
        Span::new(
            self.b.file(),
            n.start_byte() as u32,
            n.end_byte() as u32,
            n.start_position().row as u32 + 1,
            n.end_position().row as u32 + 1,
        )
    }

    pub(crate) fn add(
        &mut self,
        kind: NodeKind,
        payload: Payload,
        span: Span,
        children: &[NodeId],
    ) -> NodeId {
        let id = self.b.add(kind, payload, span, children);
        self.record_core_semantic_evidence(kind, payload, span, children);
        if kind == NodeKind::Seq {
            let tag = match payload {
                Payload::None => None,
                Payload::Name(symbol) => Some(self.interner.resolve(symbol)),
                _ => return id,
            };
            if let Some(surface) = sequence_surface_kind_for_tag(self.lang, tag) {
                self.record_evidence(
                    EvidenceAnchor::sequence(span),
                    EvidenceKind::SequenceSurface(surface),
                    "sequence_surface",
                );
            }
        }
        id
    }

    fn record_core_semantic_evidence(
        &mut self,
        kind: NodeKind,
        payload: Payload,
        span: Span,
        children: &[NodeId],
    ) {
        match kind {
            NodeKind::Var if self.lang == Lang::Java => {
                if matches!(payload, Payload::Name(name) if self.interner.resolve(name) == "this") {
                    self.record_evidence(
                        EvidenceAnchor::node(span, kind),
                        EvidenceKind::Place(PlaceEvidenceKind::SelfReceiver),
                        "place_self_receiver",
                    );
                }
            }
            NodeKind::Call => {
                if matches!(payload, Payload::Builtin(Builtin::Append)) && children.len() == 2 {
                    self.record_evidence(
                        EvidenceAnchor::node(span, kind),
                        EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall),
                        "effect_builder_append",
                    );
                }
                if matches!(payload, Payload::None) {
                    self.record_library_api_evidence_for_call(span, children);
                }
            }
            NodeKind::Field if self.lang == Lang::Java => {
                if let (Payload::Name(field), [receiver]) = (payload, children) {
                    if let Some(receiver_evidence) = self.self_receiver_evidence_id(*receiver) {
                        let field_hash = stable_symbol_hash(self.interner.resolve(field));
                        self.record_evidence_with_dependencies(
                            EvidenceAnchor::node(span, kind),
                            EvidenceKind::Place(PlaceEvidenceKind::SelfField { field_hash }),
                            "place_self_field",
                            vec![receiver_evidence],
                        );
                    }
                }
            }
            NodeKind::Assign => {
                if let [target, _value] = children {
                    if self.non_overloadable_index_assignment_target(*target) {
                        self.record_evidence(
                            EvidenceAnchor::node(span, kind),
                            EvidenceKind::Effect(EffectEvidenceKind::NonOverloadableIndexWrite),
                            "effect_non_overloadable_index_write",
                        );
                    } else if let Some((field_hash, place_evidence)) =
                        self.self_field_assignment_target(*target)
                    {
                        self.record_evidence_with_dependencies(
                            EvidenceAnchor::node(span, kind),
                            EvidenceKind::Effect(EffectEvidenceKind::SelfFieldWrite { field_hash }),
                            "effect_self_field_write",
                            vec![place_evidence],
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn record_library_api_evidence_for_call(&mut self, span: Span, children: &[NodeId]) {
        let Some((&callee, args)) = children.split_first() else {
            return;
        };
        let arg_count = args.len();
        if let Some(plan) = self.library_api_contract_for_call(span, callee, arg_count) {
            let api = self.record_evidence_with_dependencies(
                EvidenceAnchor::node(span, NodeKind::Call),
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: library_api_contract_id_hash(plan.id),
                    callee_hash: library_api_callee_contract_hash(plan.callee),
                    arity: arg_count as u16,
                }),
                plan.rule,
                plan.dependencies,
            );
            self.record_library_api_result_domain(span, plan.result_domain, api);
        }
    }

    fn record_library_api_result_domain(
        &mut self,
        span: Span,
        result_domain: Option<DomainEvidence>,
        api: EvidenceId,
    ) {
        if let Some(domain) = result_domain {
            self.record_evidence_with_dependencies(
                EvidenceAnchor::node(span, NodeKind::Call),
                EvidenceKind::Domain(domain),
                "library_api_result_domain",
                vec![api],
            );
        }
    }

    fn library_api_contract_for_call(
        &mut self,
        span: Span,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        if arg_count > u16::MAX as usize {
            return None;
        }
        if let Some(result) = self.static_global_method_api_contract(callee, arg_count) {
            return Some(result);
        }
        if let Some(result) = self.static_global_function_api_contract(callee, arg_count) {
            return Some(result);
        }
        if let Some(result) = self.imported_collection_factory_api_contract(callee, arg_count) {
            return Some(result);
        }
        if let Some(result) = self.imported_namespace_function_api_contract(callee, arg_count) {
            return Some(result);
        }
        if let Some(result) = self.java_util_static_member_api_contract(callee, arg_count) {
            return Some(result);
        }
        if let Some(result) = self.static_index_membership_api_contract(callee, arg_count) {
            return Some(result);
        }
        if let Some(result) = self.regex_literal_method_api_contract(callee, arg_count) {
            return Some(result);
        }
        self.js_global_constructor_api_contract(span, callee, arg_count)
    }

    fn static_global_method_api_contract(
        &self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let (receiver_node, receiver, method) = self.static_member_callee(callee)?;
        let contract = library_js_array_is_array_contract(self.lang, receiver, method, arg_count)
            .map(|contract| {
                (
                    contract.id,
                    contract.callee,
                    contract.result.qualified_path,
                    contract.result.requires_unshadowed_receiver,
                    contract.result.receiver,
                    "library_api_js_array_is_array",
                    None,
                )
            })
            .or_else(|| {
                library_map_key_view_wrapper_contract(self.lang, receiver, method, arg_count).map(
                    |contract| {
                        (
                            contract.id,
                            contract.callee,
                            contract.result.qualified_path,
                            true,
                            contract.result.receiver,
                            "library_api_map_key_view_wrapper",
                            Some(library_map_key_view_wrapper_result_domain(contract)),
                        )
                    },
                )
            })?;
        let qualified = self.qualified_global_evidence_id(callee, contract.2)?;
        let mut dependencies = vec![qualified];
        if contract.3 {
            dependencies.push(self.unshadowed_global_evidence_id(receiver_node, contract.4)?);
        }
        Some(LibraryApiEvidencePlan {
            id: contract.0,
            callee: contract.1,
            dependencies,
            rule: contract.5,
            result_domain: contract.6,
        })
    }

    fn static_member_callee(&self, callee: NodeId) -> Option<(NodeId, &str, &str)> {
        if self.b.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = self.b.payload(callee) else {
            return None;
        };
        let receiver_node = self.b.children(callee).first().copied()?;
        if self.b.kind(receiver_node) != NodeKind::Var {
            return None;
        }
        let Payload::Name(receiver_name) = self.b.payload(receiver_node) else {
            return None;
        };
        Some((
            receiver_node,
            self.interner.resolve(receiver_name),
            self.interner.resolve(method),
        ))
    }

    fn field_callee_receiver_and_method(&self, callee: NodeId) -> Option<(NodeId, &str)> {
        if self.b.kind(callee) != NodeKind::Field {
            return None;
        }
        let Payload::Name(method) = self.b.payload(callee) else {
            return None;
        };
        Some((
            self.b.children(callee).first().copied()?,
            self.interner.resolve(method),
        ))
    }

    fn static_global_function_api_contract(
        &self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let Payload::Name(function) = self.b.payload(callee) else {
            return None;
        };
        if self.b.kind(callee) != NodeKind::Var {
            return None;
        }
        let function = self.interner.resolve(function);
        let contract = library_js_boolean_coercion_contract(self.lang, function, arg_count)?;
        let mut dependencies = Vec::new();
        if contract.result.requires_unshadowed_function {
            dependencies
                .push(self.unshadowed_global_evidence_id(callee, contract.result.function)?);
        }
        Some(LibraryApiEvidencePlan {
            id: contract.id,
            callee: contract.callee,
            dependencies,
            rule: "library_api_js_boolean_coercion",
            result_domain: None,
        })
    }

    fn imported_collection_factory_api_contract(
        &mut self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        if arg_count != 1 {
            return None;
        }
        match self.b.kind(callee) {
            NodeKind::Var => {
                library_imported_collection_factory_contracts(self.lang).find_map(|contract| {
                    let LibraryApiCalleeContract::ImportedBinding {
                        module,
                        exported: expected,
                    } = contract.callee
                    else {
                        return None;
                    };
                    let dependency =
                        self.record_imported_binding_symbol_for_node(callee, module, expected)?;
                    Some(LibraryApiEvidencePlan {
                        id: contract.id,
                        callee: contract.callee,
                        dependencies: vec![dependency],
                        rule: "library_api_imported_collection_factory",
                        result_domain: library_collection_factory_result_domain_for_arity(
                            contract, arg_count,
                        ),
                    })
                })
            }
            NodeKind::Field => {
                let Payload::Name(method) = self.b.payload(callee) else {
                    return None;
                };
                let method = self.interner.resolve(method);
                let receiver = self.b.children(callee).first().copied()?;
                library_imported_collection_factory_contracts(self.lang).find_map(|contract| {
                    let LibraryApiCalleeContract::ImportedBinding { module, exported } =
                        contract.callee
                    else {
                        return None;
                    };
                    if method != exported {
                        return None;
                    }
                    let dependency =
                        self.record_imported_namespace_symbol_for_node(receiver, module)?;
                    Some(LibraryApiEvidencePlan {
                        id: contract.id,
                        callee: contract.callee,
                        dependencies: vec![dependency],
                        rule: "library_api_imported_collection_factory",
                        result_domain: library_collection_factory_result_domain_for_arity(
                            contract, arg_count,
                        ),
                    })
                })
            }
            _ => None,
        }
    }

    fn imported_namespace_function_api_contract(
        &mut self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let (receiver, function) = self.field_callee_receiver_and_method(callee)?;
        let contract =
            library_imported_namespace_function_contract(self.lang, function, arg_count)?;
        let LibraryApiCalleeContract::ImportedNamespaceFunction { module, .. } = contract.callee
        else {
            return None;
        };
        let dependency = self.record_imported_namespace_symbol_for_node(receiver, module)?;
        Some(LibraryApiEvidencePlan {
            id: contract.id,
            callee: contract.callee,
            dependencies: vec![dependency],
            rule: "library_api_imported_namespace_function",
            result_domain: None,
        })
    }

    fn java_util_static_member_api_contract(
        &mut self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let (receiver_node, receiver, method) = self.static_member_callee(callee)?;
        let (id, callee_contract, rule, result_domain) =
            library_java_collection_factory_contract(self.lang, receiver, method)
                .map(|contract| {
                    (
                        contract.id,
                        contract.callee,
                        "library_api_java_collection_factory",
                        library_collection_factory_result_domain_for_arity(contract, arg_count),
                    )
                })
                .or_else(|| {
                    library_java_map_factory_contract(self.lang, receiver, method).map(|contract| {
                        (
                            contract.id,
                            contract.callee,
                            "library_api_java_map_factory",
                            Some(library_map_factory_result_domain(contract)),
                        )
                    })
                })
                .or_else(|| {
                    (arg_count == 2)
                        .then(|| library_java_map_entry_contract(self.lang, receiver, method))
                        .flatten()
                        .map(|contract| {
                            (
                                contract.id,
                                contract.callee,
                                "library_api_java_map_entry_factory",
                                None,
                            )
                        })
                })
                .or_else(|| {
                    library_static_collection_adapter_contract(
                        self.lang, receiver, method, arg_count,
                    )
                    .map(|contract| {
                        (
                            contract.id,
                            contract.callee,
                            "library_api_java_static_collection_adapter",
                            None,
                        )
                    })
                })?;
        self.java_util_static_member_evidence_plan(
            receiver_node,
            id,
            callee_contract,
            rule,
            result_domain,
        )
    }

    fn java_util_static_member_evidence_plan(
        &mut self,
        receiver_node: NodeId,
        id: LibraryApiContractId,
        callee_contract: LibraryApiCalleeContract,
        rule: &'static str,
        result_domain: Option<DomainEvidence>,
    ) -> Option<LibraryApiEvidencePlan> {
        let LibraryApiCalleeContract::JavaUtilStaticMember {
            receiver: expected_receiver,
            ..
        } = callee_contract
        else {
            return None;
        };
        let dependency = self.record_imported_binding_symbol_for_node(
            receiver_node,
            "java.util",
            expected_receiver,
        )?;
        Some(LibraryApiEvidencePlan {
            id,
            callee: callee_contract,
            dependencies: vec![dependency],
            rule,
            result_domain,
        })
    }

    fn static_index_membership_api_contract(
        &self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let (receiver_node, method) = self.field_callee_receiver_and_method(callee)?;
        let contract = library_static_index_membership_contract(self.lang, method, arg_count)?;
        let LibraryApiCalleeContract::StaticIndexMembershipMethod { receiver, .. } =
            contract.callee
        else {
            return None;
        };
        let dependency =
            self.static_index_membership_receiver_dependency(receiver_node, receiver)?;
        Some(LibraryApiEvidencePlan {
            id: contract.id,
            callee: contract.callee,
            dependencies: vec![dependency],
            rule: "library_api_static_index_membership",
            result_domain: None,
        })
    }

    fn static_index_membership_receiver_dependency(
        &self,
        receiver: NodeId,
        contract: StaticIndexMembershipReceiverContract,
    ) -> Option<EvidenceId> {
        match contract {
            StaticIndexMembershipReceiverContract::StaticNonFloatLiteralCollection => {
                if !self.static_non_float_collection_literal(receiver) {
                    return None;
                }
                self.sequence_surface_evidence_id(receiver, SequenceSurfaceKind::Collection)
            }
        }
    }

    fn static_non_float_collection_literal(&self, node: NodeId) -> bool {
        if self.b.kind(node) != NodeKind::Seq {
            return false;
        }
        let Payload::Name(tag) = self.b.payload(node) else {
            return false;
        };
        if sequence_surface_kind_for_tag(self.lang, Some(self.interner.resolve(tag)))
            != Some(SequenceSurfaceKind::Collection)
        {
            return false;
        }
        let kids = self.b.children(node);
        !kids.is_empty()
            && kids.iter().all(|&kid| {
                self.b.kind(kid) == NodeKind::Lit
                    && matches!(
                        self.b.payload(kid),
                        Payload::LitInt(_)
                            | Payload::LitBool(_)
                            | Payload::LitStr(_)
                            | Payload::Lit(LitClass::Null)
                    )
            })
    }

    fn sequence_surface_evidence_id(
        &self,
        node: NodeId,
        surface: SequenceSurfaceKind,
    ) -> Option<EvidenceId> {
        let span = self.b.node(node).span;
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::sequence(span)
                && record.kind == EvidenceKind::SequenceSurface(surface)
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn regex_literal_method_api_contract(
        &self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let (receiver, method) = self.field_callee_receiver_and_method(callee)?;
        let contract = library_regex_test_contract(self.lang, method, arg_count)?;
        let LibraryApiCalleeContract::RegexLiteralMethod {
            required_receiver_fact,
            ..
        } = contract.callee
        else {
            return None;
        };
        let dependency = self.source_fact_evidence_id(receiver, required_receiver_fact)?;
        Some(LibraryApiEvidencePlan {
            id: contract.id,
            callee: contract.callee,
            dependencies: vec![dependency],
            rule: "library_api_regex_literal_method",
            result_domain: None,
        })
    }

    fn js_global_constructor_api_contract(
        &self,
        span: Span,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let Payload::Name(receiver) = self.b.payload(callee) else {
            return None;
        };
        if self.b.kind(callee) != NodeKind::Var {
            return None;
        }
        let receiver = self.interner.resolve(receiver);
        let contract = library_js_like_set_constructor_contract(self.lang, receiver)
            .filter(|_| arg_count == 1)
            .map(|contract| {
                (
                    contract.id,
                    contract.callee,
                    receiver,
                    "library_api_js_set_constructor",
                    library_collection_factory_result_domain_for_arity(contract, arg_count),
                )
            })
            .or_else(|| {
                library_js_like_map_constructor_contract(self.lang, receiver)
                    .filter(|_| arg_count == 1)
                    .map(|contract| {
                        (
                            contract.id,
                            contract.callee,
                            receiver,
                            "library_api_js_map_constructor",
                            Some(library_map_factory_result_domain(contract)),
                        )
                    })
            })?;
        let source = self.source_call_evidence_id(span, SourceCallKind::Construct)?;
        let mut dependencies = vec![source];
        if let LibraryApiCalleeContract::JsGlobalConstructor {
            requires_unshadowed_global,
            ..
        } = contract.1
        {
            if requires_unshadowed_global {
                dependencies.push(self.unshadowed_global_evidence_id(callee, contract.2)?);
            }
        }
        Some(LibraryApiEvidencePlan {
            id: contract.0,
            callee: contract.1,
            dependencies,
            rule: contract.3,
            result_domain: contract.4,
        })
    }

    fn source_fact_evidence_id(
        &self,
        node: NodeId,
        expected: SourceFactKind,
    ) -> Option<EvidenceId> {
        let span = self.b.node(node).span;
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::source_span(span)
                && record.kind == EvidenceKind::Source(expected)
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn source_call_evidence_id(&self, span: Span, call: SourceCallKind) -> Option<EvidenceId> {
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::source_span(span)
                && record.kind == EvidenceKind::Source(SourceFactKind::Call(call))
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn record_imported_binding_symbol_for_node(
        &mut self,
        node: NodeId,
        module: &str,
        exported: &str,
    ) -> Option<EvidenceId> {
        let expected = SymbolEvidenceKind::ImportedBinding {
            module_hash: stable_symbol_hash(module),
            exported_hash: stable_symbol_hash(exported),
        };
        let dependency = self.binding_symbol_evidence_id(node, expected)?;
        Some(self.record_evidence_with_dependencies(
            EvidenceAnchor::node(self.b.node(node).span, NodeKind::Var),
            EvidenceKind::Symbol(expected),
            "symbol_imported_binding_occurrence",
            vec![dependency],
        ))
    }

    fn record_imported_namespace_symbol_for_node(
        &mut self,
        node: NodeId,
        module: &str,
    ) -> Option<EvidenceId> {
        let expected = SymbolEvidenceKind::ImportedNamespace {
            module_hash: stable_symbol_hash(module),
        };
        let dependency = self.binding_symbol_evidence_id(node, expected)?;
        Some(self.record_evidence_with_dependencies(
            EvidenceAnchor::node(self.b.node(node).span, NodeKind::Var),
            EvidenceKind::Symbol(expected),
            "symbol_imported_namespace_occurrence",
            vec![dependency],
        ))
    }

    fn binding_symbol_evidence_id(
        &self,
        node: NodeId,
        expected: SymbolEvidenceKind,
    ) -> Option<EvidenceId> {
        if self.b.kind(node) != NodeKind::Var {
            return None;
        }
        let Payload::Name(local) = self.b.payload(node) else {
            return None;
        };
        let local_hash = stable_symbol_hash(self.interner.resolve(local));
        self.evidence.iter().find_map(|record| {
            (matches!(
                record.anchor,
                EvidenceAnchor::Binding {
                    local_hash: anchor_hash,
                    ..
                } if anchor_hash == local_hash
            ) && record.kind == EvidenceKind::Symbol(expected)
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn unshadowed_global_evidence_id(&self, node: NodeId, name: &str) -> Option<EvidenceId> {
        let span = self.b.node(node).span;
        let kind = self.b.kind(node);
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::node(span, kind)
                && record.kind
                    == EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                        name_hash: stable_symbol_hash(name),
                    })
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn qualified_global_evidence_id(&self, node: NodeId, path: &str) -> Option<EvidenceId> {
        let span = self.b.node(node).span;
        let kind = self.b.kind(node);
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::node(span, kind)
                && record.kind
                    == EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                        path_hash: stable_symbol_hash(path),
                    })
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn node_is_java_this_var(&self, node: NodeId) -> bool {
        self.lang == Lang::Java
            && self.b.kind(node) == NodeKind::Var
            && matches!(self.b.payload(node), Payload::Name(name) if self.interner.resolve(name) == "this")
    }

    fn self_receiver_evidence_id(&self, node: NodeId) -> Option<EvidenceId> {
        if !self.node_is_java_this_var(node) {
            return None;
        }
        let span = self.b.node(node).span;
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::node(span, NodeKind::Var)
                && record.kind == EvidenceKind::Place(PlaceEvidenceKind::SelfReceiver)
                && record.status == EvidenceStatus::Asserted)
                .then_some(record.id)
        })
    }

    fn non_overloadable_index_assignment_target(&self, node: NodeId) -> bool {
        matches!(self.lang, Lang::C | Lang::Go | Lang::Java) && self.b.kind(node) == NodeKind::Index
    }

    fn self_field_assignment_target(&self, node: NodeId) -> Option<(u64, EvidenceId)> {
        if self.lang != Lang::Java || self.b.kind(node) != NodeKind::Field {
            return None;
        }
        let Payload::Name(field) = self.b.payload(node) else {
            return None;
        };
        let span = self.b.node(node).span;
        let field_hash = stable_symbol_hash(self.interner.resolve(field));
        self.evidence.iter().find_map(|record| {
            (record.anchor == EvidenceAnchor::node(span, NodeKind::Field)
                && record.kind == EvidenceKind::Place(PlaceEvidenceKind::SelfField { field_hash })
                && record.status == EvidenceStatus::Asserted)
                .then_some((field_hash, record.id))
        })
    }

    pub(crate) fn record_param_semantic(&mut self, span: Span, semantic: ParamSemantic) {
        self.record_evidence(
            EvidenceAnchor::param(span),
            EvidenceKind::Domain(DomainEvidence::from_param_semantic(semantic)),
            "param_semantic",
        );
    }

    pub(crate) fn record_source_fact(&mut self, span: Span, kind: SourceFactKind) {
        self.record_evidence(
            EvidenceAnchor::source_span(span),
            EvidenceKind::Source(kind),
            "source_fact",
        );
    }

    pub(crate) fn record_evidence(
        &mut self,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        rule: &str,
    ) -> EvidenceId {
        self.record_evidence_with_dependencies(anchor, kind, rule, Vec::new())
    }

    pub(crate) fn record_evidence_with_dependencies(
        &mut self,
        anchor: EvidenceAnchor,
        kind: EvidenceKind,
        rule: &str,
        dependencies: Vec<EvidenceId>,
    ) -> EvidenceId {
        let id = EvidenceId(self.evidence.len() as u32);
        self.evidence.push(EvidenceRecord {
            id,
            anchor,
            kind,
            provenance: EvidenceProvenance {
                emitter: EvidenceEmitter::FirstParty,
                pack_hash: Some(stable_symbol_hash(nose_semantics::FIRST_PARTY_PACK_ID)),
                rule_hash: Some(stable_symbol_hash(rule)),
            },
            dependencies,
            status: EvidenceStatus::Asserted,
        });
        id
    }

    pub(crate) fn record_param_semantic_alias(&mut self, local: &str, semantic: ParamSemantic) {
        let alias = normalize_type_text(local);
        if alias.is_empty() {
            return;
        }
        if let Some((_, existing)) = self
            .param_semantic_aliases
            .iter_mut()
            .find(|(known, _)| known == &alias)
        {
            *existing = semantic;
            return;
        }
        self.param_semantic_aliases.push((alias, semantic));
    }

    pub(crate) fn clear_param_semantic_alias(&mut self, local: &str) {
        let alias = normalize_type_text(local);
        if alias.is_empty() {
            return;
        }
        self.param_semantic_aliases
            .retain(|(known, _)| known != &alias);
    }

    pub(crate) fn record_unsigned_32_alias(&mut self, local: &str) {
        let alias = normalize_type_text(local);
        if alias.is_empty() || self.unsigned_32_aliases.iter().any(|known| known == &alias) {
            return;
        }
        self.unsigned_32_aliases.push(alias);
    }

    pub(crate) fn param_semantic_from_text(&self, text: &str) -> Option<ParamSemantic> {
        param_semantic_from_text(text).or_else(|| {
            let t = normalize_type_text(text);
            self.param_semantic_aliases
                .iter()
                .find_map(|(alias, semantic)| {
                    (t.contains(&format!(":{alias}[")) || t.contains(&format!(":{alias}<")))
                        .then_some(*semantic)
                })
        })
    }

    /// An empty `Block` (used for absent loop init/update slots, empty bodies).
    pub(crate) fn empty_block(&mut self, span: Span) -> NodeId {
        self.b.add(NodeKind::Block, Payload::None, span, &[])
    }

    /// Wrap a single lowered statement in a one-child `Block`, or yield an empty block when
    /// the statement lowered to nothing. This is the shared tail of every frontend's
    /// `stmt_as_block` helper (which differ only in their language's block-node kind and
    /// `lower_stmt`); centralizing it keeps the absent-statement fallback uniform.
    pub(crate) fn block_of_stmt(&mut self, span: Span, stmt: Option<NodeId>) -> NodeId {
        match stmt {
            Some(s) => self.b.add(NodeKind::Block, Payload::None, span, &[s]),
            None => self.empty_block(span),
        }
    }

    /// A `Var` carrying the raw identifier name (canonicalized later).
    pub(crate) fn var(&mut self, name: &str, span: Span) -> NodeId {
        let sym = self.sym(name);
        self.b.add(NodeKind::Var, Payload::Name(sym), span, &[])
    }

    /// A `Var` proven by the frontend to denote a language-defined unshadowed
    /// global symbol at this source occurrence.
    pub(crate) fn unshadowed_global_var(&mut self, name: &str, span: Span) -> NodeId {
        let var = self.var(name, span);
        self.record_evidence(
            EvidenceAnchor::node(span, NodeKind::Var),
            EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
                name_hash: stable_symbol_hash(name),
            }),
            "symbol_unshadowed_global",
        );
        var
    }

    /// Record that a source node denotes an exact language-defined qualified
    /// global path, such as `Array.from` or `Object.hasOwn`.
    pub(crate) fn record_qualified_global_symbol(
        &mut self,
        span: Span,
        kind: NodeKind,
        path: &str,
    ) -> EvidenceId {
        self.record_evidence(
            EvidenceAnchor::node(span, kind),
            EvidenceKind::Symbol(SymbolEvidenceKind::QualifiedGlobal {
                path_hash: stable_symbol_hash(path),
            }),
            "symbol_qualified_global",
        )
    }

    /// Lower an integer literal, retaining its **value** as [`Payload::LitInt`] so the
    /// value-graph (the behavioral fingerprint) keeps behavior-defining constants
    /// distinct — `x % 7` ≢ `x % 11`, `return 100` ≢ `return 200` — rather than
    /// collapsing them to one abstract `Int` (a latent false merge: different behavior,
    /// identical fingerprint). This is the §AH/§AT *behavioral* axis being sound.
    ///
    /// The *candidate* axis stays fuzzy without help here: `node_tag` folds `LitInt`
    /// back to the abstract `Int` class for the structural-shape channel, and candidate
    /// mode is shape-dominant — so clones differing only in an incidental magnitude
    /// (buffer sizes, timeouts) still cluster for refactoring. Non-parseable / oversized
    /// integers fall back to the abstract class.
    pub(crate) fn int_lit(&mut self, text: &str, span: Span) -> NodeId {
        // Strip digit-group underscores (`1_000_000`, common in Rust/Python/etc.).
        let t = text.trim().replace('_', "");
        match t.parse::<i64>() {
            Ok(v) => self.b.add(NodeKind::Lit, Payload::LitInt(v), span, &[]),
            // A float-shaped numeric (`.`/`e` exponent) keeps a value hash so `3.14` ≠
            // `2.71` (JS has one `number` kind, so its floats arrive here). Hex/binary/
            // suffixed integers that don't parse stay the abstract `Int` class (unchanged).
            _ if t.contains(['.', 'e', 'E']) && !t.starts_with("0x") => self.float_lit(text, span),
            _ => self
                .b
                .add(NodeKind::Lit, Payload::Lit(LitClass::Int), span, &[]),
        }
    }

    /// Lower a float literal, retaining a hash of its source text so float constants are
    /// behavior-DISTINCT in the value graph (`3.14` ≠ `2.71`). The structural tag stays the
    /// abstract `Float` class (see `node_tag`), so shape similarity is unaffected.
    pub(crate) fn float_lit(&mut self, text: &str, span: Span) -> NodeId {
        let h = stable_symbol_hash(text.trim().trim_end_matches(['f', 'F', 'd', 'D']));
        self.b.add(NodeKind::Lit, Payload::LitFloat(h), span, &[])
    }

    /// Lower a string literal, retaining a content hash so behavior-defining string
    /// constants (`"OPTIONS"`/`"HEAD"`, locale messages, schema-format keys) are
    /// distinct in the value-graph. The structural tag stays the abstract `Str`
    /// class (see `node_tag`), so shape similarity is unaffected.
    pub(crate) fn str_lit(&mut self, text: &str, span: Span) -> NodeId {
        let content = text.trim_matches(|c| c == '"' || c == '\'' || c == '`');
        let h = stable_symbol_hash(content);
        self.b.add(NodeKind::Lit, Payload::LitStr(h), span, &[])
    }

    /// An opaque `Raw` node wrapping `children`, tagged with the original surface
    /// kind for debugging. Used for constructs a frontend does not lower.
    pub(crate) fn raw(&mut self, surface_kind: &str, span: Span, children: &[NodeId]) -> NodeId {
        let sym = self.sym(surface_kind);
        self.b
            .add(NodeKind::Raw, Payload::Name(sym), span, children)
    }

    /// Tag a detection unit.
    pub(crate) fn push_unit(&mut self, root: NodeId, kind: UnitKind, name: Option<Symbol>) {
        self.units.push(Unit { root, kind, name });
    }

    /// Collect a CST node's named children into a `Vec` (decouples from the
    /// tree cursor so the borrow checker stays happy during recursion). Comments
    /// are skipped everywhere — they are never semantic and would otherwise land
    /// as `Raw` noise.
    pub(crate) fn named_children(n: TsNode<'a>) -> Vec<TsNode<'a>> {
        let mut cur = n.walk();
        n.named_children(&mut cur)
            .filter(|c| !is_trivia(c.kind()))
            .collect()
    }
}

/// Does `node` have a *direct* child token of the given `kind`? Used to read an
/// operator token (`--`, `++`) off the node it belongs to without being fooled by a
/// nested occurrence in the operand (e.g. the inner `i--` of `a[i--]++`), which a
/// substring scan over the node's whole text would wrongly match.
pub(crate) fn has_direct_token(node: TsNode, kind: &str) -> bool {
    let mut cur = node.walk();
    let found = node.children(&mut cur).any(|c| c.kind() == kind);
    found
}

/// Lower an import / `#include` / `use` statement to a `Seq` of its identifier and
/// string leaves. Imports carry no behavior, but a *duplicated import block* is real
/// copy-paste (jscpd flags it); emitting its tokens lets the contiguous copy-paste
/// channel — nose's Type-1/2 floor — cover it. These form no unit (the structural and
/// behavioral channels ignore them) and rank near-zero, so users never see import
/// noise; only the copy-paste floor does.
pub(crate) fn import_tokens(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let mut kids = Vec::new();
    collect_leaf_tokens(lo, node, &mut kids);
    lo.add(NodeKind::Seq, Payload::None, span, &kids)
}

/// A strict semantic proof fact for a static import binding:
/// local name → `(module coordinate, exported symbol)`.
///
/// Frontends only call this for import forms whose module/export identity is fully static.
/// Ambiguous forms fall back to [`import_tokens`], remaining visible to syntax/near but
/// unavailable to strict exact semantic mode.
pub(crate) fn import_binding(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    module: &str,
    exported: &str,
) -> NodeId {
    import_fact(
        lo,
        span,
        local,
        ImportFactKind::Binding,
        &[module, exported],
    )
}

/// A strict semantic proof fact for a static namespace import:
/// local namespace → module coordinate.
pub(crate) fn import_namespace(lo: &mut Lowering, span: Span, local: &str, module: &str) -> NodeId {
    import_fact(lo, span, local, ImportFactKind::Namespace, &[module])
}

/// Shared shape of static-import proof facts. The assignment remains in IL so
/// import text participates in the syntax/near floor, but the `Seq` payload is
/// deliberately untagged: semantic proof lives only in the evidence records.
fn import_fact(
    lo: &mut Lowering,
    span: Span,
    local: &str,
    kind: ImportFactKind,
    coords: &[&str],
) -> NodeId {
    let lhs = lo.var(local, span);
    let strs: Vec<NodeId> = coords.iter().map(|c| lo.str_lit(c, span)).collect();
    let rhs = lo.add(NodeKind::Seq, Payload::None, span, &strs);
    let evidence_kind = match kind {
        ImportFactKind::Binding if coords.len() == 2 => {
            EvidenceKind::Import(ImportEvidenceKind::Binding {
                module_hash: stable_symbol_hash(coords[0]),
                exported_hash: stable_symbol_hash(coords[1]),
            })
        }
        ImportFactKind::Namespace if coords.len() == 1 => {
            EvidenceKind::Import(ImportEvidenceKind::Namespace {
                module_hash: stable_symbol_hash(coords[0]),
            })
        }
        _ => {
            return lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        }
    };
    let symbol_kind = match kind {
        ImportFactKind::Binding if coords.len() == 2 => {
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash: stable_symbol_hash(coords[0]),
                exported_hash: stable_symbol_hash(coords[1]),
            })
        }
        ImportFactKind::Namespace if coords.len() == 1 => {
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace {
                module_hash: stable_symbol_hash(coords[0]),
            })
        }
        _ => {
            return lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs]);
        }
    };
    lo.record_evidence(EvidenceAnchor::sequence(span), evidence_kind, "import_fact");
    lo.record_evidence(
        EvidenceAnchor::binding(span, stable_symbol_hash(local)),
        evidence_kind,
        "import_binding_subject",
    );
    lo.record_evidence(
        EvidenceAnchor::binding(span, stable_symbol_hash(local)),
        symbol_kind,
        "symbol_import_identity",
    );
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

/// Emit a `Var` token for every named leaf (identifier, string fragment, path
/// component) in `node`'s subtree — the textual identity of an import.
fn collect_leaf_tokens(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    let named = Lowering::named_children(node);
    if named.is_empty() {
        let t = lo.text(node);
        if !t.is_empty() {
            let span = lo.span(node);
            out.push(lo.var(t, span));
        }
    } else {
        for c in named {
            collect_leaf_tokens(lo, c, out);
        }
    }
}

/// The shared parse → lower-root → finish pipeline every frontend's `lower` entry
/// point repeats. The frontend supplies only what is language-specific: the grammar
/// (`key` + `lang_fn`), its [`Lang`] tag, and `lower_root`, which turns the parsed
/// CST root into the file's `Module` node.
// The arguments are irreducible: the four file-context values (which mirror every
// frontend's `lower` signature) plus the three grammar/lang specifics and the root
// lowering. Bundling them into a struct used by this one function would add
// indirection without clarifying anything.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_file(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
    key: u16,
    lang_fn: impl FnOnce() -> tree_sitter::Language,
    lang: Lang,
    lower_root: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> anyhow::Result<Il> {
    lower_file_with_setup(
        file,
        path,
        src,
        interner,
        key,
        lang_fn,
        lang,
        |_| {},
        lower_root,
    )
}

/// Like [`lower_file`], but lets a frontend seed file-local proof facts after
/// parsing and before walking the root. This keeps language-specific facts in the
/// frontend while preserving the shared IL construction path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_file_with_setup(
    file: FileId,
    path: &str,
    src: &[u8],
    interner: &Interner,
    key: u16,
    lang_fn: impl FnOnce() -> tree_sitter::Language,
    lang: Lang,
    setup: impl FnOnce(&mut Lowering),
    lower_root: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> anyhow::Result<Il> {
    let tree = parse(key, lang_fn, src)?;
    let mut lo = Lowering::new(file, src, lang, interner);
    setup(&mut lo);
    let module = lower_root(&mut lo, tree.root_node());
    let meta = FileMeta {
        path: path.to_string(),
        lang,
    };
    let units = std::mem::take(&mut lo.units);
    let evidence = std::mem::take(&mut lo.evidence);
    let mut il = lo.b.finish(module, meta, units, Vec::new());
    il.evidence = evidence;
    record_post_lower_library_api_evidence(&mut il, interner);
    drop_suppressed_units(&mut il, src);
    Ok(il)
}

fn record_post_lower_library_api_evidence(il: &mut Il, interner: &Interner) {
    let calls: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| {
            (node.kind == NodeKind::Call && node.payload == Payload::None)
                .then_some(NodeId(idx as u32))
        })
        .collect();
    let fields: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Field).then_some(NodeId(idx as u32)))
        .collect();
    let vars: Vec<NodeId> = il
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| (node.kind == NodeKind::Var).then_some(NodeId(idx as u32)))
        .collect();
    let mut dependency_cache = LibraryApiDependencyCache::default();
    for call in calls {
        if record_post_lower_free_name_library_api(il, interner, call) {
            continue;
        }
        if record_post_lower_ruby_static_member_library_api(il, interner, call) {
            continue;
        }
        if record_post_lower_java_collection_constructor_library_api(il, interner, call) {
            continue;
        }
        record_post_lower_receiver_method_library_api(il, interner, call, &mut dependency_cache);
    }
    for field in fields {
        record_post_lower_property_library_api(il, interner, field, &mut dependency_cache);
    }
    for var in vars {
        record_post_lower_rust_option_none_library_api(il, interner, var);
    }
}

fn record_post_lower_free_name_library_api(il: &mut Il, interner: &Interner, call: NodeId) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let Some(callee_name) = post_lower_var_name(il, interner, callee) else {
        return false;
    };
    let arg_count = args.len();
    let contract = (arg_count == 1)
        .then(|| library_free_name_collection_factory_contract(il.meta.lang, callee_name))
        .flatten()
        .map(|contract| {
            (
                contract.id,
                contract.callee,
                "library_api_free_name_collection_factory",
                library_collection_factory_result_domain_for_arity(contract, arg_count),
            )
        })
        .or_else(|| {
            (arg_count == 1)
                .then(|| library_free_name_map_factory_contract(il.meta.lang, callee_name))
                .flatten()
                .map(|contract| {
                    (
                        contract.id,
                        contract.callee,
                        "library_api_free_name_map_factory",
                        Some(library_map_factory_result_domain(contract)),
                    )
                })
        })
        .or_else(|| {
            library_rust_vec_macro_factory_contract(il.meta.lang, callee_name).map(|contract| {
                (
                    contract.id,
                    contract.callee,
                    "library_api_rust_vec_macro_factory",
                    library_collection_factory_result_domain_for_arity(contract, arg_count),
                )
            })
        })
        .or_else(|| {
            (arg_count == 0)
                .then(|| library_rust_vec_new_factory_contract(il.meta.lang, callee_name))
                .flatten()
                .map(|contract| {
                    (
                        contract.id,
                        contract.callee,
                        "library_api_rust_vec_new_factory",
                        library_collection_factory_result_domain_for_arity(contract, arg_count),
                    )
                })
        })
        .or_else(|| {
            library_rust_option_some_constructor_contract(il.meta.lang, callee_name, arg_count).map(
                |contract| {
                    (
                        contract.id,
                        contract.callee,
                        "library_api_rust_option_some_constructor",
                        Some(contract.result_domain),
                    )
                },
            )
        })
        .or_else(|| {
            library_free_function_builtin_contract(il.meta.lang, callee_name, arg_count).map(
                |contract| {
                    (
                        contract.id,
                        contract.callee,
                        "library_api_free_function_builtin",
                        None,
                    )
                },
            )
        });
    let Some((id, callee_contract, rule, result_domain)) = contract else {
        return false;
    };
    if il.meta.lang == Lang::Python
        && post_lower_has_raw_marker(il, interner, "python_wildcard_import")
    {
        return false;
    }
    let mut dependencies = Vec::new();
    match callee_contract {
        LibraryApiCalleeContract::FreeName { name, shadow } => {
            if !library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                post_lower_file_defines_name_visible_at(
                    il,
                    interner,
                    candidate,
                    il.node(callee).span,
                )
            }) {
                return false;
            }
            let Some(dependency) = post_lower_unshadowed_symbol_evidence_id(il, callee, name)
            else {
                return false;
            };
            dependencies.push(dependency);
        }
        LibraryApiCalleeContract::RustMacro { name, shadow } => {
            if !library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
                post_lower_file_defines_name_visible_at(
                    il,
                    interner,
                    candidate,
                    il.node(callee).span,
                )
            }) {
                return false;
            }
            let Some(source_dependency) =
                post_lower_source_call_evidence_id(il, call, SourceCallKind::MacroInvocation)
            else {
                return false;
            };
            let Some(symbol_dependency) =
                post_lower_unshadowed_symbol_evidence_id(il, callee, name)
            else {
                return false;
            };
            dependencies.push(source_dependency);
            dependencies.push(symbol_dependency);
        }
        _ => return false,
    }
    let api = post_lower_library_api_evidence_id(
        il,
        call,
        id,
        callee_contract,
        arg_count,
        rule,
        dependencies,
    );
    post_lower_record_library_api_result_domain(il, call, result_domain, api);
    true
}

fn record_post_lower_property_library_api(
    il: &mut Il,
    interner: &Interner,
    field: NodeId,
    dependency_cache: &mut LibraryApiDependencyCache,
) -> bool {
    if il.kind(field) != NodeKind::Field {
        return false;
    }
    let Payload::Name(property) = il.node(field).payload else {
        return false;
    };
    let Some(contract) =
        library_property_builtin_contract(il.meta.lang, interner.resolve(property))
    else {
        return false;
    };
    let Some(dependencies) = library_api_property_dependencies_for_field_with_cache(
        il,
        interner,
        field,
        contract.callee,
        dependency_cache,
    ) else {
        return false;
    };
    post_lower_library_api_node_evidence_id(
        il,
        field,
        contract.id,
        contract.callee,
        0,
        "library_api_property_builtin",
        dependencies,
    );
    true
}

fn record_post_lower_rust_option_none_library_api(
    il: &mut Il,
    interner: &Interner,
    var: NodeId,
) -> bool {
    let Some(name) = post_lower_var_name(il, interner, var) else {
        return false;
    };
    let Some(contract) = library_rust_option_none_sentinel_contract(il.meta.lang, name) else {
        return false;
    };
    let LibraryApiCalleeContract::FreeName { name, shadow } = contract.callee else {
        return false;
    };
    if !library_api_free_name_shadow_safe(il.meta.lang, name, shadow, |candidate| {
        post_lower_file_defines_name_visible_at(il, interner, candidate, il.node(var).span)
    }) {
        return false;
    }
    let Some(symbol_dependency) = post_lower_unshadowed_symbol_evidence_id(il, var, name) else {
        return false;
    };
    let api = post_lower_library_api_node_evidence_id(
        il,
        var,
        contract.id,
        contract.callee,
        0,
        "library_api_rust_option_none_sentinel",
        vec![symbol_dependency],
    );
    post_lower_record_library_api_node_result_domain(il, var, contract.result_domain, api);
    true
}

fn record_post_lower_ruby_static_member_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let arg_count = args.len();
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(method);
    let Some(&receiver) = il.children(callee).first() else {
        return false;
    };
    let Some(receiver_name) = post_lower_var_name(il, interner, receiver) else {
        return false;
    };
    let Some(contract) =
        library_ruby_set_factory_contract(il.meta.lang, receiver_name, method, arg_count)
    else {
        return false;
    };
    let LibraryApiCalleeContract::RubyRequireStaticMember {
        receiver: expected_receiver,
        required_module,
        shadow_root,
        ..
    } = contract.callee
    else {
        return false;
    };
    if post_lower_file_defines_name_visible_at(il, interner, shadow_root, il.node(receiver).span) {
        return false;
    }
    let Some(receiver_dependency) =
        post_lower_unshadowed_symbol_evidence_id(il, receiver, expected_receiver)
    else {
        return false;
    };
    let Some(require_dependency) =
        post_lower_required_module_evidence_id(il, interner, required_module, il.node(call).span)
    else {
        return false;
    };
    let api = post_lower_library_api_evidence_id(
        il,
        call,
        contract.id,
        contract.callee,
        arg_count,
        "library_api_ruby_require_static_member",
        vec![receiver_dependency, require_dependency],
    );
    post_lower_record_library_api_result_domain(
        il,
        call,
        library_collection_factory_result_domain_for_arity(contract, arg_count),
        api,
    );
    true
}

fn record_post_lower_java_collection_constructor_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    let arg_count = args.len();
    let Some(type_name) = post_lower_var_name(il, interner, callee) else {
        return false;
    };
    let Some(contract) =
        library_java_collection_constructor_contract(il.meta.lang, type_name, arg_count)
    else {
        return false;
    };
    let LibraryApiCalleeContract::JavaUtilConstructor {
        simple_type,
        qualified_type,
        module,
        requires_import_for_simple_type,
        requires_no_local_type_shadow,
    } = contract.callee
    else {
        return false;
    };
    let Some(source_dependency) =
        post_lower_source_call_evidence_id(il, call, SourceCallKind::Construct)
    else {
        return false;
    };
    let mut dependencies = vec![source_dependency];
    if type_name == simple_type {
        if requires_no_local_type_shadow
            && post_lower_unit_defines_name(il, interner, simple_type, il.node(callee).span)
        {
            return false;
        }
        if requires_import_for_simple_type {
            if let Some(dependency) = post_lower_imported_binding_symbol_evidence_id(
                il,
                interner,
                callee,
                module,
                simple_type,
            ) {
                dependencies.push(dependency);
            } else {
                let Some(dependency) = post_lower_java_wildcard_import_evidence_id(
                    il,
                    interner,
                    module,
                    simple_type,
                    il.node(call).span,
                ) else {
                    return false;
                };
                dependencies.push(dependency);
            }
        }
    } else if type_name != qualified_type {
        return false;
    }
    let api = post_lower_library_api_evidence_id(
        il,
        call,
        contract.id,
        contract.callee,
        arg_count,
        "library_api_java_collection_constructor",
        dependencies,
    );
    post_lower_record_library_api_result_domain(
        il,
        call,
        library_collection_factory_result_domain_for_arity(contract, arg_count),
        api,
    );
    true
}

fn record_post_lower_receiver_method_library_api(
    il: &mut Il,
    interner: &Interner,
    call: NodeId,
    dependency_cache: &mut LibraryApiDependencyCache,
) -> bool {
    let kids = il.children(call);
    let Some((&callee, args)) = kids.split_first() else {
        return false;
    };
    if il.kind(callee) != NodeKind::Field {
        return false;
    }
    let Payload::Name(method) = il.node(callee).payload else {
        return false;
    };
    let method = interner.resolve(method);
    let arg_count = args.len();
    let Some(contract) = library_receiver_method_api_contract(il.meta.lang, method, arg_count)
    else {
        return false;
    };
    seed_post_lower_receiver_method_dependencies(il, interner, callee, contract.callee);
    let Some(dependencies) = library_api_receiver_dependencies_for_call_with_cache(
        il,
        interner,
        call,
        contract.callee,
        dependency_cache,
    ) else {
        return false;
    };
    post_lower_library_api_evidence_id(
        il,
        call,
        contract.id,
        contract.callee,
        arg_count,
        contract.rule,
        dependencies,
    );
    true
}

fn seed_post_lower_receiver_method_dependencies(
    il: &mut Il,
    interner: &Interner,
    callee: NodeId,
    callee_contract: LibraryApiCalleeContract,
) {
    let LibraryApiCalleeContract::Method { receiver, .. } = callee_contract else {
        return;
    };
    let Some(&receiver_node) = il.children(callee).first() else {
        return;
    };
    match receiver {
        MethodReceiverContract::UnshadowedGlobal(name) => {
            if post_lower_var_name(il, interner, receiver_node) == Some(name)
                && !post_lower_file_defines_name_visible_at(
                    il,
                    interner,
                    name,
                    il.node(receiver_node).span,
                )
            {
                let _ = post_lower_unshadowed_symbol_evidence_id(il, receiver_node, name);
            }
        }
        MethodReceiverContract::ImportedNamespace(module) => {
            let _ = post_lower_imported_namespace_symbol_evidence_id(
                il,
                interner,
                receiver_node,
                module,
            );
        }
        _ => {}
    }
}

fn post_lower_var_name<'a>(il: &Il, interner: &'a Interner, node: NodeId) -> Option<&'a str> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    match il.node(node).payload {
        Payload::Name(symbol) => Some(interner.resolve(symbol)),
        _ => None,
    }
}

fn post_lower_unshadowed_symbol_evidence_id(
    il: &mut Il,
    node: NodeId,
    expected: &str,
) -> Option<EvidenceId> {
    let span = il.node(node).span;
    let anchor = EvidenceAnchor::node(span, NodeKind::Var);
    let kind = EvidenceKind::Symbol(SymbolEvidenceKind::UnshadowedGlobal {
        name_hash: stable_symbol_hash(expected),
    });
    post_lower_find_or_push_evidence(
        il,
        anchor,
        kind,
        "symbol_unshadowed_global_post_lower",
        vec![],
    )
}

fn post_lower_imported_binding_symbol_evidence_id(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
    exported: &str,
) -> Option<EvidenceId> {
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(exported),
    };
    let dependency = post_lower_binding_symbol_evidence_id(il, interner, node, expected)?;
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(expected),
        "symbol_imported_binding_occurrence_post_lower",
        vec![dependency],
    )
}

fn post_lower_imported_namespace_symbol_evidence_id(
    il: &mut Il,
    interner: &Interner,
    node: NodeId,
    module: &str,
) -> Option<EvidenceId> {
    let expected = SymbolEvidenceKind::ImportedNamespace {
        module_hash: stable_symbol_hash(module),
    };
    let dependency = post_lower_binding_symbol_evidence_id(il, interner, node, expected)?;
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, NodeKind::Var),
        EvidenceKind::Symbol(expected),
        "symbol_imported_namespace_occurrence_post_lower",
        vec![dependency],
    )
}

fn post_lower_binding_symbol_evidence_id(
    il: &Il,
    interner: &Interner,
    node: NodeId,
    expected: SymbolEvidenceKind,
) -> Option<EvidenceId> {
    if il.kind(node) != NodeKind::Var {
        return None;
    }
    let Payload::Name(local) = il.node(node).payload else {
        return None;
    };
    let local_hash = stable_symbol_hash(interner.resolve(local));
    il.evidence.iter().find_map(|record| {
        (matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && record.kind == EvidenceKind::Symbol(expected)
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

fn post_lower_java_wildcard_import_evidence_id(
    il: &Il,
    interner: &Interner,
    module: &str,
    simple_type: &str,
    use_span: Span,
) -> Option<EvidenceId> {
    if post_lower_explicit_import_conflicts(il, interner, module, simple_type) {
        return None;
    }
    let kind = EvidenceKind::Import(ImportEvidenceKind::Wildcard {
        module_hash: stable_symbol_hash(module),
    });
    il.evidence.iter().find_map(|record| {
        (record.kind == kind
            && record.status == EvidenceStatus::Asserted
            && matches!(
                record.anchor,
                EvidenceAnchor::SourceSpan(span)
                    if span.file == use_span.file && span.end_byte <= use_span.start_byte
            ))
        .then_some(record.id)
    })
}

fn post_lower_explicit_import_conflicts(
    il: &Il,
    _interner: &Interner,
    module: &str,
    simple_type: &str,
) -> bool {
    let local_hash = stable_symbol_hash(simple_type);
    let expected = SymbolEvidenceKind::ImportedBinding {
        module_hash: stable_symbol_hash(module),
        exported_hash: stable_symbol_hash(simple_type),
    };
    il.evidence.iter().any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding {
                local_hash: anchor_hash,
                ..
            } if anchor_hash == local_hash
        ) && record.status == EvidenceStatus::Asserted
            && matches!(record.kind, EvidenceKind::Symbol(actual) if actual != expected)
    })
}

fn post_lower_required_module_evidence_id(
    il: &mut Il,
    interner: &Interner,
    module: &str,
    use_span: Span,
) -> Option<EvidenceId> {
    if il.meta.lang != Lang::Ruby {
        return None;
    }
    let module_hash = stable_symbol_hash(module);
    let (require_call, require_callee) =
        post_lower_top_level_statements(il)
            .into_iter()
            .find_map(|stmt| {
                let expr = if il.kind(stmt) == NodeKind::ExprStmt {
                    il.children(stmt).first().copied()
                } else {
                    Some(stmt)
                }?;
                let callee =
                    post_lower_require_call_callee_if_matches(il, interner, expr, module_hash)?;
                let require_span = il.node(expr).span;
                (require_span.file == use_span.file && require_span.end_byte <= use_span.start_byte)
                    .then_some((expr, callee))
            })?;
    if post_lower_file_defines_name_visible_at(
        il,
        interner,
        "require",
        il.node(require_callee).span,
    ) {
        return None;
    }
    let require_dependency =
        post_lower_unshadowed_symbol_evidence_id(il, require_callee, "require")?;
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::source_span(il.node(require_call).span),
        EvidenceKind::Import(ImportEvidenceKind::Require { module_hash }),
        "ruby_require_module",
        vec![require_dependency],
    )
}

fn post_lower_require_call_callee_if_matches(
    il: &Il,
    interner: &Interner,
    call: NodeId,
    module_hash: u64,
) -> Option<NodeId> {
    if il.kind(call) != NodeKind::Call {
        return None;
    }
    let kids = il.children(call);
    if kids.len() != 2 {
        return None;
    }
    (matches!(post_lower_var_name(il, interner, kids[0]), Some("require"))
        && matches!(il.node(kids[1]).payload, Payload::LitStr(hash) if hash == module_hash))
    .then_some(kids[0])
}

fn post_lower_library_api_evidence_id(
    il: &mut Il,
    call: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        rule,
        dependencies,
    )
    .expect("post-lower LibraryApi evidence insertion should always produce an id")
}

fn post_lower_library_api_node_evidence_id(
    il: &mut Il,
    node: NodeId,
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    arg_count: usize,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> EvidenceId {
    post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
            contract_hash: library_api_contract_id_hash(id),
            callee_hash: library_api_callee_contract_hash(callee),
            arity: arg_count as u16,
        }),
        rule,
        dependencies,
    )
    .expect("post-lower node LibraryApi evidence insertion should always produce an id")
}

fn post_lower_record_library_api_result_domain(
    il: &mut Il,
    call: NodeId,
    result_domain: Option<DomainEvidence>,
    api: EvidenceId,
) {
    if let Some(domain) = result_domain {
        let _ = post_lower_find_or_push_evidence(
            il,
            EvidenceAnchor::node(il.node(call).span, NodeKind::Call),
            EvidenceKind::Domain(domain),
            "library_api_result_domain",
            vec![api],
        );
    }
}

fn post_lower_record_library_api_node_result_domain(
    il: &mut Il,
    node: NodeId,
    domain: DomainEvidence,
    api: EvidenceId,
) {
    let _ = post_lower_find_or_push_evidence(
        il,
        EvidenceAnchor::node(il.node(node).span, il.kind(node)),
        EvidenceKind::Domain(domain),
        "library_api_result_domain",
        vec![api],
    );
}

fn post_lower_find_or_push_evidence(
    il: &mut Il,
    anchor: EvidenceAnchor,
    kind: EvidenceKind,
    rule: &str,
    dependencies: Vec<EvidenceId>,
) -> Option<EvidenceId> {
    Some(il.find_or_push_first_party_evidence(
        anchor,
        kind,
        nose_semantics::FIRST_PARTY_PACK_ID,
        rule,
        dependencies,
    ))
}

fn post_lower_top_level_statements(il: &Il) -> Vec<NodeId> {
    let Some(root) = il.nodes.get(il.root.0 as usize) else {
        return Vec::new();
    };
    if root.kind != NodeKind::Module {
        return il.children(il.root).to_vec();
    }
    il.children(il.root).to_vec()
}

fn post_lower_source_call_evidence_id(
    il: &Il,
    node: NodeId,
    call: SourceCallKind,
) -> Option<EvidenceId> {
    let anchor = EvidenceAnchor::source_span(il.node(node).span);
    il.evidence.iter().find_map(|record| {
        (record.anchor == anchor
            && record.kind == EvidenceKind::Source(SourceFactKind::Call(call))
            && record.status == EvidenceStatus::Asserted)
            .then_some(record.id)
    })
}

fn post_lower_has_raw_marker(il: &Il, interner: &Interner, marker: &str) -> bool {
    il.nodes.iter().any(|node| {
        node.kind == NodeKind::Raw
            && matches!(node.payload, Payload::Name(symbol) if interner.resolve(symbol) == marker)
    })
}

fn post_lower_file_defines_name_visible_at(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: Span,
) -> bool {
    nose_semantics::file_defines_name_visible_at(il, interner, name, occurrence_span)
}

fn post_lower_unit_defines_name(
    il: &Il,
    interner: &Interner,
    name: &str,
    occurrence_span: Span,
) -> bool {
    let name_hash = stable_symbol_hash(name);
    il.units.iter().any(|unit| {
        il.node(unit.root).span.file == occurrence_span.file
            && unit
                .name
                .is_some_and(|symbol| stable_symbol_hash(interner.resolve(symbol)) == name_hash)
    })
}

fn normalize_type_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn param_semantic_from_text(text: &str) -> Option<ParamSemantic> {
    let t = normalize_type_text(text);
    if t.contains("hashmap<")
        || t.contains("btreemap<")
        || t.contains("map<")
        || t.contains("dict[")
        || t.contains("dictionary[")
        || t.contains("mapping[")
        || t.contains("mapping<")
        || t.contains("map[")
    {
        return Some(ParamSemantic::Map);
    }
    if t.contains("option<") || t.contains("optional<") {
        return Some(ParamSemantic::Option);
    }
    if t.contains("set[") || t.contains("set<") || t.contains("hashset<") || t.contains("btreeset<")
    {
        return Some(ParamSemantic::Set);
    }
    if t.contains("[]")
        || t.contains(":&[")
        || t.contains("&[")
        || t.contains("list[")
        || t.contains("list<")
        || t.contains("tuple[")
        || t.contains("container[")
        || t.contains("container<")
        || t.contains("collection<")
        || t.contains("queue<")
        || t.contains("deque<")
        || t.contains("iterable<")
        || t.contains("iterable[")
        || t.contains("sequence[")
        || t.contains("array<")
        || t.contains("readonlyarray<")
        || t.contains("vec<")
        || t.contains("vecdeque<")
        || t.contains("slice<")
    {
        return Some(ParamSemantic::Collection);
    }
    if t.contains("string")
        || t == "str"
        || t == "&str"
        || t.contains(":str")
        || t.contains(":&str")
    {
        return Some(ParamSemantic::String);
    }
    if is_integer_semantic_text(&t) {
        return Some(ParamSemantic::Integer);
    }
    if is_float_semantic_text(&t) || t.contains(":number") || t == "number" {
        return Some(ParamSemantic::Number);
    }
    None
}

fn is_integer_semantic_text(t: &str) -> bool {
    matches!(
        t,
        "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "long"
            | "short"
            | "byte"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
    ) || t.contains(":int")
        || t.contains(":long")
        || t.contains(":short")
        || t.contains(":byte")
        || t.starts_with("int")
        || t.starts_with("long")
        || t.starts_with("short")
        || t.starts_with("byte")
        || t.contains(":i8")
        || t.contains(":i16")
        || t.contains(":i32")
        || t.contains(":i64")
        || t.contains(":i128")
        || t.contains(":isize")
        || t.contains(":u8")
        || t.contains(":u16")
        || t.contains(":u32")
        || t.contains(":u64")
        || t.contains(":u128")
        || t.contains(":usize")
}

fn is_float_semantic_text(t: &str) -> bool {
    matches!(
        t,
        "float" | "float32" | "float64" | "double" | "f32" | "f64"
    ) || t.contains(":float")
        || t.contains(":double")
        || t.contains(":f32")
        || t.contains(":f64")
        || t.starts_with("float")
        || t.starts_with("double")
}

pub(crate) fn stdlib_type_semantic(module: &str, exported: &str) -> Option<ParamSemantic> {
    let module = module.trim();
    let exported = exported.trim();
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "Dict" | "Mapping" | "MutableMapping")
    {
        return Some(ParamSemantic::Map);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(exported, "FrozenSet" | "MutableSet" | "Set")
    {
        return Some(ParamSemantic::Set);
    }
    if matches!(module, "typing" | "collections.abc")
        && matches!(
            exported,
            "Collection"
                | "Container"
                | "Deque"
                | "List"
                | "MutableSequence"
                | "Sequence"
                | "Tuple"
        )
    {
        return Some(ParamSemantic::Collection);
    }
    None
}

/// Inline suppression: drop any unit whose source carries a `nose-ignore` marker
/// on its first line or the line just above it (in a comment, any language). Lets a
/// maintainer mark a clone as intentionally-kept so it never shows up as a candidate.
fn drop_suppressed_units(il: &mut Il, src: &[u8]) {
    if il.units.is_empty() || !contains_marker(src) {
        return; // fast path: nothing to suppress
    }
    let keep: Vec<bool> = il
        .units
        .iter()
        .map(|u| !unit_suppressed(src, il.node(u.root).span.start_byte as usize))
        .collect();
    // Record suppressed units' byte spans so the contiguous channel excludes them too.
    for (u, &kept) in il.units.iter().zip(&keep) {
        if !kept {
            let sp = il.node(u.root).span;
            il.suppressed.push((sp.start_byte, sp.end_byte));
        }
    }
    let mut it = keep.iter();
    il.units.retain(|_| *it.next().unwrap());
}

const SUPPRESS_MARKER: &str = "nose-ignore";

fn contains_marker(src: &[u8]) -> bool {
    // cheap whole-file prescreen so the per-unit work only runs when relevant
    src.windows(SUPPRESS_MARKER.len())
        .any(|w| w.eq_ignore_ascii_case(SUPPRESS_MARKER.as_bytes()))
}

/// Is the unit starting at `start_byte` suppressed — i.e. does its first line or the
/// line immediately above contain the marker (typically in a trailing/preceding
/// comment)?
fn unit_suppressed(src: &[u8], start_byte: usize) -> bool {
    let start = start_byte.min(src.len());
    let cur_begin = src[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    let prev_begin = if cur_begin == 0 {
        0
    } else {
        src[..cur_begin - 1]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |p| p + 1)
    };
    let cur_end = src[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(src.len(), |p| start + p);
    let window = String::from_utf8_lossy(&src[prev_begin..cur_end]);
    window.contains(SUPPRESS_MARKER)
}

/// Lower each named child of `node` with `lower_one`, keeping the `Some` results,
/// and wrap them in a `kind` node (`Module` for a file root, `Block` for a body).
/// Every frontend's module/block builders are this same iterate-lower-collect loop
/// differing only in the node kind and per-language statement lowering.
pub(crate) fn collect_into(
    lo: &mut Lowering,
    node: TsNode,
    kind: NodeKind,
    mut lower_one: impl FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    let span = lo.span(node);
    let mut stmts = Vec::new();
    for child in Lowering::named_children(node) {
        if let Some(id) = lower_one(lo, child) {
            stmts.push(id);
        }
    }
    lo.add(kind, Payload::None, span, &stmts)
}

/// Lower a C-family `switch` (scrutinee in the `condition` field, case groups in
/// `body`) to an `if`/else-if chain. Case labels become `scrutinee == label`
/// conditions; a default label becomes the final `else`. Frontends supply only
/// which child nodes are case groups (`is_case`) and how to lower expressions and
/// statements.
pub(crate) fn switch_to_if_chain(
    lo: &mut Lowering,
    node: TsNode,
    is_case: impl Fn(&str) -> bool,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode) -> NodeId,
    mut lower_stmt: impl FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
) -> NodeId {
    let span = lo.span(node);
    let scrutinee = node
        .child_by_field_name("condition")
        .map(|c| lower_expr(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let cases: Vec<TsNode> = node
        .child_by_field_name("body")
        .map(|b| {
            Lowering::named_children(b)
                .into_iter()
                .filter(|c| is_case(c.kind()))
                .collect()
        })
        .unwrap_or_default();

    let mut branches = Vec::new();
    let mut default_block = None;
    for case in cases {
        let (labels, block) = lower_switch_case(lo, case, span, &mut lower_expr, &mut lower_stmt);
        match fold_switch_case_labels(lo, span, scrutinee, labels) {
            Some(cond) => branches.push((cond, block)),
            None => default_block = Some(block),
        }
    }

    let mut acc = default_block.unwrap_or_else(|| lo.empty_block(span));
    for (cond, block) in branches.into_iter().rev() {
        acc = lo.add(NodeKind::If, Payload::None, span, &[cond, block, acc]);
    }
    acc
}

fn lower_switch_case<E, S>(
    lo: &mut Lowering,
    case: TsNode,
    span: Span,
    lower_expr: &mut E,
    lower_stmt: &mut S,
) -> (Vec<NodeId>, NodeId)
where
    E: FnMut(&mut Lowering, TsNode) -> NodeId,
    S: FnMut(&mut Lowering, TsNode) -> Option<NodeId>,
{
    let mut labels = Vec::new();
    let mut stmts = Vec::new();
    let mut label_phase = true;
    let mut saw_explicit_label = false;

    for child in Lowering::named_children(case) {
        if label_phase && child.kind() == "switch_label" {
            saw_explicit_label = true;
            for label in Lowering::named_children(child) {
                labels.push(lower_expr(lo, label));
            }
            continue;
        }
        if label_phase && !saw_explicit_label && !is_switch_body_child(child.kind()) {
            labels.push(lower_expr(lo, child));
            continue;
        }

        label_phase = false;
        if let Some(id) = lower_stmt(lo, child) {
            stmts.push(id);
        }
    }

    let block = lo.add(NodeKind::Block, Payload::None, span, &stmts);
    (labels, block)
}

fn fold_switch_case_labels(
    lo: &mut Lowering,
    span: Span,
    scrutinee: NodeId,
    labels: Vec<NodeId>,
) -> Option<NodeId> {
    let mut acc = None;
    for label in labels {
        let cond = lo.add(
            NodeKind::BinOp,
            Payload::Op(Op::Eq),
            span,
            &[scrutinee, label],
        );
        acc = Some(match acc {
            None => cond,
            Some(prev) => lo.add(NodeKind::BinOp, Payload::Op(Op::Or), span, &[prev, cond]),
        });
    }
    acc
}

fn is_switch_body_child(kind: &str) -> bool {
    matches!(
        kind,
        "assert_statement"
            | "block"
            | "break_statement"
            | "compound_statement"
            | "continue_statement"
            | "declaration"
            | "do_statement"
            | "expression_statement"
            | "for_statement"
            | "if_statement"
            | "labeled_statement"
            | "local_variable_declaration"
            | "return_statement"
            | "switch_statement"
            | "synchronized_statement"
            | "throw_statement"
            | "try_statement"
            | "try_with_resources_statement"
            | "while_statement"
            | "yield_statement"
    )
}

/// Build a `Func` unit from a `name`/`parameters`/`body`-shaped node and register
/// it for detection (a `Method` when `method`, else a `Function`). Every frontend
/// shares this skeleton — extract the name, lower the parameters, lower the body,
/// push the unit; `lower_params` and `lower_body` are the only language-specific
/// pieces (param node shapes and body/return conventions differ per grammar).
pub(crate) fn function_unit(
    lo: &mut Lowering,
    node: TsNode,
    method: bool,
    lower_params: impl FnOnce(&mut Lowering, TsNode, &mut Vec<NodeId>),
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let name = node.child_by_field_name("name").map(|n| lo.sym(lo.text(n)));
    let mut kids = Vec::new();
    if let Some(params) = node.child_by_field_name("parameters") {
        lower_params(lo, params, &mut kids);
    }
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    kids.push(body);
    let func = lo.add(NodeKind::Func, Payload::None, span, &kids);
    let kind = if method {
        UnitKind::Method
    } else {
        UnitKind::Function
    };
    lo.push_unit(func, kind, name);
    func
}

/// Lower a `left`/`operator`/`right` binary-expression node into a `BinOp`. Every
/// supported grammar names those fields identically; each frontend supplies its
/// dialect's operator resolution and its expression lowering. An operator the
/// dialect doesn't recognise (or a missing operand) becomes a `Raw` node that
/// preserves the children — never a silently-wrong default operator.
pub(crate) fn binary(
    lo: &mut Lowering,
    node: TsNode,
    op_of: impl FnOnce(&str) -> Option<Op>,
    mut lower_operand: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let l = node
        .child_by_field_name("left")
        .map(|x| lower_operand(lo, x));
    let r = node
        .child_by_field_name("right")
        .map(|x| lower_operand(lo, x));
    let op = node
        .child_by_field_name("operator")
        .and_then(|o| op_of(lo.text(o)));
    match (l, r, op) {
        (Some(l), Some(r), Some(op)) => lo.add(NodeKind::BinOp, Payload::Op(op), span, &[l, r]),
        _ => {
            let kids: Vec<NodeId> = Lowering::named_children(node)
                .into_iter()
                .map(|c| lower_operand(lo, c))
                .collect();
            lo.raw(node.kind(), span, &kids)
        }
    }
}

/// Lower a `left`/`right` assignment-expression node into an `Assign`.
/// JS/TS and Rust grammars use the same field names for simple assignment; compound
/// assignment remains frontend-specific because operator spelling and rewrites differ.
pub(crate) fn assignment(
    lo: &mut Lowering,
    node: TsNode,
    mut lower_expr: impl FnMut(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let lhs = node
        .child_by_field_name("left")
        .map(|l| lower_expr(lo, l))
        .unwrap_or_else(|| lo.empty_block(span));
    let rhs = node
        .child_by_field_name("right")
        .map(|r| lower_expr(lo, r))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(NodeKind::Assign, Payload::None, span, &[lhs, rhs])
}

/// Lower a `condition`/`body`-shaped CST node into a canonical `While` [`Loop`].
/// Every C-family `while` lowers identically apart from *how* its condition and
/// body sub-nodes are lowered, so each frontend supplies those two as closures
/// and shares the field-extraction, empty-fallback, and node-construction here.
pub(crate) fn while_loop(
    lo: &mut Lowering,
    node: TsNode,
    lower_cond: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
    lower_body: impl FnOnce(&mut Lowering, TsNode) -> NodeId,
) -> NodeId {
    let span = lo.span(node);
    let cond = node
        .child_by_field_name("condition")
        .map(|c| lower_cond(lo, c))
        .unwrap_or_else(|| lo.empty_block(span));
    let body = node
        .child_by_field_name("body")
        .map(|b| lower_body(lo, b))
        .unwrap_or_else(|| lo.empty_block(span));
    lo.add(
        NodeKind::Loop,
        Payload::Loop(LoopKind::While),
        span,
        &[cond, body],
    )
}

thread_local! {
    /// Per-thread, per-grammar parser cache. `tree_sitter::Parser::new` allocates
    /// the parser's internal scan stack and lexer caches; recreating one for every
    /// file (corpora run thousands) is pure overhead. Rayon hands each worker its
    /// own thread, so a thread-local pool needs no locking and a grammar's parser
    /// is built at most once per worker.
    static PARSERS: std::cell::RefCell<std::collections::HashMap<u16, tree_sitter::Parser>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Parse `src` with a thread-local parser cached under `key` (which must uniquely
/// identify the grammar — JS/TS/TSX share a crate but need distinct slots).
/// `lang` is only evaluated the first time a thread sees `key`.
pub(crate) fn parse(
    key: u16,
    lang: impl FnOnce() -> tree_sitter::Language,
    src: &[u8],
) -> anyhow::Result<tree_sitter::Tree> {
    PARSERS.with(|cell| {
        let mut pool = cell.borrow_mut();
        let parser = match pool.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let mut p = tree_sitter::Parser::new();
                p.set_language(&lang())?;
                e.insert(p)
            }
        };
        parser
            .parse(src, None)
            .ok_or_else(|| anyhow::anyhow!("parse failed"))
    })
}

/// Stable grammar keys for the thread-local parser pool. JS/TS/TSX are distinct.
pub(crate) mod grammar {
    pub(crate) const PYTHON: u16 = 0;
    pub(crate) const JAVASCRIPT: u16 = 1;
    pub(crate) const TYPESCRIPT: u16 = 2;
    pub(crate) const TSX: u16 = 3;
    pub(crate) const GO: u16 = 4;
    pub(crate) const RUST: u16 = 5;
    pub(crate) const JAVA: u16 = 6;
    pub(crate) const C: u16 = 7;
    pub(crate) const RUBY: u16 = 8;
}

/// Comment / trivia node kinds across the supported grammars.
pub(crate) fn is_trivia(kind: &str) -> bool {
    matches!(
        kind,
        "comment" | "line_comment" | "block_comment" | "hash_bang_line"
    )
}

/// Binary-operator tokens shared by ~every C-family language. Per-language
/// frontends delegate here and then handle their own extras (JS `===`/`**`/`??`,
/// Go `&^`, …) — so the universal operator table lives in one place.
pub(crate) fn common_bin_op(text: &str) -> Option<Op> {
    Some(match text {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" => Op::Mul,
        "/" => Op::Div,
        "%" => Op::Mod,
        "==" => Op::Eq,
        "!=" => Op::Ne,
        "<" => Op::Lt,
        "<=" => Op::Le,
        ">" => Op::Gt,
        ">=" => Op::Ge,
        "&&" => Op::And,
        "||" => Op::Or,
        "&" => Op::BitAnd,
        "|" => Op::BitOr,
        "^" => Op::BitXor,
        "<<" => Op::Shl,
        ">>" => Op::Shr,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp() -> Span {
        Span::new(FileId(0), 0, 1, 1, 1)
    }

    fn sp_at(line: u32) -> Span {
        Span::new(FileId(0), line, line + 1, line, line)
    }

    #[test]
    fn import_lowering_emits_symbol_identity_evidence_for_aliases() {
        let interner = Interner::new();
        let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);

        import_binding(&mut lo, sp(), "deque", "collections", "deque");
        import_namespace(&mut lo, sp(), "math", "math");

        assert!(lo.evidence.iter().any(|record| matches!(
            record.kind,
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedBinding {
                module_hash,
                exported_hash,
            }) if module_hash == stable_symbol_hash("collections")
                && exported_hash == stable_symbol_hash("deque")
        )));
        assert!(lo.evidence.iter().any(|record| matches!(
            record.kind,
            EvidenceKind::Symbol(SymbolEvidenceKind::ImportedNamespace { module_hash })
                if module_hash == stable_symbol_hash("math")
        )));
    }

    fn library_api_evidence_count(lo: &Lowering, contract_hash: u64, callee_hash: u64) -> usize {
        library_api_evidence_count_in_records(&lo.evidence, contract_hash, callee_hash)
    }

    fn library_api_evidence_count_in_records(
        evidence: &[EvidenceRecord],
        contract_hash: u64,
        callee_hash: u64,
    ) -> usize {
        evidence
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        ..
                    }) if actual_contract == contract_hash && actual_callee == callee_hash
                )
            })
            .count()
    }

    fn library_api_evidence_ids_in_records(
        evidence: &[EvidenceRecord],
        contract_hash: u64,
        callee_hash: u64,
    ) -> Vec<EvidenceId> {
        evidence
            .iter()
            .filter_map(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash: actual_contract,
                        callee_hash: actual_callee,
                        ..
                    }) if actual_contract == contract_hash && actual_callee == callee_hash
                )
                .then_some(record.id)
            })
            .collect()
    }

    fn library_api_evidence_ids_at(
        evidence: &[EvidenceRecord],
        span: Span,
        contract_hash: u64,
        callee_hash: u64,
        arity: u16,
    ) -> Vec<EvidenceId> {
        evidence
            .iter()
            .filter_map(|record| {
                (record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                    && matches!(
                        record.kind,
                        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                            contract_hash: actual_contract,
                            callee_hash: actual_callee,
                            arity: actual_arity,
                        }) if actual_contract == contract_hash
                            && actual_callee == callee_hash
                            && actual_arity == arity
                    ))
                .then_some(record.id)
            })
            .collect()
    }

    fn library_api_evidence_ids_at_node(
        evidence: &[EvidenceRecord],
        span: Span,
        kind: NodeKind,
        contract_hash: u64,
        callee_hash: u64,
        arity: u16,
    ) -> Vec<EvidenceId> {
        evidence
            .iter()
            .filter_map(|record| {
                (record.anchor == EvidenceAnchor::node(span, kind)
                    && matches!(
                        record.kind,
                        EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                            contract_hash: actual_contract,
                            callee_hash: actual_callee,
                            arity: actual_arity,
                        }) if actual_contract == contract_hash
                            && actual_callee == callee_hash
                            && actual_arity == arity
                    ))
                .then_some(record.id)
            })
            .collect()
    }

    fn result_domain_depends_on_api(
        evidence: &[EvidenceRecord],
        span: Span,
        domain: DomainEvidence,
        api_ids: &[EvidenceId],
    ) -> bool {
        evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                && record.kind == EvidenceKind::Domain(domain)
                && record.dependencies.len() == 1
                && api_ids.contains(&record.dependencies[0])
        })
    }

    fn result_domain_depends_on_api_at_node(
        evidence: &[EvidenceRecord],
        span: Span,
        kind: NodeKind,
        domain: DomainEvidence,
        api_ids: &[EvidenceId],
    ) -> bool {
        evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::node(span, kind)
                && record.kind == EvidenceKind::Domain(domain)
                && record.dependencies.len() == 1
                && api_ids.contains(&record.dependencies[0])
        })
    }

    fn result_domain_any_count_at(evidence: &[EvidenceRecord], span: Span) -> usize {
        evidence
            .iter()
            .filter(|record| {
                record.anchor == EvidenceAnchor::node(span, NodeKind::Call)
                    && matches!(record.kind, EvidenceKind::Domain(_))
            })
            .count()
    }

    fn result_domain_depends_on_any_api(
        evidence: &[EvidenceRecord],
        domain: DomainEvidence,
        api_ids: &[EvidenceId],
    ) -> bool {
        evidence.iter().any(|record| {
            matches!(record.kind, EvidenceKind::Domain(actual) if actual == domain)
                && record.dependencies.len() == 1
                && api_ids.contains(&record.dependencies[0])
        })
    }

    fn result_domain_record_count(evidence: &[EvidenceRecord], domain: DomainEvidence) -> usize {
        evidence
            .iter()
            .filter(
                |record| matches!(record.kind, EvidenceKind::Domain(actual) if actual == domain),
            )
            .filter(|record| !record.dependencies.is_empty())
            .count()
    }

    fn call_node_with_result_domain(il: &Il, domain: DomainEvidence) -> Option<NodeId> {
        let span = il
            .evidence
            .iter()
            .find_map(|record| match (record.anchor, record.kind) {
                (
                    EvidenceAnchor::Node {
                        span,
                        kind: NodeKind::Call,
                    },
                    EvidenceKind::Domain(actual),
                ) if actual == domain => Some(span),
                _ => None,
            })?;
        il.nodes.iter().enumerate().find_map(|(idx, node)| {
            (node.kind == NodeKind::Call && node.span == span).then_some(NodeId(idx as u32))
        })
    }

    #[test]
    fn core_lowering_emits_import_backed_library_api_occurrences() {
        let interner = Interner::new();
        let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
        import_binding(&mut lo, sp_at(1), "deque", "collections", "deque");
        let callee = lo.var("deque", sp_at(2));
        let seq = lo.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp_at(3),
            &[],
        );
        lo.add(NodeKind::Call, Payload::None, sp_at(4), &[callee, seq]);

        let contract = nose_semantics::library_imported_collection_factory_contract(
            Lang::Python,
            "collections",
            "deque",
        )
        .expect("deque contract");
        assert_eq!(
            library_api_evidence_count(
                &lo,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            1
        );

        let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
        import_binding(&mut lo, sp_at(10), "Values", "collections", "deque");
        let callee = lo.var("Values", sp_at(11));
        let seq = lo.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp_at(12),
            &[],
        );
        lo.add(NodeKind::Call, Payload::None, sp_at(13), &[callee, seq]);
        assert_eq!(
            library_api_evidence_count(
                &lo,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            1
        );

        let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
        import_namespace(&mut lo, sp_at(20), "math", "math");
        let math = lo.var("math", sp_at(21));
        let callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("prod")),
            sp_at(22),
            &[math],
        );
        let seq = lo.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp_at(23),
            &[],
        );
        lo.add(NodeKind::Call, Payload::None, sp_at(24), &[callee, seq]);
        let contract = library_imported_namespace_function_contract(Lang::Python, "prod", 1)
            .expect("math.prod contract");
        assert_eq!(
            library_api_evidence_count(
                &lo,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            1
        );
    }

    #[test]
    fn core_lowering_emits_result_domain_evidence_for_library_api_factories() {
        let interner = Interner::new();

        let mut lo = Lowering::new(FileId(0), b"", Lang::Python, &interner);
        import_binding(&mut lo, sp_at(1), "Values", "collections", "deque");
        let callee = lo.var("Values", sp_at(2));
        let seq = lo.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp_at(3),
            &[],
        );
        lo.add(NodeKind::Call, Payload::None, sp_at(4), &[callee, seq]);
        let deque = nose_semantics::library_imported_collection_factory_contract(
            Lang::Python,
            "collections",
            "deque",
        )
        .expect("deque contract");
        let deque_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(deque.id),
            library_api_callee_contract_hash(deque.callee),
        );
        assert!(
            result_domain_depends_on_api(
                &lo.evidence,
                sp_at(4),
                DomainEvidence::Collection,
                &deque_api,
            ),
            "collections.deque result domain should depend on the admitted LibraryApi occurrence"
        );

        let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);
        import_binding(&mut lo, sp_at(10), "List", "java.util", "List");
        import_binding(&mut lo, sp_at(11), "Set", "java.util", "Set");
        import_binding(&mut lo, sp_at(12), "Map", "java.util", "Map");
        import_binding(&mut lo, sp_at(13), "Arrays", "java.util", "Arrays");

        let list = lo.var("List", sp_at(20));
        let list_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp_at(21),
            &[list],
        );
        let item = lo.int_lit("1", sp_at(22));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(23),
            &[list_callee, item],
        );
        let list_contract =
            library_java_collection_factory_contract(Lang::Java, "List", "of").unwrap();
        let list_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(list_contract.id),
            library_api_callee_contract_hash(list_contract.callee),
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(23),
            DomainEvidence::Collection,
            &list_api,
        ));

        let set = lo.var("Set", sp_at(30));
        let set_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp_at(31),
            &[set],
        );
        let item = lo.int_lit("1", sp_at(32));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(33),
            &[set_callee, item],
        );
        let set_contract =
            library_java_collection_factory_contract(Lang::Java, "Set", "of").unwrap();
        let set_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(set_contract.id),
            library_api_callee_contract_hash(set_contract.callee),
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(33),
            DomainEvidence::Set,
            &set_api,
        ));

        let map = lo.var("Map", sp_at(40));
        let map_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp_at(41),
            &[map],
        );
        let key = lo.str_lit("\"red\"", sp_at(42));
        let value = lo.int_lit("1", sp_at(43));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(44),
            &[map_callee, key, value],
        );
        let map_contract = library_java_map_factory_contract(Lang::Java, "Map", "of").unwrap();
        let map_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(map_contract.id),
            library_api_callee_contract_hash(map_contract.callee),
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(44),
            DomainEvidence::Map,
            &map_api,
        ));

        let arrays = lo.var("Arrays", sp_at(46));
        let as_list_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("asList")),
            sp_at(47),
            &[arrays],
        );
        let maybe_array = lo.var("items", sp_at(48));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(49),
            &[as_list_callee, maybe_array],
        );
        assert_eq!(
            result_domain_any_count_at(&lo.evidence, sp_at(49)),
            0,
            "single-argument Arrays.asList must not emit any result-domain evidence"
        );

        let arrays = lo.var("Arrays", sp_at(55));
        let as_list_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("asList")),
            sp_at(56),
            &[arrays],
        );
        let red = lo.str_lit("\"red\"", sp_at(57));
        let blue = lo.str_lit("\"blue\"", sp_at(58));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(59),
            &[as_list_callee, red, blue],
        );
        let as_list_contract =
            library_java_collection_factory_contract(Lang::Java, "Arrays", "asList").unwrap();
        let as_list_api = library_api_evidence_ids_at(
            &lo.evidence,
            sp_at(59),
            library_api_contract_id_hash(as_list_contract.id),
            library_api_callee_contract_hash(as_list_contract.callee),
            2,
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(59),
            DomainEvidence::Collection,
            &as_list_api,
        ));

        let map = lo.var("Map", sp_at(50));
        let entry_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("entry")),
            sp_at(51),
            &[map],
        );
        let key = lo.str_lit("\"red\"", sp_at(52));
        let value = lo.int_lit("1", sp_at(53));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(54),
            &[entry_callee, key, value],
        );
        let entry_contract = library_java_map_entry_contract(Lang::Java, "Map", "entry").unwrap();
        assert_eq!(
            library_api_evidence_count(
                &lo,
                library_api_contract_id_hash(entry_contract.id),
                library_api_callee_contract_hash(entry_contract.callee),
            ),
            1
        );
        assert_eq!(
            result_domain_any_count_at(&lo.evidence, sp_at(54)),
            0,
            "Map.entry returns an entry value, not any receiver-domain container"
        );

        let arrays = lo.var("Arrays", sp_at(60));
        let stream_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("stream")),
            sp_at(61),
            &[arrays],
        );
        let values = lo.var("values", sp_at(62));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(63),
            &[stream_callee, values],
        );
        assert_eq!(
            result_domain_any_count_at(&lo.evidence, sp_at(63)),
            0,
            "Arrays.stream produces a stream/protocol surface, not any receiver-domain container"
        );

        let mut lo = Lowering::new(FileId(0), b"", Lang::JavaScript, &interner);
        let set = lo.unshadowed_global_var("Set", sp_at(70));
        let seq = lo.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp_at(71),
            &[],
        );
        lo.record_source_fact(sp_at(72), SourceFactKind::Call(SourceCallKind::Construct));
        lo.add(NodeKind::Call, Payload::None, sp_at(72), &[set, seq]);
        let set_contract = library_js_like_set_constructor_contract(Lang::JavaScript, "Set")
            .expect("Set constructor");
        let set_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(set_contract.id),
            library_api_callee_contract_hash(set_contract.callee),
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(72),
            DomainEvidence::Set,
            &set_api,
        ));

        let map = lo.unshadowed_global_var("Map", sp_at(80));
        let seq = lo.add(
            NodeKind::Seq,
            Payload::Name(interner.intern("array")),
            sp_at(81),
            &[],
        );
        lo.record_source_fact(sp_at(82), SourceFactKind::Call(SourceCallKind::Construct));
        lo.add(NodeKind::Call, Payload::None, sp_at(82), &[map, seq]);
        let map_contract = library_js_like_map_constructor_contract(Lang::JavaScript, "Map")
            .expect("Map constructor");
        let map_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(map_contract.id),
            library_api_callee_contract_hash(map_contract.callee),
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(82),
            DomainEvidence::Map,
            &map_api,
        ));

        let array = lo.unshadowed_global_var("Array", sp_at(90));
        let from_callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("from")),
            sp_at(91),
            &[array],
        );
        lo.record_qualified_global_symbol(sp_at(91), NodeKind::Field, "Array.from");
        let iterable = lo.var("iterable", sp_at(92));
        lo.add(
            NodeKind::Call,
            Payload::None,
            sp_at(93),
            &[from_callee, iterable],
        );
        let from_contract =
            library_map_key_view_wrapper_contract(Lang::JavaScript, "Array", "from", 1).unwrap();
        let from_api = library_api_evidence_ids_in_records(
            &lo.evidence,
            library_api_contract_id_hash(from_contract.id),
            library_api_callee_contract_hash(from_contract.callee),
        );
        assert!(result_domain_depends_on_api(
            &lo.evidence,
            sp_at(93),
            DomainEvidence::Array,
            &from_api,
        ));

        let boolean = lo.unshadowed_global_var("Boolean", sp_at(100));
        let value = lo.var("value", sp_at(101));
        lo.add(NodeKind::Call, Payload::None, sp_at(102), &[boolean, value]);
        assert_eq!(
            result_domain_any_count_at(&lo.evidence, sp_at(102)),
            0,
            "Boolean(...) has LibraryApi identity but no container result-domain"
        );
    }

    #[test]
    fn python_lowering_emits_library_api_for_aliased_imported_collection_factory() {
        let interner = Interner::new();
        let il = crate::lower_source(
            FileId(0),
            "alias.py",
            b"from collections import deque as Values\n\n\
def f(value, other):\n    return Values([\"red\", \"blue\"]).__contains__(value)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        let contract = nose_semantics::library_imported_collection_factory_contract(
            Lang::Python,
            "collections",
            "deque",
        )
        .expect("deque contract");

        assert_eq!(
            library_api_evidence_count_in_records(
                &il.evidence,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            1
        );
    }

    #[test]
    fn post_lowering_emits_free_name_and_require_library_api_occurrences() {
        let interner = Interner::new();

        let py = crate::lower_source(
            FileId(0),
            "builtin.py",
            b"def f(values):\n    return list(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        let py_contract =
            library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &py.evidence,
                library_api_contract_id_hash(py_contract.id),
                library_api_callee_contract_hash(py_contract.callee),
            ),
            1
        );

        let shadowed_py = crate::lower_source(
            FileId(0),
            "shadowed.py",
            b"def f(list, values):\n    return list(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &shadowed_py.evidence,
                library_api_contract_id_hash(py_contract.id),
                library_api_callee_contract_hash(py_contract.callee),
            ),
            0
        );

        let wildcard_py = crate::lower_source(
            FileId(0),
            "wildcard.py",
            b"from custom import *\n\ndef f(values):\n    return list(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &wildcard_py.evidence,
                library_api_contract_id_hash(py_contract.id),
                library_api_callee_contract_hash(py_contract.callee),
            ),
            0
        );

        let py_len = crate::lower_source(
            FileId(0),
            "len.py",
            b"def f(values):\n    return len(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        let py_len_contract =
            library_free_function_builtin_contract(Lang::Python, "len", 1).unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &py_len.evidence,
                library_api_contract_id_hash(py_len_contract.id),
                library_api_callee_contract_hash(py_len_contract.callee),
            ),
            1
        );

        let shadowed_py_len = crate::lower_source(
            FileId(0),
            "shadowed_len.py",
            b"def f(len, values):\n    return len(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &shadowed_py_len.evidence,
                library_api_contract_id_hash(py_len_contract.id),
                library_api_callee_contract_hash(py_len_contract.callee),
            ),
            0
        );

        let wildcard_py_len = crate::lower_source(
            FileId(0),
            "wildcard_len.py",
            b"from custom import *\n\ndef f(values):\n    return len(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &wildcard_py_len.evidence,
                library_api_contract_id_hash(py_len_contract.id),
                library_api_callee_contract_hash(py_len_contract.callee),
            ),
            0
        );

        let go = crate::lower_source(
            FileId(0),
            "builtin.go",
            b"package p\nfunc f(xs []int, x int) int { return len(xs) }\nfunc g(xs []int, x int) []int { return append(xs, x) }\n",
            Lang::Go,
            &interner,
        )
        .expect("go lowering should succeed");
        let go_len_contract = library_free_function_builtin_contract(Lang::Go, "len", 1).unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &go.evidence,
                library_api_contract_id_hash(go_len_contract.id),
                library_api_callee_contract_hash(go_len_contract.callee),
            ),
            1
        );
        let go_append_contract =
            library_free_function_builtin_contract(Lang::Go, "append", 2).unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &go.evidence,
                library_api_contract_id_hash(go_append_contract.id),
                library_api_callee_contract_hash(go_append_contract.callee),
            ),
            1
        );

        let rust = crate::lower_source(
            FileId(0),
            "vec.rs",
            b"fn f() { let xs = Vec::new(); }",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        let rust_contract = library_rust_vec_new_factory_contract(Lang::Rust, "Vec::new").unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &rust.evidence,
                library_api_contract_id_hash(rust_contract.id),
                library_api_callee_contract_hash(rust_contract.callee),
            ),
            1
        );

        let rust_macro = crate::lower_source(
            FileId(0),
            "vec_macro.rs",
            b"fn f() { let xs = vec![1, 2]; }",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        let rust_macro_contract =
            library_rust_vec_macro_factory_contract(Lang::Rust, "vec").unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &rust_macro.evidence,
                library_api_contract_id_hash(rust_macro_contract.id),
                library_api_callee_contract_hash(rust_macro_contract.callee),
            ),
            1
        );

        let rust_function_call = crate::lower_source(
            FileId(0),
            "vec_function.rs",
            b"fn f(vec: fn(i32) -> Vec<i32>) { let xs = vec(1); }",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &rust_function_call.evidence,
                library_api_contract_id_hash(rust_macro_contract.id),
                library_api_callee_contract_hash(rust_macro_contract.callee),
            ),
            0
        );

        let rust_shadowed_macro = crate::lower_source(
            FileId(0),
            "vec_shadowed_macro.rs",
            b"macro_rules! vec { ($($x:expr),*) => { custom_vec![$($x),*] }; }\nfn f() { let xs = vec![1, 2]; }",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &rust_shadowed_macro.evidence,
                library_api_contract_id_hash(rust_macro_contract.id),
                library_api_callee_contract_hash(rust_macro_contract.callee),
            ),
            0
        );

        let ruby = crate::lower_source(
            FileId(0),
            "set.rb",
            b"require \"set\"\n\ndef f(values)\n  Set.new(values)\nend\n",
            Lang::Ruby,
            &interner,
        )
        .expect("ruby lowering should succeed");
        let ruby_contract = library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).unwrap();
        assert_eq!(
            library_api_evidence_count_in_records(
                &ruby.evidence,
                library_api_contract_id_hash(ruby_contract.id),
                library_api_callee_contract_hash(ruby_contract.callee),
            ),
            1
        );

        let missing_require = crate::lower_source(
            FileId(0),
            "set_missing_require.rb",
            b"def f(values)\n  Set.new(values)\nend\n",
            Lang::Ruby,
            &interner,
        )
        .expect("ruby lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &missing_require.evidence,
                library_api_contract_id_hash(ruby_contract.id),
                library_api_callee_contract_hash(ruby_contract.callee),
            ),
            0
        );

        let late_require = crate::lower_source(
            FileId(0),
            "set_late_require.rb",
            b"def f(values)\n  Set.new(values)\nend\n\nrequire \"set\"\n",
            Lang::Ruby,
            &interner,
        )
        .expect("ruby lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &late_require.evidence,
                library_api_contract_id_hash(ruby_contract.id),
                library_api_callee_contract_hash(ruby_contract.callee),
            ),
            0
        );

        let shadowed_require = crate::lower_source(
            FileId(0),
            "set_shadowed_require.rb",
            b"def require(name)\n  name\nend\n\nrequire \"set\"\n\ndef f(values)\n  Set.new(values)\nend\n",
            Lang::Ruby,
            &interner,
        )
        .expect("ruby lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &shadowed_require.evidence,
                library_api_contract_id_hash(ruby_contract.id),
                library_api_callee_contract_hash(ruby_contract.callee),
            ),
            0
        );
    }

    #[test]
    fn post_lowering_emits_result_domains_for_supported_factories() {
        let interner = Interner::new();

        let py_list = crate::lower_source(
            FileId(0),
            "builtin_list.py",
            b"def f(values):\n    return list(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        let list_contract =
            library_free_name_collection_factory_contract(Lang::Python, "list").unwrap();
        let list_api = library_api_evidence_ids_in_records(
            &py_list.evidence,
            library_api_contract_id_hash(list_contract.id),
            library_api_callee_contract_hash(list_contract.callee),
        );
        assert!(result_domain_depends_on_any_api(
            &py_list.evidence,
            DomainEvidence::Collection,
            &list_api,
        ));

        let py_set = crate::lower_source(
            FileId(0),
            "builtin_set.py",
            b"def f(values):\n    return set(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        let set_contract =
            library_free_name_collection_factory_contract(Lang::Python, "set").unwrap();
        let set_api = library_api_evidence_ids_in_records(
            &py_set.evidence,
            library_api_contract_id_hash(set_contract.id),
            library_api_callee_contract_hash(set_contract.callee),
        );
        assert!(result_domain_depends_on_any_api(
            &py_set.evidence,
            DomainEvidence::Set,
            &set_api,
        ));

        let shadowed_py = crate::lower_source(
            FileId(0),
            "shadowed.py",
            b"def f(list, values):\n    return list(values)\n",
            Lang::Python,
            &interner,
        )
        .expect("python lowering should succeed");
        assert_eq!(
            result_domain_record_count(&shadowed_py.evidence, DomainEvidence::Collection),
            0,
            "shadowed list(...) must not emit result-domain evidence"
        );

        let rust_vec = crate::lower_source(
            FileId(0),
            "vec.rs",
            b"fn f() { let xs = Vec::new(); }",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        let vec_contract = library_rust_vec_new_factory_contract(Lang::Rust, "Vec::new").unwrap();
        let vec_api = library_api_evidence_ids_in_records(
            &rust_vec.evidence,
            library_api_contract_id_hash(vec_contract.id),
            library_api_callee_contract_hash(vec_contract.callee),
        );
        assert!(result_domain_depends_on_any_api(
            &rust_vec.evidence,
            DomainEvidence::Collection,
            &vec_api,
        ));

        let rust_map = crate::lower_source(
            FileId(0),
            "hash_map.rs",
            b"fn f() { let xs = std::collections::HashMap::from([(\"red\", 1)]); }",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        let map_contract =
            library_free_name_map_factory_contract(Lang::Rust, "std::collections::HashMap::from")
                .unwrap();
        let map_api = library_api_evidence_ids_in_records(
            &rust_map.evidence,
            library_api_contract_id_hash(map_contract.id),
            library_api_callee_contract_hash(map_contract.callee),
        );
        assert!(result_domain_depends_on_any_api(
            &rust_map.evidence,
            DomainEvidence::Map,
            &map_api,
        ));

        let ruby = crate::lower_source(
            FileId(0),
            "set.rb",
            b"require \"set\"\n\ndef f(values)\n  Set.new(values)\nend\n",
            Lang::Ruby,
            &interner,
        )
        .expect("ruby lowering should succeed");
        let ruby_contract = library_ruby_set_factory_contract(Lang::Ruby, "Set", "new", 1).unwrap();
        let ruby_api = library_api_evidence_ids_in_records(
            &ruby.evidence,
            library_api_contract_id_hash(ruby_contract.id),
            library_api_callee_contract_hash(ruby_contract.callee),
        );
        assert!(result_domain_depends_on_any_api(
            &ruby.evidence,
            DomainEvidence::Set,
            &ruby_api,
        ));

        let missing_require = crate::lower_source(
            FileId(0),
            "set_missing_require.rb",
            b"def f(values)\n  Set.new(values)\nend\n",
            Lang::Ruby,
            &interner,
        )
        .expect("ruby lowering should succeed");
        assert_eq!(
            result_domain_record_count(&missing_require.evidence, DomainEvidence::Set),
            0,
            "Ruby Set.new must not emit result-domain evidence without require proof"
        );
    }

    #[test]
    fn result_domain_evidence_requires_live_library_api_dependency() {
        let interner = Interner::new();
        let mut il = crate::lower_source(
            FileId(0),
            "set.js",
            b"function f(value) { return new Set([value]).has(value); }",
            Lang::JavaScript,
            &interner,
        )
        .expect("js lowering should succeed");
        let call = call_node_with_result_domain(&il, DomainEvidence::Set)
            .expect("new Set result should carry Set domain evidence");
        assert_eq!(
            nose_semantics::domain_evidence_for_node(&il, call),
            Some(DomainEvidence::Set)
        );

        for record in &mut il.evidence {
            if matches!(record.kind, EvidenceKind::LibraryApi(_)) {
                record.status = EvidenceStatus::Ambiguous;
            }
        }
        assert_eq!(
            nose_semantics::domain_evidence_for_node(&il, call),
            None,
            "receiver-domain proof must close when the LibraryApi dependency is ambiguous"
        );
    }

    #[test]
    fn java_empty_collection_constructor_emits_occurrence_evidence() {
        let interner = Interner::new();
        let il = crate::lower_source(
            FileId(0),
            "C.java",
            b"import java.util.ArrayList;\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
        let contract =
            library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0).unwrap();
        let api = library_api_evidence_ids_in_records(
            &il.evidence,
            library_api_contract_id_hash(contract.id),
            library_api_callee_contract_hash(contract.callee),
        );
        assert_eq!(api.len(), 1);
        assert!(
            il.evidence.iter().any(|record| {
                record.kind == EvidenceKind::Domain(DomainEvidence::Collection)
                    && record.dependencies.len() == 1
                    && api.contains(&record.dependencies[0])
            }),
            "Java constructor result-domain evidence must depend on the LibraryApi occurrence"
        );
    }

    #[test]
    fn java_empty_collection_constructor_wildcard_import_is_dependency_backed() {
        let interner = Interner::new();
        let wildcard = crate::lower_source(
            FileId(0),
            "C.java",
            b"import java.util.*;\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
        let contract =
            library_java_collection_constructor_contract(Lang::Java, "ArrayList", 0).unwrap();
        let api = wildcard
            .evidence
            .iter()
            .find(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash,
                        callee_hash,
                        ..
                    }) if contract_hash == library_api_contract_id_hash(contract.id)
                        && callee_hash == library_api_callee_contract_hash(contract.callee)
                )
            })
            .expect("wildcard java.util import should admit supported ArrayList constructor");
        assert!(api.dependencies.iter().any(|id| {
            wildcard.evidence_record_by_id(*id).is_some_and(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::Import(ImportEvidenceKind::Wildcard { module_hash })
                        if module_hash == stable_symbol_hash("java.util")
                )
            })
        }));

        let shadowed = crate::lower_source(
            FileId(0),
            "C.java",
            b"import java.util.*;\nclass ArrayList<T> {}\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &shadowed.evidence,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            0,
            "local ArrayList type must close the java.util constructor occurrence"
        );

        let explicit_conflict = crate::lower_source(
            FileId(0),
            "C.java",
            b"import java.util.*;\nimport other.ArrayList;\nclass C { Object f() { return new ArrayList<>(); } }\n",
            Lang::Java,
            &interner,
        )
        .expect("java lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &explicit_conflict.evidence,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            0,
            "explicit same-name imports must close java.util wildcard constructor proof"
        );
    }

    #[test]
    fn post_lowering_emits_property_and_rust_option_occurrences() {
        let interner = Interner::new();
        let ts = crate::lower_source(
            FileId(0),
            "t.ts",
            b"function f(xs: number[]) { return xs.length; }\n",
            Lang::TypeScript,
            &interner,
        )
        .expect("typescript lowering should succeed");
        let property_contract =
            library_property_builtin_contract(Lang::TypeScript, "length").unwrap();
        let length_field = ts
            .nodes
            .iter()
            .enumerate()
            .find_map(|(idx, node)| {
                (node.kind == NodeKind::Field
                    && matches!(
                        node.payload,
                        Payload::Name(symbol) if interner.resolve(symbol) == "length"
                    ))
                .then_some((NodeId(idx as u32), node.span))
            })
            .expect("length field should be lowered");
        let property_api = library_api_evidence_ids_at_node(
            &ts.evidence,
            length_field.1,
            NodeKind::Field,
            library_api_contract_id_hash(property_contract.id),
            library_api_callee_contract_hash(property_contract.callee),
            0,
        );
        assert_eq!(
            property_api.len(),
            1,
            "typed exact-collection property access should carry LibraryApi occurrence evidence"
        );

        let rust_some = crate::lower_source(
            FileId(0),
            "t.rs",
            b"fn f(x: i32) -> Option<i32> { Some(x) }\n",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        let some_contract =
            library_rust_option_some_constructor_contract(Lang::Rust, "Some", 1).unwrap();
        let some_call = rust_some
            .nodes
            .iter()
            .enumerate()
            .find_map(|(idx, node)| {
                (node.kind == NodeKind::Call
                    && rust_some
                        .children(NodeId(idx as u32))
                        .first()
                        .is_some_and(|&callee| {
                            matches!(
                                rust_some.node(callee).payload,
                                Payload::Name(symbol) if interner.resolve(symbol) == "Some"
                            )
                        }))
                .then_some(node.span)
            })
            .expect("Some call should be lowered");
        let some_api = library_api_evidence_ids_at(
            &rust_some.evidence,
            some_call,
            library_api_contract_id_hash(some_contract.id),
            library_api_callee_contract_hash(some_contract.callee),
            1,
        );
        assert_eq!(some_api.len(), 1);
        assert!(result_domain_depends_on_api(
            &rust_some.evidence,
            some_call,
            DomainEvidence::Option,
            &some_api,
        ));

        let rust_none = crate::lower_source(
            FileId(0),
            "t.rs",
            b"fn f() -> Option<i32> { None }\n",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        let none_contract = library_rust_option_none_sentinel_contract(Lang::Rust, "None").unwrap();
        let none_var = rust_none
            .nodes
            .iter()
            .find_map(|node| {
                (node.kind == NodeKind::Var
                    && matches!(
                        node.payload,
                        Payload::Name(symbol) if interner.resolve(symbol) == "None"
                    ))
                .then_some(node.span)
            })
            .expect("None var should be lowered");
        let none_api = library_api_evidence_ids_at_node(
            &rust_none.evidence,
            none_var,
            NodeKind::Var,
            library_api_contract_id_hash(none_contract.id),
            library_api_callee_contract_hash(none_contract.callee),
            0,
        );
        assert_eq!(none_api.len(), 1);
        assert!(result_domain_depends_on_api_at_node(
            &rust_none.evidence,
            none_var,
            NodeKind::Var,
            DomainEvidence::Option,
            &none_api,
        ));

        let shadowed_some = crate::lower_source(
            FileId(0),
            "t.rs",
            b"fn Some(x: i32) -> Option<i32> { None }\nfn f(x: i32) -> Option<i32> { Some(x) }\n",
            Lang::Rust,
            &interner,
        )
        .expect("rust lowering should succeed");
        assert_eq!(
            library_api_evidence_count_in_records(
                &shadowed_some.evidence,
                library_api_contract_id_hash(some_contract.id),
                library_api_callee_contract_hash(some_contract.callee),
            ),
            0,
            "local Rust Some item must close the std Option constructor occurrence"
        );
    }

    #[test]
    fn js_static_index_membership_emits_occurrence_evidence() {
        let interner = Interner::new();
        let il = crate::lower_source(
            FileId(0),
            "index.js",
            b"function f(value) { return [\"red\", \"blue\"].indexOf(value) !== -1; }\n",
            Lang::JavaScript,
            &interner,
        )
        .expect("js lowering should succeed");
        let contract =
            library_static_index_membership_contract(Lang::JavaScript, "indexOf", 1).unwrap();
        let api = il
            .evidence
            .iter()
            .find(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                        contract_hash,
                        callee_hash,
                        ..
                    }) if contract_hash == library_api_contract_id_hash(contract.id)
                        && callee_hash == library_api_callee_contract_hash(contract.callee)
                )
            })
            .expect("static index membership should emit a LibraryApi occurrence");
        assert!(api.dependencies.iter().any(|id| {
            il.evidence_record_by_id(*id).is_some_and(|record| {
                matches!(
                    record.kind,
                    EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection)
                )
            })
        }));
    }

    #[test]
    fn core_lowering_emits_java_and_regex_library_api_occurrences() {
        let interner = Interner::new();
        let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);
        import_binding(&mut lo, sp_at(1), "List", "java.util", "List");
        let list = lo.var("List", sp_at(2));
        let callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("of")),
            sp_at(3),
            &[list],
        );
        let item = lo.int_lit("1", sp_at(4));
        lo.add(NodeKind::Call, Payload::None, sp_at(5), &[callee, item]);
        let contract = library_java_collection_factory_contract(Lang::Java, "List", "of")
            .expect("List.of contract");
        assert_eq!(
            library_api_evidence_count(
                &lo,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            1
        );

        let mut lo = Lowering::new(FileId(0), b"", Lang::JavaScript, &interner);
        let regex = lo.str_lit("/x/", sp_at(10));
        lo.record_source_fact(
            sp_at(10),
            SourceFactKind::Literal(nose_il::SourceLiteralKind::Regex),
        );
        let callee = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("test")),
            sp_at(11),
            &[regex],
        );
        let subject = lo.var("subject", sp_at(12));
        lo.add(NodeKind::Call, Payload::None, sp_at(13), &[callee, subject]);
        let contract =
            library_regex_test_contract(Lang::JavaScript, "test", 1).expect("regex test contract");
        assert_eq!(
            library_api_evidence_count(
                &lo,
                library_api_contract_id_hash(contract.id),
                library_api_callee_contract_hash(contract.callee),
            ),
            1
        );
    }

    #[test]
    fn core_lowering_emits_effect_and_place_evidence() {
        let interner = Interner::new();
        let mut lo = Lowering::new(FileId(0), b"", Lang::Java, &interner);

        let receiver = lo.add(
            NodeKind::Var,
            Payload::Name(interner.intern("this")),
            sp_at(1),
            &[],
        );
        let field = lo.add(
            NodeKind::Field,
            Payload::Name(interner.intern("value")),
            sp_at(2),
            &[receiver],
        );
        let value = lo.add(
            NodeKind::Var,
            Payload::Name(interner.intern("next")),
            sp_at(3),
            &[],
        );
        let assign = lo.add(NodeKind::Assign, Payload::None, sp_at(4), &[field, value]);
        let index = lo.add(NodeKind::Index, Payload::None, sp_at(5), &[receiver, value]);
        let index_assign = lo.add(NodeKind::Assign, Payload::None, sp_at(6), &[index, value]);
        let append = lo.add(
            NodeKind::Call,
            Payload::Builtin(Builtin::Append),
            sp_at(7),
            &[receiver, value],
        );

        let field_hash = stable_symbol_hash("value");
        let self_receiver = lo
            .evidence
            .iter()
            .find(|record| {
                record.anchor == EvidenceAnchor::node(sp_at(1), NodeKind::Var)
                    && record.kind == EvidenceKind::Place(PlaceEvidenceKind::SelfReceiver)
            })
            .expect("Java this should emit self-receiver place evidence");
        let self_field = lo
            .evidence
            .iter()
            .find(|record| {
                record.anchor == EvidenceAnchor::node(sp_at(2), NodeKind::Field)
                    && record.kind
                        == EvidenceKind::Place(PlaceEvidenceKind::SelfField { field_hash })
            })
            .expect("Java this.field should emit self-field place evidence");
        assert_eq!(self_field.dependencies, vec![self_receiver.id]);
        let self_field_write = lo
            .evidence
            .iter()
            .find(|record| {
                record.anchor == EvidenceAnchor::node(sp_at(4), NodeKind::Assign)
                    && record.kind
                        == EvidenceKind::Effect(EffectEvidenceKind::SelfFieldWrite { field_hash })
            })
            .expect("Java this.field assignment should emit self-field write evidence");
        assert_eq!(self_field_write.dependencies, vec![self_field.id]);
        assert!(lo.evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::node(sp_at(6), NodeKind::Assign)
                && record.kind
                    == EvidenceKind::Effect(EffectEvidenceKind::NonOverloadableIndexWrite)
        }));
        assert!(lo.evidence.iter().any(|record| {
            record.anchor == EvidenceAnchor::node(sp_at(7), NodeKind::Call)
                && record.kind == EvidenceKind::Effect(EffectEvidenceKind::BuilderAppendCall)
        }));
        assert_ne!(assign, index_assign);
        assert_ne!(append, receiver);
    }
}
