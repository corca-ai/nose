use super::*;

pub(super) fn lower(source: &str, interner: &Interner, oracle: bool) -> Il {
    let raw = nose_frontend::lower_source(
        FileId(0),
        "arrays.ts",
        source.as_bytes(),
        Lang::TypeScript,
        interner,
    )
    .unwrap();
    nose_normalize::normalize(
        &raw,
        interner,
        &nose_normalize::NormalizeOptions {
            oracle,
            ..Default::default()
        },
    )
}
pub(super) fn root(il: &Il) -> NodeId {
    il.units
        .iter()
        .find(|u| il.kind(u.root) == NodeKind::Func)
        .unwrap()
        .root
}

#[test]
fn primitive_array_evidence_survives_and_falsifies_element_order() {
    let interner = Interner::new();
    let a = lower(
        "function f(xs: boolean[]) { return xs.length > 1 ? xs[0] : false; }",
        &interner,
        true,
    );
    let b = lower(
        "function f(xs: boolean[]) { return xs.length > 1 ? xs[1] : false; }",
        &interner,
        true,
    );
    let ap = collection_input_projections(
        &a,
        &interner,
        root(&a),
        &[nose_detect::OracleInputProjection::Declared],
    );
    let bp = collection_input_projections(
        &b,
        &interner,
        root(&b),
        &[nose_detect::OracleInputProjection::Declared],
    );
    assert_eq!(
        ap,
        vec![nose_detect::OracleInputProjection::ScalarArray(
            DomainEvidence::Boolean
        )]
    );
    let outcome = falsify_pair_with_projections(FalsifyRequest {
        left: FalsifyTarget {
            il: &a,
            root: root(&a),
            projections: &ap,
        },
        right: FalsifyTarget {
            il: &b,
            root: root(&b),
            projections: &bp,
        },
        interner: &interner,
        probes: &[],
        budget: 128,
        seed: DEFAULT_FALSIFY_SEED,
        observation: FalsifyObservation::Behavior,
        module_strings: ModuleStringBindings::Exclude,
    });
    assert!(matches!(outcome, FalsifyOutcome::Witness(_)), "{outcome:?}");
}

#[test]
fn array_aliases_optional_and_nested_types_remain_unhosted() {
    let interner = Interner::new();
    for ty in [
        "Alias[]",
        "boolean[][]",
        "Array<boolean>",
        "boolean[] | null",
    ] {
        let il = lower(
            &format!("function f(xs: {ty}) {{ return xs.length; }}"),
            &interner,
            true,
        );
        assert_eq!(
            collection_input_projections(
                &il,
                &interner,
                root(&il),
                &[nose_detect::OracleInputProjection::Declared]
            ),
            vec![nose_detect::OracleInputProjection::Declared]
        );
    }
}

#[test]
fn primitive_array_oracle_matches_node_on_both_normalization_channels() {
    // Node is an independent source runtime: this test must not derive expected
    // behavior from the same lowering or interpreter that it checks.
    let rows = serde_json::json!([
        [],
        [false],
        [true],
        [false, true],
        [true, false],
        [true, true]
    ]);
    let script = format!("const rows = {rows}; console.log(JSON.stringify(rows.map(xs => xs.length > 1 ? xs[0] : false)));");
    let output = std::process::Command::new("node")
        .args(["-e", &script])
        .output()
        .expect("Node calibration runtime");
    assert!(output.status.success());
    let expected: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
    let interner = Interner::new();
    for oracle in [true, false] {
        let il = lower(
            "function f(xs: boolean[]) { return xs.length > 1 ? xs[0] : false; }",
            &interner,
            oracle,
        );
        for (row, expected) in rows.as_array().unwrap().iter().zip(&expected) {
            let input = Value::List(
                row.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| Value::Bool(v.as_bool().unwrap()))
                    .collect(),
            );
            assert_eq!(
                run_unit(&il, &interner, root(&il), &[input]).unwrap().ret,
                Value::Bool(*expected)
            );
        }
    }
}

#[test]
fn number_and_string_array_equality_matches_source_runtime() {
    for (ty, rows) in [
        (
            "number",
            serde_json::json!([[], [0], [0, 0], [-0.0, 0.0], [1, 2], [1e16, 1e16]]),
        ),
        (
            "string",
            serde_json::json!([[], ["a"], ["a", "a"], ["a", "b"], ["", ""]]),
        ),
    ] {
        let expression = "xs.length > 1 ? xs[0] === xs[1] : false";
        let script = format!("console.log(JSON.stringify({rows}.map(xs => {expression})))");
        let output = std::process::Command::new("node")
            .args(["-e", &script])
            .output()
            .unwrap();
        assert!(output.status.success());
        let expected: Vec<bool> = serde_json::from_slice(&output.stdout).unwrap();
        let interner = Interner::new();
        for oracle in [true, false] {
            let il = lower(
                &format!("function f(xs: {ty}[]) {{ return {expression}; }}"),
                &interner,
                oracle,
            );
            let projection = collection_input_projections(
                &il,
                &interner,
                root(&il),
                &[nose_detect::OracleInputProjection::Declared],
            );
            assert!(matches!(
                projection[0],
                nose_detect::OracleInputProjection::ScalarArray(_)
            ));
            for (row, expected) in rows.as_array().unwrap().iter().zip(&expected) {
                let values = row
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        if ty == "number" {
                            Value::Float(F64(v.as_f64().unwrap()))
                        } else {
                            Value::Str(vec![nose_il::stable_symbol_hash(v.as_str().unwrap())])
                        }
                    })
                    .collect();
                assert_eq!(
                    run_unit(&il, &interner, root(&il), &[Value::List(values)])
                        .unwrap()
                        .ret,
                    Value::Bool(*expected)
                );
            }
        }
    }
}

#[test]
fn caller_cannot_forge_an_array_projection_without_element_evidence() {
    let interner = Interner::new();
    let il = lower(
        "function f(xs: Unknown[]) { return xs.length; }",
        &interner,
        true,
    );
    let projection = [nose_detect::OracleInputProjection::ScalarArray(
        DomainEvidence::Boolean,
    )];
    let target = FalsifyTarget {
        il: &il,
        root: root(&il),
        projections: &projection,
    };
    assert!(matches!(
        falsify_pair_with_projections(FalsifyRequest {
            left: target,
            right: target,
            interner: &interner,
            probes: &[],
            budget: 8,
            seed: DEFAULT_FALSIFY_SEED,
            observation: FalsifyObservation::Behavior,
            module_strings: ModuleStringBindings::Exclude,
        }),
        FalsifyOutcome::Skipped {
            reason: "array projection lacks source element evidence"
        }
    ));
}
