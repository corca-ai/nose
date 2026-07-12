use super::*;

fn normalized_contains_map(src: &str) -> bool {
    let interner = Interner::new();
    let il = nose_frontend::lower_source(
        FileId(0),
        "t.ts",
        src.as_bytes(),
        Lang::TypeScript,
        &interner,
    )
    .expect("lower TypeScript callback fixture");
    let normalized = normalize(&il, &interner, &NormalizeOptions::default());
    normalized.nodes.iter().any(|node| {
        node.kind == nose_il::NodeKind::HoF
            && node.payload == nose_il::Payload::HoF(nose_il::HoFKind::Map)
    })
}

#[test]
fn destructured_inner_binding_cannot_borrow_outer_param_purity_proof() {
    let pure = r#"
function pure(xs: number[]): number[] {
  return xs.map((x: number) => x + 1);
}
"#;
    assert!(
        normalized_contains_map(pure),
        "positive control must admit Array.map"
    );

    let shadowed = r#"
declare function observe(): void;
function outer(x: number): number[] {
  function middle(): number[] {
    let [x] = [{ valueOf() { observe(); return 1; } }];
    return [1].map((y: number) => x + 1);
  }
  return middle();
}
"#;
    assert!(
        !normalized_contains_map(shadowed),
        "the fresh inner Func binding must not inherit the outer number parameter proof"
    );
}
