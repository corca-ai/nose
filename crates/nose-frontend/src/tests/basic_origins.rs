use super::*;

#[test]
fn swift_protocol_unit_origin_is_type_contract() {
    let interner = Interner::new();
    let il = lower_source(
        FileId(0),
        "P.swift",
        b"protocol P {\n  var name: String { get }\n  func run()\n}\n",
        Lang::Swift,
        &interner,
    )
    .expect("lower swift protocol");
    let unit = il
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Class)
        .expect("protocol unit");
    assert!(unit.origin.has_domain(UnitDomain::TypeContract));
    assert_eq!(unit.origin.subkind, UnitSubkind::InterfaceTraitProtocol);
    assert_eq!(unit.origin.body_kind, UnitBodyKind::DeclarationOnly);
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::TypeOnly));
}

#[test]
fn swift_class_origin_ignores_nested_type_bodies() {
    let interner = Interner::new();
    let il = lower_source(
        FileId(0),
        "Outer.swift",
        b"class Outer {\n  class Helper {\n    func run() { print(1) }\n  }\n}\n",
        Lang::Swift,
        &interner,
    )
    .expect("lower swift class");
    let unit = il
        .units
        .iter()
        .find(|unit| {
            unit.kind == UnitKind::Class
                && unit.origin.subkind == UnitSubkind::Class
                && unit.origin.body_kind == UnitBodyKind::DeclarationOnly
        })
        .expect("outer class unit should stay declaration-only");
    assert_eq!(unit.origin.body_kind, UnitBodyKind::DeclarationOnly);
    assert!(unit.origin.has_evidence(UnitEvidenceFlag::DeclarationOnly));
    assert!(!unit.origin.has_evidence(UnitEvidenceFlag::HasReusableBody));
}
