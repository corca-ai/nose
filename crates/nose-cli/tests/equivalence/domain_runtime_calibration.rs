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
const PYTHON_BITAND: &str = "def bitand(a, b):\n    return a & b\n";
const JS_BITAND: &str = "function bitand(a, b) { return a & b; }\n";
const MUTATE_ZERO: &str = "def mutate(a, value):\n    a[0] = value\n    return a\n";
const MUTATE_ONE: &str = "def mutate(a, value):\n    a[1] = value\n    return a\n";

#[derive(Clone)]
struct CalibrationReceipt {
    string_frontend: (Vec<u64>, Vec<u64>),
    string_interpreter: (Value, Value),
    float_frontend_distinct: (bool, bool),
    float_interpreter_bits: ((String, String), (String, String)),
    integer_frontend_distinct: bool,
    integer_interpreter: (String, String),
    mutation_frontend_distinct: bool,
    mutation_interpreter: (Value, Value),
}

fn lowered_behavior(interner: &Interner, source: &str, lang: Lang, args: &[Value]) -> Behavior {
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

fn actual_receipt() -> CalibrationReceipt {
    let interner = Interner::new();
    let string_args = [Value::Str(vec![0x5eed_0001]), Value::Str(vec![0x5eed_0002])];
    let float_args = [
        Value::Float(F64(1e16)),
        Value::Float(F64(-1e16)),
        Value::Float(F64(1.0)),
    ];
    let integer_args = [Value::Int(0xF_0000_0003), Value::Int(0xF_0000_0005)];
    let mutation_args = [
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::Int(9),
    ];

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
    validate_receipt(&artifact, &actual_receipt()).expect("all independent channels must agree");
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
