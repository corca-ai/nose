use super::*;

#[test]
fn trailing_continue_guard_preserves_evaluation_issue_1005() {
    let i = Interner::new();
    for condition in ["seen.append(x)", "predicate(x)", "seen"] {
        let source = format!("def f(xs, seen):\n    total = 0\n    for x in xs:\n        total += x\n        if {condition}:\n            continue\n    return total\n");
        let bare = "def f(xs, seen):\n    total = 0\n    for x in xs:\n        total += x\n    return total\n";
        assert_ne!(
            unit_hash(&i, &source, Lang::Python),
            unit_hash(&i, bare, Lang::Python),
            "condition evaluation/truthiness must survive: {condition}"
        );
        let il =
            nose_frontend::lower_source(FileId(0), "t.py", source.as_bytes(), Lang::Python, &i)
                .unwrap();
        let normalized = normalize(&il, &i, &NormalizeOptions::default());
        assert!(
            count_nodes(
                &normalized,
                first_func(&normalized),
                Some(nose_il::NodeKind::If)
            ) > 0
        );
    }
}

#[test]
fn string_delimiters_do_not_consume_content_issue_1003() {
    let i = Interner::new();
    for (lang, quoted, plain, equivalent) in [
        (
            Lang::Python,
            "def f():\n    return \"'prefix:'\"\n",
            "def f():\n    return \"prefix:\"\n",
            "def f():\n    return 'prefix:'\n",
        ),
        (
            Lang::JavaScript,
            "function f() { return \"'prefix:'\"; }",
            "function f() { return \"prefix:\"; }",
            "function f() { return 'prefix:'; }",
        ),
        (
            Lang::Ruby,
            "def f()\n  return \"'prefix:'\"\nend",
            "def f()\n  return \"prefix:\"\nend",
            "def f()\n  return 'prefix:'\nend",
        ),
    ] {
        assert_ne!(
            value_fp(&i, quoted, lang),
            value_fp(&i, plain, lang),
            "{lang:?}: quotes inside the literal are data"
        );
        assert_eq!(
            value_fp(&i, plain, lang),
            value_fp(&i, equivalent, lang),
            "{lang:?}: delimiter spelling alone is incidental"
        );
    }
}

#[test]
fn prefixed_strings_and_escaped_fragments_do_not_alias_source_text() {
    let i = Interner::new();
    for (lang, left, right) in [
        (
            Lang::Python,
            "def f():\n    return r\"x\"\n",
            "def f():\n    return '''r\"x\"'''\n",
        ),
        (
            Lang::Ruby,
            "def f()\n  return \"a\\nb\"\nend",
            "def f()\n  return \"ab\"\nend",
        ),
        (
            Lang::JavaScript,
            "function f(x) { return `\"${x}\"`; }",
            "function f(x) { return `${x}`; }",
        ),
    ] {
        assert_ne!(
            value_fp(&i, left, lang),
            value_fp(&i, right, lang),
            "{lang:?}: source syntax cannot become literal value content"
        );
    }
}

#[test]
fn ruby_symbol_is_not_a_string_value() {
    let i = Interner::new();
    let symbol = value_fp(&i, "def f()\n  return :name\nend", Lang::Ruby);
    assert_ne!(
        symbol,
        value_fp(&i, "def f()\n  return 'name'\nend", Lang::Ruby)
    );
    assert_eq!(
        symbol,
        value_fp(&i, "def f()\n  return :'name'\nend", Lang::Ruby)
    );
    assert_ne!(
        value_fp(&i, "def f(x)\n  return :\"#{x}\"\nend", Lang::Ruby),
        value_fp(&i, "def f(x)\n  return x\nend", Lang::Ruby)
    );
    assert_ne!(
        value_fp(&i, "def f()\n  v = :name\n  return v\nend", Lang::Ruby),
        value_fp(&i, "def f()\n  v = 'name'\n  return v\nend", Lang::Ruby)
    );
}
