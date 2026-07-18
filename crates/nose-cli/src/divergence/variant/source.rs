use super::*;
use crate::divergence::Site;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn source_signals(
    evidence: &mut VariantEvidence,
    changed: &Site,
    current_path: &str,
    current_span: Option<(u32, u32)>,
    skipped: &Site,
    current_root: &Path,
    base_root: &Path,
    lines: &mut FileLineCache,
) {
    let (current_start, current_end) = current_span.unwrap_or_else(|| {
        let (start, end) = effective_span(changed);
        (start, end.saturating_add(32))
    });
    let changed_lines = site_lines(
        current_path,
        current_start,
        current_end,
        current_root,
        lines,
    );
    let (skipped_start, skipped_end) = effective_span(skipped);
    let skipped_lines = site_lines(&skipped.file, skipped_start, skipped_end, base_root, lines);
    let (Some(changed_lines), Some(skipped_lines)) = (changed_lines, skipped_lines) else {
        evidence.caveat(VariantCaveatCode::SourceUnavailable, std::iter::empty());
        return;
    };
    let changed_decorators = decorator_lines(
        &changed.lang,
        &changed_lines,
        changed.enclosing_unit.is_some() || changed.kind != nose_il::UnitKind::Block,
    );
    let skipped_decorators = decorator_lines(
        &skipped.lang,
        &skipped_lines,
        skipped.enclosing_unit.is_some() || skipped.kind != nose_il::UnitKind::Block,
    );
    if changed_decorators != skipped_decorators
        && (!changed_decorators.is_empty() || !skipped_decorators.is_empty())
    {
        evidence.signal(
            VariantSignalCode::DecoratorMismatch,
            VariantEvidenceStrength::Strong,
            changed_decorators,
            skipped_decorators,
        );
    }
    compare_platform_guards(evidence, &changed_lines, &skipped_lines);
}

fn effective_span(site: &Site) -> (u32, u32) {
    site.enclosing_unit
        .as_ref()
        .map(|unit| (unit.start_line, unit.end_line))
        .unwrap_or((site.start_line, site.end_line))
}

fn site_lines(
    file: &str,
    start: u32,
    end: u32,
    root: &Path,
    lines: &mut FileLineCache,
) -> Option<Vec<String>> {
    let path = root.join(file).to_string_lossy().into_owned();
    lines.slice(&path, start, end)
}

fn decorator_lines(lang: &str, lines: &[String], prefix_only: bool) -> Vec<String> {
    let prefix = match lang {
        "python" | "java" | "javascript" | "typescript" => "@",
        "rust" => "#[",
        _ => return Vec::new(),
    };
    let mut decorators = Vec::new();
    for line in lines
        .iter()
        .take(if prefix_only { 32 } else { lines.len() })
    {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with("/*")
            || line.starts_with('*')
        {
            continue;
        }
        if line.starts_with(prefix) {
            if !line.starts_with("#[cfg") {
                decorators.push(cap_detail(line));
            }
            continue;
        }
        if prefix_only {
            break;
        }
    }
    decorators.sort();
    decorators.dedup();
    decorators.truncate(DETAIL_CAP);
    decorators
}

#[derive(Default)]
struct PlatformGuards {
    by_dimension: BTreeMap<&'static str, BTreeSet<String>>,
    source: Vec<String>,
}

fn compare_platform_guards(
    evidence: &mut VariantEvidence,
    changed_lines: &[String],
    skipped_lines: &[String],
) {
    let changed = platform_guards(changed_lines);
    let skipped = platform_guards(skipped_lines);
    let mut comparable = false;
    let mut disjoint = false;
    let mut conflict = false;
    for (dimension, changed_values) in &changed.by_dimension {
        let Some(skipped_values) = skipped.by_dimension.get(dimension) else {
            continue;
        };
        comparable = true;
        let overlap = changed_values.intersection(skipped_values).next().is_some();
        disjoint |= !overlap;
        conflict |= overlap && changed_values != skipped_values;
    }
    if comparable && disjoint {
        evidence.signal(
            VariantSignalCode::DisjointPlatformGuard,
            VariantEvidenceStrength::Strong,
            changed.source,
            skipped.source,
        );
    } else if conflict || (!comparable && !changed.source.is_empty() && !skipped.source.is_empty())
    {
        evidence.caveat(
            VariantCaveatCode::ConflictingPlatformGuard,
            changed.source.into_iter().chain(skipped.source),
        );
    }
}

fn platform_guards(lines: &[String]) -> PlatformGuards {
    let mut guards = PlatformGuards::default();
    for line in lines {
        let trimmed = line.trim();
        if !(trimmed.starts_with("#[cfg")
            || trimmed.starts_with("#if")
            || trimmed.starts_with("#ifdef"))
        {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        let before = guards
            .by_dimension
            .values()
            .map(BTreeSet::len)
            .sum::<usize>();
        for (dimension, key) in [
            ("target-os", "target_os"),
            ("target-arch", "target_arch"),
            ("target-env", "target_env"),
            ("pointer-width", "target_pointer_width"),
        ] {
            for value in quoted_values_after(&lower, key) {
                guards
                    .by_dimension
                    .entry(dimension)
                    .or_default()
                    .insert(value);
            }
        }
        for (dimension, function) in [("target-os", "os("), ("target-arch", "arch(")] {
            for value in call_values(&lower, function) {
                guards
                    .by_dimension
                    .entry(dimension)
                    .or_default()
                    .insert(value);
            }
        }
        for (needle, value) in [
            ("_win32", "windows"),
            ("__linux__", "linux"),
            ("__apple__", "apple"),
            ("__android__", "android"),
            ("__freebsd__", "freebsd"),
        ] {
            if lower.contains(needle) && !lower.starts_with("#ifndef") {
                guards
                    .by_dimension
                    .entry("target-os")
                    .or_default()
                    .insert(value.to_string());
            }
        }
        let after = guards
            .by_dimension
            .values()
            .map(BTreeSet::len)
            .sum::<usize>();
        if after > before {
            guards.source.push(cap_detail(trimmed));
        }
    }
    guards.source.sort();
    guards.source.dedup();
    guards.source.truncate(DETAIL_CAP);
    guards
}

fn quoted_values_after(text: &str, key: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find(key) {
        rest = &rest[index + key.len()..];
        let Some(open) = rest.find('"') else { break };
        let tail = &rest[open + 1..];
        let Some(close) = tail.find('"') else { break };
        values.push(tail[..close].to_string());
        rest = &tail[close + 1..];
    }
    values
}

fn call_values(text: &str, function: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find(function) {
        let tail = &rest[index + function.len()..];
        let Some(close) = tail.find(')') else { break };
        let value = tail[..close].trim();
        if !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            values.push(value.to_string());
        }
        rest = &tail[close + 1..];
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutually_exclusive_platform_values_are_strong_but_overlap_is_advisory() {
        let linux = vec!["#[cfg(target_os = \"linux\")]".to_string()];
        let windows = vec!["#[cfg(target_os = \"windows\")]".to_string()];
        let mut evidence = VariantEvidence::empty();
        compare_platform_guards(&mut evidence, &linux, &windows);
        assert_eq!(evidence.status, VariantEvidenceStatus::Disqualifying);
        assert_eq!(
            evidence.signals[0].code,
            VariantSignalCode::DisjointPlatformGuard
        );

        let linux_or_macos =
            vec!["#[cfg(any(target_os = \"linux\", target_os = \"macos\"))]".to_string()];
        let mut contradictory = VariantEvidence::empty();
        compare_platform_guards(&mut contradictory, &linux_or_macos, &linux);
        assert_eq!(contradictory.status, VariantEvidenceStatus::Advisory);
        assert!(contradictory.signals.is_empty());
        assert_eq!(
            contradictory.caveats[0].code,
            VariantCaveatCode::ConflictingPlatformGuard
        );
    }

    #[test]
    fn decorators_are_definition_prefix_only_and_cfg_is_priced_as_a_guard() {
        let lines = vec![
            "@cache".to_string(),
            "def work():".to_string(),
            "    @nested".to_string(),
        ];
        assert_eq!(decorator_lines("python", &lines, true), vec!["@cache"]);
        assert_eq!(
            decorator_lines("python", &lines, false),
            vec!["@cache", "@nested"]
        );
        assert!(decorator_lines("rust", &["#[cfg(unix)]".to_string()], true).is_empty());
    }
}
