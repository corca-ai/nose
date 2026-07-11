#[derive(Clone, Copy, Debug)]
pub struct DetectOptions {
    pub min_lines: u32,
    pub min_tokens: usize,
    pub threshold: f64,
    pub minhash_k: usize,
    pub bands: usize,
    pub cfg_norm: bool,
    /// Enable dead-code / dead-assignment elimination (normalization).
    pub dce: bool,
    /// Weight of the Jaccard term vs the LCS-alignment term in the final score.
    pub jaccard_weight: f64,
    /// Extract sub-function block units (loops/ifs/try plus exact statement
    /// fragments) in addition to functions/methods/classes. ON by default:
    /// measurement on the validated target showed gold clones are often
    /// sub-function fragments, and blocks lift recall (0.610→0.621),
    /// pool-precision (0.064→0.106) and AUC-PR (0.34→0.42) with HN-FP flat.
    /// Disable with `--no-blocks`.
    pub block_units: bool,
    /// Minimum duplicated run size for the contiguous copy-paste channel, in IL
    /// tokens. This is separate from structural unit size internally, but the query
    /// `--min-size` intentionally drives both so syntax gates have one size knob.
    pub contiguous_min_tokens: usize,
    /// Minimum duplicated run size for the contiguous copy-paste channel, in source
    /// lines. Query's advanced `--min-lines` drives both unit extraction and this floor.
    pub contiguous_min_lines: u32,
    /// Run the syntax copy-paste channel: a Rabin-Karp pass over each file's IL token
    /// stream that finds
    /// maximal duplicated runs *regardless of unit boundaries* (the Type-1/2 floor
    /// a token-based detector like jscpd catches). Enabled by `query --mode syntax`,
    /// and off for the strict/gold `detect` path so Type-4 benchmark numbers are stable.
    pub contiguous: bool,
    /// Run the unit detector used by the semantic and near channels. Turning this off
    /// leaves only any enabled syntax copy-paste channel.
    pub structural: bool,
    /// Generate structural candidates from value fingerprints. This is the semantic
    /// Type-4 path: loop/reduce/comprehension rewrites converge here even when their
    /// surface shape differs.
    pub value_candidates: bool,
    /// Generate structural candidates from syntactic shape fingerprints. This is the
    /// near Type-3 path: code can reach scoring even when behavior-defining literals or
    /// operators differ and therefore the value fingerprint no longer matches.
    pub shape_candidates: bool,
    /// Build syntactic unit features (`shapes`, `shape_minhash`, `linear`) for fuzzy
    /// structural scoring. Exact semantic runs do not need them: candidate generation
    /// and scoring both use the value graph only.
    pub shape_features: bool,
    /// Attach experimental abstraction witnesses to near-derived families whose normalized
    /// structure differs by exactly one supported literal leaf. This is a weak refactoring
    /// claim and never participates in exact semantic acceptance.
    pub abstraction_witnesses: bool,
    /// Materialize and sort raw accepted pair output. Hidden `nose detect` and library callers
    /// keep this on; query ranks grouped families and does not need the pair list.
    pub emit_pairs: bool,
}

impl Default for DetectOptions {
    fn default() -> Self {
        DetectOptions {
            min_lines: 5,
            min_tokens: 24,
            // 0.86: balanced operating point chosen from the unbiased precision
            // curve (§O). The 0.70–0.86 score bands are ~0% precision noise; 0.86
            // ~doubles precision (18%→33%) for a 0.07 recall cost and halves the
            // prediction count. Lower it for recall-completeness, raise for precision.
            threshold: 0.86,
            // 128/32 catches lower-similarity candidates (better recall ceiling)
            // at modest extra cost vs 64/16; bands=64 (rows=2) explodes candidates.
            minhash_k: 128,
            bands: 32,
            cfg_norm: true,
            dce: false,
            jaccard_weight: 0.5,
            block_units: true,
            contiguous_min_tokens: 24,
            contiguous_min_lines: 5,
            contiguous: false,
            structural: true,
            value_candidates: true,
            shape_candidates: false,
            shape_features: true,
            abstraction_witnesses: false,
            emit_pairs: true,
        }
    }
}
