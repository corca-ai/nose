use crate::baseline;
use crate::baseline_comparison::{BaselineComparison, BaselineStatus};
use crate::cli_args::QueryArgs;
use crate::ignores;
use crate::query_opportunities::family_hint;
use crate::query_options::{DetectionChannels, FailOn};
use anyhow::{Context, Result};

pub(crate) fn write_query_baseline(
    args: &QueryArgs,
    families: &[nose_detect::RefactorFamily],
) -> Result<()> {
    let path = args
        .baseline
        .as_ref()
        .expect("--write-baseline requires --baseline");
    baseline::write(path, families, family_hint)
        .with_context(|| format!("writing baseline {}", path.display()))?;
    eprintln!(
        "nose: wrote baseline of {} families to {}",
        families.len(),
        path.display()
    );
    Ok(())
}

/// Compare against an accepted baseline: build the comparison, then drop already-accepted
/// families in place so only new/changed duplication is reported and gated. `None` when no
/// `--baseline` is set. (`--write-baseline` is handled earlier, before this runs.)
pub(crate) fn apply_query_baseline(
    args: &QueryArgs,
    families: &mut Vec<nose_detect::RefactorFamily>,
) -> Result<Option<BaselineComparison>> {
    let Some(path) = args.baseline.as_ref() else {
        return Ok(None);
    };
    let accepted = baseline::load(path)?;
    let comparison = BaselineComparison::new(path, &accepted, families)?;
    families.retain(|f| {
        !comparison
            .suppressed_keys
            .contains(&baseline::family_key(f))
    });
    Ok(Some(comparison))
}

/// `since=<baseline>`: compare to a saved snapshot WITHOUT hiding anything — the temporal
/// exploration lens. Unlike `--baseline` (which drops accepted families for the gate), this
/// keeps every family and lets the caller slice by the `status` field. Use `status!=unchanged`
/// for the reportable new-or-changed set.
pub(crate) fn compare_since(
    path: &str,
    families: &[nose_detect::RefactorFamily],
) -> Result<BaselineComparison> {
    let snapshot = baseline::load(std::path::Path::new(path))?;
    BaselineComparison::new(std::path::Path::new(path), &snapshot, families)
}

/// A family's status against a `since=` snapshot: `new`/`changed` (in the comparison) or
/// `unchanged` (present in the snapshot, so absent from the changed/new map).
pub(crate) fn family_status(
    f: &nose_detect::RefactorFamily,
    cmp: &BaselineComparison,
) -> &'static str {
    cmp.statuses
        .get(&baseline::family_key(f))
        .map_or("unchanged", |status| status.status.as_str())
}

pub(crate) fn partition_ignored(
    families: Vec<nose_detect::RefactorFamily>,
    ignore_set: Option<&ignores::IgnoreSet>,
) -> Vec<nose_detect::RefactorFamily> {
    let Some(ignore_set) = ignore_set else {
        return families;
    };
    let mut active = Vec::with_capacity(families.len());
    for family in families {
        if ignore_set.match_family(&family).is_none() {
            active.push(family);
        }
    }
    active
}

pub(crate) fn enforce_query_fail_on_selection(
    args: &QueryArgs,
    channels: DetectionChannels,
    reportable: &[&nose_detect::RefactorFamily],
    baseline_comparison: Option<&BaselineComparison>,
) {
    if let (true, Some(comparison)) = (
        matches!(args.fail_on, Some(FailOn::New)) && !reportable.is_empty(),
        baseline_comparison,
    ) {
        let mut new_families = 0usize;
        let mut changed_families = 0usize;
        for family in reportable {
            if let Some(status) = comparison.statuses.get(&baseline::family_key(family)) {
                match status.status {
                    BaselineStatus::Changed => changed_families += 1,
                    BaselineStatus::New => new_families += 1,
                }
            }
        }
        let reportable_families = new_families + changed_families;
        eprintln!(
            "\nnose: {} new and {} changed {} found (--fail-on new)",
            new_families,
            changed_families,
            channels.report_label(reportable_families)
        );
        std::process::exit(1);
    }
    if matches!(args.fail_on, Some(FailOn::Any)) && !reportable.is_empty() {
        eprintln!(
            "\nnose: {} {} found (--fail-on any)",
            reportable.len(),
            channels.report_label(reportable.len())
        );
        std::process::exit(1);
    }
}
