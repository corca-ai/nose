use crate::source_lines::{anti_unify_all, line_diff, read_lines};

/// Print a unified diff between two family members' source — the few lines that
/// differ are what a reader needs to judge how cleanly the copies can be merged.
pub(crate) fn print_member_diff(a: &nose_detect::Loc, b: &nose_detect::Loc) {
    let (Some(la), Some(lb)) = (
        read_lines(&a.file, a.start_line, a.end_line),
        read_lines(&b.file, b.start_line, b.end_line),
    ) else {
        return;
    };
    println!(
        "     diff  {}:{}-{}  vs  {}:{}-{}",
        a.file, a.start_line, a.end_line, b.file, b.start_line, b.end_line
    );
    let ar: Vec<&str> = la.iter().map(String::as_str).collect();
    let br: Vec<&str> = lb.iter().map(String::as_str).collect();
    for (tag, line) in line_diff(&ar, &br) {
        println!("       {tag} {line}");
    }
}

/// Synthesize an *extraction proposal* aligned across **all** the family's copies (#360):
/// the lines invariant across *every* copy become the body of the shared helper, and each
/// maximal run that varies in *any* copy collapses to a `⟨param N⟩` placeholder — line-
/// granularity anti-unification, N-way. Turns "these are similar" into "extract this,
/// parameterize these N spots", and — unlike a pairwise skeleton — the result is safe to
/// apply to *every* member, not just the two largest, so it never claims a shared line a
/// third copy actually diverges on. Bounded to one family, paid only on `--show proposal`.
pub(crate) fn print_member_proposal(locations: &[nose_detect::Loc], action: &str) {
    // Read every copy's source; align across all of them. A copy whose source can't be
    // read is dropped, and the count reflects the copies actually aligned.
    let members: Vec<Vec<String>> = locations
        .iter()
        .filter_map(|l| read_lines(&l.file, l.start_line, l.end_line))
        .collect();
    if members.len() < 2 {
        return;
    }
    let (skeleton, shared, params) = anti_unify_all(&members);
    let copies = members.len();
    if shared == 0 {
        println!("     comparison  no invariant source lines across {copies} copies; inspect the diff before proposing an extraction");
        return;
    }
    if skeleton
        .iter()
        .filter(|line| !line.contains('⟨'))
        .all(|line| crate::source_lines::is_trivial_line(line.trim()))
    {
        println!("     comparison  only common syntax is invariant across {copies} copies; inspect the diff before proposing an extraction");
        return;
    }
    println!("     proposal  {action} · {shared} shared lines · {params} parameter(s) vary (across all {copies} copies)");
    for line in skeleton.iter().take(40) {
        println!("       │ {line}");
    }
}
