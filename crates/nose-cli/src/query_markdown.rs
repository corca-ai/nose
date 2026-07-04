use crate::baseline;
use crate::baseline_comparison::BaselineComparison;
use crate::family_display::{abstraction_witness_summary, similarity_cell};
use crate::ignores;
use crate::query_opportunities::{family_hint, family_langs, total_dup_lines_refs};
use crate::query_options::DetectionChannels;
use crate::report_text::plural;
use crate::source_lines::{anti_unify_all, line_diff, read_lines};

/// Markdown form of the all-copies extraction skeleton (#360), rendered on an `id=<fam>`
/// drilldown so `--format markdown` honors the help's "every copy + extraction skeleton"
/// promise the same way the human/JSON views do (#422). The bulk report stays a compact
/// location list; the skeleton is paid only when the consumer drills into one family.
pub(crate) fn markdown_member_proposal(locations: &[nose_detect::Loc]) {
    let members: Vec<Vec<String>> = locations
        .iter()
        .filter_map(|l| read_lines(&l.file, l.start_line, l.end_line))
        .collect();
    if members.len() < 2 {
        return;
    }
    let (skeleton, shared, params) = anti_unify_all(&members);
    let copies = members.len();
    println!(
        "**proposal** — extract a shared helper · {shared} shared lines · {params} parameter(s) vary (across all {copies} copies)\n"
    );
    println!("```text");
    for line in skeleton.iter().take(40) {
        println!("{line}");
    }
    println!("```\n");
}

/// Markdown form of the representative two-copy diff, added on `id=<fam> full` (the `full`
/// view, mirroring the human renderer's extra diff line).
pub(crate) fn markdown_member_diff(a: &nose_detect::Loc, b: &nose_detect::Loc) {
    let (Some(la), Some(lb)) = (
        read_lines(&a.file, a.start_line, a.end_line),
        read_lines(&b.file, b.start_line, b.end_line),
    ) else {
        return;
    };
    println!(
        "**diff** — `{}:{}-{}` vs `{}:{}-{}`\n",
        a.file, a.start_line, a.end_line, b.file, b.start_line, b.end_line
    );
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    println!("```diff");
    for (tag, line) in line_diff(&ar, &br) {
        println!("{tag} {line}");
    }
    println!("```\n");
}

pub(crate) fn print_refactor_markdown(
    all: &[&nose_detect::RefactorFamily],
    shown: &[&nose_detect::RefactorFamily],
    mode: DetectionChannels,
    baseline: Option<&BaselineComparison>,
    ignore_set: Option<&ignores::IgnoreSet>,
    ignored_families: usize,
    omitted_note: Option<&str>,
) {
    println!("# {}\n", mode.markdown_title());
    println!(
        "{} {} · ~{} duplicated lines · showing top {}\n",
        all.len(),
        plural(all.len(), "family", "families"),
        total_dup_lines_refs(all),
        shown.len()
    );
    if let Some(note) = omitted_note {
        println!("{note}\n");
    }
    if let Some(comparison) = baseline {
        println!("{}\n", comparison.summary.line());
    }
    if let Some(ignore_set) = ignore_set {
        println!("{}\n", ignore_set.summary(ignored_families).line());
    }
    for (i, f) in shown.iter().enumerate() {
        let xlang = match family_langs(f) {
            s if s.is_empty() => String::new(),
            s => format!(" · cross-language: {s}"),
        };
        println!(
            "## {}. `{}` — {} sites, {} {}, {} {} — ~{} dup lines ({}){}",
            i + 1,
            baseline::family_id(f),
            f.members,
            f.files,
            plural(f.files, "file", "files"),
            f.modules,
            plural(f.modules, "directory", "directories"),
            f.dup_lines,
            similarity_cell(f),
            xlang
        );
        println!("\n*{}*\n", family_hint(f));
        if let Some(witness) = &f.abstraction_witness {
            println!("_witness: {}_\n", abstraction_witness_summary(witness));
        }
        for l in &f.locations {
            let name = l
                .name
                .as_deref()
                .map(|n| format!(" `{n}`"))
                .unwrap_or_default();
            println!("- `{}:{}-{}`{}", l.file, l.start_line, l.end_line, name);
        }
        println!();
    }
}
