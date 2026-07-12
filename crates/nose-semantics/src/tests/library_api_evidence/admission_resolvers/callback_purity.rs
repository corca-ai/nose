use super::*;

mod nested_predicates;
mod nested_sources;
mod operator_effects;
mod transform_boundaries;

#[derive(Clone, Copy, Debug)]
enum CallbackShape {
    PureIdentity,
    PureCollection,
    PureMap,
    PureArithmetic,
    PureUnaryNegation,
    AbstractIntegerArithmetic,
    PurePredicate,
    PureLiteralPredicate,
    DefaultedParam,
    RestParam,
    DestructuredParam,
    ObservedCall,
    CapturedAssignment,
    ExtraObservedParam,
    CustomDispatch,
    UnprovenFieldRead,
    ImplicitArguments,
    DynamicThis,
    FreeNameRead,
    TypeMembership,
    Throwing,
    UnprovenSequence,
}

#[derive(Clone, Copy, Debug)]
struct CallbackSurface {
    lang: Lang,
    method: &'static str,
    domain: DomainEvidence,
}

const TRANSFORM_SURFACES: &[CallbackSurface] = &[
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "map",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "filter",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "flatMap",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "map",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "filter",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "flatMap",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::Ruby,
        method: "map",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Ruby,
        method: "filter",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Ruby,
        method: "reject",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Swift,
        method: "map",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Swift,
        method: "filter",
        domain: DomainEvidence::Collection,
    },
];

const VALUE_TRANSFORM_SURFACES: &[CallbackSurface] = &[
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "map",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "flatMap",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "map",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "flatMap",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::Ruby,
        method: "map",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Swift,
        method: "map",
        domain: DomainEvidence::Collection,
    },
];

const PREDICATE_SURFACES: &[CallbackSurface] = &[
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "some",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::JavaScript,
        method: "every",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "some",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::TypeScript,
        method: "every",
        domain: DomainEvidence::Array,
    },
    CallbackSurface {
        lang: Lang::Ruby,
        method: "any?",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Ruby,
        method: "all?",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Swift,
        method: "allSatisfy",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Rust,
        method: "any",
        domain: DomainEvidence::Collection,
    },
    CallbackSurface {
        lang: Lang::Rust,
        method: "all",
        domain: DomainEvidence::Collection,
    },
];

fn callback_node(
    b: &mut IlBuilder,
    interner: &Interner,
    lang: Lang,
    shape: CallbackShape,
    span_base: u32,
) -> (NodeId, NodeId) {
    let param_shape = match shape {
        CallbackShape::DefaultedParam => Some("js_default_parameter"),
        CallbackShape::RestParam => Some("js_rest_parameter"),
        CallbackShape::DestructuredParam => Some("js_destructured_parameter"),
        _ => None,
    };
    let param_children = param_shape
        .map(|tag| {
            let marker = b.add(
                NodeKind::Raw,
                Payload::Name(interner.intern(tag)),
                sp(span_base),
                &[],
            );
            vec![marker]
        })
        .unwrap_or_default();
    let value_param = b.add(
        NodeKind::Param,
        Payload::Cid(1),
        sp(span_base),
        &param_children,
    );
    let value = b.add(NodeKind::Var, Payload::Cid(1), sp(span_base + 1), &[]);
    let mut params = vec![value_param];
    let body_child = match shape {
        CallbackShape::PureIdentity
        | CallbackShape::DefaultedParam
        | CallbackShape::RestParam
        | CallbackShape::DestructuredParam => {
            callback_value_statement(b, lang, sp(span_base + 4), value)
        }
        CallbackShape::PureCollection => {
            let collection = b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("array")),
                sp(span_base + 3),
                &[value],
            );
            callback_value_statement(b, lang, sp(span_base + 4), collection)
        }
        CallbackShape::PureMap => {
            let key = b.add(
                NodeKind::Lit,
                Payload::LitStr(stable_symbol_hash("value")),
                sp(span_base + 2),
                &[],
            );
            let pair = b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("pair")),
                sp(span_base + 3),
                &[key, value],
            );
            let map = b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("hash")),
                sp(span_base + 4),
                &[pair],
            );
            callback_value_statement(b, lang, sp(span_base + 5), map)
        }
        CallbackShape::PureArithmetic => {
            let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(span_base + 2), &[]);
            let sum = b.add(
                NodeKind::BinOp,
                Payload::Op(Op::Add),
                sp(span_base + 3),
                &[value, one],
            );
            callback_value_statement(b, lang, sp(span_base + 4), sum)
        }
        CallbackShape::PureUnaryNegation => {
            let negated = b.add(
                NodeKind::UnOp,
                Payload::Op(Op::Neg),
                sp(span_base + 3),
                &[value],
            );
            callback_value_statement(b, lang, sp(span_base + 4), negated)
        }
        CallbackShape::AbstractIntegerArithmetic => {
            let abstract_integer = b.add(
                NodeKind::Lit,
                Payload::Lit(LitClass::Int),
                sp(span_base + 2),
                &[],
            );
            let one = b.add(NodeKind::Lit, Payload::LitInt(1), sp(span_base + 3), &[]);
            let sum = b.add(
                NodeKind::BinOp,
                Payload::Op(Op::Add),
                sp(span_base + 4),
                &[abstract_integer, one],
            );
            callback_value_statement(b, lang, sp(span_base + 5), sum)
        }
        CallbackShape::PurePredicate => {
            let zero = b.add(NodeKind::Lit, Payload::LitInt(0), sp(span_base + 2), &[]);
            let comparison = b.add(
                NodeKind::BinOp,
                Payload::Op(Op::Gt),
                sp(span_base + 3),
                &[value, zero],
            );
            callback_value_statement(b, lang, sp(span_base + 4), comparison)
        }
        CallbackShape::PureLiteralPredicate => {
            let value = b.add(
                NodeKind::Lit,
                Payload::LitBool(true),
                sp(span_base + 2),
                &[],
            );
            callback_value_statement(b, lang, sp(span_base + 4), value)
        }
        CallbackShape::ObservedCall => {
            let callee = b.add(
                NodeKind::Var,
                Payload::Name(interner.intern("observe")),
                sp(span_base + 2),
                &[],
            );
            b.add(
                NodeKind::Call,
                Payload::None,
                sp(span_base + 3),
                &[callee, value],
            )
        }
        CallbackShape::CapturedAssignment => {
            let captured = b.add(NodeKind::Var, Payload::Cid(9), sp(span_base + 2), &[]);
            b.add(
                NodeKind::Assign,
                Payload::None,
                sp(span_base + 3),
                &[captured, value],
            )
        }
        CallbackShape::ExtraObservedParam => {
            let index_param = b.add(NodeKind::Param, Payload::Cid(2), sp(span_base + 2), &[]);
            let index = b.add(NodeKind::Var, Payload::Cid(2), sp(span_base + 3), &[]);
            params.push(index_param);
            b.add(
                NodeKind::BinOp,
                Payload::Op(Op::Add),
                sp(span_base + 4),
                &[value, index],
            )
        }
        CallbackShape::CustomDispatch => {
            let callee = b.add(
                NodeKind::Field,
                Payload::Name(interner.intern("transform")),
                sp(span_base + 2),
                &[value],
            );
            b.add(NodeKind::Call, Payload::None, sp(span_base + 3), &[callee])
        }
        CallbackShape::UnprovenFieldRead => b.add(
            NodeKind::Field,
            Payload::Name(interner.intern("value")),
            sp(span_base + 2),
            &[value],
        ),
        CallbackShape::ImplicitArguments => b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("arguments")),
            sp(span_base + 2),
            &[],
        ),
        CallbackShape::DynamicThis => b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("this")),
            sp(span_base + 2),
            &[],
        ),
        CallbackShape::FreeNameRead => b.add(
            NodeKind::Var,
            Payload::Name(interner.intern("globalValue")),
            sp(span_base + 2),
            &[],
        ),
        CallbackShape::TypeMembership => {
            let left = b.add(NodeKind::Lit, Payload::LitInt(1), sp(span_base + 2), &[]);
            let right = b.add(NodeKind::Lit, Payload::LitInt(2), sp(span_base + 3), &[]);
            b.add(
                NodeKind::BinOp,
                Payload::Op(Op::Eq),
                sp(span_base + 4),
                &[left, right],
            )
        }
        CallbackShape::Throwing => {
            b.add(NodeKind::Throw, Payload::None, sp(span_base + 2), &[value])
        }
        CallbackShape::UnprovenSequence => {
            let sequence = b.add(
                NodeKind::Seq,
                Payload::Name(interner.intern("swift_prefix_operator")),
                sp(span_base + 2),
                &[value],
            );
            callback_value_statement(b, lang, sp(span_base + 4), sequence)
        }
    };
    let body = b.add(
        NodeKind::Block,
        Payload::None,
        sp(span_base + 5),
        &[body_child],
    );
    params.push(body);
    (
        b.add(NodeKind::Lambda, Payload::None, sp(span_base + 6), &params),
        value_param,
    )
}

fn callback_value_statement(b: &mut IlBuilder, lang: Lang, span: Span, value: NodeId) -> NodeId {
    let kind = if lang == Lang::Ruby {
        NodeKind::ExprStmt
    } else {
        NodeKind::Return
    };
    b.add(kind, Payload::None, span, &[value])
}

fn callback_surface_il(
    surface: CallbackSurface,
    shape: CallbackShape,
    param_domain: Option<DomainEvidence>,
) -> (Il, Interner, NodeId) {
    let interner = Interner::new();
    let mut b = IlBuilder::new(FileId(0));
    let receiver = b.add(NodeKind::Var, Payload::Cid(0), sp(800), &[]);
    let callee = b.add(
        NodeKind::Field,
        Payload::Name(interner.intern(surface.method)),
        sp(801),
        &[receiver],
    );
    let (callback, callback_param) = callback_node(&mut b, &interner, surface.lang, shape, 802);
    let call = b.add(NodeKind::Call, Payload::None, sp(820), &[callee, callback]);
    let root = b.add(NodeKind::Func, Payload::None, sp(821), &[call]);
    let mut il = finish_il(b, root, surface.lang);
    il.evidence.push(evidence(
        0,
        EvidenceAnchor::node(il.node(receiver).span, il.kind(receiver)),
        EvidenceKind::Domain(surface.domain),
        EvidenceStatus::Asserted,
    ));
    if let Some(domain) = param_domain {
        il.evidence.push(evidence(
            1,
            EvidenceAnchor::node(il.node(callback_param).span, NodeKind::Param),
            EvidenceKind::Domain(domain),
            EvidenceStatus::Asserted,
        ));
    }
    if matches!(shape, CallbackShape::PureCollection) {
        let sequence = il
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Seq)
            .map(|index| NodeId(index as u32))
            .expect("pure collection callback sequence");
        il.evidence.push(language_core_evidence(
            3,
            EvidenceAnchor::sequence(il.node(sequence).span),
            EvidenceKind::SequenceSurface(SequenceSurfaceKind::Collection),
            EvidenceStatus::Asserted,
            surface.lang,
        ));
    }
    if matches!(shape, CallbackShape::PureMap) {
        for (offset, sequence_kind) in [SequenceSurfaceKind::Pair, SequenceSurfaceKind::Map]
            .into_iter()
            .enumerate()
        {
            let sequence = il
                .nodes
                .iter()
                .enumerate()
                .find(|(_, node)| {
                    node.kind == NodeKind::Seq
                        && sequence_surface_kind_for_tag(
                            il.meta.lang,
                            match node.payload {
                                Payload::Name(name) => Some(interner.resolve(name)),
                                _ => None,
                            },
                        ) == Some(sequence_kind)
                })
                .map(|(index, _)| NodeId(index as u32))
                .expect("pure map callback sequence surface");
            il.evidence.push(language_core_evidence(
                5 + offset as u32,
                EvidenceAnchor::sequence(il.node(sequence).span),
                EvidenceKind::SequenceSurface(sequence_kind),
                EvidenceStatus::Asserted,
                surface.lang,
            ));
        }
    }
    if matches!(shape, CallbackShape::TypeMembership) {
        let operator = il
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::BinOp)
            .map(|index| NodeId(index as u32))
            .expect("type-membership callback operator");
        il.evidence.push(evidence(
            4,
            EvidenceAnchor::source_span(il.node(operator).span),
            EvidenceKind::Source(SourceFactKind::Operator(SourceOperatorKind::TypeMembership)),
            EvidenceStatus::Asserted,
        ));
    }
    let contract = library_method_call_contract(surface.lang, surface.method, 1)
        .expect("callback surface contract");
    il.evidence
        .push(library_api_record_with_provenance_and_arity(
            2,
            il.node(call).span,
            contract.id,
            contract.callee,
            1,
            EvidenceStatus::Asserted,
            &[0],
            contract.pack_id,
            contract.producer_id,
        ));
    (il, interner, call)
}

fn callback_surface_is_admitted(
    surface: CallbackSurface,
    shape: CallbackShape,
    param_domain: Option<DomainEvidence>,
) -> bool {
    let (il, interner, call) = callback_surface_il(surface, shape, param_domain);
    admitted_library_method_call_at_call(&il, &interner, call).is_some()
}

#[test]
fn pure_predicate_obligation_preserves_existing_terminal_effect_boundary() {
    for &surface in PREDICATE_SURFACES {
        assert!(
            callback_surface_is_admitted(surface, CallbackShape::PureLiteralPredicate, None),
            "{surface:?} should retain pure inline predicate admission"
        );
        assert!(
            !callback_surface_is_admitted(surface, CallbackShape::ObservedCall, None),
            "{surface:?} should retain the observed predicate-effect split"
        );
    }
}

#[test]
fn predicate_coordinate_policy_matrix_is_explicit() {
    for &surface in PREDICATE_SURFACES {
        let admitted =
            callback_surface_is_admitted(surface, CallbackShape::ExtraObservedParam, None);
        assert_eq!(
            admitted,
            surface.lang == Lang::Rust,
            "{surface:?} unary-value coordinate policy drifted"
        );
    }
}

#[test]
fn predicate_and_transform_obligations_remain_separate() {
    let javascript_filter = CallbackSurface {
        lang: Lang::JavaScript,
        method: "filter",
        domain: DomainEvidence::Array,
    };
    assert!(
        callback_surface_is_admitted(javascript_filter, CallbackShape::PurePredicate, None),
        "the existing pure-predicate comparison perimeter must stay admitted"
    );
}
