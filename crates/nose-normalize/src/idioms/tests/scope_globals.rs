use super::call_fixtures::*;
use super::support::*;

#[test]
fn method_bool_reduction_uses_lexical_param_scope() {
    let (il, interner, call) =
        typed_method_call_il(Lang::TypeScript, "some", ParamSemantic::Array, true);
    assert!(matches!(
        canon_call(&il, &interner, call),
        CallCanon::Builtin {
            op: Builtin::Any,
            arg_olds
        } if arg_olds.len() == 2
    ));
}

#[test]
fn method_bool_reduction_stops_at_untyped_inner_param_shadow() {
    let (il, interner, call) = typed_method_shadowed_by_untyped_inner_param_il();
    assert!(
        matches!(canon_call(&il, &interner, call), CallCanon::None),
        "an untyped inner parameter must shadow an outer typed parameter"
    );
}

#[test]
fn js_console_print_requires_no_shadowing() {
    let (il, interner, call) = console_log_il(true);
    assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));

    let (il, interner, call) = console_log_il(false);
    assert!(matches!(
        canon_call(&il, &interner, call),
        CallCanon::Builtin {
            op: Builtin::Print,
            arg_olds
        } if arg_olds.len() == 1
    ));
}

#[test]
fn go_math_abs_stays_closed_even_with_import_namespace_proof() {
    let (il, interner, call) = go_math_abs_il(false);
    assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));

    let (il, interner, call) = go_math_abs_il(true);
    assert!(matches!(canon_call(&il, &interner, call), CallCanon::None));
}
