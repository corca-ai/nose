use crate::il::Il;
use crate::intern::Interner;

/// A whole codebase: many lowered files sharing one interner. `files[i].file ==
/// FileId(i)`.
#[derive(Clone)]
pub struct Corpus {
    pub interner: Interner,
    pub files: Vec<Il>,
    /// Discovery/read/lowering failures; intentionally excluded artifacts are not failures.
    pub source_errors: Vec<String>,
    pub skipped_sources: Vec<SourceDiagnostic>,
}

impl Corpus {
    pub fn new(interner: Interner, files: Vec<Il>) -> Self {
        Corpus {
            interner,
            files,
            source_errors: Vec::new(),
            skipped_sources: Vec::new(),
        }
    }

    /// Refuse to use partial analysis as evidence that a corpus is clean.
    pub fn ensure_complete(&self) -> Result<(), IncompleteCorpus> {
        if self.source_errors.is_empty() {
            Ok(())
        } else {
            Err(IncompleteCorpus(self.source_errors.clone()))
        }
    }

    /// Total node count across all files (handy for diagnostics).
    pub fn node_count(&self) -> usize {
        self.files.iter().map(|f| f.nodes.len()).sum()
    }
}

#[derive(Debug)]
pub struct IncompleteCorpus(Vec<String>);

impl std::fmt::Display for IncompleteCorpus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "incomplete source analysis:\n{}", self.0.join("\n"))
    }
}

impl std::error::Error for IncompleteCorpus {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SourceDiagnostic {
    pub path: String,
    pub reason: String,
}
