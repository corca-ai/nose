use super::{review_key, tests::three_copy_family};
use crate::{AbstractionHole, AbstractionWitness, RefactorFamily};
use nose_il::{ContentDigest, SourceDocument};
use nose_semantics::{
    SemanticPackDependencySource, SemanticPackExternalExactProvenance, SemanticPackNearDependency,
    SemanticPackNearProvenance, SemanticPackV1Channel, SemanticPackV1ProtocolOperation,
};

fn aggregate(family: &mut RefactorFamily) {
    family.semantic_pack_near = family
        .locations
        .iter()
        .flat_map(|l| l.semantic_pack_near.clone())
        .collect();
    family.semantic_pack_external_exact = family
        .locations
        .iter()
        .flat_map(|l| l.semantic_pack_external_exact.clone())
        .collect();
}

fn with_pack(exact: bool) -> RefactorFamily {
    let mut family = three_copy_family();
    let loc = &mut family.locations[0];
    let dependency = SemanticPackNearDependency {
        coordinate: "example:collections".into(),
        declared_version: "1.2".into(),
        matched_version: "1.2".into(),
        sources: vec![SemanticPackDependencySource {
            declared_path: "owner/pom.xml".into(),
            content_digest: "dependency-content".into(),
        }],
    };
    if exact {
        loc.semantic_pack_external_exact
            .push(SemanticPackExternalExactProvenance {
                pack_id: "example".into(),
                row_id: "factory".into(),
                semantic_digest: "pack-content".into(),
                row_digest: "row-content".into(),
                lane: SemanticPackV1Channel::ExternalExact,
                assurance: "external-claim-exact".into(),
                trust: "external-opt-in".into(),
                dependency,
                receipt_digest: "receipt-content".into(),
                occurrence_file: loc.file.clone(),
                call_start_line: 2,
                call_end_line: 2,
                caveats: vec!["provider-claim".into()],
            });
    } else {
        loc.semantic_pack_near.push(SemanticPackNearProvenance {
            pack_id: "example".into(),
            row_id: "factory".into(),
            semantic_digest: "pack-content".into(),
            row_digest: "row-content".into(),
            lane: SemanticPackV1Channel::Near,
            trust: "external-opt-in".into(),
            operation: SemanticPackV1ProtocolOperation::CollectionFactory,
            dependency,
            occurrence_file: loc.file.clone(),
            call_start_line: 2,
            call_end_line: 2,
            caveats: vec!["near-only".into()],
        });
    }
    aggregate(&mut family);
    family
}

#[test]
fn pack_keys_ignore_occurrence_and_dependency_paths_but_bind_evidence() {
    for exact in [false, true] {
        let before = with_pack(exact);
        let key = review_key(&before).unwrap();
        let mut moved = before.clone();
        let loc = &mut moved.locations[0];
        loc.file = "moved.py".into();
        loc.start_line += 10;
        loc.end_line += 10;
        for e in &mut loc.semantic_pack_near {
            e.occurrence_file.clone_from(&loc.file);
            e.call_start_line += 10;
            e.call_end_line += 10;
            e.dependency.sources[0].declared_path = "moved/pom.xml".into();
        }
        for e in &mut loc.semantic_pack_external_exact {
            e.occurrence_file.clone_from(&loc.file);
            e.call_start_line += 10;
            e.call_end_line += 10;
            e.dependency.sources[0].declared_path = "moved/pom.xml".into();
        }
        aggregate(&mut moved);
        assert_eq!(review_key(&moved), Some(key));
        for mutation in 0..7 {
            let mut changed = before.clone();
            let loc = &mut changed.locations[0];
            let (row, pack, trust, dependency, caveats) = if exact {
                let e = &mut loc.semantic_pack_external_exact[0];
                (
                    &mut e.row_digest,
                    &mut e.semantic_digest,
                    &mut e.trust,
                    &mut e.dependency,
                    &mut e.caveats,
                )
            } else {
                let e = &mut loc.semantic_pack_near[0];
                (
                    &mut e.row_digest,
                    &mut e.semantic_digest,
                    &mut e.trust,
                    &mut e.dependency,
                    &mut e.caveats,
                )
            };
            match mutation {
                0 => row.push('x'),
                1 => pack.push('x'),
                2 => trust.push('x'),
                3 => dependency.matched_version.push('x'),
                4 => dependency.coordinate.push('x'),
                5 => dependency.sources[0].content_digest.push('x'),
                _ => caveats.push("new-caveat".into()),
            }
            aggregate(&mut changed);
            assert_ne!(
                review_key(&changed),
                Some(key),
                "exact={exact}, mutation={mutation}"
            );
        }
    }
    let mut exact = with_pack(true);
    let key = review_key(&exact).unwrap();
    exact.locations[0].semantic_pack_external_exact[0]
        .receipt_digest
        .push('x');
    aggregate(&mut exact);
    assert_ne!(review_key(&exact), Some(key));
}

#[test]
fn missing_or_mislocated_pack_evidence_is_unavailable() {
    let mut family = with_pack(false);
    family.locations[0].semantic_pack_near.clear();
    assert_eq!(review_key(&family), None);
    let mut family = with_pack(true);
    family.locations[0].semantic_pack_external_exact[0].call_start_line = 0;
    aggregate(&mut family);
    assert_eq!(review_key(&family), None);
}

#[test]
fn abstraction_keys_bind_claim_and_holes_without_representative_coordinates() {
    let mut family = three_copy_family();
    family.abstraction_witness = Some(AbstractionWitness {
        claim: "weak-refactoring-template",
        basis: "family",
        members_checked: 3,
        reason_code: "type-parametric",
        template_format: "normalized-il-preorder",
        template: vec!["Return".into(), "<hole 1: literal>".into()],
        holes: vec![AbstractionHole {
            index: 1,
            template_index: 1,
            kind: "literal",
            role: "leaf",
            left: "int-literal",
            right: "float-literal",
            observed: vec!["int-literal", "float-literal"],
            left_line: 2,
            right_line: 2,
        }],
        caveats: vec!["numeric-domain-sensitive"],
    });
    let key = review_key(&family).unwrap();
    let mut moved = family.clone();
    let hole = &mut moved.abstraction_witness.as_mut().unwrap().holes[0];
    hole.left_line += 100;
    hole.right_line += 10;
    std::mem::swap(&mut hole.left, &mut hole.right);
    hole.observed.reverse();
    assert_eq!(review_key(&moved), Some(key));
    for mutation in 0..4 {
        let mut changed = family.clone();
        let witness = changed.abstraction_witness.as_mut().unwrap();
        match mutation {
            0 => witness.holes[0].template_index += 1,
            1 => witness.holes[0].observed.push("string-literal"),
            2 => witness.caveats.clear(),
            _ => witness.reason_code = "literal-abstracted",
        }
        assert_ne!(review_key(&changed), Some(key));
    }
}

#[test]
fn out_of_unit_anchors_bind_source_instead_of_absolute_line_distance() {
    let mut family = three_copy_family();
    family.locations[0].shared_subdag = Some((100, 101));
    assert_eq!(review_key(&family), None);
    let source = SourceDocument::new(b"def helper(x):\n    return x * x\n".to_vec());
    family.locations[0].shared_source_region = source.line_region(1, 2);
    let key = review_key(&family).unwrap();
    family.locations[0].shared_subdag = Some((200, 201));
    assert_eq!(review_key(&family), Some(key));
    family.locations[0]
        .shared_source_region
        .as_mut()
        .unwrap()
        .content_digest = ContentDigest::sha256(b"edited helper");
    assert_ne!(review_key(&family), Some(key));
}

#[test]
fn review_analysis_relabels_occurrence_salts_without_changing_detection() {
    use nose_il::{FileId, Interner, Lang, UnitKind};
    let extract = |path: &str, source: &str, file| {
        let interner = Interner::new();
        let il = nose_frontend::lower_source(
            FileId(file),
            path,
            source.as_bytes(),
            Lang::JavaScript,
            &interner,
        )
        .unwrap();
        let options = crate::DetectOptions {
            min_lines: 1,
            min_tokens: 1,
            ..Default::default()
        };
        crate::units_of_file(&il, &interner, &options)
            .into_iter()
            .find(|u| u.kind == UnitKind::Function && u.name.as_deref() == Some("compute"))
            .unwrap()
    };
    let source = "function compute(x) { const a = x.includes(1); const b = x.includes(1); return a === b; }\n";
    let before = extract("a.js", source, 0);
    assert!(
        before.review_value.is_some(),
        "fixture must exercise source-salted values"
    );
    let moved = extract(
        "moved.js",
        &format!("// header α\r\nfunction unrelated() {{ return 7; }}\n{source}"),
        5,
    );
    assert_ne!(
        before.value, moved.value,
        "detection salts still distinguish source occurrences"
    );
    assert_eq!(
        super::unit_analysis_key(&before),
        super::unit_analysis_key(&moved)
    );
    let changed = extract("a.js", &source.replace("includes(1)", "includes(2)"), 0);
    assert_ne!(
        super::unit_analysis_key(&before),
        super::unit_analysis_key(&changed)
    );
    let constant = extract("a.js", "function compute(x) { return true; }", 0);
    assert_ne!(
        before.review_value.unwrap().returns,
        constant.returns,
        "two unproven calls must not collapse into equality even in review analysis"
    );
}
