use tree_sitter::Node;

pub(crate) fn child_at(node: Node<'_>, index: usize) -> Option<Node<'_>> {
    node.child(index.try_into().ok()?)
}

pub(crate) fn last_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let index = node.named_child_count().checked_sub(1)?;
    node.named_child(index.try_into().ok()?)
}
