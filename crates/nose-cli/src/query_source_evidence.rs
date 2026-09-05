//! Bounded live-source observations, kept separate from detector witnesses.
use crate::{
    baseline,
    source_lines::{anti_unify_all, line_diff},
};
use nose_detect::{Loc, RefactorFamily};
use serde_json::{json, Value};
use std::io::Read;

const MEMBER_LIMIT: usize = 8;
const LINE_LIMIT: usize = 120;
const FILE_LIMIT: u64 = 16 * 1024 * 1024;

fn read_member(loc: &Loc) -> Result<Vec<String>, &'static str> {
    let file = std::fs::File::open(&loc.file).map_err(|_| "source-unavailable")?;
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= FILE_LIMIT)
    {
        return Err("source-unavailable-or-file-limit");
    }
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "source-unavailable")?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("file-limit");
    }
    let text = String::from_utf8(bytes).map_err(|_| "non-utf8")?;
    if loc.start_line == 0 || loc.end_line < loc.start_line {
        return Err("invalid-range");
    }
    let count = (loc.end_line - loc.start_line + 1) as usize;
    let lines: Vec<_> = text
        .lines()
        .skip(loc.start_line as usize - 1)
        .take(count)
        .collect();
    if lines.len() != count {
        return Err("range-unavailable");
    }
    // Limit copied region bytes as well as diff work.
    if lines
        .iter()
        .take(LINE_LIMIT)
        .map(|l| l.len() + 1)
        .sum::<usize>()
        > 64 * 1024
    {
        return Err("region-byte-limit");
    }
    Ok(lines
        .into_iter()
        .take(LINE_LIMIT)
        .map(str::to_owned)
        .collect())
}

fn site(loc: &Loc) -> Value {
    json!({"boundary":crate::query_assessment::boundary(loc),"member_id":baseline::member_id(loc),"file":loc.file,"start":loc.start_line,
        "end":loc.end_line,"region":loc.source_region})
}

pub(crate) fn collect(f: &RefactorFamily, diffs: bool) -> Value {
    let mut members = Vec::new();
    let mut bodies = Vec::new();
    for (index, loc) in f.locations.iter().take(MEMBER_LIMIT).enumerate() {
        let (member, lines) = read_observation(loc);
        if let Some(lines) = lines {
            bodies.push((index, lines));
        }
        members.push(member);
    }
    let complete =
        bodies.len() == f.locations.len() && members.iter().all(|m| m["truncated"] == false);
    let mut out = json!({"basis":"literal-source-line-alignment","source":"live-unverified",
        "meaning":"Current source observations, not snapshot-verified text or edit-safety proof. Skeleton holes describe anchor differences; additions remain visible in pair diffs.",
        "coverage":{"total_members":f.locations.len(),"attempted_members":members.len(),"available_members":bodies.len(),
        "complete":complete,"omitted_members":f.locations.len().saturating_sub(MEMBER_LIMIT),
        "member_limit":MEMBER_LIMIT,"line_limit_per_member":LINE_LIMIT,"file_byte_limit":FILE_LIMIT},
        "members":members,"status":"unavailable"});
    if bodies.len() < 2 {
        return out;
    }
    out["status"] = json!(if complete { "complete" } else { "partial" });
    if f.languages == 1 {
        let lines: Vec<_> = bodies.iter().map(|(_, lines)| lines.clone()).collect();
        let (skeleton, shared, varying) = anti_unify_all(&lines);
        out["skeleton"] = json!(skeleton);
        out["shared_lines"] = json!(shared);
        out["varying_regions"] = json!(varying);
    } else {
        out["alignment_status"] = json!("cross-language-not-aligned");
    }
    if diffs {
        let (anchor, a) = &bodies[0];
        out["diffs"] = json!(bodies
            .iter()
            .skip(1)
            .map(|(index, b)| pair_diff(&f.locations[*anchor], &f.locations[*index], a, b))
            .collect::<Vec<_>>());
    }
    out
}

fn pair_diff(a: &Loc, b: &Loc, la: &[String], lb: &[String]) -> Value {
    let ar: Vec<_> = la.iter().map(String::as_str).collect();
    let br: Vec<_> = lb.iter().map(String::as_str).collect();
    let (mut ai, mut bi) = (a.start_line, b.start_line);
    let lines: Vec<_> = line_diff(&ar, &br).into_iter().map(|(tag, text)| {
        let line = json!({"tag":tag.to_string(),"text":text,
            "a_line":if tag == '+' {None} else {Some(ai)},"b_line":if tag == '-' {None} else {Some(bi)}});
        if tag != '+' { ai += 1; }
        if tag != '-' { bi += 1; }
        line
    }).collect();
    json!({"a":site(a),"b":site(b),"scope":"pair-only","basis":"literal-source-diff",
        "truncated":a.end_line - a.start_line + 1 > LINE_LIMIT as u32 || b.end_line - b.start_line + 1 > LINE_LIMIT as u32,"lines":lines})
}

pub(crate) fn render(evidence: &Value, markdown: bool) {
    let label = if markdown {
        "**source comparison**"
    } else {
        "     source comparison"
    };
    let coverage = &evidence["coverage"];
    println!(
        "{label} — {} / {} members available · {} · at most {} lines/member",
        coverage["available_members"],
        coverage["total_members"],
        evidence["status"].as_str().unwrap(),
        LINE_LIMIT
    );
    println!("  {}", evidence["meaning"].as_str().unwrap());
    for member in evidence["members"].as_array().unwrap() {
        if member["status"] == "unavailable" {
            println!(
                "  unavailable {}:{} — {}",
                member["file"].as_str().unwrap(),
                member["start"],
                member["reason"].as_str().unwrap()
            );
        }
    }
    if let Some(skeleton) = evidence["skeleton"].as_array() {
        println!(
            "  {} shared lines · {} varying anchor regions",
            evidence["shared_lines"], evidence["varying_regions"]
        );
        for line in skeleton.iter().take(40) {
            println!("       │ {}", line.as_str().unwrap());
        }
        if skeleton.len() > 40 {
            println!(
                "  skeleton display truncated: 40 / {} lines",
                skeleton.len()
            );
        }
    }
    if let Some(diffs) = evidence["diffs"].as_array() {
        for diff in diffs {
            let label = if markdown { "**diff**" } else { "     diff" };
            println!(
                "{label} {}:{} vs {}:{} · pair only · truncated={}",
                diff["a"]["file"].as_str().unwrap(),
                diff["a"]["start"],
                diff["b"]["file"].as_str().unwrap(),
                diff["b"]["start"],
                diff["truncated"]
            );
            for line in diff["lines"].as_array().unwrap() {
                println!(
                    "       {} {}",
                    line["tag"].as_str().unwrap(),
                    line["text"].as_str().unwrap()
                );
            }
        }
    }
}

pub(crate) fn render_structural(f: &RefactorFamily) {
    let Some((grade, a, b)) = f.witness.as_ref().and_then(|w| {
        let (ai, bi) = w.graded_pair?;
        Some((
            w.graded.as_ref()?,
            f.locations.get(ai)?,
            f.locations.get(bi)?,
        ))
    }) else {
        println!("  structural correspondence: unavailable or unsupported for this family");
        return;
    };
    println!(
        "  structural correspondence: {}:{} vs {}:{} (pair only)",
        a.file, a.start_line, b.file, b.start_line
    );
    println!(
        "    {} holes · equal modulo holes: {} · modeled caveat: {}",
        grade.holes, grade.equal_modulo_holes, grade.modeled_caveat
    );
    for spot in grade.spots.iter().take(12) {
        println!(
            "    {} · a {} · b {}",
            spot.class,
            spot_side(spot.a_lines, &spot.a_text),
            spot_side(spot.b_lines, &spot.b_text)
        );
    }
    if grade.spots.len() > 12 {
        println!("    spots truncated: 12 / {}", grade.spots.len());
    }
    println!(
        "    patterns: {:?} · caveats: {:?}",
        grade.patterns, grade.caveat_names
    );
    // Referent mismatches are part of the existing grade, not inferred source intent.
    if !grade.referent_mismatches.is_empty() {
        println!(
            "    referent mismatches: {}",
            serde_json::to_string(&grade.referent_mismatches).expect("serializable referents")
        );
    }
}

pub(crate) fn selected_sources(locations: &[&Loc]) -> Value {
    let members: Vec<_> = locations
        .iter()
        .take(MEMBER_LIMIT)
        .map(|loc| {
            let (mut body, lines) = read_observation(loc);
            if let Some(lines) = lines {
                body["lines"] = json!(lines
                    .iter()
                    .enumerate()
                    .map(|(index, text)| json!({"line":loc.start_line + index as u32,"text":text}))
                    .collect::<Vec<_>>());
            }
            body
        })
        .collect();
    json!({"source":"live-unverified","scope":"selected-members","selected":locations.len(),
        "shown":members.len(),"omitted":locations.len().saturating_sub(MEMBER_LIMIT),
        "member_limit":MEMBER_LIMIT,"line_limit_per_member":LINE_LIMIT,"members":members})
}

pub(crate) fn render_selected_sources(source: &Value) {
    println!(
        "  selected source: {} / {} members · live source, not snapshot-verified",
        source["shown"], source["selected"]
    );
    for member in source["members"].as_array().unwrap() {
        println!(
            "    {}:{}-{}",
            member["file"].as_str().unwrap(),
            member["start"],
            member["end"]
        );
        println!(
            "      boundary: {}",
            member["boundary"]["meaning"].as_str().unwrap()
        );
        if let Some(lines) = member["lines"].as_array() {
            for line in lines {
                println!(
                    "      {} │ {}",
                    line["line"],
                    line["text"].as_str().unwrap()
                );
            }
            if member["truncated"] == true {
                println!("      source truncated after {LINE_LIMIT} lines");
            }
        } else {
            println!(
                "      source unavailable: {}",
                member["reason"].as_str().unwrap()
            );
        }
    }
    if source["omitted"].as_u64().unwrap_or(0) > 0 {
        println!(
            "  {} source bodies omitted; narrow member-path~ or member-dir= to inspect more",
            source["omitted"]
        );
    }
}

fn spot_side(lines: Option<(u32, u32)>, text: &str) -> String {
    let location = lines.map_or_else(
        || "source location unavailable".into(),
        |(start, end)| format!("lines {start}-{end}"),
    );
    let excerpt = if text.is_empty() {
        "source excerpt unavailable"
    } else {
        text
    };
    format!("{location}: {excerpt}")
}

fn read_observation(loc: &Loc) -> (Value, Option<Vec<String>>) {
    let mut member = site(loc);
    match read_member(loc) {
        Ok(lines) => {
            member["status"] = json!("available");
            member["lines_shown"] = json!(lines.len());
            member["truncated"] = json!(loc.end_line - loc.start_line + 1 > LINE_LIMIT as u32);
            (member, Some(lines))
        }
        Err(reason) => {
            member["status"] = json!("unavailable");
            member["reason"] = json!(reason);
            (member, None)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unmapped_spots_explain_missing_source_without_debug_option_names() {
        assert_eq!(
            super::spot_side(None, ""),
            "source location unavailable: source excerpt unavailable"
        );
        assert_eq!(
            super::spot_side(Some((10, 12)), "return x;"),
            "lines 10-12: return x;"
        );
    }
}
