use super::*;

fn declaration_only_type_origin() -> nose_il::UnitOrigin {
    use nose_il::{
        RegionKind, SourceGranularity, UnitBodyKind, UnitDomain, UnitDomains, UnitEvidenceFlag,
        UnitSubkind,
    };

    nose_il::UnitOrigin::new(
        UnitDomains::of(UnitDomain::TypeContract),
        UnitSubkind::InterfaceTraitProtocol,
        UnitBodyKind::DeclarationOnly,
        SourceGranularity::WholeUnit,
        RegionKind::Code,
    )
    .with_evidence(UnitEvidenceFlag::DeclarationOnly)
    .with_evidence(UnitEvidenceFlag::TypeOnly)
}

fn assert_type_contract_origin_fails_open(label: &str, origin: nose_il::UnitOrigin) {
    let mut family = fam_at(&[("a.ts", 1, 20), ("b.ts", 1, 20)]);
    for location in &mut family.locations {
        location.kind = nose_il::UnitKind::Class;
        location.origin = origin;
    }
    let overrides = classify_surface_overrides(std::slice::from_mut(&mut family));

    assert_eq!(
        effective_surface(&family, &overrides),
        "default",
        "{label} must fail open"
    );
    assert!(is_default_report_family(&family, &overrides), "{label}");
}

#[test]
fn strict_declaration_only_type_contracts_are_reason_coded_and_fold_stable() {
    let mut family = fam_at(&[
        ("Api.java", 1, 20),
        ("Marker.java", 1, 20),
        ("api.ts", 1, 20),
        ("alias.ts", 1, 20),
        ("api.rs", 1, 20),
        ("Api.swift", 1, 20),
    ]);
    for ((location, language), subkind) in family
        .locations
        .iter_mut()
        .zip(["java", "java", "typescript", "typescript", "rust", "swift"])
        .zip([
            nose_il::UnitSubkind::InterfaceTraitProtocol,
            nose_il::UnitSubkind::DefinedType,
            nose_il::UnitSubkind::InterfaceTraitProtocol,
            nose_il::UnitSubkind::TypeAlias,
            nose_il::UnitSubkind::InterfaceTraitProtocol,
            nose_il::UnitSubkind::InterfaceTraitProtocol,
        ])
    {
        location.lang = language.into();
        location.kind = nose_il::UnitKind::Class;
        location.origin = nose_il::UnitOrigin {
            subkind,
            ..declaration_only_type_origin()
        };
    }
    let id_before = baseline::family_id(&family);
    let overrides = classify_surface_overrides(std::slice::from_mut(&mut family));

    assert_eq!(baseline::family_id(&family), id_before);
    assert_eq!(effective_surface(&family, &overrides), "declaration");
    assert!(!is_default_report_family(&family, &overrides));
    assert!(
        is_default_opportunity_family(&family, &overrides),
        "presentation-only classification must preserve the existing fold forest"
    );
    assert_eq!(
        family_actionability_reason(&family, &overrides),
        Some("declaration-only-type-contract")
    );
    assert_eq!(
        surface_omission_note(std::slice::from_ref(&family), &overrides).as_deref(),
        Some("omitted 1 family from default output (1 declaration-only-type-contract)")
    );
    let json = query_family_json(
        &family,
        &overrides,
        &OpportunityGroups::default(),
        false,
        None,
        None,
    );
    assert_eq!(json["id"], id_before);
    assert_eq!(json["surface"], "declaration");
}

#[test]
fn incomplete_or_contradictory_type_contract_origins_fail_open() {
    use nose_il::{
        SourceGranularity, UnitBodyKind, UnitDomain, UnitDomains, UnitEvidenceFlag,
        UnitEvidenceFlags, UnitSubkind,
    };

    let base = declaration_only_type_origin();
    let variants = [
        ("unknown", nose_il::UnitOrigin::unknown()),
        (
            "member-granularity",
            nose_il::UnitOrigin {
                source_granularity: SourceGranularity::Member,
                ..base
            },
        ),
        (
            "unknown-granularity",
            nose_il::UnitOrigin {
                source_granularity: SourceGranularity::Unknown,
                ..base
            },
        ),
        (
            "mixed-body",
            nose_il::UnitOrigin {
                body_kind: UnitBodyKind::Mixed,
                ..base
            },
        ),
        (
            "missing-declaration-only-evidence",
            nose_il::UnitOrigin {
                evidence_flags: UnitEvidenceFlags::of(UnitEvidenceFlag::TypeOnly),
                ..base
            },
        ),
        (
            "missing-type-only-evidence",
            nose_il::UnitOrigin {
                evidence_flags: UnitEvidenceFlags::of(UnitEvidenceFlag::DeclarationOnly),
                ..base
            },
        ),
        (
            "missing-type-contract-domain",
            nose_il::UnitOrigin {
                domains: UnitDomains::empty(),
                ..base
            },
        ),
        (
            "imperative-domain",
            base.with_domain(UnitDomain::Imperative),
        ),
        ("data-domain", base.with_domain(UnitDomain::Data)),
        ("style-domain", base.with_domain(UnitDomain::Style)),
        (
            "implementation-domain",
            base.with_domain(UnitDomain::ImplementationType),
        ),
        (
            "enum-subkind",
            nose_il::UnitOrigin {
                subkind: UnitSubkind::Enum,
                ..base
            },
        ),
        (
            "schema-subkind",
            nose_il::UnitOrigin {
                subkind: UnitSubkind::Schema,
                ..base
            },
        ),
        (
            "extension-subkind",
            nose_il::UnitOrigin {
                subkind: UnitSubkind::ExtensionImpl,
                ..base
            },
        ),
    ];
    for (label, origin) in variants {
        assert_type_contract_origin_fails_open(label, origin);
    }
}

#[test]
fn behavior_bearing_type_contract_flags_fail_open() {
    use nose_il::UnitEvidenceFlag;

    let base = declaration_only_type_origin();
    for (label, flag) in [
        ("runtime-body", UnitEvidenceFlag::HasRuntimeBody),
        ("reusable-body", UnitEvidenceFlag::HasReusableBody),
        ("runtime-value", UnitEvidenceFlag::RuntimeValue),
        ("runtime-validation", UnitEvidenceFlag::RuntimeValidation),
        ("default-body", UnitEvidenceFlag::HasDefaultBody),
        ("protocol-extension", UnitEvidenceFlag::ProtocolExtension),
        (
            "concrete-type-extension",
            UnitEvidenceFlag::ConcreteTypeExtension,
        ),
        (
            "constrained-extension",
            UnitEvidenceFlag::ConstrainedExtension,
        ),
        (
            "interface-default-method",
            UnitEvidenceFlag::InterfaceDefaultMethod,
        ),
        (
            "interface-static-method",
            UnitEvidenceFlag::InterfaceStaticMethod,
        ),
        (
            "interface-private-method",
            UnitEvidenceFlag::InterfacePrivateMethod,
        ),
    ] {
        assert_type_contract_origin_fails_open(label, base.with_evidence(flag));
    }
}

#[test]
fn partial_or_narrowed_type_contract_families_fail_open() {
    let base = declaration_only_type_origin();

    let mut partial = fam_at(&[("a.ts", 1, 20), ("b.ts", 1, 20)]);
    for location in &mut partial.locations {
        location.kind = nose_il::UnitKind::Class;
    }
    partial.locations[0].origin = base;
    let overrides = classify_surface_overrides(std::slice::from_mut(&mut partial));
    assert_eq!(effective_surface(&partial, &overrides), "default");

    let mut sliced = fam_at(&[("a.ts", 4, 12), ("b.ts", 4, 12)]);
    for location in &mut sliced.locations {
        location.origin = base;
        location.shared_subdag = Some((4, 12));
    }
    let overrides = classify_surface_overrides(std::slice::from_mut(&mut sliced));
    assert_eq!(
        effective_surface(&sliced, &overrides),
        "default",
        "a narrowed connected block must not inherit its enclosing type surface"
    );

    let mut fragment = fam_at(&[("a.ts", 4, 12), ("b.ts", 4, 12)]);
    for location in &mut fragment.locations {
        location.kind = nose_il::UnitKind::Class;
        location.origin = base;
        location.is_fragment = true;
    }
    let overrides = classify_surface_overrides(std::slice::from_mut(&mut fragment));
    assert_eq!(effective_surface(&fragment, &overrides), "default");

    let mut empty = fam_at(&[]);
    let overrides = classify_surface_overrides(std::slice::from_mut(&mut empty));
    assert_ne!(effective_surface(&empty, &overrides), "declaration");
}
