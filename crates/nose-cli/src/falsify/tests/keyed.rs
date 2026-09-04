use super::arrays::{lower, root};
use super::*;
use nose_detect::OracleInputProjection;

fn string(value: &str) -> Value {
    Value::Str(vec![nose_il::stable_symbol_hash(value)])
}

#[test]
fn keyed_membership_and_size_match_node_on_both_channels() {
    let cases = [
        ("boolean", "[[[],false],[[false,false,true],false],[[true],false]]", vec![
            (vec![], Value::Bool(false)),
            (vec![Value::Bool(false), Value::Bool(false), Value::Bool(true)], Value::Bool(false)),
            (vec![Value::Bool(true)], Value::Bool(false)),
        ]),
        ("number", "[[[],NaN],[[NaN,NaN,-0,0,1],NaN],[[-0],0],[[0],-0],[[Infinity,-Infinity],Infinity],[[1],2]]", vec![
            (vec![], Value::Float(F64(f64::NAN))),
            (vec![Value::Float(F64(f64::NAN)), Value::Float(F64(f64::NAN)), Value::Float(F64(-0.0)), Value::Int(0), Value::Int(1)], Value::Float(F64(f64::NAN))),
            (vec![Value::Float(F64(-0.0))], Value::Int(0)),
            (vec![Value::Int(0)], Value::Float(F64(-0.0))),
            (vec![Value::Float(F64(f64::INFINITY)), Value::Float(F64(f64::NEG_INFINITY))], Value::Float(F64(f64::INFINITY))),
            (vec![Value::Int(1)], Value::Int(2)),
        ]),
        ("string", "[[[],\"\"],[[\"\",\"\",\"a\"],\"\"],[[\"a\",\"b\"],\"c\"]]", vec![
            (vec![], string("")),
            (vec![Value::Str(vec![]), string(""), string("a")], string("")),
            (vec![string("a"), string("b")], string("c")),
        ]),
    ];
    for (ty, js_rows, rows) in cases {
        for container in ["Map", "Set", "ReadonlyMap", "ReadonlySet"] {
            let is_map = container.ends_with("Map");
            let ctor = if is_map {
                "new Map(keys.map(k => [k, false]))"
            } else {
                "new Set(keys)"
            };
            let script = format!("console.log(JSON.stringify({js_rows}.map(([keys,key])=>{{const xs={ctor};return xs.has(key)?xs.size:-1}})))");
            let output = std::process::Command::new("node")
                .args(["-e", &script])
                .output()
                .unwrap();
            assert!(output.status.success());
            let expected: Vec<i64> = serde_json::from_slice(&output.stdout).unwrap();
            let annotation = if is_map {
                format!("{container}<{ty}, boolean>")
            } else {
                format!("{container}<{ty}>")
            };
            for oracle in [true, false] {
                let interner = Interner::new();
                let il = lower(&format!("function f(xs: {annotation}, key: {ty}) {{ return xs.has(key) ? xs.size : -1; }}"), &interner, oracle);
                let projection = collection_input_projections(
                    &il,
                    &interner,
                    root(&il),
                    &[OracleInputProjection::Declared; 2],
                );
                assert!(
                    matches!(projection[0], OracleInputProjection::KeyedMembership(_)),
                    "{annotation}: {projection:?}"
                );
                for ((keys, key), expected) in rows.iter().zip(&expected) {
                    let result = run_unit(
                        &il,
                        &interner,
                        root(&il),
                        &[Value::KeySet(keys.clone()), key.clone()],
                    )
                    .unwrap();
                    assert_eq!(
                        result.ret,
                        Value::Int(*expected),
                        "{annotation}: {keys:?}, {key:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn keyed_projection_rejects_shadowing_aliases_and_other_observations() {
    for source in [
        "function f(xs: Map<Alias, boolean>, key: Alias) { return xs.has(key); }",
        "function f(xs: Map<number, boolean> | null, key: number) { return xs.has(key); }",
        "function f(xs: Set<number | string>, key: number) { return xs.has(key); }",
        "function f(xs: Map<number, boolean>, key: number) { return xs.get(key); }",
        "function f(xs: Map<number, boolean>, key: number) { const ys = xs; return ys.has(key); }",
        "function f(xs: Set<number>) { return xs; }",
        "function f(xs: Set<number>, key: number) { xs.add(key); return xs.has(key); }",
        "function f(xs: Set<number>) { xs.size = 0; return xs.size; }",
        "function f(xs: Map<number, boolean>, key: number) { return xs.has(key); } type Map<K,V> = { has(k:K):boolean };",
        "interface Set<T> { has(k:T):boolean } function f(xs: Set<number>, key:number) { return xs.has(key); }",
        "function f<Map>(xs: Map<number, boolean>, key: number) { return xs.has(key); }",
    ] {
        let interner = Interner::new();
        for oracle in [true, false] {
            let il = lower(source, &interner, oracle);
            let params = parameter_domains(&il, root(&il));
            let projections = collection_input_projections(&il, &interner, root(&il), &vec![OracleInputProjection::Declared; params.len()]);
            assert_eq!(projections[0], OracleInputProjection::Declared, "{source}");
        }
    }
}

#[test]
fn key_membership_falsifier_finds_a_changed_lookup_key() {
    for annotation in ["Map<boolean, unknown>", "Set<boolean>"] {
        let interner = Interner::new();
        let a = lower(
            &format!(
                "function f(xs: {annotation}, a: boolean, b: boolean) {{ return xs.has(a); }}"
            ),
            &interner,
            true,
        );
        let b = lower(
            &format!(
                "function f(xs: {annotation}, a: boolean, b: boolean) {{ return xs.has(b); }}"
            ),
            &interner,
            true,
        );
        let ap = collection_input_projections(
            &a,
            &interner,
            root(&a),
            &[OracleInputProjection::Declared; 3],
        );
        let bp = collection_input_projections(
            &b,
            &interner,
            root(&b),
            &[OracleInputProjection::Declared; 3],
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
        assert!(
            matches!(outcome, FalsifyOutcome::Witness(_)),
            "{annotation}: {outcome:?}"
        );
    }
}

#[test]
fn caller_cannot_promote_value_lookup_to_a_membership_projection() {
    let interner = Interner::new();
    let il = lower(
        "function f(xs: Map<number, boolean>, key: number) { return xs.get(key); }",
        &interner,
        true,
    );
    let projection = [
        OracleInputProjection::KeyedMembership(DomainEvidence::Number),
        OracleInputProjection::Declared,
    ];
    let target = FalsifyTarget {
        il: &il,
        root: root(&il),
        projections: &projection,
    };
    let result = falsify_pair_with_projections(FalsifyRequest {
        left: target,
        right: target,
        interner: &interner,
        probes: &[],
        budget: 16,
        seed: DEFAULT_FALSIFY_SEED,
        observation: FalsifyObservation::Behavior,
        module_strings: ModuleStringBindings::Exclude,
    });
    assert!(
        matches!(result, FalsifyOutcome::Skipped { .. }),
        "{result:?}"
    );
}

#[test]
fn unknown_keys_and_unmodeled_string_equality_never_become_concrete() {
    let interner = Interner::new();
    let il = lower(
        "function f(xs: Set<number>, key: number) { return xs.has(key); }",
        &interner,
        true,
    );
    let behavior = run_unit(
        &il,
        &interner,
        root(&il),
        &[Value::KeySet(vec![Value::Sym(1)]), Value::Int(1)],
    )
    .unwrap();
    assert!(behavior_has_sym(&behavior));
    let il = lower(
        "function f(xs: Set<string>, key: string) { return xs.has(key + 'suffix'); }",
        &interner,
        true,
    );
    assert!(run_unit(
        &il,
        &interner,
        root(&il),
        &[Value::KeySet(vec![string("asuffix")]), string("a")]
    )
    .is_none());
}
