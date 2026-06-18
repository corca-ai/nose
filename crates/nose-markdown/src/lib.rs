//! Same-language Markdown near-duplicate detection.
//!
//! A self-contained, deterministic, no-LLM pipeline over a character-n-gram substrate of
//! normalized prose. Built and validated by the algorithm survey
//! (`docs/markdown-dup-detection-algorithm-survey-2026-06-18.md`). Deliberately separate from the
//! value-graph code-clone engine: prose is not code, so it uses MinHash-LSH + winnowing +
//! containment (candidate gen) → TF-IDF + commonness (verify/rank) → local alignment / diff
//! (span witness), not value-graph/shape fingerprints.
//!
//! Design-principle note (epic #435): nose **detects, witnesses, and surfaces orthogonal
//! evidence** (similarity, span, commonness, removable, spread); it does **not** judge whether a
//! repetition is intentional or worth removing. Boilerplate copies are true duplicates, surfaced
//! with high commonness — never silently suppressed.

pub mod detect;
pub mod eval;
pub mod fingerprint;
pub mod norm;
pub mod synth;
pub mod unit;
pub mod verify;
pub mod witness;

pub use detect::{detect, Family, Member, Options, WitnessRef};
pub use eval::{dump_pairs, evaluate, score_pairs, Golden, Metrics, Ref, ScoredPair};
pub use fingerprint::{candidate_pairs, containment, jaccard, minhash_est, Fingerprint};
pub use synth::recall_curve;
pub use unit::{split_units, Unit, UnitKind};
pub use verify::CorpusModel;
pub use witness::{witness, Span};
