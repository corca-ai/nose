use rayon::prelude::*;
use rustc_hash::FxHashSet;

use crate::source_lines::FileLineCache;

mod generated;

use generated::{generated_source_indexes, GeneratedSourceIndexes};
#[cfg(test)]
pub(crate) use generated::{
    has_version_tag, head_has_jazzy_generated_provenance, looks_compiled_css,
};

/// Compute the surface overrides for every output format and flag generated
/// locations. The generated indexes are one head-read per discovered file
/// (#224 — the #216 audit's re2c case) and the declaration analysis is one
/// span-read per family; both run only when families exist.
pub(crate) fn classify_surface_overrides(
    families: &mut [nose_detect::RefactorFamily],
) -> SurfaceOverrides {
    let generated = if families.is_empty() {
        GeneratedSourceIndexes::default()
    } else {
        generated_source_indexes(families)
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

fn family_declaration_run(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    overrides
        .declaration_run_families
        .contains(&family_handle(family))
}

fn family_declaration_only_type_contract(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    overrides
        .declaration_only_type_contract_families
        .contains(&family_handle(family))
}

/// Apply the typed product form of `declaration-only-type.v1` frozen by #841.
///
/// Every member needs an un-sliced type-unit location plus positive whole-unit,
/// type-only, declaration-only proof. The location-kind check prevents a
/// pair-local connected witness from reusing its enclosing unit's origin after
/// the actionable span has been narrowed to a block.
/// The taxonomy's abstract `runtime` domain maps to the IL's `Imperative`
/// domain and explicit runtime-value/validation flags. Data/implementation
/// domains and every reusable/default/extension body signal are also defensive
/// disqualifiers. Because every condition is positive and all-member, unknown,
/// partial, mixed, default-body, extension, enum, and schema origins remain on
/// their ranked surface.
fn declaration_only_type_contract_families(
    families: &[nose_detect::RefactorFamily],
) -> FxHashSet<FamilyHandle> {
    families
        .iter()
        .filter(|family| is_declaration_only_type_contract(family))
        .map(family_handle)
        .collect()
}

fn is_declaration_only_type_contract(family: &nose_detect::RefactorFamily) -> bool {
    use nose_il::{
        SourceGranularity, UnitBodyKind, UnitDomain, UnitEvidenceFlag, UnitKind, UnitSubkind,
    };

    !family.locations.is_empty()
        && family.locations.iter().all(|location| {
            let origin = location.origin;
            location.kind == UnitKind::Class
                && !location.is_fragment
                && origin.has_domain(UnitDomain::TypeContract)
                && origin
                    .domains
                    .iter()
                    .all(|domain| domain == UnitDomain::TypeContract)
                && matches!(
                    origin.subkind,
                    UnitSubkind::InterfaceTraitProtocol
                        | UnitSubkind::TypeAlias
                        | UnitSubkind::DefinedType
                )
                && origin.body_kind == UnitBodyKind::DeclarationOnly
                && origin.source_granularity == SourceGranularity::WholeUnit
                && origin.has_evidence(UnitEvidenceFlag::DeclarationOnly)
                && origin.has_evidence(UnitEvidenceFlag::TypeOnly)
                && !origin.evidence_flags.iter().any(|flag| {
                    matches!(
                        flag,
                        UnitEvidenceFlag::HasRuntimeBody
                            | UnitEvidenceFlag::HasReusableBody
                            | UnitEvidenceFlag::RuntimeValue
                            | UnitEvidenceFlag::RuntimeValidation
                            | UnitEvidenceFlag::HasDefaultBody
                            | UnitEvidenceFlag::ProtocolExtension
                            | UnitEvidenceFlag::ConcreteTypeExtension
                            | UnitEvidenceFlag::ConstrainedExtension
                            | UnitEvidenceFlag::InterfaceDefaultMethod
                            | UnitEvidenceFlag::InterfaceStaticMethod
                            | UnitEvidenceFlag::InterfacePrivateMethod
                    )
                })
        })
}

/// Classify the mechanically-decidable declaration runs in `families`.
///
/// A *declaration run* is a family whose every member span consists solely of
/// import/include/use/re-export declarations (plus blank lines and full-line
/// comments). The duplication is real — the syntax channel is right that the
/// lines match — but the language mandates these declarations per file, so no
/// extraction exists and no judgment is owed (design.md: provable
/// non-actionability is the detector's job, not the consumer's).
///
/// Fail-open by construction: any line not provably part of a declaration, an
/// unsupported extension, an unreadable span, or an unclosed multi-line
/// statement keeps the family on its ranked surface. Misclassifying a real
/// finding is the error class this guards against; missing an import run is
/// only a ranking nuisance.
fn declaration_run_families(families: &[nose_detect::RefactorFamily]) -> FxHashSet<FamilyHandle> {
    // Three passes (coevo s4 perf packet): a cheap serial prescreen picks the
    // candidate families, the unique candidate files parse in PARALLEL (the
    // serial per-file AST parse cost +29% wall on sympy), and the final pass
    // classifies against the shared facts.
    let mut lines = FileLineCache::default();
    let mut candidates: Vec<&nose_detect::RefactorFamily> = Vec::new();
    let mut wanted: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in families {
        if !declaration_run_candidate(f) {
            continue;
        }
        let pass = f.locations.iter().all(|l| {
            lines
                .whole(&l.file)
                .is_some_and(|all| declaration_prescreen(all, l.start_line, l.end_line))
        });
        if pass {
            candidates.push(f);
            wanted.extend(f.locations.iter().map(|l| l.file.clone()));
        }
    }
    let facts: std::collections::HashMap<String, Option<nose_frontend::DeclarationFacts>> = wanted
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|file| {
            let parsed = std::path::Path::new(&file)
                .extension()
                .and_then(|e| e.to_str())
                .and_then(|ext| {
                    let src = std::fs::read_to_string(&file).ok()?;
                    nose_frontend::declaration_facts(ext, &src)
                });
            (file, parsed)
        })
        .collect();
    candidates
        .iter()
        .filter(|f| {
            f.locations
                .iter()
                .all(|l| declaration_run_span(l, &mut lines, &facts))
        })
        .map(|f| family_handle(f))
        .collect()
}

fn declaration_run_candidate(family: &nose_detect::RefactorFamily) -> bool {
    !family.locations.is_empty()
        && family
            .witness
            .as_ref()
            .is_some_and(|w| w.kind == "copy-paste-run")
        && family.locations.iter().all(|l| {
            declaration_candidate_lang(&l.lang)
                && l.kind == nose_il::UnitKind::Block
                && l.name.is_none()
                && l.end_line.saturating_sub(l.start_line) <= DECLARATION_SPAN_CAP
        })
}

fn declaration_candidate_lang(lang: &str) -> bool {
    matches!(lang, "javascript" | "typescript")
}

/// An import run longer than this is implausible; skip the read and fail open.
const DECLARATION_SPAN_CAP: u32 = 80;

fn declaration_run_span(
    loc: &nose_detect::Loc,
    lines: &mut FileLineCache,
    facts: &std::collections::HashMap<String, Option<nose_frontend::DeclarationFacts>>,
) -> bool {
    if loc.end_line.saturating_sub(loc.start_line) > DECLARATION_SPAN_CAP {
        return false;
    }
    let Some(Some(facts)) = facts.get(&loc.file) else {
        return false;
    };
    let Some(all) = lines.whole(&loc.file) else {
        return false;
    };
    span_is_declarations(facts, all, loc.start_line, loc.end_line)
}

/// Cheap starter check before the AST parse. Comment lines are transparent;
/// the first content line must begin like wiring. False negatives only fail
/// open (the family keeps its ranked surface), so this can never misclassify.
fn declaration_prescreen(all: &[String], start: u32, end: u32) -> bool {
    const STARTERS: &[&str] = &[
        "import",
        "from ",
        "use ",
        "pub use ",
        "pub mod ",
        "pub extern ",
        "pub(",
        "#include",
        "#pragma",
        "package ",
        "require",
        "export ",
        "extern ",
        "mod ",
    ];
    let end = (end as usize).min(all.len());
    if start == 0 || start as usize > end {
        return false;
    }
    for line in &all[start as usize - 1..end] {
        // A leading UTF-8 BOM is invisible to the AST classifier (it strips
        // one) — the prescreen must too, or a BOM'd first import never reaches
        // the parse (coevo S4-C3).
        let t = line.trim_start_matches('\u{feff}').trim_start();
        if t.is_empty() || t.starts_with("//") || t.starts_with("/*") {
            continue;
        }
        if t.starts_with('#') && !t.starts_with("#include") && !t.starts_with("#pragma") {
            continue;
        }
        // A span may begin INSIDE a multi-line import (specifier list or its
        // closer) — the AST node covers those lines, so let the parse decide.
        if t.starts_with('}') || t.starts_with(')') {
            return true;
        }
        if t.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | ',' | ' ' | '.'))
        {
            return true;
        }
        // CommonJS wiring needs the call, not just the keyword.
        for head in ["const ", "let ", "var "] {
            if t.starts_with(head) {
                return t.contains("= require(");
            }
        }
        return STARTERS.iter().any(|s| t.starts_with(s));
    }
    false
}

/// The line rule over AST facts: every line in the span must be blank, a
/// comment, or part of a declaration statement; a single code-poisoned line
/// (any named leaf outside declarations/comments — `import os; evil()` puts
/// `evil()`'s leaves on the import's line) disqualifies the span; and at
/// least one declaration line must be present.
pub(crate) fn span_is_declarations(
    facts: &nose_frontend::DeclarationFacts,
    all: &[String],
    start: u32,
    end: u32,
) -> bool {
    let end = (end as usize).min(all.len()) as u32;
    if start == 0 || start > end {
        return false;
    }
    let mut any = false;
    for line_no in start..=end {
        if facts.is_code_line(line_no) {
            return false;
        }
        if facts.is_declaration_line(line_no) {
            any = true;
            continue;
        }
        if facts.is_comment_line(line_no) || all[line_no as usize - 1].trim().is_empty() {
            continue;
        }
        // Uncovered non-blank content (stray tokens, mid-statement cuts).
        return false;
    }
    any
}

fn family_all_generated_source(
    family: &nose_detect::RefactorFamily,
    generated_sources: &FxHashSet<String>,
    additional_generated_surface_sources: &FxHashSet<String>,
) -> bool {
    !family.locations.is_empty()
        && family.locations.iter().all(|loc| {
            generated_sources.contains(&loc.file)
                || additional_generated_surface_sources.contains(&loc.file)
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
    ) || family_is_compiled_css_pipeline(family, &overrides.generated_sources)
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
