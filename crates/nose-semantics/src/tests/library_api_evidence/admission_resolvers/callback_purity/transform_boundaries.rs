use super::*;

#[test]
fn pure_transform_callback_obligation_admits_inline_local_value_computation() {
    for &surface in TRANSFORM_SURFACES {
        assert!(
            callback_surface_is_admitted(surface, CallbackShape::PureIdentity, None),
            "{surface:?} should admit a pure inline value callback"
        );
    }

    let typescript_map = CallbackSurface {
        lang: Lang::TypeScript,
        method: "map",
        domain: DomainEvidence::Array,
    };
    assert!(
        callback_surface_is_admitted(typescript_map, CallbackShape::PureCollection, None),
        "a proof-backed collection literal should remain effect-closed"
    );

    let swift_map = CallbackSurface {
        lang: Lang::Swift,
        method: "map",
        domain: DomainEvidence::Collection,
    };
    assert!(
        !callback_surface_is_admitted(swift_map, CallbackShape::PureCollection, None),
        "Swift collection literals remain contextual protocol dispatch"
    );
    assert!(
        !callback_surface_is_admitted(swift_map, CallbackShape::PureLiteralPredicate, None),
        "Swift transform literals remain contextual protocol dispatch"
    );

    let ruby_map = CallbackSurface {
        lang: Lang::Ruby,
        method: "map",
        domain: DomainEvidence::Collection,
    };
    assert!(
        !callback_surface_is_admitted(ruby_map, CallbackShape::PureMap, None),
        "map/pair construction remains effect-open without key hashing proof"
    );
}

#[test]
fn pure_transform_callback_obligation_rejects_each_effect_and_coordinate_boundary() {
    for &surface in TRANSFORM_SURFACES {
        for shape in [
            CallbackShape::ObservedCall,
            CallbackShape::CapturedAssignment,
            CallbackShape::CustomDispatch,
            CallbackShape::UnprovenFieldRead,
            CallbackShape::FreeNameRead,
            CallbackShape::Throwing,
            CallbackShape::UnprovenSequence,
        ] {
            assert!(
                !callback_surface_is_admitted(surface, shape, None),
                "{surface:?} must reject callback boundary {shape:?}"
            );
        }
        assert!(
            !callback_surface_is_admitted(surface, CallbackShape::ExtraObservedParam, None),
            "{surface:?} must reject an extra callback coordinate"
        );
        for shape in [
            CallbackShape::DefaultedParam,
            CallbackShape::RestParam,
            CallbackShape::DestructuredParam,
        ] {
            assert!(
                !callback_surface_is_admitted(surface, shape, None),
                "{surface:?} must reject non-plain unary parameter shape {shape:?}"
            );
        }
        if js_like_lang(surface.lang) {
            for shape in [CallbackShape::ImplicitArguments, CallbackShape::DynamicThis] {
                assert!(
                    !callback_surface_is_admitted(surface, shape, None),
                    "{surface:?} must reject implicit callback context {shape:?}"
                );
            }
        }
    }
}
