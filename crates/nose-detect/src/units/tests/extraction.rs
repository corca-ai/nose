use super::super::{
    abstraction_family_witness, extract, ExtractFeatures, UnitFeat, EXACT_VALUE_MIN,
};
use crate::fragment::FragmentKind;
use nose_il::{FileId, Interner, Lang, UnitKind};

fn lowered_java_unit_with_features(
    src: &str,
    interner: &Interner,
    kind: UnitKind,
    name: &str,
    shape_features: bool,
    abstraction_witnesses: bool,
) -> UnitFeat {
    lowered_java_units_with_features(src, interner, shape_features, abstraction_witnesses)
        .into_iter()
        .find(|unit| unit.kind == kind && unit.name.as_deref() == Some(name))
        .expect("requested Java unit")
}

fn lowered_java_units_with_features(
    src: &str,
    interner: &Interner,
    shape_features: bool,
    abstraction_witnesses: bool,
) -> Vec<UnitFeat> {
    let raw =
        nose_frontend::lower_source(FileId(0), "T.java", src.as_bytes(), Lang::Java, interner)
            .expect("lower Java source");
    let il =
        nose_normalize::normalize(&raw, interner, &nose_normalize::NormalizeOptions::default());
    let seeds = crate::minhash::seeds(64);
    extract(
        &il,
        interner,
        &seeds,
        1,
        1,
        true,
        ExtractFeatures {
            shape_features,
            abstraction_witnesses,
        },
    )
}

fn lowered_java_units(src: &str, interner: &Interner) -> Vec<UnitFeat> {
    lowered_java_units_with_features(src, interner, false, false)
}

fn lowered_java_unit(src: &str, interner: &Interner, kind: UnitKind, name: &str) -> UnitFeat {
    lowered_java_unit_with_features(src, interner, kind, name, false, false)
}

fn lowered_java_method_unit(src: &str, interner: &Interner) -> UnitFeat {
    lowered_java_unit(src, interner, UnitKind::Method, "f")
}

#[test]
fn declaration_only_java_methods_do_not_enter_semantic_units() {
    let interner = Interner::new();
    let units = lowered_java_units(
        "abstract class T { abstract int f(int x); native int g(int x); int h(int x) { return x + 1; } }\n",
        &interner,
    );

    assert!(
        units.iter().all(|unit| unit.name.as_deref() != Some("f")),
        "abstract declarations have no reusable semantic body"
    );
    assert!(
        units.iter().all(|unit| unit.name.as_deref() != Some("g")),
        "native declarations have no reusable semantic body"
    );
    assert!(
        units.iter().any(|unit| unit.name.as_deref() == Some("h")),
        "implemented methods must remain eligible"
    );
}

fn lowered_fragment_units(src: &str, lang: Lang, interner: &Interner) -> Vec<UnitFeat> {
    let raw = nose_frontend::lower_source(FileId(0), "fragment", src.as_bytes(), lang, interner)
        .expect("lower source");
    let il =
        nose_normalize::normalize(&raw, interner, &nose_normalize::NormalizeOptions::default());
    let seeds = crate::minhash::seeds(64);
    extract(
        &il,
        interner,
        &seeds,
        99,
        999,
        true,
        ExtractFeatures {
            shape_features: false,
            abstraction_witnesses: false,
        },
    )
    .into_iter()
    .filter(|unit| unit.fragment_kind.is_some())
    .collect()
}

#[test]
fn exact_fragment_collector_produces_contract_recognized_direct_return() {
    let interner = Interner::new();
    let fragments = lowered_fragment_units(
        "function f(x) { console.log(x); return (x + 1) * (x + 2); }\n",
        Lang::JavaScript,
        &interner,
    );

    assert!(
        fragments
            .iter()
            .any(|unit| unit.fragment_kind == Some(FragmentKind::DirectReturn)),
        "contract-first collector should still produce the exact direct-return fragment"
    );
}

#[test]
fn exact_fragment_collector_does_not_enter_lambda_bodies() {
    let interner = Interner::new();
    let fragments = lowered_fragment_units(
        "function f(x) { const g = () => { return (x + 1) * (x + 2); }; return x; }\n",
        Lang::JavaScript,
        &interner,
    );

    assert!(
        fragments
            .iter()
            .all(|unit| unit.fragment_kind != Some(FragmentKind::DirectReturn)),
        "lambda-local returns must not become enclosing-file exact fragments"
    );
}

#[test]
fn exact_fragment_collector_keeps_self_field_body_blocks() {
    let interner = Interner::new();
    let fragments = lowered_fragment_units(
        "class C { int value; int limit; void set(int v, int n) { this.value = (v + 1) * (v + 1); this.limit = n + 3; } }\n",
        Lang::Java,
        &interner,
    );

    assert!(
        fragments
            .iter()
            .any(|unit| unit.fragment_kind == Some(FragmentKind::SelfFieldBody)),
        "body-level self-field fragments are rooted at Block nodes"
    );
}

#[test]
fn abstraction_tokens_do_not_depend_on_shape_features() {
    let interner = Interner::new();
    let left = lowered_java_unit_with_features(
        "class Left { static int f() { return 1; } }\n",
        &interner,
        UnitKind::Method,
        "f",
        false,
        true,
    );
    let right = lowered_java_unit_with_features(
        "class Right { static int f() { return 2; } }\n",
        &interner,
        UnitKind::Method,
        "f",
        false,
        true,
    );

    assert!(
        left.shapes.is_empty(),
        "shape features should stay disabled"
    );
    assert!(
        left.linear.is_empty(),
        "linear shape features should stay disabled"
    );
    assert!(
        !left.abstraction_tokens.is_empty() && !right.abstraction_tokens.is_empty(),
        "abstraction witnesses need their own tokens even when shape features are off"
    );
    let witness = abstraction_family_witness([&left, &right])
        .expect("one changed integer literal should produce an abstraction witness");
    assert_eq!(witness.basis, "family");
    assert_eq!(witness.members_checked, 2);
    assert_eq!(witness.reason_code, "literal-abstracted");
    assert_eq!(witness.holes[0].left, "int-literal");
    assert_eq!(witness.holes[0].right, "int-literal");
}

#[test]
fn abstraction_family_witness_requires_one_shared_hole_position() {
    let interner = Interner::new();
    let base = lowered_java_unit_with_features(
        "class Base { static int f(int x) { int a = 1; int b = 2; return x + a + b; } }\n",
        &interner,
        UnitKind::Method,
        "f",
        false,
        true,
    );
    let same_hole = lowered_java_unit_with_features(
        "class SameHole { static int f(int x) { int a = 3; int b = 2; return x + a + b; } }\n",
        &interner,
        UnitKind::Method,
        "f",
        false,
        true,
    );
    let also_same_hole = lowered_java_unit_with_features(
        "class AlsoSameHole { static int f(int x) { int a = 4; int b = 2; return x + a + b; } }\n",
        &interner,
        UnitKind::Method,
        "f",
        false,
        true,
    );
    let witness = abstraction_family_witness([&base, &same_hole, &also_same_hole])
        .expect("same literal position across the family should produce a witness");
    assert_eq!(witness.basis, "family");
    assert_eq!(witness.members_checked, 3);
    assert_eq!(witness.reason_code, "literal-abstracted");
    assert_eq!(witness.holes[0].observed, vec!["int-literal"]);
}

#[test]
fn lowered_java_static_collection_factories_share_exact_fingerprint() {
    let interner = Interner::new();
    let list = lowered_java_method_unit(
        "import java.util.List;\n\nclass JavaListOf { static boolean f(String value, String other) { return List.of(\"red\", \"blue\").contains(value); } }\n",
        &interner,
    );
    let set = lowered_java_method_unit(
        "import java.util.Set;\n\nclass JavaSetOf { static boolean f(String value, String other) { return Set.of(\"red\", \"blue\").contains(value); } }\n",
        &interner,
    );
    let arrays = lowered_java_method_unit(
        "import java.util.Arrays;\n\nclass JavaArraysAsList { static boolean f(String value, String other) { return Arrays.asList(\"red\", \"blue\").contains(value); } }\n",
        &interner,
    );
    let module_method = lowered_java_unit(
        "import java.util.List;\n\nclass ModuleList {\n    static final List<String> VALUES = List.of(\"red\", \"blue\");\n\n    static boolean moduleList(String value, String other) {\n        return VALUES.contains(value);\n    }\n}\n",
        &interner,
        UnitKind::Method,
        "moduleList",
    );
    assert!(list.exact_safe, "List.of method must stay exact-safe");
    assert!(set.exact_safe, "Set.of method must stay exact-safe");
    assert!(
        arrays.exact_safe,
        "Arrays.asList method must stay exact-safe"
    );
    assert!(
        module_method.exact_safe,
        "class-level List.of binding must stay exact-safe"
    );
    assert!(
        list.value.len() >= EXACT_VALUE_MIN,
        "List.of method should produce a dense semantic fingerprint"
    );
    assert_eq!(list.value, set.value);
    assert_eq!(list.value, arrays.value);
    assert_eq!(list.value, module_method.value);
}
