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

const MAX_TOKENS: usize = 2_000;
const MIN_MAPPED_NODES: u32 = 20;
const MIN_COMPLETE_EXIT_NODES: u32 = 18;
const MAX_HOLES: usize = 8;

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
        value if value == kind(NodeKind::Assign) => child_roots(tokens, root)
            .and_then(|children| children.first().copied())
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
        if left_token.kind() == right_token.kind()
            && left_token.arity() == right_token.arity()
            && left_token.tag == right_token.tag
        {
            let (Some(left_children), Some(right_children)) = (
                child_roots(left, left_index),
                child_roots(right, right_index),
            ) else {
                return false;
            };
            for (position, (&left_child, &right_child)) in
                left_children.iter().zip(&right_children).enumerate().rev()
            {
                stack.push((left_child, right_child, Some(left_token.kind()), position));
            }
            continue;
        }
        let direct_callee = parent_kind == Some(kind(NodeKind::Call)) && child_position == 0;
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
            if match_tree(left, left_block, right, right_block, &mut whole) {
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
            for (left_offset, &left_child) in left_children.iter().enumerate() {
                for (right_offset, &right_child) in right_children.iter().enumerate() {
                    let mut state = MatchState::default();
                    let mut step = 0usize;
                    while left_offset + step < left_children.len()
                        && right_offset + step < right_children.len()
                    {
                        let left_root = left_children[left_offset + step];
                        let right_root = right_children[right_offset + step];
                        let mut extended = state.clone();
                        if !match_tree(left, left_root, right, right_root, &mut extended) {
                            break;
                        }
                        state = extended;
                        let left_span = (left[left_child].start_line(), left[left_root].end_line());
                        let right_span = (
                            right[right_child].start_line(),
                            right[right_root].end_line(),
                        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use nose_il::{FileId, Lang};

    fn witness(
        source: &str,
        lang: Lang,
        left_name: &str,
        right_name: &str,
    ) -> Option<ConnectedWitness> {
        let interner = Interner::new();
        let raw =
            nose_frontend::lower_source(FileId(0), "fixture", source.as_bytes(), lang, &interner)
                .expect("lower connected fixture");
        let il = nose_normalize::normalize(&raw, &interner, &Default::default());
        let tokens = |name: &str| {
            let unit = il
                .units
                .iter()
                .find(|unit| {
                    unit.name
                        .is_some_and(|symbol| interner.resolve(symbol) == name)
                })
                .unwrap_or_else(|| panic!("unit named {name}"));
            let mut preorder = Vec::new();
            collect_preorder(&il, unit.root, &mut preorder);
            let span = il.node(unit.root).span;
            (
                mapped_tokens(&il, &interner, &preorder),
                LineSpan::new(span.start_line, span.end_line),
            )
        };
        let (left, left_span) = tokens(left_name);
        let (right, right_span) = tokens(right_name);
        connected_witness(&left, &right, left_span, right_span)
    }

    fn collect_preorder(il: &Il, root: NodeId, out: &mut Vec<NodeId>) {
        out.push(root);
        for &child in il.children(root) {
            collect_preorder(il, child, out);
        }
    }

    #[test]
    fn complete_effectful_exit_suffix_is_a_connected_witness() {
        let source = r#"
package fixture
func unsigned(code int, c *context) error {
  switch code { case 1: c.read(1); case 2: c.read(2); default: panic("u") }
  if c.err != nil { return c.err }
  c.stack = append(c.stack, c.value)
  return nil
}
func signed(code int, c *context) error {
  switch code { case 3: c.convert(c.read(1)); case 4: c.convert(c.read(2)); default: panic("s") }
  if c.err != nil { return c.err }
  c.stack = append(c.stack, c.value)
  return nil
}
"#;
        let found = witness(source, Lang::Go, "unsigned", "signed")
            .expect("the complete common exit suffix should map");
        assert!(found.mapped_nodes >= MIN_COMPLETE_EXIT_NODES);
        assert!(found.left_lines.1 >= 7 && found.right_lines.1 >= 13);
    }

    #[test]
    fn locally_bound_anonymous_recorder_is_visible_as_one_region() {
        let source = r#"
class SearchTest {
  void first() {
    Visitor v = new Visitor() { void visit(Node n) { seen.add(n.id()); trace.record(n); } };
    graph.add(1, 2); graph.add(2, 3); run(v); assertOrder(1, 2, 3);
  }
  void second() {
    Visitor v = new Visitor() { void visit(Node n) { seen.add(n.id()); trace.record(n); } };
    graph.add(4, 5); graph.add(5, 6); graph.add(6, 7); run(v); assertOrder(4, 5, 6, 7);
  }
}
"#;
        let found = witness(source, Lang::Java, "first", "second")
            .expect("the anonymous recorder body should map independently of fixtures");
        assert!(found.left_lines.1 - found.left_lines.0 <= 2);
    }

    #[test]
    fn consistently_mapped_receivers_can_parameterize_a_call_sequence() {
        let source = r#"
class Runner
  def basic(input)
    shell = Basic.new(input)
    shell.prepare(input)
    shell.configure(input)
    shell.execute(input)
    shell.finish(input)
    shell.result(input)
  end
  def readline(input)
    shell = Readline.new(input)
    shell.prepare(input)
    shell.configure(input)
    shell.execute(input)
    shell.finish(input)
    shell.result(input)
  end
end
"#;
        let found = witness(source, Lang::Ruby, "basic", "readline")
            .expect("one consistent receiver mapping should be allowed");
        assert!(found.holes <= MAX_HOLES as u32);
    }

    #[test]
    fn complete_test_phases_with_value_parameters_remain_connected() {
        let source = r#"
class ListenerTest {
  void found() {
    prepare("coffee"); prepare("soda"); invoke("soda");
    verify(listener); assertName("mock"); assertCount(2); assertFound(value); finish();
  }
  void missing() {
    prepare("coffee"); invoke("soda");
    verify(listener); assertName("mock"); assertCount(1); assertFound(none); finish();
  }
}
"#;
        assert!(
            witness(source, Lang::Java, "found", "missing").is_some(),
            "the ordered invocation/verification phase must survive setup variation"
        );
    }

    #[test]
    fn one_contiguous_statement_window_can_exclude_unrelated_neighbors() {
        let source = r#"
fn first(state: &mut State) {
    prepare_a(state);
    if state.ready { state.open(); state.scan(); state.record(); state.close(); }
    finish_a(state);
}
fn second(state: &mut State) {
    prepare_b(state); audit_b(state);
    if state.ready { state.open(); state.scan(); state.record(); state.close(); }
    finish_b(state);
}
"#;
        let found = witness(source, Lang::Rust, "first", "second")
            .expect("the common control block should form one window");
        assert!(found.left_lines.0 > 2 && found.right_lines.0 > 7);
    }

    #[test]
    fn inverse_lookup_tables_do_not_map() {
        let source = r#"
class Types {
  Object wrap(Object type) {
    if (type == int.class) return Integer.class;
    if (type == float.class) return Float.class;
    if (type == byte.class) return Byte.class;
    if (type == long.class) return Long.class;
    return type;
  }
  Object unwrap(Object type) {
    if (type == Integer.class) return int.class;
    if (type == Float.class) return float.class;
    if (type == Byte.class) return byte.class;
    if (type == Long.class) return long.class;
    return type;
  }
}
"#;
        let found = witness(source, Lang::Java, "wrap", "unwrap");
        assert!(found.is_none(), "inverse witness: {found:?}");
    }

    #[test]
    fn different_scalar_and_iterable_callees_do_not_map() {
        let source = r#"
def follow(response, selector, headers, cookies, meta, encoding, priority):
    url = selector.get()
    validate(url)
    return response.follow(url, headers=headers, cookies=cookies, meta=meta, encoding=encoding, priority=priority)

def follow_all(response, selector, headers, cookies, meta, encoding, priority):
    urls = selector.getall()
    validate_all(urls)
    return response.follow_all(urls, headers=headers, cookies=cookies, meta=meta, encoding=encoding, priority=priority)
"#;
        assert!(witness(source, Lang::Python, "follow", "follow_all").is_none());
    }

    #[test]
    fn return_value_and_in_place_mutation_do_not_map() {
        let source = r#"
fn multiplied(values: &[u64], factor: u64) -> Vec<u64> {
    let mut out = Vec::new();
    for value in values { out.push(value * factor); }
    verify(&out); audit(&out); record(&out);
    out
}
fn multiply_in_place(values: &mut [u64], factor: u64) {
    for value in values { *value *= factor; }
    verify(values); audit(values); record(values);
}
"#;
        assert!(witness(source, Lang::Rust, "multiplied", "multiply_in_place").is_none());
    }
}
