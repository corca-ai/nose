//! Content candidates in already-projected changed files. This never expands
//! discovery, asserts ancestry, or participates in the frozen divergence gate.
use super::*;
use nose_il::{ContentDigest, SourceRegion};

const MAX_MATCHES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct RegionMatches {
    schema: &'static str,
    status: &'static str,
    search_scope: &'static str,
    complete: bool,
    files_examined: usize,
    files_in_scope: usize,
    max_files: usize,
    max_candidates: usize,
    base: SourceRegion,
    candidates: Vec<RegionCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct RegionCandidate {
    file: String,
    lang: String,
    kind: UnitKind,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
    source: SourceRegion,
}

pub(super) struct SourceMatchIndex {
    by_content: BTreeMap<ContentDigest, Vec<RegionCandidate>>,
    complete: bool,
    files_examined: usize,
}

impl WitnessBuilder<'_> {
    pub(super) fn enrich_source_matches(
        &mut self,
        site: &Site,
        witness: &mut SemanticChangeWitness,
    ) {
        witness.region_matches = self.source_matches(site);
        if witness.region_matches.is_some()
            && witness.status == SemanticWitnessStatus::Complete
            && matches!(
                witness.alignment,
                SemanticAlignment::ChangedRange | SemanticAlignment::NearestSpan
            )
        {
            // A competing source-content occurrence means that path/range alignment
            // alone cannot establish which current computation replaced this site.
            witness.status = SemanticWitnessStatus::Advisory;
            witness
                .caveats
                .push(SemanticWitnessCaveat::HeuristicAlignment);
            witness.caveats.sort();
            witness.caveats.dedup();
        }
    }

    fn source_matches(&mut self, site: &Site) -> Option<RegionMatches> {
        if site.is_fragment {
            return None;
        }
        let unit = self.base_unit(site).ok()?;
        let file = self.load_file(Tree::Base, &site.file).ok()?;
        let base = source_region(file, &unit)?;
        let index = self
            .source_match_index
            .get_or_init(|| self.build_source_match_index());
        let candidates: Vec<_> = index
            .by_content
            .get(&base.content_digest)?
            .iter()
            .filter(|candidate| candidate.kind == site.kind && candidate.lang == site.lang)
            .collect();
        let expected = self.current_path(&site.file);
        if candidates.is_empty()
            || matches!(candidates.as_slice(), [only] if Some(&only.file) == expected.as_ref())
        {
            return None;
        }
        let capped = candidates.len() > MAX_MATCHES;
        Some(RegionMatches {
            schema: "nose.changed-region-candidates/v1",
            status: if capped {
                "budget-exceeded"
            } else if !index.complete {
                "partial"
            } else if candidates.len() == 1 {
                "unique-content-candidate"
            } else {
                "ambiguous"
            },
            search_scope: "already-projected-changed-files",
            complete: index.complete && !capped,
            files_examined: index.files_examined,
            files_in_scope: self.current_changed.len(),
            max_files: MAX_FILES,
            max_candidates: MAX_MATCHES,
            base,
            candidates: if capped {
                Vec::new()
            } else {
                candidates.into_iter().cloned().collect()
            },
        })
    }

    fn build_source_match_index(&self) -> SourceMatchIndex {
        let mut index = SourceMatchIndex {
            by_content: BTreeMap::new(),
            complete: self.current_changed.len() <= MAX_FILES,
            files_examined: 0,
        };
        let mut paths: Vec<_> = self.current_changed.keys().collect();
        paths.sort();
        for path in paths.into_iter().take(MAX_FILES) {
            let state = self
                .files
                .get(&(Tree::Current, path.clone()))
                .or_else(|| self.preprojected_current_files.get(path));
            let Some(LoadState::Ready(file)) = state else {
                index.complete = false;
                continue;
            };
            index.files_examined += 1;
            for unit in &file.units {
                let Some(source) = source_region(file, unit) else {
                    index.complete = false;
                    continue;
                };
                index
                    .by_content
                    .entry(source.content_digest)
                    .or_default()
                    .push(RegionCandidate {
                        file: path.clone(),
                        lang: file.normalized.meta.lang.name().into(),
                        kind: unit.kind,
                        name: unit.name.clone(),
                        start_line: unit.start_line,
                        end_line: unit.end_line,
                        source,
                    });
            }
        }
        for candidates in index.by_content.values_mut() {
            candidates.sort_by(|a, b| {
                (&a.file, a.start_line, a.end_line, &a.name).cmp(&(
                    &b.file,
                    b.start_line,
                    b.end_line,
                    &b.name,
                ))
            });
            candidates.dedup();
        }
        index
    }
}

fn source_region(file: &FileProjection, unit: &UnitSkeleton) -> Option<SourceRegion> {
    let span = file.normalized.node(unit.root).span;
    if span.file != file.normalized.file {
        return None;
    }
    file.normalized
        .source
        .as_ref()?
        .region(span.start_byte, span.end_byte)
}

impl RegionMatches {
    pub(crate) fn concise_label(&self) -> String {
        if self.status == "budget-exceeded" {
            return format!(
                "source-region candidates exceed {}; inspect changed files",
                self.max_candidates
            );
        }
        let locations = self
            .candidates
            .iter()
            .take(3)
            .map(|c| format!("{}:{}", c.file, c.start_line))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{} source-region match(es), {}: {}",
            self.candidates.len(),
            self.status,
            locations
        )
    }
}
