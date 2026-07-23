use rustc_hash::FxHashSet;

mod declarations;
mod generated;
mod generated_paths;

#[cfg(test)]
pub(crate) use declarations::span_is_declarations;
use declarations::{
    declaration_only_type_contract_families, declaration_run_families,
    family_declaration_only_type_contract, family_declaration_run,
};
use generated::{generated_source_indexes, GeneratedSourceIndexes};
#[cfg(test)]
pub(crate) use generated::{
    has_version_tag, head_has_declared_generator_provenance, head_has_jazzy_generated_provenance,
    looks_compiled_css,
};
pub(crate) use generated_paths::GeneratedPathAssertions;

/// Compute the surface overrides for every output format and flag generated
/// locations. The generated indexes are one head-read per discovered file
/// (#224 — the #216 audit's re2c case) and the declaration analysis is one
/// span-read per family; both run only when families exist.
#[cfg(test)]
pub(crate) fn classify_surface_overrides(
    families: &mut [nose_detect::RefactorFamily],
) -> SurfaceOverrides {
    classify_surface_overrides_with_generated_paths(families, &GeneratedPathAssertions::default())
}

pub(crate) fn classify_surface_overrides_with_generated_paths(
    families: &mut [nose_detect::RefactorFamily],
    caller_generated_paths: &GeneratedPathAssertions,
) -> SurfaceOverrides {
    let generated = if families.is_empty() {
        GeneratedSourceIndexes::default()
    } else {
        generated_source_indexes(families, caller_generated_paths)
    };
    for f in families.iter_mut() {
        for l in &mut f.locations {
            // `looks_generated` also participates in helper selection and other semantic
            // report fields. Keep it on the established, source-local header/build-artifact
            // contract; provenance that only changes the output surface must not alter the
            // family records exposed by `all top=0` (#842).
            l.looks_generated = generated.sources.contains(&l.file);
        }
    }
    SurfaceOverrides {
        generated_sources: generated.sources,
        additional_generated_surface_sources: generated.additional_surface_sources,
        caller_generated_surface_sources: generated.caller_surface_sources,
        declaration_run_families: declaration_run_families(families),
        declaration_only_type_contract_families: declaration_only_type_contract_families(families),
    }
}

/// Process-local identity for a ranked family.
///
/// Surface overrides live only as long as the query's family vector. The location buffer is
/// not resized after classification, so its address and length remain stable even if the outer
/// family vector is sorted. This avoids repeatedly sorting and hashing every location merely to
/// ask whether an already-classified family has a presentation override (#892).
type FamilyHandle = (usize, usize);

fn family_handle(family: &nose_detect::RefactorFamily) -> FamilyHandle {
    (family.locations.as_ptr() as usize, family.locations.len())
}

/// The mechanically-decidable non-actionable classes (design.md §2b: the
/// decidability boundary). These are *classifications, not deletions*: the
/// families stay in `--format json --top 0` under an honest surface name; only
/// the action-oriented surfaces (human/markdown/SARIF/`--fail-on`) omit them.
pub(crate) struct SurfaceOverrides {
    /// Files whose head or stylesheet distribution markers classify them as generated (#224).
    pub(crate) generated_sources: FxHashSet<String>,
    /// Additional source-coherent provenance used only to classify output surfaces. These
    /// files deliberately do not set `Loc::looks_generated`, which has semantic effects on
    /// helper selection and folding beyond presentation (#842).
    pub(crate) additional_generated_surface_sources: FxHashSet<String>,
    /// Caller assertions from root-anchored `--generated-path` / config globs. Like
    /// other presentation-only provenance, these never set `Loc::looks_generated`.
    pub(crate) caller_generated_surface_sources: FxHashSet<String>,
    /// Families whose every member span is provably only import/include/
    /// use/re-export declarations — duplication the language mandates per
    /// file, with no extraction action to take.
    pub(crate) declaration_run_families: FxHashSet<FamilyHandle>,
    /// Families whose every member carries the complete, already-lowered
    /// declaration-only type-contract proof frozen by #841. This consumes only
    /// language-neutral `UnitOrigin` facets; missing or mixed evidence fails open.
    pub(crate) declaration_only_type_contract_families: FxHashSet<FamilyHandle>,
}

/// The surface an integration should treat this family as: the ranked
/// `recommended_surface`, except that generated families report as `generated`,
/// and mechanically non-actionable declaration runs or declaration-only type
/// contracts report as `declaration` — the same families the human report omits
/// from default output.
pub(crate) fn effective_surface(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> &'static str {
    if family_generated_source(family, overrides) {
        "generated"
    } else if family_declaration_run(family, overrides)
        || family_declaration_only_type_contract(family, overrides)
    {
        "declaration"
    } else {
        family.recommended_surface()
    }
}

pub(crate) fn is_default_report_family(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    effective_surface(family, overrides) == "default"
}

/// The stable family set used to compute overlap folds. Additional provenance may move a
/// whole opportunity from `default` to `generated`, but presentation classification must
/// not dissolve its existing fold forest and expose new slice ids in `all top=0` (#842).
/// Established generated/build-artifact and declaration rules retain their prior behavior.
pub(crate) fn is_default_opportunity_family(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    if family_established_generated_source(family, &overrides.generated_sources)
        || family_declaration_run(family, overrides)
    {
        false
    } else {
        family.recommended_surface() == "default"
    }
}

/// The decidable `actionability_reason` for the JSON contract (#11): the source-derived
/// CLI-side non-action classes take precedence —
/// mirroring [`effective_surface`] — then the detector's pure-shape codes (`trivial`,
/// `shallow-extraction`). `None` for a clean candidate. A reason, not a verdict.
#[cfg(test)]
pub(crate) fn family_actionability_reason(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> Option<&'static str> {
    if family_generated_source(family, overrides) {
        Some("generated-source")
    } else if family_declaration_run(family, overrides) {
        Some("declaration-run")
    } else if family_declaration_only_type_contract(family, overrides) {
        Some("declaration-only-type-contract")
    } else {
        family.actionability_reason()
    }
}

fn family_all_generated_source(
    family: &nose_detect::RefactorFamily,
    generated_sources: &FxHashSet<String>,
    additional_generated_surface_sources: &FxHashSet<String>,
    caller_generated_surface_sources: &FxHashSet<String>,
) -> bool {
    !family.locations.is_empty()
        && family.locations.iter().all(|loc| {
            generated_sources.contains(&loc.file)
                || additional_generated_surface_sources.contains(&loc.file)
                || caller_generated_surface_sources.contains(&loc.file)
        })
}

fn family_generated_source(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    family_all_generated_source(
        family,
        &overrides.generated_sources,
        &overrides.additional_generated_surface_sources,
        &overrides.caller_generated_surface_sources,
    ) || family_is_compiled_css_pipeline(family, &overrides.generated_sources)
}

pub(crate) fn generated_provenance_json(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> Option<serde_json::Value> {
    let all_members = family_all_generated_source(
        family,
        &overrides.generated_sources,
        &overrides.additional_generated_surface_sources,
        &overrides.caller_generated_surface_sources,
    );
    let compiled_css_pipeline =
        family_is_compiled_css_pipeline(family, &overrides.generated_sources);
    if !all_members && !compiled_css_pipeline {
        return None;
    }

    let caller = family.locations.iter().any(|location| {
        overrides
            .caller_generated_surface_sources
            .contains(&location.file)
    });
    let inferred = compiled_css_pipeline
        || family.locations.iter().any(|location| {
            overrides.generated_sources.contains(&location.file)
                || overrides
                    .additional_generated_surface_sources
                    .contains(&location.file)
        });
    let mut sources = Vec::with_capacity(2);
    if caller {
        sources.push("caller-path");
    }
    if inferred {
        sources.push("nose-inferred");
    }
    Some(serde_json::json!({
        "basis": if all_members { "all-members" } else { "compiled-css-pipeline" },
        "sources": sources,
    }))
}

fn family_established_generated_source(
    family: &nose_detect::RefactorFamily,
    generated_sources: &FxHashSet<String>,
) -> bool {
    (!family.locations.is_empty()
        && family
            .locations
            .iter()
            .all(|loc| generated_sources.contains(&loc.file)))
        || family_is_compiled_css_pipeline(family, generated_sources)
}

/// A CSS build-pipeline family: every member is a stylesheet and AT MOST ONE is a
/// hand-written source — the rest are its compiled/minified outputs (`generated_sources`).
/// Such a family is one source rule echoed through the build (source → compiled → minified),
/// not a cross-source duplication a maintainer would dedupe, so it is kept off the default
/// surface like other generated code. A genuine source dedup spans ≥2 source files (≥2
/// non-compiled members) and stays on the surface. This catches the `src/_x.css` +
/// `bundle.css` + `bundle.min.css` families the all-compiled rule misses (the lone source
/// partial keeps them off the all-generated path). Measured on the frontend gold set: 255
/// generated families demoted (108 beyond the all-compiled rule), 0 worthy — sound.
pub(crate) fn family_is_compiled_css_pipeline(
    family: &nose_detect::RefactorFamily,
    generated_sources: &FxHashSet<String>,
) -> bool {
    if family.locations.is_empty() || !family.locations.iter().all(|l| l.file.ends_with(".css")) {
        return false;
    }
    let compiled = family
        .locations
        .iter()
        .filter(|l| generated_sources.contains(&l.file))
        .count();
    let source = family.locations.len() - compiled;
    compiled >= 1 && source <= 1
}

pub(crate) fn surface_omission_note(
    families: &[nose_detect::RefactorFamily],
    overrides: &SurfaceOverrides,
) -> Option<String> {
    let mut generated = 0;
    let mut declaration = 0;
    let mut type_contract = 0;
    let mut shallow = 0;
    let mut divergence = 0;
    let mut hidden = 0;
    let mut debug = 0;
    for family in families {
        match effective_surface(family, overrides) {
            "generated" => generated += 1,
            "declaration" if family_declaration_run(family, overrides) => declaration += 1,
            "declaration" if family_declaration_only_type_contract(family, overrides) => {
                type_contract += 1
            }
            "declaration" => declaration += 1,
            "shallow" => shallow += 1,
            "divergence" => divergence += 1,
            "hidden" => hidden += 1,
            "debug" => debug += 1,
            _ => {}
        }
    }
    let omitted = generated + declaration + type_contract + shallow + divergence + hidden + debug;
    if omitted == 0 {
        return None;
    }
    if generated == 0
        && declaration == 0
        && type_contract == 0
        && shallow == 0
        && divergence == 0
        && hidden == 1
        && debug == 0
    {
        return Some("omitted 1 hidden proof-only family from default output".to_string());
    }
    let mut parts = Vec::new();
    if generated > 0 {
        parts.push(format!("{generated} generated-code"));
    }
    if declaration > 0 {
        parts.push(format!("{declaration} declaration-run"));
    }
    if type_contract > 0 {
        parts.push(format!("{type_contract} declaration-only-type-contract"));
    }
    if shallow > 0 {
        parts.push(format!("{shallow} shallow-extraction"));
    }
    if divergence > 0 {
        parts.push(format!("{divergence} divergence"));
    }
    if hidden > 0 {
        parts.push(format!("{hidden} hidden"));
    }
    if debug > 0 {
        parts.push(format!("{debug} debug"));
    }
    let family_word = if omitted == 1 { "family" } else { "families" };
    Some(format!(
        "omitted {omitted} {family_word} from default output ({})",
        parts.join(", ")
    ))
}
