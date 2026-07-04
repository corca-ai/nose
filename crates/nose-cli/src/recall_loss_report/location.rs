use super::model::Location;
use crate::verify_collect::{VerifyExcludedUnit, VerifyRec};
use std::path::Path;

pub(super) fn verify_rec_location(rec: &VerifyRec) -> Location {
    Location {
        file: rec.file.clone(),
        start_line: rec.start,
        end_line: rec.end,
        tokens: rec.tokens,
        language: language_from_path(&rec.file),
    }
}

pub(super) fn excluded_unit_location(unit: &VerifyExcludedUnit) -> Location {
    Location {
        file: unit.file.clone(),
        start_line: unit.start,
        end_line: unit.end,
        tokens: unit.tokens,
        language: language_from_path(&unit.file),
    }
}

fn language_from_path(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    match ext {
        "c" | "h" => "c",
        "css" => "css",
        "go" => "go",
        "html" | "htm" => "html",
        "java" => "java",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "md" | "markdown" => "markdown",
        "py" => "python",
        "rb" => "ruby",
        "rs" => "rust",
        "swift" => "swift",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        _ => "unknown",
    }
    .to_string()
}
