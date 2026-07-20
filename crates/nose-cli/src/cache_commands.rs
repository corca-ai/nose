use crate::cli_args::{CacheCmd, StatsFormat};
use anyhow::Result;

pub(crate) fn run(cmd: CacheCmd) -> Result<()> {
    match cmd {
        CacheCmd::Status {
            dir,
            max_bytes,
            format,
        } => {
            let report = crate::cache::store_status(&dir, max_bytes);
            if format == StatsFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("cache: {}", report.root);
                println!("size: {} bytes in {} files", report.bytes, report.files);
                println!("limit: {} bytes", report.max_bytes);
                println!(
                    "generations: {} active / {} total",
                    report.active_generations, report.generations
                );
                println!(
                    "reclaimable: {} bytes in {} files",
                    report.reclaimable_bytes, report.reclaimable_files
                );
            }
            Ok(())
        }
        CacheCmd::Prune {
            dir,
            max_bytes,
            format,
        } => {
            let report = crate::cache::prune_store(&dir, max_bytes)?;
            print_mutation(&report, format)
        }
        CacheCmd::Clear { dir, format } => {
            let report = crate::cache::clear_store(&dir)?;
            print_mutation(&report, format)
        }
    }
}

fn print_mutation(report: &crate::cache::PruneReport, format: StatsFormat) -> Result<()> {
    if format == StatsFormat::Json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "removed {} bytes in {} files; cache is now {} bytes",
            report.removed_bytes, report.removed_files, report.after_bytes
        );
    }
    Ok(())
}
