use crate::EquivalenceWitness;
use serde::Serialize;

/// Channel-specific evidence. Required measurements belong to their variant;
/// an exact proof cannot accidentally carry fuzzy similarity measurements.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WitnessEvidence {
    ExactValueGraph {
        value_nodes: usize,
    },
    SharedSubDag,
    CopyPasteRun,
    StructuralSimilarity {
        mean_value_jaccard: f64,
        mean_shape_jaccard: f64,
    },
    ConnectedMappedSubDag {
        value_nodes: usize,
    },
    BoundedSameUnitWindow {
        value_nodes: usize,
    },
}

impl EquivalenceWitness {
    pub fn kind(&self) -> &'static str {
        match self.evidence {
            WitnessEvidence::ExactValueGraph { .. } => "exact-value-graph",
            WitnessEvidence::SharedSubDag => "shared-sub-dag",
            WitnessEvidence::CopyPasteRun => "copy-paste-run",
            WitnessEvidence::StructuralSimilarity { .. } => "structural-similarity",
            WitnessEvidence::ConnectedMappedSubDag { .. } => "connected-mapped-sub-dag",
            WitnessEvidence::BoundedSameUnitWindow { .. } => "bounded-same-unit-window",
        }
    }

    pub fn value_nodes(&self) -> Option<usize> {
        match self.evidence {
            WitnessEvidence::ExactValueGraph { value_nodes }
            | WitnessEvidence::ConnectedMappedSubDag { value_nodes }
            | WitnessEvidence::BoundedSameUnitWindow { value_nodes } => Some(value_nodes),
            _ => None,
        }
    }
}
