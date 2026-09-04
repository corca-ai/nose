use anyhow::Result;
use nose_il::Corpus;

#[derive(Clone, Copy, PartialEq, clap::ValueEnum)]
pub(crate) enum ReportFormat {
    /// Ranked, human-readable terminal report.
    Human,
    /// Machine-readable JSON report with a versioned top-level schema.
    Json,
    /// Revisioned newline-delimited JSON for `query --watch` sessions.
    Jsonl,
    /// Markdown report (for PRs / issues / docs).
    Markdown,
    /// SARIF 2.1.0 (GitHub annotations).
    Sarif,
}

/// One `--mode` channel. Fuzzy modes carry their acceptance threshold inline
/// (`near:0.8` / `abstraction:0.5`), so there is no separate `--threshold` flag
/// to mis-combine.
#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(try_from = "String")]
pub(crate) enum DetectionMode {
    /// CPD-style syntax copy-paste runs (the Type-1/2 floor).
    Syntax,
    /// Exact value-fingerprint Type-4 semantic clones.
    Semantic,
    /// Fuzzy Type-3 near-duplicate candidates, with an optional acceptance threshold.
    Near(Option<f64>),
    /// Experimental weak refactoring-template witnesses over near candidates.
    Abstraction(Option<f64>),
}

impl std::str::FromStr for DetectionMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let s = s.trim();
        match s {
            "syntax" => Ok(DetectionMode::Syntax),
            "semantic" => Ok(DetectionMode::Semantic),
            "near" => Ok(DetectionMode::Near(None)),
            "abstraction" => Ok(DetectionMode::Abstraction(None)),
            _ => {
                if let Some(t) = s.strip_prefix("near:") {
                    let v: f64 = t
                        .parse()
                        .map_err(|_| format!("invalid near threshold in `{s}`"))?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err(format!("near threshold must be in [0,1], got {v}"));
                    }
                    Ok(DetectionMode::Near(Some(v)))
                } else if let Some(t) = s.strip_prefix("abstraction:") {
                    let v: f64 = t
                        .parse()
                        .map_err(|_| format!("invalid abstraction threshold in `{s}`"))?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err(format!("abstraction threshold must be in [0,1], got {v}"));
                    }
                    Ok(DetectionMode::Abstraction(Some(v)))
                } else {
                    Err(format!(
                        "unknown mode `{s}` (expected syntax, semantic, near, near:T, abstraction, or abstraction:T)"
                    ))
                }
            }
        }
    }
}

impl TryFrom<String> for DetectionMode {
    type Error = String;

    fn try_from(s: String) -> std::result::Result<Self, String> {
        s.parse()
    }
}

fn parse_positive_usize(s: &str, label: &str) -> std::result::Result<usize, String> {
    let value = s
        .parse::<usize>()
        .map_err(|_| format!("{label} must be positive"))?;
    if value == 0 {
        Err(format!("{label} must be positive"))
    } else {
        Ok(value)
    }
}

pub(crate) fn parse_minhash_k(s: &str) -> std::result::Result<usize, String> {
    parse_positive_usize(s, "minhash-k")
}

pub(crate) fn parse_bands(s: &str) -> std::result::Result<usize, String> {
    parse_positive_usize(s, "bands")
}

const THRESHOLD_ERROR: &str = "threshold must be a finite number in [0,1]";

fn valid_threshold(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub(crate) fn parse_threshold(s: &str) -> std::result::Result<f64, String> {
    let value = s.parse::<f64>().map_err(|_| THRESHOLD_ERROR.to_string())?;
    if valid_threshold(value) {
        Ok(value)
    } else {
        Err(THRESHOLD_ERROR.to_string())
    }
}

const MIN_VALUE_ERROR: &str = "min-value must be a finite non-negative number";

fn valid_min_value(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

pub(crate) fn parse_min_value(s: &str) -> std::result::Result<f64, String> {
    let value = s.parse::<f64>().map_err(|_| MIN_VALUE_ERROR.to_string())?;
    if valid_min_value(value) {
        Ok(value)
    } else {
        Err(MIN_VALUE_ERROR.to_string())
    }
}

pub(crate) fn validate_min_value(value: f64) -> Result<f64> {
    if valid_min_value(value) {
        Ok(value)
    } else {
        anyhow::bail!("{MIN_VALUE_ERROR}")
    }
}

/// The `analysis` default surface: include unthresholded `near` (experiments §BM:
/// +8.2pp held-out worthy-recall at no held-out P@10 price — consumer 1 filters).
pub(crate) const QUERY_DEFAULT_MODES: &[DetectionMode] = &[
    DetectionMode::Syntax,
    DetectionMode::Semantic,
    DetectionMode::Near(None),
];

/// The `divergence` default stays the conservative mix: divergence feeds a gate
/// (consumer 2 — false fires are the failure mode), and §BM priced the analysis
/// surface only. Revisit with the fire-precision benchmark (#243/#245).
pub(crate) const DIVERGENCE_DEFAULT_MODES: &[DetectionMode] =
    &[DetectionMode::Syntax, DetectionMode::Semantic];

#[derive(Clone, Copy)]
pub(crate) struct DetectionChannels {
    pub(crate) syntax: bool,
    pub(crate) semantic: bool,
    pub(crate) near: bool,
    pub(crate) abstraction: bool,
    /// The shared fuzzy acceptance threshold, if one was given in the mode spec.
    threshold: Option<f64>,
}

impl DetectionChannels {
    pub(crate) fn resolve(
        cli: Vec<DetectionMode>,
        cfg: Vec<DetectionMode>,
        default: &[DetectionMode],
    ) -> Result<Self> {
        let selected = if !cli.is_empty() {
            cli
        } else if !cfg.is_empty() {
            cfg
        } else {
            default.to_vec()
        };
        let mut channels = DetectionChannels {
            syntax: false,
            semantic: false,
            near: false,
            abstraction: false,
            threshold: None,
        };
        for mode in selected {
            match mode {
                DetectionMode::Syntax => channels.syntax = true,
                DetectionMode::Semantic => channels.semantic = true,
                DetectionMode::Near(t) => {
                    channels.near = true;
                    channels.set_threshold("near", t)?;
                }
                DetectionMode::Abstraction(t) => {
                    channels.abstraction = true;
                    channels.set_threshold("abstraction", t)?;
                }
            }
        }
        if !channels.syntax && !channels.semantic && !channels.near && !channels.abstraction {
            anyhow::bail!(
                "--mode must include at least one of syntax, semantic, near, or abstraction"
            );
        }
        Ok(channels)
    }

    fn set_threshold(&mut self, mode: &'static str, threshold: Option<f64>) -> Result<()> {
        let Some(next) = threshold else {
            return Ok(());
        };
        if let Some(prev) = self.threshold {
            if (prev - next).abs() > f64::EPSILON {
                anyhow::bail!(
                    "conflicting --mode thresholds: near and abstraction share one acceptance threshold; got {prev} and {mode}:{next}"
                );
            }
        }
        self.threshold = Some(next);
        Ok(())
    }

    pub(crate) fn structural(self) -> bool {
        self.semantic || self.near || self.abstraction
    }

    pub(crate) fn report_label(self, count: usize) -> &'static str {
        let singular = count == 1;
        match (
            self.syntax,
            self.semantic,
            self.near,
            self.abstraction,
            singular,
        ) {
            (true, false, false, false, true) => "syntax clone family",
            (true, false, false, false, false) => "syntax clone families",
            (false, true, false, false, true) => "semantic clone family",
            (false, true, false, false, false) => "semantic clone families",
            (false, false, true, false, true) => "near-duplicate family",
            (false, false, true, false, false) => "near-duplicate families",
            (false, false, false, true, true) => "abstraction candidate family",
            (false, false, false, true, false) => "abstraction candidate families",
            (_, _, _, _, true) => "clone family",
            (_, _, _, _, false) => "clone families",
        }
    }

    pub(crate) fn markdown_title(self) -> &'static str {
        match (self.syntax, self.semantic, self.near, self.abstraction) {
            (true, false, false, false) => "Syntax Clone Families",
            (false, true, false, false) => "Semantic Clone Families",
            (false, false, true, false) => "Near-Duplicate Families",
            (false, false, false, true) => "Abstraction Candidate Families",
            _ => "Clone Families",
        }
    }

    pub(crate) fn threshold(self) -> f64 {
        if self.near || self.abstraction {
            self.threshold.unwrap_or(if self.abstraction && !self.near {
                0.50
            } else {
                0.70
            })
        } else {
            1.0
        }
    }

    pub(crate) fn abstraction_only(self) -> bool {
        self.abstraction && !self.syntax && !self.semantic && !self.near
    }
}

/// How to rank families — what "most worth your attention first" means.
#[derive(Clone, Copy, PartialEq, clap::ValueEnum, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SortKey {
    /// How cleanly it extracts: invariant (shared) lines × copies × spread, penalized
    /// by the number of parameters the helper would need. Surfaces the duplication you
    /// can actually fold into one helper, not the biggest block that merely *looks*
    /// similar (a *fixability* axis). The default.
    Extractability,
    /// Raw duplicated volume: duplicated lines (mean span × copies) × similarity ×
    /// spread. Ranks by how much *code* repeats, NOT by the `removable` field — a
    /// structural (Type-4) family can have high volume yet `removable=0` when no
    /// literal lines survive across all copies (nothing cleanly extractable).
    Value,
    /// Most copies first — the most-repeated patterns.
    Sites,
    /// Divergent-edit *hazard*: how likely a family is to be edited inconsistently
    /// (one copy fixed, the siblings missed) and cause a bug. A severity axis, not a
    /// fixability one — surfaces copies that share little text yet are behaviorally the
    /// same (the invisible siblings a developer won't update). Calibrated against mined
    /// history as a *divergence-propensity* signal — it is NOT yet a validated *harm*
    /// ranker (an LLM-gold audit found ~chance harm discrimination); see
    /// `docs/hazard-ranking.md`. Opt-in via `sort=hazard`.
    Hazard,
}

impl SortKey {
    /// The ranking score for `f` under this key (higher = ranked first).
    pub(crate) fn score(self, f: &nose_detect::RefactorFamily) -> f64 {
        match self {
            SortKey::Extractability => f.extractability(),
            SortKey::Value => f.value,
            SortKey::Sites => f.members as f64,
            SortKey::Hazard => f.hazard(),
        }
    }
}

/// CI fail-gate policy, selected with `--fail-on`.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FailOn {
    /// Any reported family (after filters) fails the run.
    Any,
    /// Only families new or changed vs `--baseline` fail. Requires `--baseline`.
    New,
}

/// What query actually looked at: the file count and per-language breakdown, shown as
/// a one-line header. A repo where `.gitignore`/`--exclude` pruned vendored deps analyzes
/// far fewer files than sit on disk; surfacing the count (and which languages) makes
/// that scope visible instead of a silent gap the reader has to guess at. (The *ignored*
/// count is deliberately not shown — computing it means descending into the very trees
/// `.gitignore` exists to skip, slow on exactly the big repos where it matters.)
pub(crate) struct QueryScope {
    pub(crate) files: usize,
    pub(crate) skipped_sources: Vec<nose_il::SourceDiagnostic>,
    /// `(language name, file count)`, largest first.
    pub(crate) langs: Vec<(&'static str, usize)>,
}

impl QueryScope {
    pub(crate) fn from_corpus(corpus: &Corpus) -> Self {
        let mut scope = Self::from_langs(corpus.files.iter().map(|file| file.meta.lang));
        scope.skipped_sources = corpus.skipped_sources.clone();
        scope
    }

    pub(crate) fn from_langs(langs: impl IntoIterator<Item = nose_il::Lang>) -> Self {
        let mut counts: std::collections::HashMap<&'static str, usize> =
            std::collections::HashMap::new();
        let mut files = 0;
        for lang in langs {
            files += 1;
            *counts.entry(lang.name()).or_insert(0) += 1;
        }
        let mut langs: Vec<(&'static str, usize)> = counts.into_iter().collect();
        // Largest language first; name as a stable tie-break for deterministic output.
        langs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        QueryScope {
            files,
            langs,
            skipped_sources: Vec::new(),
        }
    }

    pub(crate) fn with_sources(mut self, sources: &[crate::cache::CachedSourceFile]) -> Self {
        self.skipped_sources = sources
            .iter()
            .filter_map(|source| {
                source
                    .skip_reason
                    .as_ref()
                    .map(|reason| nose_il::SourceDiagnostic {
                        path: source.path.clone(),
                        reason: reason.clone(),
                    })
            })
            .collect();
        self
    }

    /// `analyzed 1113 files · typescript 900 · tsx 213` (languages omitted when unknown).
    pub(crate) fn summary(&self) -> String {
        let unit = if self.files == 1 { "file" } else { "files" };
        if self.langs.is_empty() {
            return format!("analyzed {} {unit}", self.files);
        }
        let langs = self
            .langs
            .iter()
            .map(|(l, n)| format!("{l} {n}"))
            .collect::<Vec<_>>()
            .join(" · ");
        format!("analyzed {} {unit} · {langs}", self.files)
    }
}
