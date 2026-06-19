//! Turn raw clone groups into ranked **refactoring opportunities**.
//!
//! For architecture/design-level refactoring, what matters is not "these two
//! functions are similar" but "this structure repeats across the codebase — extract
//! an abstraction." So we rank *families* (clone groups) by a refactoring-value
//! score that rewards:
//!   - **how much code** could be removed (`dup_lines` ≈ (members−1) × mean span),
//!   - **how clean** the extraction is (mean similarity),
//!   - **design-level spread** — a family spanning many files / modules signals a
//!     missing abstraction, weighted above a local copy-paste.

mod model;
mod paths;
mod ranking;
mod score;

#[cfg(test)]
mod tests;

pub use model::{RefactorFamily, VaryingSpot};
pub use paths::{is_test_loc, is_test_path};
pub use ranking::rank_families;
