use super::*;

pub(super) fn annotate_semantic_pack_near(
    families: &mut [nose_detect::RefactorFamily],
    registry: &nose_semantics::SemanticPackNearRegistry,
) {
    if !registry.is_active() {
        return;
    }
    for family in families.iter_mut().filter(|family| {
        family.witness.as_ref().map(|witness| witness.kind) == Some("structural-similarity")
    }) {
        let protocols = family
            .locations
            .iter()
            .map(|location| {
                registry.protocols_for_unit(&location.file, location.start_line, location.end_line)
            })
            .collect::<Vec<_>>();
        let mut aggregate = BTreeSet::new();
        for (index, location) in family.locations.iter_mut().enumerate() {
            let mut member = BTreeSet::new();
            for protocol in &protocols[index] {
                let Some(provenance) = &protocol.provenance else {
                    continue;
                };
                let supported = protocols.iter().enumerate().any(|(other_index, others)| {
                    other_index != index
                        && others
                            .iter()
                            .any(|other| other.operation == protocol.operation)
                });
                if supported {
                    member.insert(provenance.clone());
                    aggregate.insert(provenance.clone());
                }
            }
            location.semantic_pack_near = member.into_iter().collect();
        }
        family.semantic_pack_near = aggregate.into_iter().collect();
    }
}

pub(super) fn annotate_semantic_pack_external_exact(
    families: &mut [nose_detect::RefactorFamily],
    registry: &nose_semantics::SemanticPackExternalExactRegistry,
) {
    if !registry.is_active() {
        return;
    }
    for family in families.iter_mut().filter(|family| {
        family.witness.as_ref().map(|witness| witness.kind) == Some("exact-value-graph")
    }) {
        let mut aggregate = BTreeSet::new();
        for location in &mut family.locations {
            let claims =
                registry.claims_for_unit(&location.file, location.start_line, location.end_line);
            aggregate.extend(claims.iter().cloned());
            location.semantic_pack_external_exact = claims;
        }
        family.semantic_pack_external_exact = aggregate.into_iter().collect();
    }
}

/// Compute the honest shared-line count for each family, before ranking. This layer has
/// source access; the detector deals only in IL. Cross-language families keep the
/// detector's structural estimate because they have no shared source lines to diff.
pub(super) fn weight_shared_lines(
    families: &mut [nose_detect::RefactorFamily],
    refs: &[&std::path::Path],
    exclude: &[String],
    cached: Option<&cache::CachedLineContext>,
) {
    let needs_shared = |f: &nose_detect::RefactorFamily| f.languages == 1 && f.locations.len() >= 2;
    if !families.iter().any(needs_shared) {
        return;
    }
    let mut lines = FileLineCache::default();
    if let Some(context) = cached {
        let (mut idf, mut stats, mut changed_lines, mut file_count, complete) =
            cached_line_idf(context, &mut lines, false);
        let mut family_stats = apply_cached_family_lines(
            families,
            &idf,
            &mut lines,
            context,
            &changed_lines,
            file_count,
            complete,
        );
        if family_stats.is_none() {
            lines = FileLineCache::default();
            let full = cached_line_idf(context, &mut lines, true);
            idf = full.0;
            stats = full.1;
            changed_lines = full.2;
            file_count = full.3;
            family_stats = apply_cached_family_lines(
                families,
                &idf,
                &mut lines,
                context,
                &changed_lines,
                file_count,
                true,
            );
        }
        let family_stats = family_stats.expect("full line index covers every family");
        if std::env::var_os("NOSE_CACHE_STATS").is_some() {
            eprintln!("  [line-index] {}", cache::line_index_stats_json(&stats));
            eprintln!(
                "  [family-lines] {}",
                serde_json::to_string(&family_stats)
                    .expect("family line stats are JSON serializable")
            );
        }
        return;
    }
    let idf = corpus_line_idf(refs, exclude, &mut lines);
    for f in families.iter_mut().filter(|f| needs_shared(f)) {
        f.varying_spots = f.locations[1..]
            .iter()
            .find_map(|b| varying_spots_of(&f.locations[0], b, &mut lines))
            .unwrap_or_default();
        if let Some(s) = shared_lines_of(&f.locations, &mut lines) {
            let substantive: f64 = s
                .rank_lines
                .iter()
                .filter(|l| !is_trivial_line(l))
                .map(|l| idf.weight(l))
                .sum();
            let gate = (substantive / 2.0).clamp(0.0, 1.0);
            f.shared_lines = s.display;
            f.shared_weight = s.rank_lines.len() as f64 * gate;
            f.params = s.params;
            f.display_params = Some(s.display_params);
        }
    }
}
