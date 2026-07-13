//! Pair-local connected witnesses for existing near-candidate seeds.
//!
//! A value/shape multiset can say that two units share substantial material while losing
//! where that material occurs. This module keeps a compact normalized-IL preorder and accepts
//! only one mapped tree or one contiguous statement window under a common `Block` on both
//! sides. Tree arity, statement order, call targets, control nodes, and mutation/exit nodes
//! remain exact. Only value leaves may become consistently-mapped holes.

use crate::{model::ConnectedWitness, LineSpan};
use nose_il::{Il, Interner, NodeId, NodeKind};
use nose_normalize::node_tag_valued;
use std::collections::BTreeMap;

const MAX_TOKENS: usize = 2_000;
const MIN_MAPPED_NODES: u32 = 20;
const MIN_COMPLETE_EXIT_NODES: u32 = 18;
const MAX_HOLES: usize = 8;
const MIN_SAME_UNIT_LINES: u32 = 3;
const MAX_SAME_UNIT_ROOT_PAIRS: usize = 256;

/// Two words per normalized IL node. `tag` retains leaf values so variations count as mapped
/// holes; `meta` packs the node kind, arity, preorder-subtree length, and source span. Keeping
/// this compact matters:
/// units are cached and a large repository can retain hundreds of thousands of nodes.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct MappedToken {
    tag: u64,
    meta: u64,
}

const KIND_BITS: u32 = 6;
const ARITY_BITS: u32 = 8;
const SUBTREE_BITS: u32 = 16;
const LINE_BITS: u32 = 17;
const KIND_SHIFT: u32 = 0;
const ARITY_SHIFT: u32 = KIND_SHIFT + KIND_BITS;
const SUBTREE_SHIFT: u32 = ARITY_SHIFT + ARITY_BITS;
const START_SHIFT: u32 = SUBTREE_SHIFT + SUBTREE_BITS;
const END_SHIFT: u32 = START_SHIFT + LINE_BITS;
const KIND_MASK: u64 = (1 << KIND_BITS) - 1;
const ARITY_MASK: u64 = (1 << ARITY_BITS) - 1;
const SUBTREE_MASK: u64 = (1 << SUBTREE_BITS) - 1;
const LINE_MASK: u64 = (1 << LINE_BITS) - 1;

impl MappedToken {
    fn new(
        tag: u64,
        kind: NodeKind,
        arity: usize,
        subtree_len: usize,
        start_line: u32,
        end_line: u32,
    ) -> Option<Self> {
        if kind as u64 > KIND_MASK
            || arity as u64 > ARITY_MASK
            || subtree_len == 0
            || subtree_len as u64 > SUBTREE_MASK
            || u64::from(start_line) > LINE_MASK
            || u64::from(end_line) > LINE_MASK
        {
            return None;
        }
        let meta = (kind as u64) << KIND_SHIFT
            | (arity as u64) << ARITY_SHIFT
            | (subtree_len as u64) << SUBTREE_SHIFT
            | u64::from(start_line) << START_SHIFT
            | u64::from(end_line) << END_SHIFT;
        Some(Self { tag, meta })
    }

    fn kind(self) -> u64 {
        (self.meta >> KIND_SHIFT) & KIND_MASK
    }

    fn arity(self) -> usize {
        ((self.meta >> ARITY_SHIFT) & ARITY_MASK) as usize
    }

    fn subtree_len(self) -> usize {
        ((self.meta >> SUBTREE_SHIFT) & SUBTREE_MASK) as usize
    }

    fn start_line(self) -> u32 {
        ((self.meta >> START_SHIFT) & LINE_MASK) as u32
    }

    fn end_line(self) -> u32 {
        ((self.meta >> END_SHIFT) & LINE_MASK) as u32
    }
}

pub(crate) fn mapped_tokens(il: &Il, interner: &Interner, preorder: &[NodeId]) -> Vec<MappedToken> {
    if preorder.is_empty() || preorder.len() > MAX_TOKENS {
        return Vec::new();
    }
    let mut positions = vec![usize::MAX; il.nodes.len()];
    for (position, node) in preorder.iter().enumerate() {
        positions[node.0 as usize] = position;
    }
    let mut subtree_lens = vec![1usize; preorder.len()];
    for (position, &node) in preorder.iter().enumerate().rev() {
        let mut len = 1usize;
        for &child in il.children(node) {
            let child_position = positions[child.0 as usize];
            if child_position == usize::MAX {
                return Vec::new();
            }
            len = match len.checked_add(subtree_lens[child_position]) {
                Some(len) => len,
                None => return Vec::new(),
            };
        }
        subtree_lens[position] = len;
    }
    preorder
        .iter()
        .enumerate()
        .map(|(position, &node)| {
            let node = il.node(node);
            MappedToken::new(
                node_tag_valued(node.kind, node.payload, interner),
                node.kind,
                node.child_len as usize,
                subtree_lens[position],
                node.span.start_line,
                node.span.end_line,
            )
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

#[derive(Clone, Default)]
struct MatchState {
    nodes: u32,
    observable: u32,
    holes: Vec<((u64, u64), (u64, u64))>,
}

impl MatchState {
    fn add_hole(&mut self, left: MappedToken, right: MappedToken) -> bool {
        let left_key = (left.kind(), left.tag);
        let right_key = (right.kind(), right.tag);
        if self.holes.iter().any(|(known_left, known_right)| {
            (*known_left == left_key && *known_right != right_key)
                || (*known_right == right_key && *known_left != left_key)
        }) {
            return false;
        }
        // A two-cycle is not parameter renaming: it reverses the roles of two values
        // (`primitive -> wrapper` versus `wrapper -> primitive`). Treating that as one
        // abstraction would turn inverse lookup tables into clones.
        if left_key != right_key && self.holes.contains(&(right_key, left_key)) {
            return false;
        }
        if !self.holes.contains(&(left_key, right_key)) {
            if self.holes.len() >= MAX_HOLES {
                return false;
            }
            self.holes.push((left_key, right_key));
        }
        true
    }
}

fn kind(kind: NodeKind) -> u64 {
    kind as u64
}

fn parameter_leaf(token: MappedToken) -> bool {
    token.arity() == 0
        && matches!(
            token.kind(),
            value if value == kind(NodeKind::Var)
                || value == kind(NodeKind::Lit)
                || value == kind(NodeKind::Param)
        )
}

fn same_unit_named_role(token: MappedToken) -> bool {
    token.kind() == kind(NodeKind::Field)
}

fn child_roots(tokens: &[MappedToken], root: usize) -> Option<Vec<usize>> {
    let token = *tokens.get(root)?;
    let end = root.checked_add(token.subtree_len())?;
    if end > tokens.len() {
        return None;
    }
    let mut children = Vec::with_capacity(token.arity());
    let mut next = root + 1;
    for _ in 0..token.arity() {
        let child = *tokens.get(next)?;
        children.push(next);
        next = next.checked_add(child.subtree_len())?;
    }
    (next == end).then_some(children)
}

fn observable_boundary(tokens: &[MappedToken], root: usize) -> bool {
    let token = tokens[root];
    match token.kind() {
        value
            if value == kind(NodeKind::Return)
                || value == kind(NodeKind::Throw)
                || value == kind(NodeKind::Break)
                || value == kind(NodeKind::Continue)
                || value == kind(NodeKind::Call) =>
        {
            true
        }
        value if value == kind(NodeKind::Assign) => (token.arity() > 0 && token.subtree_len() > 1)
            .then_some(root + 1)
            .filter(|&target| target < tokens.len())
            .is_some_and(|target| {
                matches!(
                    tokens[target].kind(),
                    target_kind
                        if target_kind == kind(NodeKind::Field)
                            || target_kind == kind(NodeKind::Index)
                )
            }),
        _ => false,
    }
}

fn match_tree(
    left: &[MappedToken],
    left_root: usize,
    right: &[MappedToken],
    right_root: usize,
    state: &mut MatchState,
    allow_named_role_holes: bool,
) -> bool {
    // `(left, right, parent kind, child position)`; the parent role prevents a direct
    // free-function callee from becoming a parameter hole. A method/constructor receiver may
    // still vary below an exactly named `Field` (e.g. `Basic.new` vs `Readline.new`).
    let mut stack = vec![(left_root, right_root, None, 0usize)];
    while let Some((left_index, right_index, parent_kind, child_position)) = stack.pop() {
        let (Some(&left_token), Some(&right_token)) =
            (left.get(left_index), right.get(right_index))
        else {
            return false;
        };
        state.nodes += 1;
        if observable_boundary(left, left_index) {
            if !observable_boundary(right, right_index) {
                return false;
            }
            state.observable += 1;
        } else if observable_boundary(right, right_index) {
            return false;
        }
        let direct_callee = parent_kind == Some(kind(NodeKind::Call)) && child_position == 0;
        let same_role =
            left_token.kind() == right_token.kind() && left_token.arity() == right_token.arity();
        let named_role_hole = allow_named_role_holes
            && same_role
            && left_token.tag != right_token.tag
            && !direct_callee
            && same_unit_named_role(left_token)
            && state.add_hole(left_token, right_token);
        if same_role && (left_token.tag == right_token.tag || named_role_hole) {
            let Some(left_end) = left_index.checked_add(left_token.subtree_len()) else {
                return false;
            };
            let Some(right_end) = right_index.checked_add(right_token.subtree_len()) else {
                return false;
            };
            let (mut left_child, mut right_child) = (left_index + 1, right_index + 1);
            let children_start = stack.len();
            for position in 0..left_token.arity() {
                let (Some(left_node), Some(right_node)) =
                    (left.get(left_child), right.get(right_child))
                else {
                    return false;
                };
                stack.push((left_child, right_child, Some(left_token.kind()), position));
                left_child = match left_child.checked_add(left_node.subtree_len()) {
                    Some(next) => next,
                    None => return false,
                };
                right_child = match right_child.checked_add(right_node.subtree_len()) {
                    Some(next) => next,
                    None => return false,
                };
            }
            if left_child != left_end || right_child != right_end {
                return false;
            }
            stack[children_start..].reverse();
            continue;
        }
        if direct_callee
            || !parameter_leaf(left_token)
            || !parameter_leaf(right_token)
            || !state.add_hole(left_token, right_token)
        {
            return false;
        }
    }
    true
}

fn overlaps(span: (u32, u32), constraint: LineSpan) -> bool {
    span.0 <= constraint.end_line && constraint.start_line <= span.1
}

fn candidate(
    left_span: (u32, u32),
    right_span: (u32, u32),
    state: &MatchState,
    left_constraint: LineSpan,
    right_constraint: LineSpan,
    complete_exit_suffix: bool,
) -> Option<ConnectedWitness> {
    let overlaps_seed =
        overlaps(left_span, left_constraint) && overlaps(right_span, right_constraint);
    let substantial = state.nodes >= MIN_MAPPED_NODES
        || (complete_exit_suffix
            && state.nodes >= MIN_COMPLETE_EXIT_NODES
            && state.observable >= 4);
    if !substantial
        || state.observable == 0
        || left_span.0 == 0
        || right_span.0 == 0
        || left_span.0 > left_span.1
        || right_span.0 > right_span.1
        || (!overlaps_seed && !complete_exit_suffix)
    {
        return None;
    }
    Some(ConnectedWitness {
        left_lines: left_span,
        right_lines: right_span,
        mapped_nodes: state.nodes,
        holes: state.holes.len() as u32,
        complete_exit: complete_exit_suffix,
    })
}

fn better(left: ConnectedWitness, right: ConnectedWitness) -> ConnectedWitness {
    if (
        right.mapped_nodes,
        std::cmp::Reverse(right.holes),
        std::cmp::Reverse(right.left_lines),
    ) > (
        left.mapped_nodes,
        std::cmp::Reverse(left.holes),
        std::cmp::Reverse(left.left_lines),
    ) {
        right
    } else {
        left
    }
}

pub(crate) fn connected_witness(
    left: &[MappedToken],
    right: &[MappedToken],
    left_constraint: LineSpan,
    right_constraint: LineSpan,
) -> Option<ConnectedWitness> {
    if left.is_empty() || right.is_empty() {
        return None;
    }
    let left_blocks = left
        .iter()
        .enumerate()
        .filter_map(|(index, token)| (token.kind() == kind(NodeKind::Block)).then_some(index));
    let right_blocks = right
        .iter()
        .enumerate()
        .filter_map(|(index, token)| (token.kind() == kind(NodeKind::Block)).then_some(index))
        .collect::<Vec<_>>();
    let mut best = None;

    for left_block in left_blocks {
        let Some(left_children) = child_roots(left, left_block) else {
            continue;
        };
        for &right_block in &right_blocks {
            let Some(right_children) = child_roots(right, right_block) else {
                continue;
            };

            // A whole block is the strongest boundary: both entry and exit are explicit.
            let mut whole = MatchState::default();
            if match_tree(left, left_block, right, right_block, &mut whole, false) {
                if let Some(found) = candidate(
                    (left[left_block].start_line(), left[left_block].end_line()),
                    (
                        right[right_block].start_line(),
                        right[right_block].end_line(),
                    ),
                    &whole,
                    left_constraint,
                    right_constraint,
                    false,
                ) {
                    best = Some(best.map_or(found, |current| better(current, found)));
                }
            }

            // Otherwise find one contiguous, order-preserving statement window under the two
            // blocks. No gaps are skipped: disconnected A-B-C mass cannot become a witness.
            for left_offset in 0..left_children.len() {
                for right_offset in 0..right_children.len() {
                    let mut state = MatchState::default();
                    let mut step = 0usize;
                    let mut left_span = (u32::MAX, 0);
                    let mut right_span = (u32::MAX, 0);
                    while left_offset + step < left_children.len()
                        && right_offset + step < right_children.len()
                    {
                        let left_root = left_children[left_offset + step];
                        let right_root = right_children[right_offset + step];
                        let mut extended = state.clone();
                        if !match_tree(left, left_root, right, right_root, &mut extended, false) {
                            break;
                        }
                        state = extended;
                        left_span.0 = left_span.0.min(left[left_root].start_line());
                        left_span.1 = left_span.1.max(left[left_root].end_line());
                        right_span.0 = right_span.0.min(right[right_root].start_line());
                        right_span.1 = right_span.1.max(right[right_root].end_line());
                        if let Some(found) = candidate(
                            left_span,
                            right_span,
                            &state,
                            left_constraint,
                            right_constraint,
                            left_offset + step + 1 == left_children.len()
                                && right_offset + step + 1 == right_children.len(),
                        ) {
                            best = Some(best.map_or(found, |current| better(current, found)));
                        }
                        step += 1;
                    }
                }
            }
        }
    }
    best
}

/// Find one pair of disjoint mapped regions owned by the same enclosing unit.
///
/// Unlike [`connected_witness`], this path has no cross-unit candidate seed. It therefore
/// compares only whole normalized subtrees with an identical tree size/role envelope, then
/// applies the same pair-local mapping proof. A value-varied proposal must expose consistent
/// holes. An exact mapped subtree is retained only when it contains an observable call/effect;
/// ordinary syntax output is ranked first and suppresses a co-located same-unit duplicate.
pub(crate) fn same_unit_witness(tokens: &[MappedToken]) -> Option<ConnectedWitness> {
    if tokens.is_empty() {
        return None;
    }

    let mut roots: BTreeMap<(u64, usize, usize), Vec<usize>> = BTreeMap::new();
    for (index, token) in tokens.iter().copied().enumerate() {
        if token.subtree_len() < MIN_MAPPED_NODES as usize
            || token.start_line() == 0
            || token.start_line() > token.end_line()
            || token.end_line() - token.start_line() + 1 < MIN_SAME_UNIT_LINES
        {
            continue;
        }
        roots
            .entry((token.kind(), token.arity(), token.subtree_len()))
            .or_default()
            .push(index);
    }

    let mut pairs = roots
        .into_iter()
        .filter(|(_, roots)| roots.len() >= 2)
        .flat_map(|((_, _, subtree_len), roots)| {
            let mut pairs = Vec::new();
            for (offset, &left) in roots.iter().enumerate() {
                for &right in &roots[offset + 1..] {
                    let left_span = (tokens[left].start_line(), tokens[left].end_line());
                    let right_span = (tokens[right].start_line(), tokens[right].end_line());
                    if spans_are_disjoint(left_span, right_span) {
                        pairs.push((subtree_len, left, right));
                    }
                }
            }
            pairs
        })
        .collect::<Vec<_>>();
    pairs.sort_unstable_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| {
                tokens[left.1]
                    .start_line()
                    .cmp(&tokens[right.1].start_line())
            })
            .then_with(|| {
                tokens[left.2]
                    .start_line()
                    .cmp(&tokens[right.2].start_line())
            })
    });
    pairs.truncate(MAX_SAME_UNIT_ROOT_PAIRS);

    let mut found = Vec::new();
    for (_, raw_left, raw_right) in pairs {
        let (left_root, right_root) =
            if tokens[raw_left].start_line() <= tokens[raw_right].start_line() {
                (raw_left, raw_right)
            } else {
                (raw_right, raw_left)
            };
        let mut state = MatchState::default();
        let matched = match_tree(tokens, left_root, tokens, right_root, &mut state, true);
        if !matched || (state.holes.is_empty() && state.observable == 0) {
            continue;
        }
        let left_span = (tokens[left_root].start_line(), tokens[left_root].end_line());
        let right_span = (
            tokens[right_root].start_line(),
            tokens[right_root].end_line(),
        );
        found.push(ConnectedWitness {
            left_lines: left_span,
            right_lines: right_span,
            mapped_nodes: state.nodes,
            holes: state.holes.len() as u32,
            complete_exit: false,
        });
    }
    found.sort_unstable_by(|left, right| {
        right
            .mapped_nodes
            .cmp(&left.mapped_nodes)
            .then_with(|| left.holes.cmp(&right.holes))
            .then_with(|| left.left_lines.cmp(&right.left_lines))
            .then_with(|| left.right_lines.cmp(&right.right_lines))
    });
    found.into_iter().next()
}

fn spans_are_disjoint(left: (u32, u32), right: (u32, u32)) -> bool {
    left.1 < right.0 || right.1 < left.0
}

#[cfg(test)]
mod tests;
