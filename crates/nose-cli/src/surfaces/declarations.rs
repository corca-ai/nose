use super::*;
use crate::source_lines::FileLineCache;
use rayon::prelude::*;

pub(super) fn family_declaration_run(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    overrides
        .declaration_run_families
        .contains(&family_handle(family))
}

pub(super) fn family_declaration_only_type_contract(
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
pub(super) fn declaration_only_type_contract_families(
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
/// extraction exists and no judgment is owed.
///
/// Fail-open by construction: any line not provably part of a declaration, an
/// unsupported extension, an unreadable span, or an unclosed multi-line
/// statement keeps the family on its ranked surface.
pub(super) fn declaration_run_families(
    families: &[nose_detect::RefactorFamily],
) -> FxHashSet<FamilyHandle> {
    // Prescreen serially, parse the unique candidate files in parallel, then
    // classify against the shared facts. Parallel parsing avoids the measured
    // large-repository cost without duplicating work for repeated locations.
    let mut lines = FileLineCache::default();
    let mut candidates: Vec<&nose_detect::RefactorFamily> = Vec::new();
    let mut wanted: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for family in families {
        if !declaration_run_candidate(family) {
            continue;
        }
        let pass = family.locations.iter().all(|location| {
            lines.whole(&location.file).is_some_and(|all| {
                declaration_prescreen(all, location.start_line, location.end_line)
            })
        });
        if pass {
            candidates.push(family);
            wanted.extend(
                family
                    .locations
                    .iter()
                    .map(|location| location.file.clone()),
            );
        }
    }
    let facts: std::collections::HashMap<String, Option<nose_frontend::DeclarationFacts>> = wanted
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|file| {
            let parsed = std::path::Path::new(&file)
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(|extension| {
                    let source = std::fs::read_to_string(&file).ok()?;
                    nose_frontend::declaration_facts(extension, &source)
                });
            (file, parsed)
        })
        .collect();
    candidates
        .iter()
        .filter(|family| {
            family
                .locations
                .iter()
                .all(|location| declaration_run_span(location, &mut lines, &facts))
        })
        .map(|family| family_handle(family))
        .collect()
}

fn declaration_run_candidate(family: &nose_detect::RefactorFamily) -> bool {
    !family.locations.is_empty()
        && family
            .witness
            .as_ref()
            .is_some_and(|witness| witness.kind() == "copy-paste-run")
        && family.locations.iter().all(|location| {
            declaration_candidate_lang(&location.lang)
                && location.kind == nose_il::UnitKind::Block
                && location.name.is_none()
                && location.end_line.saturating_sub(location.start_line) <= DECLARATION_SPAN_CAP
        })
}

fn declaration_candidate_lang(lang: &str) -> bool {
    matches!(lang, "javascript" | "typescript")
}

/// An import run longer than this is implausible; skip the read and fail open.
const DECLARATION_SPAN_CAP: u32 = 80;

fn declaration_run_span(
    location: &nose_detect::Loc,
    lines: &mut FileLineCache,
    facts: &std::collections::HashMap<String, Option<nose_frontend::DeclarationFacts>>,
) -> bool {
    if location.end_line.saturating_sub(location.start_line) > DECLARATION_SPAN_CAP {
        return false;
    }
    let Some(Some(facts)) = facts.get(&location.file) else {
        return false;
    };
    let Some(all) = lines.whole(&location.file) else {
        return false;
    };
    span_is_declarations(facts, all, location.start_line, location.end_line)
}

/// Cheap starter check before the AST parse. Comment lines are transparent;
/// the first content line must begin like wiring. False negatives only fail
/// open, so they cannot misclassify an actionable family.
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
        // The AST classifier strips one leading UTF-8 BOM, so the prescreen
        // must do the same.
        let text = line.trim_start_matches('\u{feff}').trim_start();
        if text.is_empty() || text.starts_with("//") || text.starts_with("/*") {
            continue;
        }
        if text.starts_with('#') && !text.starts_with("#include") && !text.starts_with("#pragma") {
            continue;
        }
        // A reported span can start inside a multi-line declaration.
        if text.starts_with('}') || text.starts_with(')') {
            return true;
        }
        if text.chars().all(|character| {
            character.is_alphanumeric() || matches!(character, '_' | '$' | ',' | ' ' | '.')
        }) {
            return true;
        }
        // CommonJS wiring needs the call, not only the declaration keyword.
        for head in ["const ", "let ", "var "] {
            if text.starts_with(head) {
                return text.contains("= require(");
            }
        }
        return STARTERS.iter().any(|starter| text.starts_with(starter));
    }
    false
}

/// Every line in the span must be blank, a comment, or part of a declaration
/// statement, and at least one declaration line must be present. A named code
/// leaf on a declaration line poisons that line, so mixed `import; work()`
/// statements fail open.
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
        return false;
    }
    any
}
