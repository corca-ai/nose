use crate::query_options::parse_min_value;

#[derive(clap::Args)]
pub(crate) struct QueryLimits {
    /// Maximum distinct candidate pairs; larger limits cost more time and memory. [default: 16000000]
    #[arg(long, value_parser = clap::value_parser!(usize))]
    pub(crate) max_candidate_pairs: Option<usize>,
    /// Ignore units smaller than this size, in IL tokens (the unit's node count). [default: 24]
    #[arg(long)]
    pub(crate) min_size: Option<usize>,
    /// Advanced: also require this many source lines (most uses only need --min-size). [default: 5]
    #[arg(long, hide = true)]
    pub(crate) min_lines: Option<u32>,
    /// Hide families whose refactoring value is below this (noise floor on large repos).
    #[arg(long, value_parser = parse_min_value)]
    pub(crate) min_value: Option<f64>,
    /// Keep only families with at least this many duplicated copies. [default: 2]
    #[arg(long)]
    pub(crate) min_members: Option<usize>,
}
