use super::{value_fp, value_fp_named};
use nose_il::{FileId, Interner, Lang};
use nose_normalize::{normalize, run_unit, Behavior, NormalizeOptions, Value, F64};

const ARTIFACT: &str =
    include_str!("../../../../bench/soundness/0.20.0/source-runtime-calibration.v1.json");

const LARGE_SHR: &str = "function f() { return 9223372036854775807 >> 0; }";
const LARGE_BITAND: &str = "function f() { return 9223372036854775807 & 1; }";
const LARGE_BITNOT: &str = "function f() { return ~9223372036854775807; }";
const NESTED_BITWISE: &str = "function f() { return (\"1\" - 0) & 1; }";
const NESTED_POW: &str = "function f() { return 2 ** (\"a\" - \"b\"); }";
const EXACT_INTEGER_LEFT: &str =
    "function f(a:number,b:number,c:number):number[] { return [-(-1),a,b,c]; }";
const EXACT_INTEGER_RIGHT: &str =
    "function f(a:number,b:number,c:number):number[] { return [1,a,b,c]; }";
const EXACT_ZERO_LEFT: &str =
    "function f(a:number,b:number,c:number):number[] { return [-(-0),a,b,c]; }";
const EXACT_ZERO_RIGHT: &str =
    "function f(a:number,b:number,c:number):number[] { return [0,a,b,c]; }";
const HELPER_ZERO_LEFT: &str = r#"
function identity(value: number): number { return value; }
function f(x: number, a: number, b: number, c: number): number[] {
    return [identity(x - x), a, b, c];
}
"#;
const HELPER_ZERO_RIGHT: &str = r#"
function f(x: number, a: number, b: number, c: number): number[] {
    return [x - x, a, b, c];
}
"#;
const FACTOR_LEFT: &str = "function f(x:number,y:number,k:number):number { return x*k + y*k; }";
const FACTOR_RIGHT: &str = "function f(x:number,y:number,k:number):number { return (x+y)*k; }";
const REDUCE_LEFT: &str = "function f(xs:number[],a:number,b:number):number { let total=0; for(const x of xs){ total += (x+a)+b; } return total; }";
const REDUCE_RIGHT: &str = "function f(xs:number[],a:number,b:number):number { let total=0; for(const x of xs){ total += x+(a+b); } return total; }";

fn behavior(interner: &Interner, source: &str, args: &[Value]) -> Option<Behavior> {
    let il = nose_frontend::lower_source(
        FileId(0),
        "review.ts",
        source.as_bytes(),
        Lang::TypeScript,
        interner,
    )
    .expect("review calibration source must lower");
    let core = normalize(
        &il,
        interner,
        &NormalizeOptions {
            oracle: true,
            ..NormalizeOptions::default()
        },
    );
    let root = core
        .units
        .iter()
        .find(|unit| {
            unit.name
                .is_some_and(|symbol| interner.resolve(symbol) == "f")
        })
        .map(|unit| unit.root)
        .expect("review calibration source must contain function f");
    run_unit(&core, interner, root, args)
}

fn product_unit(interner: &Interner, source: &str) -> nose_detect::UnitFeat {
    let il = nose_frontend::lower_source(
        FileId(0),
        "review.ts",
        source.as_bytes(),
        Lang::TypeScript,
        interner,
    )
    .expect("review calibration source must lower");
    nose_detect::units_of_file(&il, interner, &nose_detect::DetectOptions::default())
        .into_iter()
        .find(|unit| unit.name.as_deref() == Some("f"))
        .expect("function f must reach product extraction")
}

fn returned(interner: &Interner, source: &str, args: &[Value]) -> Value {
    behavior(interner, source, args)
        .expect("review calibration source must interpret")
        .ret
}

fn integer(value: Value) -> i64 {
    match value {
        Value::Int(value) => value,
        other => panic!("expected calibrated integer, got {other:?}"),
    }
}

fn float_bits(value: Value) -> String {
    match value {
        Value::Float(F64(value)) => format!("{:016x}", value.to_bits()),
        Value::Int(value) => format!("{:016x}", (value as f64).to_bits()),
        other => panic!("expected calibrated Number, got {other:?}"),
    }
}

fn assert_exact_integer_equivalence(interner: &Interner, edges: &serde_json::Value) {
    let exact_left = value_fp(interner, EXACT_INTEGER_LEFT, Lang::TypeScript);
    let exact_right = value_fp(interner, EXACT_INTEGER_RIGHT, Lang::TypeScript);
    assert_eq!(exact_left, exact_right);
    assert!(
        exact_left.len() >= 4,
        "regression pair must remain above the product exact-value floor"
    );
    let exact_args = [
        Value::Float(F64(2.0)),
        Value::Float(F64(3.5)),
        Value::Float(F64(-4.0)),
    ];
    assert_eq!(
        behavior(interner, EXACT_INTEGER_LEFT, &exact_args),
        behavior(interner, EXACT_INTEGER_RIGHT, &exact_args),
        "equivalent JavaScript Numbers must not manufacture a hard false-merge"
    );
    assert_eq!(edges["exact_integer_equivalence"].as_bool(), Some(true));

    let zero_left = value_fp(interner, EXACT_ZERO_LEFT, Lang::TypeScript);
    let zero_right = value_fp(interner, EXACT_ZERO_RIGHT, Lang::TypeScript);
    assert_eq!(zero_left, zero_right);
    assert!(zero_left.len() >= 4);
    assert_eq!(
        behavior(interner, EXACT_ZERO_LEFT, &exact_args),
        behavior(interner, EXACT_ZERO_RIGHT, &exact_args),
        "double-negated positive zero must not manufacture a hard false-merge"
    );
    assert_eq!(edges["exact_zero_equivalence"].as_bool(), Some(true));
}

fn assert_helper_inline_zero_equivalence(interner: &Interner, edges: &serde_json::Value) {
    let helper_fp = value_fp_named(interner, HELPER_ZERO_LEFT, Lang::TypeScript, "f");
    let inline_fp = value_fp_named(interner, HELPER_ZERO_RIGHT, Lang::TypeScript, "f");
    assert_eq!(helper_fp, inline_fp);

    let helper_unit = product_unit(interner, HELPER_ZERO_LEFT);
    let inline_unit = product_unit(interner, HELPER_ZERO_RIGHT);
    assert!(nose_detect::exact_claim_eligible(&helper_unit));
    assert!(nose_detect::exact_claim_eligible(&inline_unit));
    assert_eq!(helper_unit.value, inline_unit.value);

    let args = [
        Value::Float(F64(1.0)),
        Value::Float(F64(2.0)),
        Value::Float(F64(-2.0)),
        Value::Float(F64(-0.0)),
    ];
    assert_eq!(
        behavior(interner, HELPER_ZERO_LEFT, &args),
        behavior(interner, HELPER_ZERO_RIGHT, &args),
        "an internal identity call must preserve the same +0 lane as its inlined form"
    );
    assert_eq!(
        edges["helper_inline_zero_equivalence"].as_bool(),
        Some(true)
    );
}

#[test]
fn production_javascript_number_boundaries_match_independent_node_runtime() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let edges = &artifact["observations"]["node"]["number_edges"];
    let interner = Interner::new();

    let large = &edges["large_literal_bitwise"];
    assert_eq!(
        integer(returned(&interner, LARGE_SHR, &[])),
        large["shift_right"].as_i64().expect("large literal shift")
    );
    assert_eq!(
        integer(returned(&interner, LARGE_BITAND, &[])),
        large["bitand"].as_i64().expect("large literal bitand")
    );
    assert_eq!(
        integer(returned(&interner, LARGE_BITNOT, &[])),
        large["bitnot"].as_i64().expect("large literal bitnot")
    );

    assert_eq!(edges["nested_bitwise"].as_str(), Some("1"));
    assert_eq!(edges["nested_pow_nan"].as_bool(), Some(true));
    assert!(behavior(&interner, NESTED_BITWISE, &[]).is_none());
    assert!(behavior(&interner, NESTED_POW, &[]).is_none());

    assert_exact_integer_equivalence(&interner, edges);
    assert_helper_inline_zero_equivalence(&interner, edges);

    assert_ne!(
        value_fp(&interner, FACTOR_LEFT, Lang::TypeScript),
        value_fp(&interner, FACTOR_RIGHT, Lang::TypeScript)
    );
    let factor_args = [
        Value::Float(F64(0.0)),
        Value::Float(F64(1.0)),
        Value::Float(F64(f64::INFINITY)),
    ];
    assert!(matches!(
        returned(&interner, FACTOR_LEFT, &factor_args),
        Value::Float(F64(value)) if value.is_nan()
    ));
    assert!(matches!(
        returned(&interner, FACTOR_RIGHT, &factor_args),
        Value::Float(F64(value)) if value == f64::INFINITY
    ));
    assert_eq!(
        edges["factor_distribution"]["left_nan"].as_bool(),
        Some(true)
    );
    assert_eq!(
        edges["factor_distribution"]["right"].as_str(),
        Some("Infinity")
    );

    assert_ne!(
        value_fp(&interner, REDUCE_LEFT, Lang::TypeScript),
        value_fp(&interner, REDUCE_RIGHT, Lang::TypeScript)
    );
    let reduce_args = [
        Value::List(vec![Value::Float(F64(1e16))]),
        Value::Float(F64(-1e16)),
        Value::Float(F64(1.0)),
    ];
    assert_eq!(
        float_bits(returned(&interner, REDUCE_LEFT, &reduce_args)),
        edges["reduce_association"]["left_bits"]
            .as_str()
            .expect("reduce left bits")
    );
    assert_eq!(
        float_bits(returned(&interner, REDUCE_RIGHT, &reduce_args)),
        edges["reduce_association"]["right_bits"]
            .as_str()
            .expect("reduce right bits")
    );
}
