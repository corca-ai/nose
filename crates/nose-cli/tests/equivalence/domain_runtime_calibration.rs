use super::{first_func, value_fp};
use nose_il::{FileId, Interner, Lang};
use nose_normalize::{normalize, run_unit, Behavior, NormalizeOptions, Value, F64};

const ARTIFACT: &str =
    include_str!("../../../../bench/soundness/0.20.0/source-runtime-calibration.v1.json");

const STRING_FORWARD: &str = "def forward(a, b):\n    return a + b\n";
const STRING_REVERSE: &str = "def reverse(a, b):\n    return b + a\n";
const FLOAT_LEFT: &str = "def left(a, b, c):\n    return (a + b) + c\n";
const FLOAT_RIGHT: &str = "def right(a, b, c):\n    return a + (b + c)\n";
const TS_FLOAT_LEFT: &str =
    "function left(a: number, b: number, c: number): number { return (a + b) + c; }\n";
const TS_FLOAT_RIGHT: &str =
    "function right(a: number, b: number, c: number): number { return a + (b + c); }\n";
const TS_DERIVED_LEFT: &str = "function left(a:number,b:number,c:number,d:number,e:number,f:number): number { return (a*b + c*d) + e*f; }\n";
const TS_DERIVED_RIGHT: &str = "function right(a:number,b:number,c:number,d:number,e:number,f:number): number { return a*b + (c*d + e*f); }\n";
const TS_DIV_PAIR_LEFT: &str = "function left(x: number): number { return (x / 0) + 1; }\n";
const TS_DIV_PAIR_RIGHT: &str = "function right(x: number): number { return (0 / 0) + (x * 0); }\n";
const TS_POSITIVE_DIV_ZERO: &str = "function f(): number { return 1 / 0; }\n";
const TS_NEGATIVE_DIV_ZERO: &str = "function f(): number { return -1 / 0; }\n";
const TS_ZERO_DIV_ZERO: &str = "function f(): number { return 0 / 0; }\n";
const TS_NAN_TRUTHY: &str = "function f(x: number): number { return x ? 1 : 2; }\n";
const TS_NAN_NOT_EQUAL_ZERO: &str = "function f(x: number): number { return x !== 0 ? 1 : 2; }\n";
const JS_ARRAY_TRUTHY: &str = "function f(x) { return x ? 1 : 2; }\n";
const JS_ARRAY_NOT: &str = "function f() { return ![]; }\n";
const TS_SHIFT_LEFT: &str = "function f(a: number, b: number): number { return a << b; }\n";
const TS_SHIFT_RIGHT: &str = "function f(a: number, b: number): number { return a >> b; }\n";
const TS_LITERAL_LEFT: &str =
    "function f(): number { return (100000000*100000000 + -100000000*100000000) + 1; }\n";
const TS_LITERAL_RIGHT: &str =
    "function f(): number { return 100000000*100000000 + (-100000000*100000000 + 1); }\n";
const TS_BITWISE_LEFT: &str = "function f(): number { return ((3|0)*(3|0))*4503599627370495; }\n";
const TS_BITWISE_RIGHT: &str = "function f(): number { return (3|0)*((3|0)*4503599627370495); }\n";
const JS_BITWISE_COERCIONS: &str = "function f() { return (true & 3) | (null | 0); }\n";
const TS_OVERFLOW_PRODUCT: &str = "function f(): number { return 4611686018427387904 * 4; }\n";
const TS_ZERO_PRODUCT: &str = "function f(): number { return 0 * 4; }\n";
const JS_COERCIVE_POW: &str = "function f() { return \"2\" ** \"3\"; }\n";
const PYTHON_BITAND: &str = "def bitand(a, b):\n    return a & b\n";
const JS_BITAND: &str = "function bitand(a, b) { return a & b; }\n";
const MUTATE_ZERO: &str = "def mutate(a, value):\n    a[0] = value\n    return a\n";
const MUTATE_ONE: &str = "def mutate(a, value):\n    a[1] = value\n    return a\n";

#[derive(Clone, Debug, PartialEq, Eq)]
struct JsNumberEdges {
    division_pair_left: String,
    division_pair_right: String,
    bitwise_assoc_bits: (String, String),
    bitwise_coercions: i64,
    empty_array_not: bool,
    empty_array_truthy: bool,
    literal_assoc_bits: (String, String),
    overflow_product_bits: (String, String),
    positive_div_zero: String,
    negative_div_zero: String,
    zero_div_zero_nan: bool,
    nan_truthy: bool,
    nan_not_equal_zero: bool,
    shift_left: i64,
    shift_right: i64,
    shift_masked: i64,
}

#[derive(Clone)]
struct CalibrationReceipt {
    string_frontend: (Vec<u64>, Vec<u64>),
    string_interpreter: (Value, Value),
    float_frontend_distinct: (bool, bool),
    float_interpreter_bits: ((String, String), (String, String)),
    derived_float_frontend_distinct: bool,
    derived_float_interpreter_bits: (String, String),
    js_number_frontend_distinct: [bool; 5],
    js_number_interpreter: JsNumberEdges,
    js_pow_fail_closed: bool,
    integer_frontend_distinct: bool,
    integer_interpreter: (String, String),
    mutation_frontend_distinct: bool,
    mutation_interpreter: (Value, Value),
}

fn maybe_lowered_behavior(
    interner: &Interner,
    source: &str,
    lang: Lang,
    args: &[Value],
) -> Option<Behavior> {
    let il =
        nose_frontend::lower_source(FileId(0), "calibration", source.as_bytes(), lang, interner)
            .expect("calibration source must lower");
    let core = normalize(
        &il,
        interner,
        &NormalizeOptions {
            oracle: true,
            ..NormalizeOptions::default()
        },
    );
    run_unit(&core, interner, first_func(&core), args)
}

fn lowered_behavior(interner: &Interner, source: &str, lang: Lang, args: &[Value]) -> Behavior {
    maybe_lowered_behavior(interner, source, lang, args)
        .expect("calibration source must interpret through the production IL")
}

fn returned(interner: &Interner, source: &str, lang: Lang, args: &[Value]) -> Value {
    lowered_behavior(interner, source, lang, args).ret
}

fn float_bits(value: Value) -> String {
    match value {
        Value::Float(F64(value)) => format!("{:016x}", value.to_bits()),
        other => panic!("expected calibrated float, got {other:?}"),
    }
}

fn integer_string(value: Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        other => panic!("expected calibrated integer, got {other:?}"),
    }
}

fn float_string(value: Value) -> String {
    match value {
        Value::Float(F64(value)) if value == f64::INFINITY => "Infinity".to_string(),
        Value::Float(F64(value)) if value == f64::NEG_INFINITY => "-Infinity".to_string(),
        Value::Float(F64(value)) => value.to_string(),
        other => panic!("expected calibrated float, got {other:?}"),
    }
}

fn is_nan(value: Value) -> bool {
    matches!(value, Value::Float(F64(value)) if value.is_nan())
}

fn boolean(value: Value) -> bool {
    match value {
        Value::Bool(value) => value,
        other => panic!("expected calibrated boolean, got {other:?}"),
    }
}

fn actual_js_number(interner: &Interner) -> ([bool; 5], JsNumberEdges, bool) {
    let shift = |source: &str, count: f64| {
        integer_string(returned(
            interner,
            source,
            Lang::TypeScript,
            &[Value::Float(F64(-8.0)), Value::Float(F64(count))],
        ))
        .parse()
        .expect("shift integer")
    };
    let nan_arg = [Value::Float(F64(f64::NAN))];
    (
        [
            value_fp(interner, TS_DIV_PAIR_LEFT, Lang::TypeScript)
                != value_fp(interner, TS_DIV_PAIR_RIGHT, Lang::TypeScript),
            value_fp(interner, TS_SHIFT_LEFT, Lang::TypeScript)
                != value_fp(interner, TS_SHIFT_RIGHT, Lang::TypeScript),
            value_fp(interner, TS_LITERAL_LEFT, Lang::TypeScript)
                != value_fp(interner, TS_LITERAL_RIGHT, Lang::TypeScript),
            value_fp(interner, TS_BITWISE_LEFT, Lang::TypeScript)
                != value_fp(interner, TS_BITWISE_RIGHT, Lang::TypeScript),
            value_fp(interner, TS_OVERFLOW_PRODUCT, Lang::TypeScript)
                != value_fp(interner, TS_ZERO_PRODUCT, Lang::TypeScript),
        ],
        JsNumberEdges {
            division_pair_left: float_string(returned(
                interner,
                TS_DIV_PAIR_LEFT,
                Lang::TypeScript,
                &[Value::Float(F64(1.0))],
            )),
            division_pair_right: float_string(returned(
                interner,
                TS_DIV_PAIR_RIGHT,
                Lang::TypeScript,
                &[Value::Float(F64(1.0))],
            )),
            bitwise_assoc_bits: (
                float_bits(returned(interner, TS_BITWISE_LEFT, Lang::TypeScript, &[])),
                float_bits(returned(interner, TS_BITWISE_RIGHT, Lang::TypeScript, &[])),
            ),
            bitwise_coercions: integer_string(returned(
                interner,
                JS_BITWISE_COERCIONS,
                Lang::JavaScript,
                &[],
            ))
            .parse()
            .expect("bitwise coercion integer"),
            empty_array_not: boolean(returned(interner, JS_ARRAY_NOT, Lang::JavaScript, &[])),
            empty_array_truthy: returned(
                interner,
                JS_ARRAY_TRUTHY,
                Lang::JavaScript,
                &[Value::List(Vec::new())],
            ) == Value::Int(1),
            literal_assoc_bits: (
                float_bits(returned(interner, TS_LITERAL_LEFT, Lang::TypeScript, &[])),
                float_bits(returned(interner, TS_LITERAL_RIGHT, Lang::TypeScript, &[])),
            ),
            overflow_product_bits: (
                float_bits(returned(
                    interner,
                    TS_OVERFLOW_PRODUCT,
                    Lang::TypeScript,
                    &[],
                )),
                float_bits(returned(interner, TS_ZERO_PRODUCT, Lang::TypeScript, &[])),
            ),
            positive_div_zero: float_string(returned(
                interner,
                TS_POSITIVE_DIV_ZERO,
                Lang::TypeScript,
                &[],
            )),
            negative_div_zero: float_string(returned(
                interner,
                TS_NEGATIVE_DIV_ZERO,
                Lang::TypeScript,
                &[],
            )),
            zero_div_zero_nan: is_nan(returned(interner, TS_ZERO_DIV_ZERO, Lang::TypeScript, &[])),
            nan_truthy: returned(interner, TS_NAN_TRUTHY, Lang::TypeScript, &nan_arg)
                == Value::Int(1),
            nan_not_equal_zero: returned(
                interner,
                TS_NAN_NOT_EQUAL_ZERO,
                Lang::TypeScript,
                &nan_arg,
            ) == Value::Int(1),
            shift_left: shift(TS_SHIFT_LEFT, 1.0),
            shift_right: shift(TS_SHIFT_RIGHT, 1.0),
            shift_masked: shift(TS_SHIFT_LEFT, 33.0),
        },
        maybe_lowered_behavior(interner, JS_COERCIVE_POW, Lang::JavaScript, &[]).is_none(),
    )
}

fn actual_receipt() -> CalibrationReceipt {
    let interner = Interner::new();
    let string_args = [Value::Str(vec![0x5eed_0001]), Value::Str(vec![0x5eed_0002])];
    let float_args = [
        Value::Float(F64(1e16)),
        Value::Float(F64(-1e16)),
        Value::Float(F64(1.0)),
    ];
    let integer_args = [Value::Int(0xF_0000_0003), Value::Int(0xF_0000_0005)];
    let derived_args = [
        Value::Float(F64(1e16)),
        Value::Float(F64(1.0)),
        Value::Float(F64(-1e16)),
        Value::Float(F64(1.0)),
        Value::Float(F64(1.0)),
        Value::Float(F64(1.0)),
    ];
    let mutation_args = [
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::Int(9),
    ];
    let (js_number_frontend_distinct, js_number_interpreter, js_pow_fail_closed) =
        actual_js_number(&interner);

    CalibrationReceipt {
        string_frontend: (
            value_fp(&interner, STRING_FORWARD, Lang::Python),
            value_fp(&interner, STRING_REVERSE, Lang::Python),
        ),
        string_interpreter: (
            returned(&interner, STRING_FORWARD, Lang::Python, &string_args),
            returned(&interner, STRING_REVERSE, Lang::Python, &string_args),
        ),
        float_frontend_distinct: (
            value_fp(&interner, FLOAT_LEFT, Lang::Python)
                != value_fp(&interner, FLOAT_RIGHT, Lang::Python),
            value_fp(&interner, TS_FLOAT_LEFT, Lang::TypeScript)
                != value_fp(&interner, TS_FLOAT_RIGHT, Lang::TypeScript),
        ),
        float_interpreter_bits: (
            (
                float_bits(returned(&interner, FLOAT_LEFT, Lang::Python, &float_args)),
                float_bits(returned(&interner, FLOAT_RIGHT, Lang::Python, &float_args)),
            ),
            (
                float_bits(returned(
                    &interner,
                    TS_FLOAT_LEFT,
                    Lang::TypeScript,
                    &float_args,
                )),
                float_bits(returned(
                    &interner,
                    TS_FLOAT_RIGHT,
                    Lang::TypeScript,
                    &float_args,
                )),
            ),
        ),
        derived_float_frontend_distinct: value_fp(&interner, TS_DERIVED_LEFT, Lang::TypeScript)
            != value_fp(&interner, TS_DERIVED_RIGHT, Lang::TypeScript),
        derived_float_interpreter_bits: (
            float_bits(returned(
                &interner,
                TS_DERIVED_LEFT,
                Lang::TypeScript,
                &derived_args,
            )),
            float_bits(returned(
                &interner,
                TS_DERIVED_RIGHT,
                Lang::TypeScript,
                &derived_args,
            )),
        ),
        js_number_frontend_distinct,
        js_number_interpreter,
        js_pow_fail_closed,
        integer_frontend_distinct: value_fp(&interner, JS_BITAND, Lang::JavaScript)
            != value_fp(&interner, PYTHON_BITAND, Lang::Python),
        integer_interpreter: (
            integer_string(returned(
                &interner,
                JS_BITAND,
                Lang::JavaScript,
                &integer_args,
            )),
            integer_string(returned(
                &interner,
                PYTHON_BITAND,
                Lang::Python,
                &integer_args,
            )),
        ),
        mutation_frontend_distinct: value_fp(&interner, MUTATE_ZERO, Lang::Python)
            != value_fp(&interner, MUTATE_ONE, Lang::Python),
        mutation_interpreter: (
            returned(&interner, MUTATE_ZERO, Lang::Python, &mutation_args),
            returned(&interner, MUTATE_ONE, Lang::Python, &mutation_args),
        ),
    }
}

fn expected_js_number(number_edges: &serde_json::Value) -> JsNumberEdges {
    JsNumberEdges {
        division_pair_left: number_edges["division_pair_left"]
            .as_str()
            .expect("division pair left")
            .to_string(),
        division_pair_right: number_edges["division_pair_right"]
            .as_str()
            .expect("division pair right")
            .to_string(),
        bitwise_assoc_bits: calibrated_bits(number_edges, "bitwise_assoc"),
        bitwise_coercions: number_edges["bitwise_coercions"]
            .as_i64()
            .expect("bitwise coercions"),
        empty_array_not: number_edges["empty_array_not"]
            .as_bool()
            .expect("empty array negation"),
        empty_array_truthy: number_edges["empty_array_truthy"]
            .as_bool()
            .expect("empty array truthiness"),
        literal_assoc_bits: calibrated_bits(number_edges, "literal_assoc"),
        overflow_product_bits: calibrated_bits(number_edges, "overflow_product"),
        positive_div_zero: number_edges["positive_div_zero"]
            .as_str()
            .expect("positive division by zero")
            .to_string(),
        negative_div_zero: number_edges["negative_div_zero"]
            .as_str()
            .expect("negative division by zero")
            .to_string(),
        zero_div_zero_nan: number_edges["zero_div_zero_nan"]
            .as_bool()
            .expect("zero division NaN"),
        nan_truthy: number_edges["nan_truthy"]
            .as_bool()
            .expect("NaN truthiness"),
        nan_not_equal_zero: number_edges["nan_not_equal_zero"]
            .as_bool()
            .expect("NaN inequality"),
        shift_left: number_edges["shift_left"].as_i64().expect("left shift"),
        shift_right: number_edges["shift_right"].as_i64().expect("right shift"),
        shift_masked: number_edges["shift_masked"].as_i64().expect("masked shift"),
    }
}

fn calibrated_bits(edges: &serde_json::Value, name: &str) -> (String, String) {
    let pair = &edges[name];
    (
        pair["left_bits"].as_str().expect("left bits").to_string(),
        pair["right_bits"].as_str().expect("right bits").to_string(),
    )
}

fn validate_receipt(
    artifact: &serde_json::Value,
    receipt: &CalibrationReceipt,
) -> Result<(), Vec<&'static str>> {
    let mut failures = Vec::new();
    let observations = &artifact["observations"];

    let expected_string = (
        calibrated_string(&observations["python"]["string_order"]["forward"]),
        calibrated_string(&observations["python"]["string_order"]["reverse"]),
    );
    if receipt.string_frontend.0 == receipt.string_frontend.1
        || receipt.string_interpreter != expected_string
    {
        failures.push("string_order");
    }

    let expected_float = |runtime: &str| {
        (
            observations[runtime]["float_associativity"]["left_bits"]
                .as_str()
                .expect("left float bits"),
            observations[runtime]["float_associativity"]["right_bits"]
                .as_str()
                .expect("right float bits"),
        )
    };
    let python_float = expected_float("python");
    let node_float = expected_float("node");
    if !receipt.float_frontend_distinct.0
        || !receipt.float_frontend_distinct.1
        || receipt.float_interpreter_bits.0 .0 != python_float.0
        || receipt.float_interpreter_bits.0 .1 != python_float.1
        || receipt.float_interpreter_bits.1 .0 != node_float.0
        || receipt.float_interpreter_bits.1 .1 != node_float.1
    {
        failures.push("float_associativity");
    }

    let expected_derived = (
        observations["node"]["derived_float_associativity"]["left_bits"]
            .as_str()
            .expect("derived left float bits"),
        observations["node"]["derived_float_associativity"]["right_bits"]
            .as_str()
            .expect("derived right float bits"),
    );
    if !receipt.derived_float_frontend_distinct
        || receipt.derived_float_interpreter_bits.0 != expected_derived.0
        || receipt.derived_float_interpreter_bits.1 != expected_derived.1
    {
        failures.push("derived_float_associativity");
    }

    let expected_number = expected_js_number(&observations["node"]["number_edges"]);
    if !receipt
        .js_number_frontend_distinct
        .iter()
        .all(|&value| value)
        || receipt.js_number_interpreter != expected_number
        || !receipt.js_pow_fail_closed
        || observations["node"]["number_edges"]["coercive_pow"].as_str() != Some("8")
    {
        failures.push("javascript_number_edges");
    }

    let expected_integer = (
        observations["node"]["integer_width"]["bitand"]
            .as_str()
            .expect("Node integer"),
        observations["python"]["integer_width"]["bitand"]
            .as_str()
            .expect("Python integer"),
    );
    if !receipt.integer_frontend_distinct
        || receipt.integer_interpreter.0 != expected_integer.0
        || receipt.integer_interpreter.1 != expected_integer.1
    {
        failures.push("javascript_int32_width");
    }

    let expected_mutation = (
        json_list(&observations["python"]["mutation_coordinate"]["index_0"]),
        json_list(&observations["python"]["mutation_coordinate"]["index_1"]),
    );
    if !receipt.mutation_frontend_distinct
        || receipt.mutation_interpreter.0 != expected_mutation.0
        || receipt.mutation_interpreter.1 != expected_mutation.1
    {
        failures.push("mutation_coordinate");
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn calibrated_string(value: &serde_json::Value) -> Value {
    Value::Str(
        value
            .as_str()
            .expect("calibrated string")
            .chars()
            .map(|character| match character {
                'a' => 0x5eed_0001,
                'b' => 0x5eed_0002,
                other => panic!("unexpected calibration character: {other}"),
            })
            .collect(),
    )
}

fn json_list(value: &serde_json::Value) -> Value {
    Value::List(
        value
            .as_array()
            .expect("calibrated list")
            .iter()
            .map(|value| Value::Int(value.as_i64().expect("calibrated integer")))
            .collect(),
    )
}

#[test]
fn production_frontend_and_interpreter_match_independent_source_runtimes() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let receipt = actual_receipt();
    validate_receipt(&artifact, &receipt).expect("all independent channels must agree");
}

#[test]
fn shared_frontend_and_interpreter_mutant_is_rejected() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let mut mutant = actual_receipt();
    mutant.float_frontend_distinct.0 = false;
    mutant.float_interpreter_bits.0 .1 = mutant.float_interpreter_bits.0 .0.clone();

    assert_eq!(
        validate_receipt(&artifact, &mutant),
        Err(vec!["float_associativity"])
    );
}

#[test]
fn typescript_number_shared_mutant_is_rejected() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let mut mutant = actual_receipt();
    mutant.float_frontend_distinct.1 = false;
    mutant.float_interpreter_bits.1 .1 = mutant.float_interpreter_bits.1 .0.clone();

    assert_eq!(
        validate_receipt(&artifact, &mutant),
        Err(vec!["float_associativity"])
    );
}

#[test]
fn derived_typescript_number_shared_mutant_is_rejected() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let mut mutant = actual_receipt();
    mutant.derived_float_frontend_distinct = false;
    mutant.derived_float_interpreter_bits.1 = mutant.derived_float_interpreter_bits.0.clone();

    assert_eq!(
        validate_receipt(&artifact, &mutant),
        Err(vec!["derived_float_associativity"])
    );
}

#[test]
fn javascript_number_edges_shared_mutant_is_rejected() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let mut mutant = actual_receipt();
    mutant.js_number_frontend_distinct = [false; 5];
    mutant.js_number_interpreter.shift_right = mutant.js_number_interpreter.shift_left;

    assert_eq!(
        validate_receipt(&artifact, &mutant),
        Err(vec!["javascript_number_edges"])
    );
}

#[test]
fn shared_string_order_mutant_is_rejected() {
    let artifact: serde_json::Value = serde_json::from_str(ARTIFACT).expect("calibration JSON");
    let mut mutant = actual_receipt();
    std::mem::swap(&mut mutant.string_frontend.0, &mut mutant.string_frontend.1);
    std::mem::swap(
        &mut mutant.string_interpreter.0,
        &mut mutant.string_interpreter.1,
    );

    assert_eq!(
        validate_receipt(&artifact, &mutant),
        Err(vec!["string_order"])
    );
}
