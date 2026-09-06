use super::{json, read_source, site, Loc, Value, LINE_LIMIT};

struct Window {
    requested_start: u32,
    start: u32,
    end: u32,
    lines: Vec<String>,
}

pub(super) fn read(loc: &Loc, padding: usize) -> Value {
    let mut body = site(loc);
    match lines(loc, padding) {
        Ok(Window {
            requested_start,
            start,
            end,
            lines,
        }) => {
            body["status"] = json!("available");
            body["truncated"] = json!(
                start > requested_start
                    || u64::from(end) - u64::from(start) + 1 > lines.len() as u64
            );
            body["lines_shown"] = json!(lines.len());
            body["context"] = json!({"requested_start":requested_start,"start":start,"end":end,
                "shown_end":start + lines.len() as u32 - 1,"requested_lines_each_side":padding,
                "meaning":"Surrounding live source only; member coordinates and detector evidence are unchanged."});
            body["lines"] = json!(lines.iter().enumerate().map(|(index, text)| {
                let line = start + index as u32;
                json!({"line":line,"text":text,"in_member":line >= loc.start_line && line <= loc.end_line})
            }).collect::<Vec<_>>());
        }
        Err(reason) => {
            body["status"] = json!("unavailable");
            body["reason"] = json!(reason);
        }
    }
    body
}

fn lines(loc: &Loc, padding: usize) -> Result<Window, &'static str> {
    let text = read_source(&loc.file)?;
    let total = text.lines().count() as u32;
    if loc.start_line == 0 || loc.end_line < loc.start_line {
        return Err("invalid-range");
    }
    if loc.end_line > total {
        return Err("range-unavailable");
    }
    let (mut low, mut high) = (1, total);
    if let Some(parent) = &loc.enclosing_unit {
        if parent.file == loc.file
            && parent.start_line > 0
            && parent.start_line <= loc.start_line
            && parent.end_line >= loc.end_line
            && parent.end_line <= total
        {
            (low, high) = (parent.start_line, parent.end_line);
        }
    }
    let padding = u32::try_from(padding).unwrap_or(u32::MAX);
    let requested_start = loc.start_line.saturating_sub(padding).max(low);
    // Do not spend the entire display budget on preceding context.
    let start = requested_start.max(loc.start_line.saturating_sub((LINE_LIMIT / 3) as u32));
    let end = loc.end_line.saturating_add(padding).min(high);
    let lines: Vec<_> = text
        .lines()
        .skip(start as usize - 1)
        .take((end - start + 1) as usize)
        .take(LINE_LIMIT)
        .collect();
    if lines.iter().map(|line| line.len() + 1).sum::<usize>() > 64 * 1024 {
        return Err("region-byte-limit");
    }
    Ok(Window {
        requested_start,
        start,
        end,
        lines: lines.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn context_is_bounded_keeps_the_member_and_rejects_missing_source() {
        use nose_detect::{LineSpan, Loc, LocInit};
        let path =
            std::env::temp_dir().join(format!("nose source-context {}.txt", std::process::id()));
        std::fs::write(
            &path,
            (1..=400).map(|n| format!("line {n}\n")).collect::<String>(),
        )
        .unwrap();
        let mut loc = Loc::new(LocInit {
            file: path.to_string_lossy().into_owned(),
            source_span: LineSpan::new(201, 205),
            lang: "rust".into(),
            kind: nose_il::UnitKind::Function,
            origin: nose_il::UnitOrigin::unknown(),
            name: None,
            sem: 1,
            span_tokens: 1,
        });
        for padding in [0, 20, usize::MAX] {
            let body = super::read(&loc, padding);
            assert_eq!(body["status"], "available");
            let lines = body["lines"].as_array().unwrap();
            assert!(lines.len() <= super::LINE_LIMIT);
            assert!(lines.iter().any(|line| line["in_member"] == true));
            assert_eq!(body["start"], 201);
            assert_eq!(body["end"], 205);
            if padding == usize::MAX {
                assert_eq!(body["truncated"], true);
                assert_eq!(body["context"]["requested_start"], 1);
            }
        }
        loc.end_line = 401;
        assert_eq!(super::read(&loc, 20)["reason"], "range-unavailable");
        loc.start_line = 1;
        loc.end_line = 1;
        std::fs::write(&path, "x".repeat(64 * 1024 + 1)).unwrap();
        assert_eq!(super::read(&loc, 20)["reason"], "region-byte-limit");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(super::read(&loc, 20)["reason"], "source-unavailable");
    }
}
