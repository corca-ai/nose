use rayon::prelude::*;
use rustc_hash::FxHashSet;

use crate::path_utils::relativize;
use crate::source_lines::FileLineCache;

/// Compute the surface overrides for every output format and flag generated
/// locations. The generated index is one head-read per discovered file (#224
/// — the #216 audit's re2c case) and the declaration analysis is one span-read
/// per family; both run only when families exist.
pub(crate) fn classify_surface_overrides(
    families: &mut [nose_detect::RefactorFamily],
) -> SurfaceOverrides {
    let generated_sources = if families.is_empty() {
        FxHashSet::default()
    } else {
        generated_source_index(families)
    };
    for f in families.iter_mut() {
        for l in &mut f.locations {
            l.looks_generated = generated_sources.contains(&l.file);
        }
    }
    SurfaceOverrides {
        generated_sources,
        declaration_run_ids: declaration_run_ids(families),
    }
}

/// The mechanically-decidable non-actionable classes (design.md §2b: the
/// decidability boundary). Both are *classifications, not deletions*: the
/// families stay in `--format json --top 0` under an honest surface name; only
/// the action-oriented surfaces (human/markdown/SARIF/`--fail-on`) omit them.
pub(crate) struct SurfaceOverrides {
    /// Files whose head or stylesheet distribution markers classify them as generated (#224).
    pub(crate) generated_sources: FxHashSet<String>,
    /// Family ids whose every member span is provably only import/include/
    /// use/re-export declarations — duplication the language mandates per
    /// file, with no extraction action to take.
    pub(crate) declaration_run_ids: FxHashSet<String>,
}

/// The surface an integration should treat this family as: the ranked
/// `recommended_surface`, except that generated-header families and CSS build
/// pipelines report as `generated`, and a family whose every member is a declaration
/// run reports as `declaration` — the same families the human report omits from
/// default output.
pub(crate) fn effective_surface(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> &'static str {
    if family_generated_source(family, &overrides.generated_sources) {
        "generated"
    } else if family_declaration_run(family, overrides) {
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

/// The decidable `actionability_reason` for the JSON contract (#11): the source-derived
/// CLI-side non-action classes (`generated-source`, `declaration-run`) take precedence —
/// mirroring [`effective_surface`] — then the detector's pure-shape codes (`trivial`,
/// `shallow-extraction`). `None` for a clean candidate. A reason, not a verdict.
#[cfg(test)]
pub(crate) fn family_actionability_reason(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> Option<&'static str> {
    if family_generated_source(family, &overrides.generated_sources) {
        Some("generated-source")
    } else if family_declaration_run(family, overrides) {
        Some("declaration-run")
    } else {
        family.actionability_reason()
    }
}

fn family_declaration_run(
    family: &nose_detect::RefactorFamily,
    overrides: &SurfaceOverrides,
) -> bool {
    overrides
        .declaration_run_ids
        .contains(&crate::baseline::family_id(family))
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
fn declaration_run_ids(families: &[nose_detect::RefactorFamily]) -> FxHashSet<String> {
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
        .map(|f| crate::baseline::family_id(f))
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
) -> bool {
    !family.locations.is_empty()
        && family
            .locations
            .iter()
            .all(|loc| generated_sources.contains(&loc.file))
}

fn family_generated_source(
    family: &nose_detect::RefactorFamily,
    generated_sources: &FxHashSet<String>,
) -> bool {
    family_all_generated_source(family, generated_sources)
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
    let mut shallow = 0;
    let mut divergence = 0;
    let mut hidden = 0;
    let mut debug = 0;
    for family in families {
        match effective_surface(family, overrides) {
            "generated" => generated += 1,
            "declaration" => declaration += 1,
            "shallow" => shallow += 1,
            "divergence" => divergence += 1,
            "hidden" => hidden += 1,
            "debug" => debug += 1,
            _ => {}
        }
    }
    let omitted = generated + declaration + shallow + divergence + hidden + debug;
    if omitted == 0 {
        return None;
    }
    if generated == 0
        && declaration == 0
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

fn generated_source_index(families: &[nose_detect::RefactorFamily]) -> FxHashSet<String> {
    let cwd = std::env::current_dir().ok();
    let mut generated = FxHashSet::default();
    let files = families
        .iter()
        .flat_map(|f| f.locations.iter().map(|l| l.file.as_str()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let generated_files = files
        .into_par_iter()
        .filter(|path| source_has_generated_header(path))
        .collect::<Vec<_>>();
    for path in generated_files {
        generated.insert(path.clone());
        if let Some(cwd) = &cwd {
            generated.insert(relativize(&path, cwd));
        }
    }
    generated
}

fn source_has_generated_header(file: &str) -> bool {
    if file.ends_with(".css") {
        let Some(text) = std::fs::read_to_string(file).ok() else {
            return false;
        };
        return text.lines().take(8).any(is_generated_header_line)
            || looks_compiled_css(file, &text);
    }
    source_head_has_generated_header(file)
}

const GENERATED_HEADER_READ_BYTES: u64 = 64 * 1024;

fn source_head_has_generated_header(file: &str) -> bool {
    let Ok(mut f) = std::fs::File::open(file) else {
        return false;
    };
    let mut head = String::new();
    let mut limited = std::io::Read::take(&mut f, GENERATED_HEADER_READ_BYTES);
    if std::io::Read::read_to_string(&mut limited, &mut head).is_err() {
        return false;
    }
    head.lines().take(8).any(is_generated_header_line)
}

fn is_generated_header_line(line: &str) -> bool {
    let line = line.trim().to_ascii_lowercase();
    line.contains("@generated")
        || line.contains("generated by")
        || line.contains("code generated")
        || line.contains("automatically generated")
        || line.contains("auto-generated")
        || line.contains("autogenerated")
        || (line.contains("generated") && line.contains("do not edit"))
}

/// A compiled / distributed stylesheet (CSS built from SCSS/Less, or a shipped dist
/// bundle) is a build artifact, not the maintainer's hand-edited source — like other
/// generated code it is not theirs to dedupe, so it is kept off the default surface (its
/// "duplication" is the expansion of preprocessor loops/mixins). Detected by distribution
/// markers a hand-written app stylesheet does not carry: a preserved `/*!` license banner
/// or a versioned header comment, a trailing `sourceMappingURL`, or a sibling `.css.map`.
/// `.min.css` paths are also treated as compiled output here. Measured on the
/// frontend gold set (`bench/labels/frontend_families.v1.json`): drops 147 generated
/// families with 0 worthy — sound.
pub(crate) fn looks_compiled_css(file: &str, text: &str) -> bool {
    if !file.ends_with(".css") {
        return false;
    }
    // A stylesheet under a preprocessor source dir is the INPUT, not compiled output.
    if file
        .split('/')
        .any(|seg| matches!(seg, "scss" | "sass" | "less" | "styl"))
    {
        return false;
    }
    // Minified bundle (also caught path-side by `is_generated_loc`, but its content-index
    // must agree so a family spanning min + non-min variants is uniformly generated).
    if file.ends_with(".min.css") {
        return true;
    }
    if std::path::Path::new(&format!("{file}.map")).exists() {
        return true;
    }
    // A banner in the first few non-blank lines: `/*! … */` (preserved through minifiers)
    // or a versioned header like `/* Sakura.css v1.5.1 */`. A minified file collapses the
    // banner onto the first line behind an optional `@charset "…";`, so accept `/*!` that
    // begins the line OR immediately follows a leading `@charset` declaration.
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(8)
    {
        if line.starts_with("/*!")
            || (line.starts_with("@charset") && line.contains("/*!"))
            || (line.starts_with("/*") && has_version_tag(line))
        {
            return true;
        }
    }
    // A compiled bundle ends with a source-map reference.
    text.lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .any(|l| l.contains("sourceMappingURL"))
}

/// A `vN.N`(.N) version token (e.g. `Sakura.css v1.5.1`) — a release marker of a
/// distributed stylesheet.
pub(crate) fn has_version_tag(s: &str) -> bool {
    let b = s.as_bytes();
    for i in 0..b.len().saturating_sub(2) {
        if (b[i] | 0x20) == b'v' && b[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j + 1 < b.len() && b[j] == b'.' && b[j + 1].is_ascii_digit() {
                return true;
            }
        }
    }
    false
}
