//! JSON schemas for gold sets, predictions, hard negatives, and the corpus
//! split used by the benchmark tooling.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Region {
    #[serde(default)]
    pub repo: Option<String>,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(default)]
    #[allow(dead_code)] // part of the schema; not used in scoring
    pub symbol: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GoldPair {
    pub repo: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub clone_type: Option<String>,
    pub left: Region,
    pub right: Region,
}

/// A judged prediction in the precision pool: `(repo, file, start, end)` tuples
/// plus the human/oracle `clone` label. Defines a real precision denominator on a
/// non-exhaustive gold — a prediction overlapping a pooled pair is "judged", and
/// counts toward precision iff that pair was labeled a true clone.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PoolPair {
    pub clone: bool,
    pub left: (String, String, u32, u32),
    pub right: (String, String, u32, u32),
}

#[derive(Debug, Deserialize)]
pub(crate) struct Gold {
    pub duplicates: Vec<GoldPair>,
    #[serde(default)]
    pub pool: Vec<PoolPair>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredRegion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredPair {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    pub left: PredRegion,
    pub right: PredRegion,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Predictions {
    pub schema_version: String,
    pub tool: String,
    pub duplicates: Vec<PredPair>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HardNeg {
    pub repo: String,
    pub left: Region,
    pub right: Region,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HardNegatives {
    pub hard_negatives: Vec<HardNeg>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CorpusRepo {
    pub id: String,
    #[serde(default)]
    pub split: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Corpus {
    pub repositories: Vec<CorpusRepo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitRegion {
    pub repo: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnitsDump {
    pub units: Vec<UnitRegion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CandidatesDump {
    pub candidates: Vec<(u32, u32)>,
}
