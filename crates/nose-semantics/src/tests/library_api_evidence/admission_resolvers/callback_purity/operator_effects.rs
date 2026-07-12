use super::*;

#[test]
fn value_transform_operators_require_non_dispatching_primitive_evidence() {
    for &surface in VALUE_TRANSFORM_SURFACES {
        assert!(
            !callback_surface_is_admitted(surface, CallbackShape::PureArithmetic, None),
            "{surface:?} must not treat an untyped or overloadable operator as effect-closed"
        );
    }

    let typescript_map = CallbackSurface {
        lang: Lang::TypeScript,
        method: "map",
        domain: DomainEvidence::Array,
    };
    assert!(
        callback_surface_is_admitted(
            typescript_map,
            CallbackShape::PureArithmetic,
            Some(DomainEvidence::Number),
        ),
        "TypeScript numeric evidence should close Number addition dispatch"
    );
    assert!(
        !callback_surface_is_admitted(
            typescript_map,
            CallbackShape::AbstractIntegerArithmetic,
            None,
        ),
        "an abstract JS integer literal may be BigInt and must not prove Number arithmetic"
    );
    assert!(
        !callback_surface_is_admitted(
            typescript_map,
            CallbackShape::PureArithmetic,
            Some(DomainEvidence::Integer),
        ),
        "a coarse JS Integer domain must not stand in for exact Number evidence"
    );
    assert!(
        !callback_surface_is_admitted(typescript_map, CallbackShape::TypeMembership, None),
        "JavaScript instanceof/type-membership can throw and must remain effect-open"
    );
    assert!(
        callback_surface_is_admitted(
            typescript_map,
            CallbackShape::PureUnaryNegation,
            Some(DomainEvidence::Number),
        ),
        "exact TypeScript Number evidence should close unary numeric negation"
    );

    let swift_map = CallbackSurface {
        lang: Lang::Swift,
        method: "map",
        domain: DomainEvidence::Collection,
    };
    assert!(
        !callback_surface_is_admitted(
            swift_map,
            CallbackShape::PureUnaryNegation,
            Some(DomainEvidence::Float),
        ),
        "Swift domain labels do not prove stdlib nominal identity for operator dispatch"
    );

    let ruby_map = CallbackSurface {
        lang: Lang::Ruby,
        method: "map",
        domain: DomainEvidence::Collection,
    };
    assert!(
        !callback_surface_is_admitted(
            ruby_map,
            CallbackShape::PureArithmetic,
            Some(DomainEvidence::Integer),
        ),
        "Ruby Integer operators remain overloadable even with an element-domain hint"
    );
}

#[test]
fn ambiguous_type_membership_fact_cannot_fall_back_to_primitive_equality() {
    let typescript_map = CallbackSurface {
        lang: Lang::TypeScript,
        method: "map",
        domain: DomainEvidence::Array,
    };
    let (mut il, interner, call) =
        callback_surface_il(typescript_map, CallbackShape::TypeMembership, None);
    let record = il
        .evidence
        .iter_mut()
        .find(|record| {
            record.kind
                == EvidenceKind::Source(SourceFactKind::Operator(
                    SourceOperatorKind::TypeMembership,
                ))
        })
        .expect("type-membership source fact");
    record.status = EvidenceStatus::Ambiguous;

    assert!(
        il.nodes
            .iter()
            .enumerate()
            .find(|(_, node)| node.kind == NodeKind::BinOp)
            .map(|(idx, _)| NodeId(idx as u32))
            .is_some_and(|operator| source_operator_at_node(&il, operator).is_none()),
        "the ambiguous source fact should reproduce the resolver's None result"
    );
    assert!(
        admitted_library_method_call_at_call(&il, &interner, call).is_none(),
        "missing/ambiguous Eq source identity must fail closed instead of reinterpreting instanceof"
    );
}
