use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticPackExecutableOracle {
    FixtureExpectations,
}

impl SemanticPackExecutableOracle {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackExecutableOracle::FixtureExpectations => "fixture-expectations",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackFixtureKind {
    Positive,
    HardNegative,
}

impl SemanticPackFixtureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackFixtureKind::Positive => "positive",
            SemanticPackFixtureKind::HardNegative => "hard-negative",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackFixtureIssue {
    MissingPath,
    MissingFile,
    MissingExpectation,
    AbsolutePath,
}

impl SemanticPackFixtureIssue {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackFixtureIssue::MissingPath => "missing-path",
            SemanticPackFixtureIssue::MissingFile => "missing-file",
            SemanticPackFixtureIssue::MissingExpectation => "missing-expectation",
            SemanticPackFixtureIssue::AbsolutePath => "absolute-path",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackFixtureCheck {
    pub kind: SemanticPackFixtureKind,
    pub id: String,
    pub description: String,
    pub declared_path: Option<String>,
    pub resolved_path: Option<PathBuf>,
    pub expectation: Option<String>,
    pub issues: Vec<SemanticPackFixtureIssue>,
}

impl SemanticPackFixtureCheck {
    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticPackExecutableConformanceIssue {
    UnknownFixture,
    WrongFixtureKind,
    MissingExpectation,
    ExpectationMismatch,
    FixtureIssue,
}

impl SemanticPackExecutableConformanceIssue {
    pub const fn as_str(self) -> &'static str {
        match self {
            SemanticPackExecutableConformanceIssue::UnknownFixture => "unknown-fixture",
            SemanticPackExecutableConformanceIssue::WrongFixtureKind => "wrong-fixture-kind",
            SemanticPackExecutableConformanceIssue::MissingExpectation => "missing-expectation",
            SemanticPackExecutableConformanceIssue::ExpectationMismatch => "expectation-mismatch",
            SemanticPackExecutableConformanceIssue::FixtureIssue => "fixture-issue",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackExecutableConformanceCheck {
    pub gate_id: String,
    pub kind: ExternalRowKind,
    pub row_id: String,
    pub row_hash: u64,
    pub pack_id: String,
    pub pack_hash: u64,
    pub manifest_path: PathBuf,
    pub channel: SemanticPackChannel,
    pub oracle: SemanticPackExecutableOracle,
    pub positive_fixtures: Vec<String>,
    pub hard_negatives: Vec<String>,
    pub issues: Vec<SemanticPackExecutableConformanceIssue>,
}

impl SemanticPackExecutableConformanceCheck {
    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackConformanceManifest {
    pub pack: SemanticPackSummary,
    pub manifest_path: PathBuf,
    pub conformance_command: Option<String>,
    pub proof_links: Vec<String>,
    pub fixtures: Vec<SemanticPackFixtureCheck>,
    pub executable: Vec<SemanticPackExecutableConformanceCheck>,
}

impl SemanticPackConformanceManifest {
    pub fn passed(&self) -> bool {
        self.fixture_issue_count() == 0 && self.executable_conformance_issue_count() == 0
    }

    pub fn fixture_issue_count(&self) -> usize {
        self.fixtures
            .iter()
            .map(|fixture| fixture.issues.len())
            .sum()
    }

    pub fn executable_conformance_issue_count(&self) -> usize {
        self.executable.iter().map(|gate| gate.issues.len()).sum()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SemanticPackConformanceReport {
    pub manifests: Vec<SemanticPackConformanceManifest>,
}

impl SemanticPackConformanceReport {
    pub fn passed(&self) -> bool {
        self.manifests
            .iter()
            .all(SemanticPackConformanceManifest::passed)
    }

    pub fn manifest_count(&self) -> usize {
        self.manifests.len()
    }

    pub fn positive_fixture_count(&self) -> usize {
        self.manifests
            .iter()
            .map(|manifest| manifest.pack.counts.positive_fixtures)
            .sum()
    }

    pub fn hard_negative_count(&self) -> usize {
        self.manifests
            .iter()
            .map(|manifest| manifest.pack.counts.hard_negatives)
            .sum()
    }

    pub fn fixture_issue_count(&self) -> usize {
        self.manifests
            .iter()
            .map(SemanticPackConformanceManifest::fixture_issue_count)
            .sum()
    }

    pub fn executable_conformance_count(&self) -> usize {
        self.manifests
            .iter()
            .map(|manifest| manifest.executable.len())
            .sum()
    }

    pub fn passed_executable_conformance_count(&self) -> usize {
        self.manifests
            .iter()
            .flat_map(|manifest| &manifest.executable)
            .filter(|gate| gate.passed())
            .count()
    }

    pub fn executable_conformance_issue_count(&self) -> usize {
        self.manifests
            .iter()
            .map(SemanticPackConformanceManifest::executable_conformance_issue_count)
            .sum()
    }

    pub fn executable_conformance_passed_for(
        &self,
        kind: ExternalRowKind,
        row_hash: u64,
        pack_hash: u64,
        manifest_path: &std::path::Path,
    ) -> bool {
        let mut has_gate = false;
        for gate in self
            .manifests
            .iter()
            .flat_map(|manifest| &manifest.executable)
        {
            if gate.kind == kind
                && gate.row_hash == row_hash
                && gate.pack_hash == pack_hash
                && gate.manifest_path == manifest_path
            {
                has_gate = true;
                if !gate.passed() {
                    return false;
                }
            }
        }
        has_gate
    }
}
