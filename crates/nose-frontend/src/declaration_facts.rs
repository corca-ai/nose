//! AST-derived declaration facts for the CLI's declaration-run classifier.
//!
//! Four hardening waves of a text line-grammar (experiments §BY/§BZ/§CA/§CB)
//! kept leaking payload-validation holes because line text is the wrong
//! abstraction level — the parser already knows which statements are
//! import/use/include wiring. This module exposes that knowledge as per-line
//! facts so the classifier's claim ("provably only declarations") rests on the
//! grammar, not on regex-shaped approximations of it.
//!
//! Fail-open posture is inherited from the parser: subtrees containing ERROR
//! nodes are never marked as declarations (tree-sitter's error tolerance is
//! exactly why "the file parsed" proves nothing — §CA), and any named leaf
//! outside a declaration/comment poisons its lines as code.

use crate::lower::{grammar, is_trivia, parse};
use tree_sitter::Node;

/// Per-line classification of one source file, 1-based and inclusive.
pub struct DeclarationFacts {
    /// Lines carrying import/use/include/package wiring statements.
    declaration: Vec<(u32, u32)>,
    /// Lines carrying comments (inert on a declaration run).
    comment: Vec<(u32, u32)>,
    /// Lines where any OTHER named code starts or continues — one poisoned
    /// line disqualifies a span no matter what else covers it.
    code: Vec<(u32, u32)>,
}

impl DeclarationFacts {
    fn mark(ranges: &mut Vec<(u32, u32)>, node: Node) {
        let start = node.start_position().row as u32 + 1;
        let end_pos = node.end_position();
        // A node whose text ends in a newline "ends" at column 0 of the NEXT
        // row; counting that row would over-claim the following line.
        let mut end = end_pos.row as u32 + 1;
        if end_pos.column == 0 && end > start {
            end -= 1;
        }
        ranges.push((start, end));
    }

    fn covers(ranges: &[(u32, u32)], line: u32) -> bool {
        ranges.iter().any(|&(s, e)| s <= line && line <= e)
    }

    pub fn is_declaration_line(&self, line: u32) -> bool {
        Self::covers(&self.declaration, line)
    }

    pub fn is_comment_line(&self, line: u32) -> bool {
        Self::covers(&self.comment, line)
    }

    pub fn is_code_line(&self, line: u32) -> bool {
        Self::covers(&self.code, line)
    }
}

/// Compute declaration facts for one file, selected by extension (the same
/// vocabulary `nose scan` discovers by). Returns `None` for extensions without
/// a standalone grammar here — notably the embedded-script containers
/// (`vue`/`svelte`/`html`), which fail open in the classifier (zero corpus
/// presence on the declaration surface, measured §BY).
pub fn declaration_facts(ext: &str, src: &str) -> Option<DeclarationFacts> {
    let (key, lang_fn): (u16, fn() -> tree_sitter::Language) = match ext {
        "py" | "pyi" => (grammar::PYTHON, || tree_sitter_python::LANGUAGE.into()),
        "js" | "jsx" | "mjs" | "cjs" => (grammar::JAVASCRIPT, || {
            tree_sitter_javascript::LANGUAGE.into()
        }),
        "ts" | "mts" | "cts" => (grammar::TYPESCRIPT, || {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        }),
        "tsx" => (grammar::TSX, || tree_sitter_typescript::LANGUAGE_TSX.into()),
        "go" => (grammar::GO, || tree_sitter_go::LANGUAGE.into()),
        "rs" => (grammar::RUST, || tree_sitter_rust::LANGUAGE.into()),
        "java" => (grammar::JAVA, || tree_sitter_java::LANGUAGE.into()),
        "c" | "h" => (grammar::C, || tree_sitter_c::LANGUAGE.into()),
        "rb" => (grammar::RUBY, || tree_sitter_ruby::LANGUAGE.into()),
        _ => return None,
    };
    // A UTF-8 BOM (Windows-authored files) makes tree-sitter emit an error
    // leaf in the line-1 region, which poisoned the first declaration and
    // flipped import-only families onto the default surface (coevo S4-C3).
    // The main IL-lowering path already tolerates it; strip it here too, on a
    // single owned buffer that also normalizes the missing-EOF-newline case
    // (C preprocessor directives read a missing final newline as MISSING).
    let stripped = src.strip_prefix('\u{feff}').unwrap_or(src);
    let owned;
    let src = if stripped.ends_with('\n') && std::ptr::eq(stripped, src) {
        src
    } else {
        owned = if stripped.ends_with('\n') {
            stripped.to_string()
        } else {
            format!("{stripped}\n")
        };
        &owned
    };
    let tree = parse(key, lang_fn, src.as_bytes()).ok()?;
    let mut facts = DeclarationFacts {
        declaration: Vec::new(),
        comment: Vec::new(),
        code: Vec::new(),
    };
    walk(tree.root_node(), key, src.as_bytes(), &mut facts);
    Some(facts)
}

fn walk(node: Node, key: u16, src: &[u8], facts: &mut DeclarationFacts) {
    if is_trivia(node.kind()) {
        DeclarationFacts::mark(&mut facts.comment, node);
        return;
    }
    // A subtree containing a parse ERROR can never prove anything.
    if !node.has_error() && is_declaration(node, key, src) {
        DeclarationFacts::mark(&mut facts.declaration, node);
        return;
    }
    if node.named_child_count() == 0 {
        if node.is_named() {
            DeclarationFacts::mark(&mut facts.code, node);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, key, src, facts);
    }
}

fn is_declaration(node: Node, key: u16, src: &[u8]) -> bool {
    let kind = node.kind();
    match key {
        grammar::PYTHON => matches!(
            kind,
            "import_statement" | "import_from_statement" | "future_import_statement"
        ),
        grammar::JAVASCRIPT | grammar::TYPESCRIPT | grammar::TSX => match kind {
            "import_statement" => true,
            // Re-exports are wiring only with a `from` source.
            "export_statement" => node.child_by_field_name("source").is_some(),
            // CommonJS: `const x = require('lit');` — exactly one declarator,
            // a bare require call with one string-literal argument, and a
            // binding target that EXECUTES NOTHING. The early-return mark
            // skips the subtree, so a destructuring default/computed key
            // (`const { a = steal() } = require('lit')`) would smuggle a call
            // onto the import's line undetected (coevo S4-C1) — the binding
            // pattern must therefore be call-free.
            "lexical_declaration" | "variable_declaration" => {
                let mut cursor = node.walk();
                let declarators: Vec<Node> = node
                    .named_children(&mut cursor)
                    .filter(|n| n.kind() == "variable_declarator")
                    .collect();
                declarators.len() == 1
                    && node.named_child_count() == 1
                    && declarators[0]
                        .child_by_field_name("value")
                        .is_some_and(|value| is_require_call(value, src))
                    && declarators[0]
                        .child_by_field_name("name")
                        .is_none_or(|name| !subtree_executes(name))
            }
            _ => false,
        },
        grammar::GO => matches!(kind, "import_declaration" | "package_clause"),
        grammar::RUST => match kind {
            "use_declaration" | "extern_crate_declaration" => true,
            // `mod x;` is wiring; `mod x { … }` is code.
            "mod_item" => node.child_by_field_name("body").is_none(),
            _ => false,
        },
        grammar::JAVA => matches!(kind, "import_declaration" | "package_declaration"),
        grammar::C => match kind {
            "preproc_include" => true,
            // `#pragma once` is header wiring; other pragmas are semantics.
            "preproc_call" => node
                .utf8_text(src)
                .is_ok_and(|text| text.trim() == "#pragma once"),
            _ => false,
        },
        grammar::RUBY => {
            kind == "call"
                && node
                    .child_by_field_name("method")
                    .and_then(|m| m.utf8_text(src).ok())
                    .is_some_and(|m| m == "require" || m == "require_relative")
                && node.child_by_field_name("receiver").is_none()
                && lone_string_arguments(node, "argument_list")
                // `require('x') { launch() }` carries a block that executes
                // (coevo S4-C1) — a bare require has none.
                && node.child_by_field_name("block").is_none()
        }
        _ => false,
    }
}

/// Does this subtree contain a node that EXECUTES code (a call, an awaited
/// expression, a defined function body)? Used to reject call-shaped
/// declaration entries whose binding target smuggles execution — the
/// early-return mark would otherwise skip the subtree. Bounded DAG walk.
fn subtree_executes(node: Node) -> bool {
    const EXECUTES: &[&str] = &[
        "call_expression",
        "call",
        "await_expression",
        "arrow_function",
        "function_expression",
        "function",
        "new_expression",
        "yield_expression",
    ];
    if EXECUTES.contains(&node.kind()) {
        return true;
    }
    let mut cursor = node.walk();
    let children: Vec<Node> = node.named_children(&mut cursor).collect();
    children.into_iter().any(subtree_executes)
}

/// `require('lit')` / `require("lit")` with exactly one plain string argument.
fn is_require_call(node: Node, src: &[u8]) -> bool {
    node.kind() == "call_expression"
        && node
            .child_by_field_name("function")
            .and_then(|f| f.utf8_text(src).ok())
            .is_some_and(|f| f == "require")
        && lone_string_arguments(node, "arguments")
}

/// The node's argument list holds exactly one plain string (no interpolation,
/// no expressions riding along).
fn lone_string_arguments(node: Node, list_kind: &str) -> bool {
    let Some(args) = node
        .child_by_field_name("arguments")
        .filter(|args| args.kind() == list_kind || list_kind == "arguments")
    else {
        return false;
    };
    let mut cursor = args.walk();
    let named: Vec<Node> = args.named_children(&mut cursor).collect();
    named.len() == 1
        && named[0].kind() == "string"
        && named[0].named_children(&mut named[0].walk()).all(|part| {
            matches!(
                part.kind(),
                "string_content" | "string_fragment" | "string_start" | "string_end"
            )
        })
}
