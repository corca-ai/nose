use super::*;

pub(super) fn lower_lambda(lo: &mut Lowering, node: TsNode) -> NodeId {
    let span = lo.span(node);
    let is_async = swift_lambda_is_async(lo.text(node));
    let mut kids = Vec::new();
    if let Some(lambda_type) = node.child_by_field_name("type") {
        lower_lambda_type_params(lo, lambda_type, &mut kids);
    }
    for child in Lowering::named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "lambda_function_type")
    {
        lower_lambda_type_params(lo, child, &mut kids);
    }
    if kids.is_empty() {
        for name in lambda_parameter_names_from_text(lo.text(node)) {
            kids.push(lo.add(NodeKind::Param, Payload::Name(lo.sym(&name)), span, &[]));
        }
    }
    let body = first_statements_child(node)
        .map(|body| lower_function_body(lo, body))
        .unwrap_or_else(|| lo.empty_block(span));
    dedupe_lambda_params(lo, &mut kids);
    kids.push(if is_async {
        lo.protocol_boundary(
            span,
            SourceProtocolKind::AsyncFunction,
            "async_function",
            &[body],
        )
    } else {
        body
    });
    lo.add(NodeKind::Lambda, Payload::None, span, &kids)
}
pub(super) fn dedupe_lambda_params(lo: &Lowering, kids: &mut Vec<NodeId>) {
    let mut seen = Vec::new();
    kids.retain(|&kid| {
        if lo.b.kind(kid) != NodeKind::Param {
            return true;
        }
        let Payload::Name(name) = lo.b.payload(kid) else {
            return true;
        };
        if seen.contains(&name) {
            false
        } else {
            seen.push(name);
            true
        }
    });
}
pub(super) fn lower_lambda_type_params(lo: &mut Lowering, node: TsNode, out: &mut Vec<NodeId>) {
    for child in Lowering::named_children(node) {
        if child.kind() == "lambda_parameter" {
            lower_param(lo, child, out);
        } else if matches!(
            child.kind(),
            "lambda_function_type" | "lambda_function_type_parameters"
        ) {
            lower_lambda_type_params(lo, child, out);
        }
    }
}
pub(super) fn lambda_parameter_names_from_text(text: &str) -> Vec<String> {
    let Some(inner) = text
        .trim()
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))
    else {
        return Vec::new();
    };
    let inner = inner.trim();
    if let Some((header, _body)) = split_swift_lambda_in(inner) {
        return swift_lambda_parameter_header(header)
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split(',')
            .filter_map(lambda_parameter_name_from_header_part)
            .collect();
    }
    if inner.contains("$0") {
        return vec!["$0".to_string()];
    }
    Vec::new()
}

pub(super) fn swift_lambda_is_async(text: &str) -> bool {
    swift_lambda_header(text).is_some_and(swift_lambda_header_has_async)
}

fn swift_lambda_header(text: &str) -> Option<&str> {
    let inner = text
        .trim()
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))?
        .trim();
    split_swift_lambda_in(inner).map(|(header, _)| header.trim())
}

fn swift_lambda_header_has_async(header: &str) -> bool {
    let header = swift_lambda_header_without_capture_list(header.trim());
    swift_lambda_async_modifier_offset(header).is_some()
}

fn swift_lambda_parameter_header(header: &str) -> &str {
    let header = swift_lambda_header_without_capture_list(header.trim());
    let before_return = swift_lambda_top_level_return_arrow(header)
        .map(|idx| &header[..idx])
        .unwrap_or(header)
        .trim();
    swift_lambda_signature_modifier_start(before_return)
        .map(|idx| before_return[..idx].trim())
        .unwrap_or(before_return)
}

fn swift_lambda_header_tokens(header: &str) -> impl Iterator<Item = &str> {
    header
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | ',' | ':' | '-' | '>' | '[' | ']' | '{' | '}'
                )
        })
        .filter(|token| !token.is_empty())
}

fn swift_lambda_header_without_capture_list(header: &str) -> &str {
    let trimmed = header.trim_start();
    if !trimmed.starts_with('[') {
        return trimmed;
    }
    let mut depth = 0usize;
    for (idx, ch) in trimmed.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return trimmed[idx + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    trimmed
}

fn split_swift_lambda_in(text: &str) -> Option<(&str, &str)> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for (idx, ch) in text.char_indices() {
        if paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
            && is_swift_keyword_at(text, idx, "in")
        {
            return Some((&text[..idx], &text[idx + "in".len()..]));
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn swift_lambda_signature_modifier_start(header: &str) -> Option<usize> {
    ["async", "throws", "rethrows"]
        .into_iter()
        .filter_map(|keyword| swift_lambda_modifier_offset(header, keyword))
        .min()
}

fn swift_lambda_async_modifier_offset(header: &str) -> Option<usize> {
    swift_lambda_modifier_offset(header, "async")
}

fn swift_lambda_modifier_offset(header: &str, keyword: &str) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for (idx, ch) in header.char_indices() {
        if paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
            && swift_lambda_keyword_is_signature_modifier(header, idx, keyword)
        {
            return Some(idx);
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn swift_lambda_top_level_return_arrow(header: &str) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for (idx, ch) in header.char_indices() {
        if paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
            && header[idx..].starts_with("->")
        {
            return Some(idx);
        } else {
            match ch {
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    None
}

fn swift_lambda_keyword_is_signature_modifier(header: &str, idx: usize, keyword: &str) -> bool {
    if !is_swift_keyword_at(header, idx, keyword) {
        return false;
    }
    let before = header[..idx].trim_end();
    let after = header[idx + keyword.len()..].trim_start();
    swift_lambda_modifier_prefix_is_valid(before)
        && swift_lambda_modifier_tail_is_valid(keyword, after)
}

fn swift_lambda_modifier_prefix_is_valid(before: &str) -> bool {
    if swift_lambda_has_top_level_colon(before) {
        return false;
    }
    if before.ends_with(')') {
        return true;
    }
    swift_lambda_header_tokens(before)
        .filter(|token| !token.starts_with('@'))
        .any(|token| !matches!(token, "async" | "throws" | "rethrows"))
}

fn swift_lambda_modifier_tail_is_valid(keyword: &str, after: &str) -> bool {
    if after.is_empty() || after.starts_with("->") {
        return true;
    }
    if keyword == "async" {
        return consume_swift_keyword(after, "throws")
            .or_else(|| consume_swift_keyword(after, "rethrows"))
            .is_some_and(|rest| {
                let rest = rest.trim_start();
                rest.is_empty() || rest.starts_with("->")
            });
    }
    false
}

fn swift_lambda_has_top_level_colon(text: &str) -> bool {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for ch in text.chars() {
        if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 && ch == ':' {
            return true;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn is_swift_keyword_at(text: &str, idx: usize, keyword: &str) -> bool {
    if !text[idx..].starts_with(keyword) {
        return false;
    }
    let before = text[..idx].chars().next_back();
    let after = text[idx + keyword.len()..].chars().next();
    !before.is_some_and(is_swift_identifier_continue)
        && !after.is_some_and(is_swift_identifier_continue)
}

pub(super) fn lambda_parameter_name_from_header_part(part: &str) -> Option<String> {
    let before_type = part.trim().split(':').next()?.trim();
    let name = before_type
        .split_whitespace()
        .last()
        .unwrap_or(before_type)
        .trim();
    if name.is_empty() || name == "_" {
        None
    } else {
        Some(name.to_string())
    }
}
