use crate::*;

pub(super) type FragmentQuery = (PathBuf, String, Vec<serde_json::Value>);

/// Write `fixtures` into a unique temp dir and run a semantic JSON query,
/// returning the project dir, the raw query output, and the parsed families.
pub(super) fn query_fragment_fixtures(tag: &str, fixtures: &[(&str, &str)]) -> FragmentQuery {
    query_fragment_fixtures_with(tag, fixtures, &[])
}

/// Like [`query_fragment_fixtures`], but raises the size gates so only exact
/// fragments can report.
pub(super) fn query_fragment_only_fixtures(tag: &str, fixtures: &[(&str, &str)]) -> FragmentQuery {
    query_fragment_fixtures_with(tag, fixtures, &["--min-lines", "100", "--min-size", "100"])
}

pub(super) fn query_fragment_only_fixture_families(
    tag: &str,
    fixtures: &[(&str, &str)],
) -> (String, Vec<serde_json::Value>) {
    let (dir, out, families) = query_fragment_only_fixtures(tag, fixtures);
    let _ = fs::remove_dir_all(dir);
    (out, families)
}

fn query_fragment_fixtures_with(
    tag: &str,
    fixtures: &[(&str, &str)],
    size_args: &[&str],
) -> FragmentQuery {
    let dir = make_temp_dir(tag);
    write_files(&dir, fixtures);
    let mut args = vec!["query", dir.to_str().unwrap(), "--mode", "semantic"];
    args.extend_from_slice(size_args);
    args.extend_from_slice(&["--format", "json", "top=0"]);
    let out = run(&args);
    let families = query_families(&query_json(&out)).to_vec();
    (dir, out, families)
}

pub(super) fn family_locations(family: &serde_json::Value) -> &[serde_json::Value] {
    family["locations"].as_array().expect("locations")
}

pub(super) fn location_files(family: &serde_json::Value) -> Vec<&str> {
    family_locations(family)
        .iter()
        .filter_map(|loc| loc["file"].as_str())
        .collect()
}

pub(super) fn family_all_blocks(family: &serde_json::Value) -> bool {
    family_locations(family)
        .iter()
        .all(|loc| loc["kind"] == "Block")
}

/// Locations of `family` whose file ends with `left` or `right`.
pub(super) fn pair_locations<'a>(
    family: &'a serde_json::Value,
    left: &str,
    right: &str,
) -> Vec<&'a serde_json::Value> {
    family_locations(family)
        .iter()
        .filter(|loc| {
            loc["file"].as_str().unwrap_or("").ends_with(left)
                || loc["file"].as_str().unwrap_or("").ends_with(right)
        })
        .collect()
}

/// First family whose locations include both a `left` and a `right` file.
pub(super) fn find_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let files = location_files(family);
        files.iter().any(|file| file.ends_with(left))
            && files.iter().any(|file| file.ends_with(right))
    })
}

pub(super) fn has_pair_family(families: &[serde_json::Value], left: &str, right: &str) -> bool {
    find_pair_family(families, left, right).is_some()
}

pub(super) fn assert_block_pair_family(
    families: &[serde_json::Value],
    out: &str,
    left: &str,
    right: &str,
    negative: &str,
    context: &str,
) {
    let family = find_pair_family(families, left, right)
        .unwrap_or_else(|| panic!("missing exact {context} family {left}/{right}: {out}"));
    assert!(
        family_all_blocks(family),
        "{context} fragments should report as Block units: {family:?}"
    );
    assert!(
        location_files(family)
            .iter()
            .all(|file| !file.ends_with(negative)),
        "hard negative must not merge into {left}/{right}: {family:?}"
    );
}

#[derive(Clone, Copy)]
pub(super) struct BranchPairCase<'a> {
    left: &'a str,
    right: &'a str,
    negative: &'a str,
    start_line: u64,
    end_line: u64,
}

pub(super) fn branch_pair<'a>(
    left: &'a str,
    right: &'a str,
    negative: &'a str,
    start_line: u64,
    end_line: u64,
) -> BranchPairCase<'a> {
    BranchPairCase {
        left,
        right,
        negative,
        start_line,
        end_line,
    }
}

#[derive(Clone, Copy)]
pub(super) struct BranchNonPairCase<'a> {
    left: &'a str,
    right: &'a str,
    left_start: u64,
    left_end: u64,
    right_start: u64,
    right_end: u64,
}

pub(super) fn branch_non_pair<'a>(
    left: &'a str,
    right: &'a str,
    left_span: (u64, u64),
    right_span: (u64, u64),
) -> BranchNonPairCase<'a> {
    BranchNonPairCase {
        left,
        right,
        left_start: left_span.0,
        left_end: left_span.1,
        right_start: right_span.0,
        right_end: right_span.1,
    }
}

pub(super) fn assert_branch_pair_cases(
    families: &[serde_json::Value],
    out: &str,
    context: &str,
    positive: &[BranchPairCase<'_>],
    negative: &[BranchNonPairCase<'_>],
) {
    for case in positive {
        let family = block_branch_pair_family(
            families,
            case.left,
            case.right,
            case.negative,
            case.start_line,
            case.end_line,
        )
        .unwrap_or_else(|| {
            panic!(
                "missing exact {context} branch family {}/{}: {out}",
                case.left, case.right
            )
        });
        assert!(
            family_all_blocks(family),
            "{context} fragments should report as Block units: {family:?}"
        );
    }

    for case in negative {
        assert!(
            !families_pair_locations(
                families,
                (case.left, case.left_start, case.left_end),
                (case.right, case.right_start, case.right_end),
            ),
            "{context} boundary must not merge {}/{}: {out}",
            case.left,
            case.right
        );
    }
}

/// First all-Block family that pairs `left` with `right` and excludes `negative`.
pub(super) fn find_block_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let files = location_files(family);
        files.iter().any(|file| file.ends_with(left))
            && files.iter().any(|file| file.ends_with(right))
            && family_all_blocks(family)
            && files.iter().all(|file| !file.ends_with(negative))
    })
}

/// Like [`find_block_pair_family`], but `left`/`right` must report the
/// `start_line..end_line` span.
pub(super) fn find_block_pair_family_at<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
    start_line: u64,
    end_line: u64,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let span_files: Vec<&str> = family_locations(family)
            .iter()
            .filter(|loc| loc["start_line"] == start_line && loc["end_line"] == end_line)
            .filter_map(|loc| loc["file"].as_str())
            .collect();
        span_files.iter().any(|file| file.ends_with(left))
            && span_files.iter().any(|file| file.ends_with(right))
            && family_all_blocks(family)
            && location_files(family)
                .iter()
                .all(|file| !file.ends_with(negative))
    })
}

/// Like [`find_block_pair_family`], but some location must span multiple lines.
pub(super) fn find_multiline_block_pair_family<'a>(
    families: &'a [serde_json::Value],
    left: &str,
    right: &str,
    negative: &str,
) -> Option<&'a serde_json::Value> {
    families.iter().find(|family| {
        let files = location_files(family);
        files.iter().any(|file| file.ends_with(left))
            && files.iter().any(|file| file.ends_with(right))
            && family_all_blocks(family)
            && family_locations(family).iter().any(|loc| {
                loc["end_line"].as_u64().unwrap_or(0) > loc["start_line"].as_u64().unwrap_or(0) + 1
            })
            && files.iter().all(|file| !file.ends_with(negative))
    })
}
