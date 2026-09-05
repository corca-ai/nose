use super::*;

/// Read attributes from CST items, never from strings or comments inside a body.
/// Only unconditional test attributes establish test-only context here.
pub(super) fn contains(lo: &Lowering, mut node: TsNode) -> bool {
    loop {
        let mut previous = node.prev_named_sibling();
        while let Some(attribute) = previous {
            match attribute.kind() {
                "attribute_item" => {
                    if is_test_attribute(lo.text(attribute)) {
                        return true;
                    }
                }
                "line_comment" | "block_comment" => {}
                _ => break,
            }
            previous = attribute.prev_named_sibling();
        }
        if matches!(node.kind(), "source_file" | "declaration_list")
            && Lowering::named_children(node).iter().any(|child| {
                child.kind() == "inner_attribute_item" && is_test_attribute(lo.text(*child))
            })
        {
            return true;
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}

fn is_test_attribute(text: &str) -> bool {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    matches!(
        compact.as_str(),
        "#[test]" | "#[cfg(test)]" | "#![cfg(test)]"
    )
}
