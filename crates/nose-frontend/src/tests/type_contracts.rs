use super::*;
use nose_il::SourceGranularity;

fn assert_declaration_only_type_contract(unit: &nose_il::Unit) {
    assert!(unit.origin.has_domain(UnitDomain::TypeContract));
    assert_eq!(unit.origin.body_kind, UnitBodyKind::DeclarationOnly);
    assert_eq!(unit.origin.source_granularity, SourceGranularity::WholeUnit);
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::DeclarationOnly));
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::TypeOnly));
    assert!(!unit.origin.has_domain(UnitDomain::Imperative));
    assert!(!unit.origin.has_domain(UnitDomain::ImplementationType));
    assert!(!unit.origin.has_domain(UnitDomain::Data));
    assert!(!unit.origin.has_evidence(UnitEvidenceFlag::HasRuntimeBody));
    assert!(!unit.origin.has_evidence(UnitEvidenceFlag::HasReusableBody));
}

#[test]
fn supported_type_declarations_share_the_strict_origin_contract() {
    let fixtures = [
        (
            "Api.java",
            b"interface Api { int size(); boolean contains(int value); }\n".as_slice(),
            Lang::Java,
            "Api",
        ),
        (
            "Marker.java",
            b"@interface Marker { String value() default \"plain\"; int priority(); }\n".as_slice(),
            Lang::Java,
            "Marker",
        ),
        (
            "api.ts",
            b"interface Api { size(): number; contains(value: number): boolean; }\n".as_slice(),
            Lang::TypeScript,
            "Api",
        ),
        (
            "alias.ts",
            b"type Alias = { size: number; contains: boolean };\n".as_slice(),
            Lang::TypeScript,
            "Alias",
        ),
        (
            "api.rs",
            b"trait Api { type Item; const SIZE: usize; fn size(&self) -> usize; }\n".as_slice(),
            Lang::Rust,
            "Api",
        ),
        (
            "Api.swift",
            b"protocol Api { associatedtype Item; var size: Int { get } }\n".as_slice(),
            Lang::Swift,
            "Api",
        ),
    ];

    for (index, (path, source, language, name)) in fixtures.into_iter().enumerate() {
        let interner = Interner::new();
        let il = lower_source(FileId(index as u32), path, source, language, &interner)
            .unwrap_or_else(|_| panic!("lower {path}"));
        assert_declaration_only_type_contract(unit_named(&il, &interner, UnitKind::Class, name));
    }
}

#[test]
fn behavior_bearing_type_declarations_expose_fail_open_facets() {
    let interner = Interner::new();
    let java = lower_source(
        FileId(0),
        "Api.java",
        b"interface Api { default int size() { return 1; } }\n",
        Lang::Java,
        &interner,
    )
    .expect("lower Java default method");
    let unit = unit_named(&java, &interner, UnitKind::Class, "Api");
    assert_eq!(unit.origin.body_kind, UnitBodyKind::Mixed);
    assert!(unit
        .origin
        .has_evidence(UnitEvidenceFlag::InterfaceDefaultMethod));

    let typescript = lower_source(
        FileId(1),
        "state.ts",
        b"enum State { Ready, Running }\n",
        Lang::TypeScript,
        &interner,
    )
    .expect("lower TypeScript runtime enum");
    let unit = unit_named(&typescript, &interner, UnitKind::Class, "State");
    assert!(unit.origin.has_domain(UnitDomain::Data));
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::RuntimeValue));

    let rust = lower_source(
        FileId(2),
        "api.rs",
        b"trait Api { fn size(&self) -> usize { 1 } }\n",
        Lang::Rust,
        &interner,
    )
    .expect("lower Rust default method");
    let unit = unit_named(&rust, &interner, UnitKind::Class, "Api");
    assert_eq!(unit.origin.body_kind, UnitBodyKind::Mixed);
    assert!(unit.origin.has_domain(UnitDomain::ImplementationType));
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::HasDefaultBody));

    let swift = lower_source(
        FileId(3),
        "Api.swift",
        b"extension Api { func size() -> Int { 1 } }\n",
        Lang::Swift,
        &interner,
    )
    .expect("lower Swift protocol extension");
    let unit = unit_named(&swift, &interner, UnitKind::Class, "Api");
    assert!(unit.origin.has_domain(UnitDomain::ImplementationType));
}

#[test]
fn type_contract_producers_close_runtime_and_recovery_boundaries() {
    let interner = Interner::new();
    let java_initializer = lower_source(
        FileId(4),
        "RuntimeApi.java",
        b"interface RuntimeApi { long START = System.nanoTime(); void run(); }\n",
        Lang::Java,
        &interner,
    )
    .expect("lower Java interface initializer");
    let unit = unit_named(&java_initializer, &interner, UnitKind::Class, "RuntimeApi");
    assert_eq!(unit.origin.body_kind, UnitBodyKind::Mixed);
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::RuntimeValue));

    let annotation_initializer = lower_source(
        FileId(5),
        "Marker.java",
        b"@interface Marker { long START = System.nanoTime(); String value(); }\n",
        Lang::Java,
        &interner,
    )
    .expect("lower Java annotation initializer");
    let unit = unit_named(
        &annotation_initializer,
        &interner,
        UnitKind::Class,
        "Marker",
    );
    assert_eq!(unit.origin.body_kind, UnitBodyKind::Mixed);
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::RuntimeValue));

    for (file_id, path, source, name) in [
        (
            6,
            "const_api.rs",
            b"trait ConstApi { const SIZE: usize = 42; fn run(&self); }\n".as_slice(),
            "ConstApi",
        ),
        (
            7,
            "macro_api.rs",
            b"trait MacroApi { injected_items!(); fn run(&self); }\n".as_slice(),
            "MacroApi",
        ),
        (
            8,
            "attribute_api.rs",
            b"#[async_trait] trait AttributeApi { fn run(&self); }\n".as_slice(),
            "AttributeApi",
        ),
        (
            9,
            "type_default_api.rs",
            b"trait TypeDefaultApi { type Item = usize; fn run(&self); }\n".as_slice(),
            "TypeDefaultApi",
        ),
    ] {
        let rust = lower_source(FileId(file_id), path, source, Lang::Rust, &interner)
            .unwrap_or_else(|_| panic!("lower {path}"));
        let unit = unit_named(&rust, &interner, UnitKind::Class, name);
        assert_eq!(unit.origin.body_kind, UnitBodyKind::Mixed, "{path}");
        assert!(
            unit.origin.has_domain(UnitDomain::ImplementationType),
            "{path}"
        );
        assert!(
            unit.origin.has_evidence(UnitEvidenceFlag::HasDefaultBody),
            "{path}"
        );
    }

    let recovered_protocol = lower_source(
        FileId(10),
        "Broken.swift",
        b"protocol Broken { func run() { print(\"runtime\") } }\n",
        Lang::Swift,
        &interner,
    )
    .expect("lower recovered Swift protocol body");
    let unit = unit_named(&recovered_protocol, &interner, UnitKind::Class, "Broken");
    assert_eq!(unit.origin.body_kind, UnitBodyKind::Mixed);
    assert!(unit.origin.has_domain(UnitDomain::ImplementationType));
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::HasReusableBody));
}
