use super::*;
use crate::fragment::{Effect, EffectSite, Exit, FragmentKind};
use nose_il::{FileId, Interner, Lang};
use nose_normalize::{normalize, NormalizeOptions};

mod projections;

/// Lower + normalize `src`, returning the normalized IL.
fn norm(interner: &Interner, src: &str, lang: Lang) -> Il {
    let il = nose_frontend::lower_source(FileId(0), "t", src.as_bytes(), lang, interner)
        .expect("lowering should succeed");
    normalize(&il, interner, &NormalizeOptions::default())
}

/// Find the first `Return` node with one computed (non-var/lit) child — a direct-return
/// fragment root.
fn first_direct_return(il: &Il, node: NodeId) -> Option<NodeId> {
    if il.kind(node) == NodeKind::Return {
        let kids = il.children(node);
        if kids.len() == 1 && !matches!(il.kind(kids[0]), NodeKind::Var | NodeKind::Lit) {
            return Some(node);
        }
    }
    for &c in il.children(node) {
        if let Some(found) = first_direct_return(il, c) {
            return Some(found);
        }
    }
    None
}

fn direct_return_contract(il: &Il, root: NodeId) -> FragmentContract {
    FragmentContract::value_sink(
        FragmentKind::DirectReturn,
        root,
        free_input_cids(il, root),
        Exit::Return,
    )
}

/// A small single-argument battery — enough to separate the spike's fragments.
fn battery_1() -> Vec<Vec<Value>> {
    [-2i64, -1, 0, 1, 2, 3, 5]
        .into_iter()
        .map(|n| vec![Value::Int(n)])
        .collect()
}

fn behavior_vector(
    il: &Il,
    interner: &Interner,
    c: &FragmentContract,
    battery: &[Vec<Value>],
) -> Vec<Behavior> {
    battery
        .iter()
        .map(|row| {
            fragment_behavior(il, interner, c, row)
                .expect("direct-return fragment must be interpretable")
        })
        .collect()
}

#[test]
fn wrapper_synthesis_runs_a_direct_return_fragment() {
    let i = Interner::new();
    let il = norm(&i, "function f(a){ return a*a + 1; }", Lang::JavaScript);
    let root = first_direct_return(&il, il.root).expect("a direct-return fragment");
    let contract = direct_return_contract(&il, root);
    assert_eq!(contract.arity(), 1, "one free input (the parameter)");

    let (synth, func) = synthesize_wrapper(&il, &i, &contract).expect("wrapper synthesizes");
    assert_eq!(synth.kind(func), NodeKind::Func);
    let b = run_unit(&synth, &i, func, &[Value::Int(4)]).expect("interpretable");
    assert_eq!(b.ret, Value::Int(17), "4*4 + 1 = 17");
}

#[test]
fn wrapper_preserves_domains_for_the_fragment_free_inputs() {
    let i = Interner::new();
    let il = norm(
        &i,
        "function f(skip: boolean, xs: number[], n: number) { return xs[n] + 1; }",
        Lang::TypeScript,
    );
    let root = first_direct_return(&il, il.root).expect("a direct-return fragment");
    let contract = direct_return_contract(&il, root);
    let func =
        find(&il, il.root, &|il, n| il.kind(n) == NodeKind::Func).expect("enclosing function");
    let expected: Vec<_> = contract
        .inputs
        .iter()
        .map(|cid| {
            il.children(func)
                .iter()
                .find(|&&node| {
                    il.kind(node) == NodeKind::Param && il.node(node).payload == Payload::Cid(*cid)
                })
                .and_then(|&param| nose_semantics::domain_evidence_for_param(&il, param))
        })
        .collect();

    let (wrapper, wrapper_func) =
        synthesize_wrapper(&il, &i, &contract).expect("wrapper synthesizes");
    let actual: Vec<_> = wrapper
        .children(wrapper_func)
        .iter()
        .filter(|&&node| wrapper.kind(node) == NodeKind::Param)
        .map(|&param| nose_semantics::domain_evidence_for_param(&wrapper, param))
        .collect();

    assert!(!expected.is_empty());
    assert!(expected.iter().all(Option::is_some));
    assert_eq!(actual, expected, "wrapper must retain declaration evidence");
}

#[test]
fn wrapper_preserves_proven_immutable_module_string_binding() {
    let interner = Interner::new();
    let il = norm(
            &interner,
            "let SWIFT_PREFIX = \"pre\"\n\nfunc check(_ subject: String) -> Bool {\n    return subject.hasPrefix(SWIFT_PREFIX)\n}\n",
            Lang::Swift,
        );
    let root = first_direct_return(&il, il.root).expect("direct return");
    let contract = direct_return_contract(&il, root);
    let binding = nose_normalize::module_facts::immutable_module_string_bindings(&il, &interner)
        .into_iter()
        .find(|binding| interner.resolve(binding.name) == "SWIFT_PREFIX")
        .expect("proven module string");
    let (wrapper, func) = synthesize_wrapper(&il, &interner, &contract).expect("wrapper");

    let behavior = run_unit(
        &wrapper,
        &interner,
        func,
        &[Value::Str(vec![binding.literal_hash, 0x55])],
    )
    .expect("module string is executable");
    assert_eq!(behavior.ret, Value::Bool(true));

    assert!(
        nose_normalize::run_unit_paths_diagnostic_with_module_strings(
            &wrapper,
            &interner,
            func,
            &[Value::Str(vec![binding.literal_hash, 0x55])],
            false,
        )
        .is_err(),
        "the provenance ablation must remove interpreter module-string proof"
    );
    let (ablated_wrapper, ablated_func) =
        synthesize_wrapper_with_module_strings(&il, &interner, &contract, false)
            .expect("ablated wrapper");
    assert!(
        run_unit(
            &ablated_wrapper,
            &interner,
            ablated_func,
            &[Value::Str(vec![binding.literal_hash, 0x55])],
        )
        .is_none(),
        "the provenance ablation must remove the copied module statement"
    );
}

#[test]
fn shadowed_swift_string_parameter_cannot_open_stdlib_affix_execution() {
    let interner = Interner::new();
    let il = norm(
        &interner,
        "typealias String = PrefixBox\nstruct PrefixBox {\n    func hasPrefix(_ prefix: Swift.String) -> Bool { false }\n}\nlet PREFIX = \"pre\"\nfunc check(_ subject: String) -> Bool {\n    subject.hasPrefix(PREFIX)\n}\n",
        Lang::Swift,
    );
    let function = il
        .units
        .iter()
        .find(|unit| {
            unit.name
                .is_some_and(|name| interner.resolve(name) == "check")
        })
        .expect("check function");
    let parameter = il
        .children(function.root)
        .iter()
        .copied()
        .find(|&node| il.kind(node) == NodeKind::Param)
        .expect("subject parameter");

    assert_eq!(
        nose_semantics::domain_evidence_for_param(&il, parameter),
        None,
        "a local String alias must close the standard-library receiver domain"
    );
}

#[test]
fn wrapper_rejects_contextually_converted_and_shadowed_swift_string_literals() {
    let interner = Interner::new();
    let il = norm(
        &interner,
        "typealias String = Character\nlet inferred = \"inferred\"\nlet substring: Substring = \"substring\"\nlet character: Character = \"c\"\nlet alias: String = \"a\"\nlet qualified: Swift.String = \"qualified\"\nfunc shadow() { let alias: Swift.String = \"local\" }\n",
        Lang::Swift,
    );
    let mut names: Vec<_> =
        nose_normalize::module_facts::immutable_module_string_bindings(&il, &interner)
            .into_iter()
            .map(|binding| interner.resolve(binding.name))
            .collect();
    names.sort_unstable();
    assert_eq!(names, vec!["inferred", "qualified"]);
}

#[test]
fn reassigned_module_string_is_not_copied_into_wrapper() {
    let interner = Interner::new();
    let il = norm(
            &interner,
            "var SWIFT_PREFIX = \"pre\"\nSWIFT_PREFIX = \"post\"\n\nfunc check(_ subject: String) -> Bool {\n    return subject.hasPrefix(SWIFT_PREFIX)\n}\n",
            Lang::Swift,
        );
    let root = first_direct_return(&il, il.root).expect("direct return");
    let contract = direct_return_contract(&il, root);
    assert!(
        nose_normalize::module_facts::immutable_module_string_bindings(&il, &interner).is_empty()
    );
    let (wrapper, func) = synthesize_wrapper(&il, &interner, &contract).expect("wrapper");
    assert!(run_unit(&wrapper, &interner, func, &[Value::Str(vec![0x55])],).is_none());
}

#[test]
fn mutable_module_string_is_not_copied_without_a_reassignment() {
    let interner = Interner::new();
    let il = norm(
        &interner,
        "var SWIFT_PREFIX = \"pre\"\n\nfunc check(_ subject: String) -> Bool {\n    return subject.hasPrefix(SWIFT_PREFIX)\n}\n",
        Lang::Swift,
    );
    let root = first_direct_return(&il, il.root).expect("direct return");
    let contract = direct_return_contract(&il, root);
    assert!(
        nose_normalize::module_facts::immutable_module_string_bindings(&il, &interner).is_empty()
    );
    let (wrapper, func) = synthesize_wrapper(&il, &interner, &contract).expect("wrapper");
    assert!(run_unit(&wrapper, &interner, func, &[Value::Str(vec![0x55])]).is_none());
}

#[test]
fn equivalent_fragments_agree_on_the_battery() {
    let i = Interner::new();
    // Same spec, different surface: squared-plus-one.
    let f = norm(&i, "function f(a){ return a*a + 1; }", Lang::JavaScript);
    let g = norm(&i, "function g(b){ return 1 + b*b; }", Lang::JavaScript);
    let cf = direct_return_contract(&f, first_direct_return(&f, f.root).unwrap());
    let cg = direct_return_contract(&g, first_direct_return(&g, g.root).unwrap());

    let battery = battery_1();
    assert_eq!(
        behavior_vector(&f, &i, &cf, &battery),
        behavior_vector(&g, &i, &cg, &battery),
        "equivalent direct-return fragments must agree on every battery input"
    );
}

#[test]
fn distinct_fragments_diverge_on_the_battery() {
    let i = Interner::new();
    let f = norm(&i, "function f(a){ return a*a + 1; }", Lang::JavaScript);
    let h = norm(&i, "function h(a){ return a*a - 1; }", Lang::JavaScript);
    let cf = direct_return_contract(&f, first_direct_return(&f, f.root).unwrap());
    let ch = direct_return_contract(&h, first_direct_return(&h, h.root).unwrap());

    let battery = battery_1();
    assert_ne!(
        behavior_vector(&f, &i, &cf, &battery),
        behavior_vector(&h, &i, &ch, &battery),
        "behaviorally distinct fragments must diverge on the battery"
    );
}

// ---- binding-aware free-input inference --------------------------------------------

fn find<P: Fn(&Il, NodeId) -> bool>(il: &Il, node: NodeId, pred: &P) -> Option<NodeId> {
    if pred(il, node) {
        return Some(node);
    }
    il.children(node).iter().find_map(|&c| find(il, c, pred))
}

fn first_foreach(il: &Il) -> NodeId {
    find(il, il.root, &|il, n| {
        il.kind(n) == NodeKind::Loop
            && matches!(il.node(n).payload, Payload::Loop(LoopKind::ForEach))
    })
    .expect("a for-each loop")
}

/// The body `Block` of the first `Func` — the multi-statement fragment body.
fn first_func_body(il: &Il) -> NodeId {
    let func = find(il, il.root, &|il, n| il.kind(n) == NodeKind::Func).expect("a func");
    *il.children(func).last().expect("func has a body block")
}

#[test]
fn free_inputs_exclude_the_foreach_loop_variable() {
    // The loop variable `x` is bound by the for-each pattern, not read from outside; only
    // the appended-to list `out` and the iterable `xs` are genuine free inputs. Without
    // binding-aware inference this would be arity 3 and the wrapper would misbind `x`.
    let i = Interner::new();
    let il = norm(
        &i,
        "function f(out, xs){ for (const x of xs){ out.push(x); } }",
        Lang::JavaScript,
    );
    let loop_node = first_foreach(&il);
    let inputs = free_input_cids(&il, loop_node);
    assert_eq!(
        inputs.len(),
        2,
        "only `out` and `xs` are free; the loop variable `x` must be excluded, got {inputs:?}"
    );
}

#[test]
fn free_inputs_exclude_a_local_temp() {
    // `t` is assigned then read inside the fragment, so it is a local, not a free input.
    let i = Interner::new();
    let il = norm(
        &i,
        "function f(a){ let t = a * a; return t + 1; }",
        Lang::JavaScript,
    );
    let body = first_func_body(&il);
    let inputs = free_input_cids(&il, body);
    assert_eq!(
        inputs.len(),
        1,
        "only `a` is free; the temp `t` must be excluded, got {inputs:?}"
    );
}

#[test]
fn equivalent_foreach_loops_agree_through_the_oracle() {
    // Two for-each append loops with the same spec must agree; a different appended value
    // must diverge — exercising binding-aware inputs + multi-statement loop lowering.
    let battery = || {
        vec![vec![
            Value::List(vec![]),
            Value::List(vec![Value::Int(2), Value::Int(5)]),
        ]]
    };
    let run = |src: &str| -> Vec<Behavior> {
        let i = Interner::new();
        let il = norm(&i, src, Lang::TypeScript);
        let loop_node = first_foreach(&il);
        let c = FragmentContract::single_effect(
            FragmentKind::LoopEffect,
            loop_node,
            free_input_cids(&il, loop_node),
            EffectSite::observable(Effect::Append),
        );
        assert_eq!(c.arity(), 2, "loop var excluded → arity 2");
        battery()
            .iter()
            .map(|row| fragment_behavior(&il, &i, &c, row).expect("loop fragment interpretable"))
            .collect()
    };
    let f = run(
        "function f(out: number[], xs: number[]): void { for (const x of xs){ out.push(x); } }",
    );
    let g = run(
        "function g(acc: number[], ys: number[]): void { for (const y of ys){ acc.push(y); } }",
    );
    let h = run(
        "function h(out: number[], xs: number[]): void { for (const x of xs){ out.push(x * 2); } }",
    );
    assert!(
        f.iter().all(|b| !b.effects.is_empty()),
        "loop append surfaces as effects"
    );
    assert_eq!(f, g, "equivalent for-each append loops must agree");
    assert_ne!(f, h, "appending a different value must diverge");
}

// ---- ordered multi-effect, multi-statement body -----------------------------------

#[test]
fn ordered_multi_effect_body_observes_statement_order() {
    // A two-append body lowered as an ordered-effect contract: the effect order is
    // observable, so swapping the two appends diverges while an identical body agrees.
    let run = |src: &str| -> Behavior {
        let i = Interner::new();
        let il = norm(&i, src, Lang::TypeScript);
        let body = first_func_body(&il);
        let c = FragmentContract::ordered_effects(
            FragmentKind::ExprEffect,
            body,
            free_input_cids(&il, body),
            Exit::Normal,
            vec![
                EffectSite::observable(Effect::Append),
                EffectSite::observable(Effect::Append),
            ],
        );
        assert_eq!(c.arity(), 1, "only `out` is free (literals are not inputs)");
        fragment_behavior(&il, &i, &c, &[Value::List(vec![])]).expect("interpretable")
    };
    let fwd = run("function f(out: number[]): void { out.push(1); out.push(2); }");
    let fwd2 = run("function h(out: number[]): void { out.push(1); out.push(2); }");
    let rev = run("function g(out: number[]): void { out.push(2); out.push(1); }");
    assert_eq!(fwd.effects.len(), 2, "both appends are recorded in order");
    assert_eq!(fwd, fwd2, "identical ordered bodies must agree");
    assert_ne!(fwd, rev, "swapping the append order must be observable");
}

#[test]
fn append_effect_wrapper_preserves_receiver_identity() {
    let run = |src: &str| -> Behavior {
        let i = Interner::new();
        let il = norm(&i, src, Lang::TypeScript);
        let body = first_func_body(&il);
        let c = FragmentContract::ordered_effects(
            FragmentKind::ExprEffect,
            body,
            free_input_cids(&il, body),
            Exit::Normal,
            vec![
                EffectSite::observable(Effect::Append),
                EffectSite::observable(Effect::Append),
            ],
        );
        fragment_behavior(&il, &i, &c, &[Value::List(vec![]), Value::List(vec![])])
            .expect("interpretable")
    };

    let same =
        run("function f(out: number[], other: number[]): void { out.push(1); other.push(2); }");
    let renamed =
        run("function g(dst: number[], aux: number[]): void { dst.push(1); aux.push(2); }");
    let swapped =
        run("function h(out: number[], other: number[]): void { other.push(1); out.push(2); }");

    assert_eq!(same, renamed, "alpha-renamed receiver roles should agree");
    assert_ne!(
        same, swapped,
        "append effects must preserve which receiver role was mutated"
    );
}

#[test]
fn append_effect_wrapper_agrees_before_and_after_canonicalization() {
    let interner = Interner::new();
    let source = "fn collect(out: &mut Vec<u32>, root: u32) { out.push(root); }";
    let raw =
        nose_frontend::lower_source(FileId(0), "t.rs", source.as_bytes(), Lang::Rust, &interner)
            .expect("lowering should succeed");
    let core = normalize(
        &raw,
        &interner,
        &NormalizeOptions {
            oracle: true,
            ..NormalizeOptions::default()
        },
    );
    let full = normalize(&raw, &interner, &NormalizeOptions::default());
    let contract = |il: &Il| {
        crate::recognized_fragment_contracts(il, &interner)
            .into_iter()
            .find(|contract| contract.kind == FragmentKind::ExprEffect)
            .expect("append expression contract")
    };
    let core_contract = contract(&core);
    let full_contract = contract(&full);
    assert_eq!(core_contract.inputs, full_contract.inputs);
    assert_eq!(core_contract.effects, full_contract.effects);

    let run = |il: &Il, contract: &FragmentContract| {
        fragment_behavior(
            il,
            &interner,
            contract,
            &[Value::List(Vec::new()), Value::Int(7)],
        )
        .expect("append wrapper should be interpretable")
    };
    assert_eq!(
        run(&core, &core_contract),
        run(&full, &full_contract),
        "wrapper behavior must not manufacture a canon-preservation violation"
    );
}
