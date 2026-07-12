use super::*;
use nose_il::DomainEvidence;

#[test]
fn parameter_type_annotation_records_domain() {
    let il = il(r#"
func lookup(_ dict: Dictionary<String, Int>, _ value: Any) -> Int {
return dict["red", default: 0]
}
"#);
    assert_eq!(
        il.evidence
            .iter()
            .filter(|record| record.kind == EvidenceKind::Domain(DomainEvidence::Map))
            .count(),
        1,
        "only Dictionary parameters should record a Map domain"
    );
}

#[test]
fn bracket_array_parameter_records_a_stronger_language_core_domain() {
    let il = il(r#"
func compare(_ bracket: [Bool], _ modified: inout [Bool], _ nominal: Array<Bool>, _ protocolValue: Collection) {}
"#);
    let bracket_proofs = il
        .evidence
        .iter()
        .filter(|record| matches!(record.anchor, EvidenceAnchor::Param { .. }))
        .filter(|record| {
            record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bracket_proofs.len(),
        1,
        "only plain `[T]` proves the bracket-array source surface"
    );
    assert_eq!(
        il.evidence
            .iter()
            .filter(|record| matches!(record.anchor, EvidenceAnchor::Param { .. }))
            .filter(|record| record.kind == EvidenceKind::Domain(DomainEvidence::Collection))
            .count(),
        4,
        "the existing conservative collection-domain classification stays unchanged"
    );
    assert!(
        bracket_proofs
            .iter()
            .all(|record| nose_semantics::language_core_record_has_provenance(&il, record)),
        "the bracket-array proof must be owned by Swift language-core lowering"
    );
}

#[test]
fn property_wrapped_bracket_array_parameter_does_not_prove_source_identity() {
    for (label, trivia) in [
        ("block comment", "/* source-altering wrapper */"),
        ("documentation comment", "/** source-altering wrapper */"),
        (
            "nested block comment",
            "/* outer /* source-altering wrapper */ outer */",
        ),
        ("line comment", "// source-altering wrapper\n    "),
    ] {
        let source = format!(
            r#"
@propertyWrapper
struct ForceTrue {{
    var wrappedValue: [Bool]
    init(wrappedValue: [Bool]) {{ self.wrappedValue = [true] }}
}}
func transform(@ForceTrue {trivia} _ values: [Bool]) -> [Bool] {{ values }}
"#
        );
        let il = il(&source);
        let bracket_proofs = il
            .evidence
            .iter()
            .filter(|record| {
                record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            bracket_proofs.len(),
            1,
            "only the plain wrappedValue initializer parameter should retain bracket-array proof across {label}"
        );
        assert_eq!(
            il.evidence
                .iter()
                .filter(|record| matches!(record.anchor, EvidenceAnchor::Param { .. }))
                .filter(|record| record.kind == EvidenceKind::Domain(DomainEvidence::Collection))
                .count(),
            2,
            "the conservative collection-domain classification remains available across {label}"
        );
        assert!(
            bracket_proofs
                .iter()
                .all(|record| nose_semantics::language_core_record_has_provenance(&il, record)),
            "the remaining plain proof must retain language-core provenance"
        );
    }
}

#[test]
fn parser_recovered_parameter_modifiers_do_not_prove_source_identity() {
    for modifier in ["sending", "__shared", "__owned"] {
        let source = format!(
            r#"
func transform(_ values: {modifier} [Bool]) -> [Bool] {{ values }}
"#
        );
        let il = il(&source);
        assert!(
            !il.evidence.iter().any(|record| {
                record.kind == EvidenceKind::Type(TypeEvidenceKind::SwiftBracketArrayParameter)
            }),
            "parser-recovered `{modifier}` must not be mistaken for a plain bracket-array parameter"
        );
    }
}

#[test]
fn property_type_annotation_records_binding_domain() {
    let il = il(r#"
func build(_ xs: [Int]) -> [Int] {
var out: [Int] = []
for x in xs {
    out.append(x)
}
return out
}
"#);
    assert!(il.evidence.iter().any(|record| {
        matches!(
            record.anchor,
            EvidenceAnchor::Binding { local_hash, .. }
                if local_hash == stable_symbol_hash("out")
        ) && record.kind == EvidenceKind::Domain(DomainEvidence::Collection)
    }));
}
