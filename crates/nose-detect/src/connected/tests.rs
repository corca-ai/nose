use super::*;
use nose_il::{FileId, Lang};

fn witness(
    source: &str,
    lang: Lang,
    left_name: &str,
    right_name: &str,
) -> Option<ConnectedWitness> {
    let interner = Interner::new();
    let raw = nose_frontend::lower_source(FileId(0), "fixture", source.as_bytes(), lang, &interner)
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
    Visitor v = new Visitor() { void visit(Node n) { seen.add(n.id()); trace.record(n); audit.record(n); count.increment(); } };
    graph.add(1, 2); graph.add(2, 3); run(v); assertOrder(1, 2, 3);
  }
  void second() {
    Visitor v = new Visitor() { void visit(Node n) { seen.add(n.id()); trace.record(n); audit.record(n); count.increment(); } };
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
