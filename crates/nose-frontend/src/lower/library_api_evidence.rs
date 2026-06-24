use super::*;

struct LibraryApiEvidencePlan {
    id: LibraryApiContractId,
    callee: LibraryApiCalleeContract,
    dependencies: Vec<EvidenceId>,
    pack_id: &'static str,
    rule: &'static str,
    result_domain: Option<DomainEvidence>,
}

impl<'a> Lowering<'a> {
    pub(crate) fn record_library_api_evidence_for_call(&mut self, span: Span, children: &[NodeId]) {
        let Some((&callee, args)) = children.split_first() else {
            return;
        };
        let arg_count = args.len();
        if let Some(plan) = self.library_api_contract_for_call(span, callee, arg_count) {
            let api = self.record_evidence_with_pack_dependencies(
                EvidenceAnchor::node(span, NodeKind::Call),
                EvidenceKind::LibraryApi(LibraryApiEvidenceKind::Contract {
                    contract_hash: library_api_contract_id_hash(plan.id),
                    callee_hash: library_api_callee_contract_hash(plan.callee),
                    arity: arg_count as u16,
                }),
                plan.pack_id,
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
                    contract.pack_id,
                    JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
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
                            contract.pack_id,
                            JS_LIKE_BUILTIN_ARRAY_PRODUCER_ID,
                            Some(library_map_key_view_wrapper_result_domain(contract)),
                        )
                    },
                )
            })
            .or_else(|| {
                library_promise_resolve_contract(self.lang, receiver, method, arg_count).map(
                    |contract| {
                        (
                            contract.id,
                            contract.callee,
                            contract.result.qualified_path,
                            true,
                            contract.result.receiver,
                            contract.pack_id,
                            JS_LIKE_BUILTIN_PROMISE_PRODUCER_ID,
                            Some(contract.result.result_domain),
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
            pack_id: contract.5,
            rule: contract.6,
            result_domain: contract.7,
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
            pack_id: contract.pack_id,
            rule: JS_LIKE_BUILTIN_BOOLEAN_PRODUCER_ID,
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
                        pack_id: PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
                        rule: PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
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
                        pack_id: PYTHON_STDLIB_COLLECTION_FACTORY_PACK_ID,
                        rule: PYTHON_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
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
            pack_id: contract.pack_id,
            rule: PYTHON_STDLIB_MATH_PRODUCER_ID,
            result_domain: None,
        })
    }

    fn java_util_static_member_api_contract(
        &mut self,
        callee: NodeId,
        arg_count: usize,
    ) -> Option<LibraryApiEvidencePlan> {
        let (receiver_node, receiver, method) = self.static_member_callee(callee)?;
        let (id, callee_contract, pack_id, rule, result_domain) =
            library_java_collection_factory_contract(self.lang, receiver, method)
                .map(|contract| {
                    let rule = match contract.pack_id {
                        JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID => {
                            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID
                        }
                        _ => JAVA_STDLIB_COLLECTION_FACTORY_PRODUCER_ID,
                    };
                    (
                        contract.id,
                        contract.callee,
                        contract.pack_id,
                        rule,
                        library_collection_factory_result_domain_for_arity(contract, arg_count),
                    )
                })
                .or_else(|| {
                    library_java_map_factory_contract(self.lang, receiver, method).map(|contract| {
                        let rule = match contract.pack_id {
                            JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PACK_ID => {
                                JAVA_GUAVA_IMMUTABLE_COLLECTION_FACTORY_PRODUCER_ID
                            }
                            _ => JAVA_STDLIB_MAP_FACTORY_PRODUCER_ID,
                        };
                        (
                            contract.id,
                            contract.callee,
                            contract.pack_id,
                            rule,
                            library_api_materialized_result_domain_for_arity(
                                contract.id,
                                contract.callee,
                                arg_count as u16,
                            ),
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
                                contract.pack_id,
                                JAVA_STDLIB_MAP_ENTRY_PRODUCER_ID,
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
                            contract.pack_id,
                            JAVA_STDLIB_STATIC_COLLECTION_ADAPTER_PRODUCER_ID,
                            None,
                        )
                    })
                })?;
        self.java_util_static_member_evidence_plan(
            receiver_node,
            id,
            callee_contract,
            pack_id,
            rule,
            result_domain,
        )
    }

    fn java_util_static_member_evidence_plan(
        &mut self,
        receiver_node: NodeId,
        id: LibraryApiContractId,
        callee_contract: LibraryApiCalleeContract,
        pack_id: &'static str,
        rule: &'static str,
        result_domain: Option<DomainEvidence>,
    ) -> Option<LibraryApiEvidencePlan> {
        let (module, expected_receiver, requires_import) = match callee_contract {
            LibraryApiCalleeContract::JavaUtilStaticMember { receiver, .. } => {
                ("java.util", receiver, true)
            }
            LibraryApiCalleeContract::JavaStaticMember {
                module,
                receiver,
                requires_import_for_simple_receiver,
                ..
            } => (module, receiver, requires_import_for_simple_receiver),
            _ => return None,
        };
        let dependencies = if requires_import {
            vec![self.record_imported_binding_symbol_for_node(
                receiver_node,
                module,
                expected_receiver,
            )?]
        } else {
            Vec::new()
        };
        Some(LibraryApiEvidencePlan {
            id,
            callee: callee_contract,
            dependencies,
            pack_id,
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
            pack_id: contract.pack_id,
            rule: JS_LIKE_BUILTIN_STATIC_INDEX_MEMBERSHIP_PRODUCER_ID,
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
            pack_id: contract.pack_id,
            rule: JS_LIKE_BUILTIN_REGEX_PRODUCER_ID,
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
                    contract.pack_id,
                    JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
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
                            contract.pack_id,
                            JS_LIKE_BUILTIN_COLLECTION_CONSTRUCTOR_PRODUCER_ID,
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
            pack_id: contract.3,
            rule: contract.4,
            result_domain: contract.5,
        })
    }
}
