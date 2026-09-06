use super::*;

thread_local! {
    /// Per-thread, per-grammar parser cache. `tree_sitter::Parser::new` allocates
    /// the parser's internal stack and lexer caches; recreating one for every
    /// file (corpora run thousands) is pure overhead. Rayon hands each worker its
    /// own thread, so a thread-local pool needs no locking and a grammar's parser
    /// is built at most once per worker.
    static PARSERS: std::cell::RefCell<std::collections::HashMap<u16, tree_sitter::Parser>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Parse `src` with a thread-local parser cached under `key` (which must uniquely
/// identify the grammar — JS/TS/TSX share a crate but need distinct slots).
/// `lang` is only evaluated the first time a thread sees `key`.
pub(crate) fn parse(
    key: u16,
    lang: impl FnOnce() -> tree_sitter::Language,
    src: &[u8],
) -> anyhow::Result<tree_sitter::Tree> {
    with_parser(key, lang, |parser| {
        let tree = parser
            .parse(src, None)
            .ok_or_else(|| anyhow::anyhow!("parse failed"))?;
        check_tree_budget(&tree)?;
        Ok(tree)
    })
}

pub(crate) fn is_clean_c(src: &[u8]) -> bool {
    with_parser(
        grammar::C,
        || tree_sitter_c::LANGUAGE.into(),
        |parser| {
            let mut progress = |state: &tree_sitter::ParseState| {
                if state.has_error() {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            };
            let mut input = |offset, _| src.get(offset..).unwrap_or_default();
            let tree = parser.parse_with_options(
                &mut input,
                None,
                Some(tree_sitter::ParseOptions::new().progress_callback(&mut progress)),
            );
            // A canceled parse is resumable; the next file must start from scratch.
            parser.reset();
            Ok(tree.is_some_and(|tree| {
                !tree.root_node().has_error() && check_tree_budget(&tree).is_ok()
            }))
        },
    )
    .unwrap_or(false)
}

fn with_parser<T>(
    key: u16,
    lang: impl FnOnce() -> tree_sitter::Language,
    run: impl FnOnce(&mut tree_sitter::Parser) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    PARSERS.with(|cell| {
        let mut pool = cell.borrow_mut();
        let parser = match pool.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let mut p = tree_sitter::Parser::new();
                p.set_language(&lang())?;
                e.insert(p)
            }
        };
        run(parser)
    })
}

/// Validate with a cursor before any recursive language lowering runs. A cursor
/// keeps auxiliary memory constant even for hostile nesting or very wide trees.
fn check_tree_budget(tree: &tree_sitter::Tree) -> anyhow::Result<()> {
    // The pinned corpus reaches 5,002 levels in a flat Flow union fixture.
    // Keep room for long operator/call chains while bounding recursive adapters.
    const MAX_DEPTH: usize = 8_192;
    const MAX_NODES: usize = 2_000_000;
    check_tree_limits(tree, MAX_DEPTH, MAX_NODES)
}

fn check_tree_limits(
    tree: &tree_sitter::Tree,
    max_depth: usize,
    max_nodes: usize,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        tree.root_node().descendant_count() <= max_nodes,
        "source syntax nodes exceed analysis limit of {max_nodes}"
    );
    let mut cursor = tree.walk();
    let mut depth = 0;
    loop {
        anyhow::ensure!(
            depth <= max_depth,
            "source syntax depth exceeds analysis limit of {max_depth}"
        );
        // Even a chain cannot be deeper than its descendant count minus one.
        // Tree-sitter stores that count, so ordinary subtrees need no second walk.
        let bounded = cursor.node().descendant_count() - 1 <= max_depth - depth;
        if !bounded && cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Ok(());
            }
            depth -= 1;
        }
    }
}

/// Stable grammar keys for the thread-local parser pool. JS/TS/TSX are distinct.
pub(crate) mod grammar {
    pub(crate) const PYTHON: u16 = 0;
    pub(crate) const JAVASCRIPT: u16 = 1;
    pub(crate) const TYPESCRIPT: u16 = 2;
    pub(crate) const TSX: u16 = 3;
    pub(crate) const GO: u16 = 4;
    pub(crate) const RUST: u16 = 5;
    pub(crate) const JAVA: u16 = 6;
    pub(crate) const C: u16 = 7;
    pub(crate) const RUBY: u16 = 8;
    pub(crate) const CSS: u16 = 9;
    pub(crate) const HTML: u16 = 10;
    pub(crate) const SWIFT: u16 = 11;
}

/// Comment / trivia node kinds across the supported grammars.
pub(crate) fn is_trivia(kind: &str) -> bool {
    matches!(
        kind,
        "comment" | "line_comment" | "block_comment" | "multiline_comment" | "hash_bang_line"
    )
}

/// Binary-operator tokens shared by ~every C-family language. Per-language
/// frontends delegate here and then handle their own extras (JS `===`/`**`/`??`,
/// Go `&^`, …) — so the universal operator table lives in one place.
pub(crate) fn common_bin_op(text: &str) -> Option<Op> {
    Some(match text {
        "+" => Op::Add,
        "-" => Op::Sub,
        "*" => Op::Mul,
        "/" => Op::Div,
        "%" => Op::Mod,
        // Exponentiation in the languages that spell it `**` (Python/JS/Ruby);
        // the C-family grammars never produce it as a binary operator.
        "**" => Op::Pow,
        "==" => Op::Eq,
        "!=" => Op::Ne,
        "<" => Op::Lt,
        "<=" => Op::Le,
        ">" => Op::Gt,
        ">=" => Op::Ge,
        "&&" => Op::And,
        "||" => Op::Or,
        "&" => Op::BitAnd,
        "|" => Op::BitOr,
        "^" => Op::BitXor,
        "<<" => Op::Shl,
        ">>" => Op::Shr,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_token_errors_reach_progress_before_the_remaining_source() {
        let source = format!(
            "struct broken {{ long : 64; }};\n{}",
            "typedef unsigned long item;\n".repeat(20_000)
        );
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .unwrap();
        let mut first_error = None;
        let mut progress = |state: &tree_sitter::ParseState| {
            if state.has_error() {
                first_error.get_or_insert(state.current_byte_offset());
            }
            std::ops::ControlFlow::<()>::Continue(())
        };
        let mut input = |offset, _| source.as_bytes().get(offset..).unwrap_or_default();
        let tree = parser
            .parse_with_options(
                &mut input,
                None,
                Some(tree_sitter::ParseOptions::new().progress_callback(&mut progress)),
            )
            .unwrap();
        assert!(tree.root_node().has_error());
        assert!(first_error.is_some_and(|offset| offset < source.len() / 10));
    }

    #[test]
    fn clean_c_admission_matches_full_parsing_and_resets_canceled_state() {
        let inputs = [
            "struct broken { long : 64; };",
            "typedef int class; struct namespace { class x; int (*namespace)(int); };",
            "int broken( { return ;",
            "int recovered(int x) { return x * x + 1; }",
            "#ifndef H\n#define H\nextern int f(int);\n#endif\n",
            "",
        ];
        for _ in 0..3 {
            for source in inputs {
                let expected = parse(
                    grammar::C,
                    || tree_sitter_c::LANGUAGE.into(),
                    source.as_bytes(),
                )
                .is_ok_and(|tree| !tree.root_node().has_error());
                assert_eq!(is_clean_c(source.as_bytes()), expected, "{source}");
            }
        }
    }

    #[test]
    fn clean_c_admission_keeps_the_syntax_depth_budget() {
        let source = format!(
            "int f(void) {{ return {}0{}; }}",
            "(".repeat(8_200),
            ")".repeat(8_200)
        );
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        assert!(!tree.root_node().has_error());
        assert!(check_tree_budget(&tree).is_err());
        assert!(!is_clean_c(source.as_bytes()));
        assert!(is_clean_c(b"int valid(void) { return 1; }"));
    }

    #[test]
    fn subtree_bounds_match_exhaustive_depth_and_node_limits() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        for source in [
            "",
            "// comment\nlet x = a + b;",
            "function f(x) { return (((x))); }",
            "const x = [1, 2, 3, 4, 5];",
            "const broken = (((;",
        ] {
            let tree = parser.parse(source, None).unwrap();
            let mut pending = vec![(tree.root_node(), 0)];
            let (mut nodes, mut depth) = (0, 0);
            while let Some((node, level)) = pending.pop() {
                nodes += 1;
                depth = depth.max(level);
                let mut cursor = node.walk();
                pending.extend(node.children(&mut cursor).map(|child| (child, level + 1)));
            }
            assert_eq!(tree.root_node().descendant_count(), nodes);
            for max_depth in 0..=depth + 1 {
                for max_nodes in [0, nodes - 1, nodes, nodes + 1] {
                    assert_eq!(
                        check_tree_limits(&tree, max_depth, max_nodes).is_ok(),
                        depth <= max_depth && nodes <= max_nodes,
                        "{source:?}"
                    );
                }
            }
        }
    }
}
