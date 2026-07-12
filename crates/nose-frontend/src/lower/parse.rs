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
        parser
            .parse(src, None)
            .ok_or_else(|| anyhow::anyhow!("parse failed"))
    })
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
