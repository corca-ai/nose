use crate::{reinvented::ReinventedHelper, FragmentKind, GradedWitness};
use nose_il::{UnitKind, UnitOrigin};
use nose_semantics::ValueLaw;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct EnclosingUnit {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub kind: UnitKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub unit_key: String,
}

impl EnclosingUnit {
    pub fn refresh_unit_key(&mut self) {
        self.unit_key = unit_key(
            &self.file,
            self.kind,
            self.name.as_deref(),
            self.start_line,
            self.end_line,
        );
    }
}

#[derive(Serialize, Clone)]
pub struct Loc {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lang: String,
    /// What kind of syntactic unit this site is (function/method/class/block) —
    /// lets the report suggest the right refactor (helper vs base class).
    pub kind: UnitKind,
    #[serde(default, skip_serializing_if = "UnitOrigin::is_unknown")]
    pub origin: UnitOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Size of this unit's value graph (number of distinct computed values). A
    /// unit that computes things has a rich value graph; a pure type definition
    /// or data/match table has a near-empty one and can only match on *shape* —
    /// the signal the refactor ranking uses to discount structural-only families.
    pub sem: usize,
    /// Explicit source-line span so consumers do not have to recalculate inclusive
    /// ranges. Kept for every location, not only fragments.
    pub span_lines: u32,
    /// Stable normalized-token span used by the detector's size gates.
    pub span_tokens: usize,
    /// Whether this location is an exact sub-function fragment rather than a whole
    /// function/method/class location.
    pub is_fragment: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_kind: Option<FragmentKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_unit: Option<EnclosingUnit>,
    /// The location sits inside an inline test module (`mod tests`) — counted
    /// as test scope even when the file path looks like production (#226).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_test_module: bool,
    /// The file is recognized as generated/distributed output. This location-level signal
    /// comes from source-aware cues such as generated-code headers and CSS distribution
    /// markers. Families move to `generated` only when all locations are generated, or when
    /// source plus compiled CSS outputs form a build pipeline (#224).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub looks_generated: bool,
    /// For a sub-DAG (partial) clone, the inclusive source line range AT THIS SITE of the heavy
    /// shared computation the family is grouped on (the anchor present in every member). `None`
    /// when the family shares no heavy sub-DAG, or the anchor carries no source span. Lets a
    /// partial clone point at *where* the shared computation lives in each copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_subdag: Option<(u32, u32)>,
}

/// Inclusive source-line range used to construct a [`Loc`].
#[derive(Clone, Copy)]
pub struct LineSpan {
    /// First 1-based source line included in the location.
    pub start_line: u32,
    /// Last 1-based source line included in the location.
    pub end_line: u32,
}

impl LineSpan {
    /// Build an inclusive source-line range.
    pub fn new(start_line: u32, end_line: u32) -> Self {
        Self {
            start_line,
            end_line,
        }
    }

    /// Inclusive line count, saturating for malformed ranges.
    pub fn line_count(self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// Constructor input for [`Loc`].
///
/// Keeping this as a named struct makes location metadata additions explicit at call sites
/// without widening a positional constructor.
#[derive(Clone)]
pub struct LocInit {
    /// Source file path as reported to users.
    pub file: String,
    /// Inclusive source-line range.
    pub source_span: LineSpan,
    /// Normalized language name.
    pub lang: String,
    /// Syntactic unit kind at this location.
    pub kind: UnitKind,
    /// Language-neutral facts about the source construct that produced this unit.
    pub origin: UnitOrigin,
    /// Optional function/method/class name.
    pub name: Option<String>,
    /// Value-graph size for this location.
    pub sem: usize,
    /// Normalized-token span used by detector size gates.
    pub span_tokens: usize,
}

impl Loc {
    pub fn new(init: LocInit) -> Self {
        let LocInit {
            file,
            source_span,
            lang,
            kind,
            origin,
            name,
            sem,
            span_tokens,
        } = init;
        Loc {
            file,
            start_line: source_span.start_line,
            end_line: source_span.end_line,
            lang,
            kind,
            origin,
            name,
            sem,
            span_lines: source_span.line_count(),
            span_tokens,
            is_fragment: false,
            fragment_kind: None,
            reason_code: None,
            enclosing_unit: None,
            in_test_module: false,
            looks_generated: false,
            shared_subdag: None,
        }
    }
}

#[derive(Serialize)]
pub struct DupPair {
    pub left: Loc,
    pub right: Loc,
    pub score: f64,
    pub cross_language: bool,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct AbstractionWitness {
    pub claim: &'static str,
    pub basis: &'static str,
    pub members_checked: u32,
    pub reason_code: &'static str,
    pub template_format: &'static str,
    pub template: Vec<String>,
    pub holes: Vec<AbstractionHole>,
    pub caveats: Vec<&'static str>,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct AbstractionHole {
    pub index: u32,
    pub template_index: u32,
    pub kind: &'static str,
    pub role: &'static str,
    pub left: &'static str,
    pub right: &'static str,
    pub observed: Vec<&'static str>,
    pub left_line: u32,
    pub right_line: u32,
}

#[derive(Serialize)]
pub struct Group {
    pub score: f64,
    pub members: Vec<Loc>,
    #[serde(skip)]
    pub semantic_laws: Vec<ValueLaw>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstraction_witness: Option<AbstractionWitness>,
    /// WHY the members merged — the agent-facing equivalence witness (#222).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<EquivalenceWitness>,
}

/// The evidence behind a group: which kind of convergence merged its members,
/// derived from the same predicates the channels gate on. An agent reading
/// JSON could not previously tell an exact value-graph proof from shape likeness
/// (`shared_lines: 0` with `mean_score: 1.0` was uninterpretable — the #216
/// audit's top gap); this names it without re-plumbing the scorer.
#[derive(Clone, Serialize)]
pub struct EquivalenceWitness {
    /// `exact-value-graph` (every member strict-exact-safe with one identical
    /// value multiset), `shared-sub-dag` (a common heavy anchor — see each
    /// location's `shared_subdag` span), `copy-paste-run` (token-identical
    /// contiguous run), or `structural-similarity` (the fuzzy near channel).
    pub kind: &'static str,
    /// For `exact-value-graph`: the size of the shared value multiset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_nodes: Option<usize>,
    /// For `structural-similarity`: mean value-graph Jaccard vs the first member
    /// — high here with low shape similarity means behaviorally-driven
    /// convergence, not surface likeness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_value_jaccard: Option<f64>,
    /// For `structural-similarity`: mean shape Jaccard vs the first member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_shape_jaccard: Option<f64>,
    /// For same-language near/shared-core families: the anti-unification grade of the
    /// two representative copies — "equal except these k holes", with each hole's value
    /// class and a referent check (#315). Computed by the presentation layer, which has
    /// source access; `None` for exact/copy-paste witnesses and until that layer runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graded: Option<GradedWitness>,
    /// Internal presentation-layer indices of the two locations compared for `graded`.
    /// Query JSON maps these to stable member ids; raw witness serialization omits them
    /// to keep the detect-report contract unchanged.
    #[serde(skip_serializing)]
    pub graded_pair: Option<(usize, usize)>,
}

#[derive(Serialize)]
pub struct Metrics {
    pub files: usize,
    pub units: usize,
    pub candidate_pairs: usize,
    pub accepted_pairs: usize,
    pub groups: usize,
}

#[derive(Serialize)]
pub struct Report {
    pub tool: &'static str,
    pub version: &'static str,
    pub detector: String,
    pub duplicates: Vec<DupPair>,
    pub groups: Vec<Group>,
    /// Reinvented-helper containment findings — see [`ReinventedHelper`].
    pub reinvented: Vec<ReinventedHelper>,
    pub metrics: Metrics,
}

fn unit_kind_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Function => "Function",
        UnitKind::Method => "Method",
        UnitKind::Class => "Class",
        UnitKind::Block => "Block",
    }
}

fn unit_key(
    file: &str,
    kind: UnitKind,
    name: Option<&str>,
    start_line: u32,
    end_line: u32,
) -> String {
    format!(
        "{}:{}:{}-{}:{}",
        file,
        unit_kind_name(kind),
        start_line,
        end_line,
        name.unwrap_or("")
    )
}

#[derive(Serialize)]
pub struct UnitLoc {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Diagnostic dump: all extracted units and all LSH candidate index pairs (into
/// `units`). Lets the evaluator split recall loss across extraction / candidate
/// generation / scoring.
pub struct Dump {
    pub units: Vec<UnitLoc>,
    pub candidates: Vec<(u32, u32)>,
}
