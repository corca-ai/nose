use crate::baseline;
use crate::baseline_comparison::BaselineComparison;
use crate::family_display::{abstraction_witness_summary, similarity_cell};
use crate::ignores;
use crate::query_opportunities::{family_hint, family_langs, total_dup_lines_refs};
use crate::query_options::DetectionChannels;
use crate::report_text::plural;
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
