//! Presentation bounds are independent of source collection and alignment.
const LINE_CHAR_LIMIT: usize = 240;
const SECTION_CHAR_LIMIT: usize = 4_000;

pub(crate) fn bounded_lines(lines: &[&str]) -> Vec<String> {
    let mut displayed = Vec::new();
    let mut remaining = SECTION_CHAR_LIMIT;
    let mut clipped = false;
    for line in lines {
        if remaining < 80 {
            break;
        }
        let count = line.chars().count();
        let limit = LINE_CHAR_LIMIT.min(remaining);
        let rendered = if count > limit {
            // Reserve room for the notice so the entire displayed line stays bounded.
            let shown = limit.saturating_sub(60);
            let prefix: String = line.chars().take(shown).collect();
            clipped = true;
            format!(
                "{prefix} … [display clipped: {} characters omitted]",
                count - shown
            )
        } else {
            (*line).to_owned()
        };
        remaining -= rendered.chars().count();
        displayed.push(rendered);
    }
    let omitted = lines.len() - displayed.len();
    if omitted > 0 {
        clipped = true;
        displayed.push(format!(
            "display clipped: {omitted} lines omitted (4,000 characters/section)"
        ));
    }
    if clipped {
        displayed.push("Use the full JSON command below for unclipped collected evidence (collection limits still apply).".into());
    }
    displayed
}

pub(super) fn print_lines(lines: &[String]) {
    let refs: Vec<_> = lines.iter().map(String::as_str).collect();
    for line in bounded_lines(&refs) {
        println!("{line}");
    }
}
