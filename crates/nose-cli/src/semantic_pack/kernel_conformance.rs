use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub(super) struct KernelConformanceReport {
    pub status: &'static str,
    pub receipts: Vec<nose_semantics::SemanticPackConformanceReceiptV1>,
}

impl KernelConformanceReport {
    pub(super) fn passed(&self) -> bool {
        self.receipts.iter().all(|receipt| receipt.passed)
    }

    pub(super) fn fixture_count(&self) -> usize {
        self.receipts
            .iter()
            .map(|receipt| receipt.fixtures.len())
            .sum()
    }

    pub(super) fn passed_fixture_count(&self) -> usize {
        self.receipts
            .iter()
            .flat_map(|receipt| &receipt.fixtures)
            .filter(|fixture| fixture.passed)
            .count()
    }
}

pub(super) fn run(paths: &[PathBuf]) -> Result<KernelConformanceReport> {
    let packs = nose_semantics::SemanticPackSet::new_local(paths)?;
    let manifest_paths = packs
        .packs()
        .iter()
        .filter_map(|summary| {
            summary
                .manifest_path
                .as_ref()
                .map(|path| (summary.id.as_str(), path.as_path()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut receipts = Vec::new();
    for pack in packs.compiled_external_v1_packs() {
        if pack.conformance_fixtures().is_empty() {
            continue;
        }
        let manifest_path = manifest_paths
            .get(pack.pack_id())
            .copied()
            .context("compiled v1 pack is missing its manifest path")?;
        receipts.push(run_pack(pack, manifest_path));
    }
    let status = if receipts.is_empty() {
        "unavailable"
    } else if receipts.iter().all(|receipt| receipt.passed) {
        "ok"
    } else {
        "failed"
    };
    Ok(KernelConformanceReport { status, receipts })
}

fn run_pack(
    pack: &nose_semantics::CompiledSemanticPackV1,
    manifest_path: &Path,
) -> nose_semantics::SemanticPackConformanceReceiptV1 {
    let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut fixtures = pack
        .conformance_fixtures()
        .iter()
        .map(|fixture| run_fixture(pack, root, fixture))
        .collect::<Vec<_>>();
    fixtures.sort_by(|left, right| left.id.cmp(&right.id));
    let mut rows = pack
        .contracts_by_id()
        .values()
        .filter(|contract| contract.channel == nose_semantics::SemanticPackV1Channel::ExternalExact)
        .map(
            |contract| nose_semantics::SemanticPackConformanceReceiptRow {
                row_id: contract.id.clone(),
                row_digest: pack
                    .row_digest(&contract.id)
                    .expect("compiled row has a digest")
                    .to_string(),
                channel: contract.channel,
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.row_id.cmp(&right.row_id));
    let passed = !fixtures.is_empty() && fixtures.iter().all(|fixture| fixture.passed);
    nose_semantics::SemanticPackConformanceReceiptV1 {
        api_version: nose_semantics::SEMANTIC_PACK_RECEIPT_API_VERSION_V1.to_string(),
        nose_version: env!("CARGO_PKG_VERSION").to_string(),
        kernel_capability: nose_semantics::SEMANTIC_PACK_EXACT_KERNEL_CAPABILITY_V1.to_string(),
        pack_id: pack.pack_id().to_string(),
        pack_version: pack.pack_version().to_string(),
        semantic_digest: pack.semantic_digest().to_string(),
        rows,
        fixtures,
        passed,
    }
}

fn run_fixture(
    pack: &nose_semantics::CompiledSemanticPackV1,
    root: &Path,
    fixture: &nose_semantics::SemanticPackV1ConformanceFixture,
) -> nose_semantics::SemanticPackConformanceReceiptFixture {
    let fixture_path = nose_semantics::resolve_fixture_path(root, &fixture.path);
    let dependency_path = nose_semantics::resolve_fixture_path(root, &fixture.dependency);
    let fixture_digest = fixture_path
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|path| nose_semantics::semantic_pack_fixture_digest(path));
    let dependency_digest = dependency_path
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|path| nose_semantics::semantic_pack_file_digest(path));
    let observed = match (
        fixture_path.as_ref(),
        dependency_path.as_ref(),
        fixture_digest.as_ref(),
        dependency_digest.as_ref(),
    ) {
        (Ok(fixture_path), Ok(dependency_path), Ok(_), Ok(_)) => {
            observe_fixture(pack, fixture, fixture_path, dependency_path)
        }
        _ if [
            fixture_digest.as_ref().err(),
            dependency_digest.as_ref().err(),
        ]
        .into_iter()
        .flatten()
        .any(|message| message.contains("exceeds")) =>
        {
            nose_semantics::SemanticPackV1ObservedExpectation::ResourceLimit
        }
        _ => nose_semantics::SemanticPackV1ObservedExpectation::AnalysisFailure,
    };
    let passed = matches!(
        (fixture.expectation, observed),
        (
            nose_semantics::SemanticPackV1Expectation::ExternalExactMatch,
            nose_semantics::SemanticPackV1ObservedExpectation::ExternalExactMatch
        ) | (
            nose_semantics::SemanticPackV1Expectation::NoExternalExactMatch,
            nose_semantics::SemanticPackV1ObservedExpectation::NoExternalExactMatch
        )
    );
    nose_semantics::SemanticPackConformanceReceiptFixture {
        id: fixture.id.clone(),
        row_id: fixture.row_id.clone(),
        kind: fixture.kind,
        path: fixture.path.clone(),
        dependency: fixture.dependency.clone(),
        fixture_digest: fixture_digest.unwrap_or_default(),
        dependency_digest: dependency_digest.unwrap_or_default(),
        expectation: fixture.expectation,
        observed,
        passed,
    }
}

fn observe_fixture(
    pack: &nose_semantics::CompiledSemanticPackV1,
    fixture: &nose_semantics::SemanticPackV1ConformanceFixture,
    fixture_path: &Path,
    dependency_path: &Path,
) -> nose_semantics::SemanticPackV1ObservedExpectation {
    let discovered = nose_frontend::discover_unique_paths(&[fixture_path], &[]);
    if discovered.is_empty()
        || discovered
            .iter()
            .any(|(_, language)| *language != nose_il::Lang::Java)
        || discovered.iter().any(|(path, _)| {
            std::fs::read(path)
                .map(|source| !nose_frontend::java_source_parses_cleanly(&source))
                .unwrap_or(true)
        })
    {
        return nose_semantics::SemanticPackV1ObservedExpectation::AnalysisFailure;
    }
    let mut corpus = nose_frontend::lower_corpus_filtered(&[fixture_path], &[]);
    if corpus.files.len() != discovered.len() {
        return nose_semantics::SemanticPackV1ObservedExpectation::AnalysisFailure;
    }
    let evidence = nose_semantics::SemanticPackEvidenceIndex::build_for_conformance(
        pack,
        &fixture.row_id,
        &[dependency_path.to_path_buf()],
        &corpus,
    );
    let registry = nose_semantics::SemanticPackExternalExactRegistry::build_for_conformance(
        pack, &evidence, &corpus,
    );
    let opts = nose_detect::DetectOptions {
        min_lines: 1,
        min_tokens: 1,
        block_units: false,
        shape_features: false,
        emit_pairs: false,
        ..Default::default()
    };
    let baseline_pairs = exact_family_pairs(&detect_exact_families(&corpus, &opts));
    registry.apply(&mut corpus);
    let families = detect_exact_families(&corpus, &opts);
    let has_external_exact_match = families.iter().any(|family| {
        if family.witness.as_ref().map(|witness| witness.kind) != Some("exact-value-graph") {
            return false;
        }
        let claims = family
            .locations
            .iter()
            .map(|location| {
                !registry
                    .claims_for_unit(&location.file, location.start_line, location.end_line)
                    .is_empty()
            })
            .collect::<Vec<_>>();
        claims.iter().any(|claimed| *claimed)
            && claims.iter().any(|claimed| !*claimed)
            && family_pairs(family)
                .iter()
                .any(|pair| !baseline_pairs.contains(pair))
    });
    if has_external_exact_match {
        nose_semantics::SemanticPackV1ObservedExpectation::ExternalExactMatch
    } else {
        nose_semantics::SemanticPackV1ObservedExpectation::NoExternalExactMatch
    }
}

type LocationKey = (String, u32, u32);
type LocationPair = (LocationKey, LocationKey);

fn detect_exact_families(
    corpus: &nose_il::Corpus,
    opts: &nose_detect::DetectOptions,
) -> Vec<nose_detect::RefactorFamily> {
    let features = nose_detect::corpus_features(corpus, opts);
    let report = nose_detect::detect_from_units(
        features.units,
        features.files,
        &features.streams,
        opts,
        &nose_detect::ExactBehaviorDetector,
    )
    .0;
    nose_detect::rank_families(&report)
}

fn exact_family_pairs(
    families: &[nose_detect::RefactorFamily],
) -> std::collections::BTreeSet<LocationPair> {
    families
        .iter()
        .filter(|family| {
            family.witness.as_ref().map(|witness| witness.kind) == Some("exact-value-graph")
        })
        .flat_map(family_pairs)
        .collect()
}

fn family_pairs(family: &nose_detect::RefactorFamily) -> Vec<LocationPair> {
    let locations = family
        .locations
        .iter()
        .map(|location| {
            (
                location.file.clone(),
                location.start_line,
                location.end_line,
            )
        })
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for (index, left) in locations.iter().enumerate() {
        for right in locations.iter().skip(index + 1) {
            pairs.push(if left <= right {
                (left.clone(), right.clone())
            } else {
                (right.clone(), left.clone())
            });
        }
    }
    pairs
}
