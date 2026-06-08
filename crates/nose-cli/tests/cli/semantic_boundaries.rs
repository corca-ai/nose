use super::*;

#[test]
fn scan_mode_semantic_rejects_cross_receiver_field_state() {
    let dir = std::env::temp_dir().join(format!("nose_field_place_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("read_other.py"),
        "def f(a, b):\n    a.x = 7\n    return b.x\n",
    )
    .unwrap();
    fs::write(
        dir.join("read_written.py"),
        "def f(a, b):\n    a.x = 7\n    return a.x\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "1",
        "--min-lines",
        "1",
        "--format",
        "json",
    ]));
    assert!(
        scan_families(&json).is_empty(),
        "same-named fields on different receivers must not report as exact semantic clones: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Regression (semantic-kernel migration): empty `java.util` collection constructors
/// (`new ArrayList<>()` / `new LinkedList<>()`) authorized only via a wildcard
/// `import java.util.*;` must still canonicalize to an empty collection and form a
/// semantic family, exactly like the explicit-import form. The migration moved Java
/// collection constructors onto LibraryApi occurrence evidence in the value graph but
/// left the exact-safe gate (`strict_exact_safe_call`) admitting the constructor only
/// when its callee name happened to be a proven top-level binding — which an explicit
/// import incidentally provides but a wildcard import does not.
#[test]
fn scan_mode_semantic_matches_wildcard_imported_java_empty_collection_constructors() {
    let dir = std::env::temp_dir().join(format!("nose_java_wildcard_ctor_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("A.java"),
        "import java.util.*;\nclass A {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new ArrayList<>();\n    r.add(a);\n    r.add(b);\n    return r;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("B.java"),
        "import java.util.*;\nclass B {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new LinkedList<>();\n    r.add(a);\n    r.add(b);\n    return r;\n  }\n}\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    assert!(
        family_contains_all(&json, &["A.java", "B.java"]),
        "wildcard-imported empty java.util collection constructors with identical appends must form one semantic family: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Soundness guard for the regression fix above: making the wildcard constructor
/// exact-safe must not over-merge. Two wildcard-imported builders that append the same
/// elements in a DIFFERENT order are not behaviorally equivalent and must not form a
/// semantic family.
#[test]
fn scan_mode_semantic_rejects_wildcard_java_collections_with_divergent_append_order() {
    let dir = std::env::temp_dir().join(format!("nose_java_wildcard_neg_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("A.java"),
        "import java.util.*;\nclass A {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new ArrayList<>();\n    r.add(a);\n    r.add(b);\n    return r;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("B.java"),
        "import java.util.*;\nclass B {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new LinkedList<>();\n    r.add(b);\n    r.add(a);\n    return r;\n  }\n}\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    assert!(
        !family_contains_all(&json, &["A.java", "B.java"]),
        "builders appending the same elements in different order must not be exact semantic clones: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Soundness (semantic-kernel binding-domain evidence): a parameter whose binding is
/// reassigned must not retain its declared-domain proof. `y: list[int]; y = z` makes `y`
/// a list when `z` is a list but a string when `z` is a string — and `e in y` is element
/// membership for the list yet substring membership for the string, so the two are NOT
/// behaviorally equivalent and must not form one exact semantic family. The Cid form of
/// `domain_evidence_for_var_reference` (the form that runs on the alpha-renamed normalized
/// IL) previously returned the stale parameter domain, while the Name form already guards
/// reassignment — an asymmetric fail-open that admitted the unsound merge.
#[test]
fn scan_mode_semantic_rejects_reassigned_param_with_stale_collection_domain() {
    let dir = std::env::temp_dir().join(format!("nose_stale_domain_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // `y` reassigned to a list: `e in y` is list element membership.
    fs::write(
        dir.join("list_membership.py"),
        "def memb(e, y: list[int], z: list[int]):\n    y = z\n    return e in y\n",
    )
    .unwrap();
    // `y` reassigned to a str: `e in y` is substring membership — NOT equivalent.
    fs::write(
        dir.join("substring_membership.py"),
        "def memb(e, y: list[int], z: str):\n    y = z\n    return e in y\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
    ]));
    assert!(
        !family_contains_all(&json, &["list_membership.py", "substring_membership.py"]),
        "a reassigned parameter's declared domain is not proof of the current receiver's domain: list membership and substring membership must not merge: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Soundness (semantic-kernel async protocol boundary): `await x` is not
/// equivalent to `x` until a language/protocol contract proves that erasure.
/// The old lowering stripped `await`, which made a sync function and an async
/// function form an exact semantic family even though Promise/thenable
/// scheduling and error propagation have different observable semantics.
#[test]
fn scan_mode_semantic_rejects_unproven_js_await_sync_convergence() {
    let dir = std::env::temp_dir().join(format!("nose_js_await_boundary_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("sync.js"),
        "function id(x) {\n  return x + 1;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("async.js"),
        "async function idAsync(x) {\n  return await x + 1;\n}\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    assert!(
        !family_contains_all(&json, &["sync.js", "async.js"]),
        "await must not be erased into a sync exact semantic family without protocol evidence: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Same async protocol boundary for Python: `await x` is a coroutine protocol
/// operation, not a plain value read unless a future contract proves it.
#[test]
fn scan_mode_semantic_rejects_unproven_python_await_sync_convergence() {
    let dir = std::env::temp_dir().join(format!("nose_py_await_boundary_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("sync.py"), "def id(x):\n    return x + 1\n").unwrap();
    fs::write(
        dir.join("async.py"),
        "async def id_async(x):\n    return await x + 1\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    assert!(
        !family_contains_all(&json, &["sync.py", "async.py"]),
        "await must not be erased into a sync exact semantic family without protocol evidence: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Rust `.await` and `async {}` are Future protocol operations, not plain
/// wrappers around the body. Exact sync/async convergence requires future
/// protocol proof that is not modeled yet.
#[test]
fn scan_mode_semantic_rejects_unproven_rust_await_sync_convergence() {
    let dir = std::env::temp_dir().join(format!("nose_rs_await_boundary_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("sync.rs"), "fn id(x: i32) -> i32 { x + 1 }\n").unwrap();
    fs::write(
        dir.join("async.rs"),
        "async fn id_async(x: i32) -> i32 { async move { x + 1 }.await }\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    assert!(
        !family_contains_all(&json, &["sync.rs", "async.rs"]),
        "Rust async/await must not be erased into a sync exact semantic family without future protocol evidence: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Go concurrency and channel operations have synchronization/scheduling
/// semantics. They must not be erased into ordinary calls or value reads until
/// a language protocol contract proves the required demand/effect obligations.
#[test]
fn scan_mode_semantic_rejects_unproven_go_concurrency_protocol_convergence() {
    let dir =
        std::env::temp_dir().join(format!("nose_go_protocol_boundary_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("direct_call.go"),
        "package p\nfunc direct(x int) { record(x) }\n",
    )
    .unwrap();
    fs::write(
        dir.join("goroutine.go"),
        "package p\nfunc goroutine(x int) { go record(x) }\n",
    )
    .unwrap();
    fs::write(
        dir.join("deferred.go"),
        "package p\nfunc deferred(x int) { defer record(x) }\n",
    )
    .unwrap();
    fs::write(
        dir.join("plain_value.go"),
        "package p\nfunc plain(ch int) int { return ch }\n",
    )
    .unwrap();
    fs::write(
        dir.join("channel_receive.go"),
        "package p\nfunc receive(ch chan int) int { return <-ch }\n",
    )
    .unwrap();
    fs::write(
        dir.join("channel_status.go"),
        "package p\nfunc status(ch chan int) bool { _, ok := <-ch; return ok }\n",
    )
    .unwrap();
    fs::write(
        dir.join("constant_status.go"),
        "package p\nfunc constant(ch chan int) bool { return false }\n",
    )
    .unwrap();
    fs::write(
        dir.join("send_a.go"),
        "package p\nfunc sendA(ch chan int, x int) { ch <- x }\n",
    )
    .unwrap();
    fs::write(
        dir.join("send_b.go"),
        "package p\nfunc sendB(ch chan int, x int) { ch <- x }\n",
    )
    .unwrap();
    fs::write(
        dir.join("select_receive.go"),
        "package p\nfunc selectReceive(ch chan int) int { select { case v := <-ch: return v; default: return 0 } }\n",
    )
    .unwrap();
    fs::write(
        dir.join("if_receive.go"),
        "package p\nfunc ifReceive(ch chan int) int { v := <-ch; if v != 0 { return v }; return 0 }\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    for pair in [
        ["direct_call.go", "goroutine.go"],
        ["direct_call.go", "deferred.go"],
        ["plain_value.go", "channel_receive.go"],
        ["channel_status.go", "constant_status.go"],
        ["send_a.go", "send_b.go"],
        ["select_receive.go", "if_receive.go"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "Go protocol boundary must not be erased into an ordinary exact semantic family for {pair:?}: {json}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

/// Python comprehension source surfaces are not interchangeable. A list
/// comprehension is eager and materialized, a generator expression is lazy and
/// one-shot, and a set comprehension deduplicates and is unordered.
#[test]
fn scan_mode_semantic_rejects_unproven_python_comprehension_surface_convergence() {
    let dir = std::env::temp_dir().join(format!(
        "nose_py_comprehension_boundary_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("list_value.py"),
        "def f(xs):\n    return [x * x for x in xs]\n",
    )
    .unwrap();
    fs::write(
        dir.join("generator_value.py"),
        "def f(xs):\n    return (x * x for x in xs)\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_value.py"),
        "def f(xs):\n    return {x * x for x in xs}\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    for pair in [
        ["list_value.py", "generator_value.py"],
        ["list_value.py", "set_value.py"],
        ["generator_value.py", "set_value.py"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "Python comprehension surfaces must not merge without materialization/demand proof for {pair:?}: {json}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

/// Terminal consumers may reopen supported list/generator count reductions, but
/// `len(generator)` is a TypeError and `len(set_comprehension)` observes
/// deduplication rather than iteration count.
#[test]
fn scan_mode_semantic_respects_python_comprehension_cardinality_boundaries() {
    let dir = std::env::temp_dir().join(format!(
        "nose_py_comprehension_cardinality_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("list_len.py"),
        "def f(xs):\n    return len([x for x in xs if x > 0])\n",
    )
    .unwrap();
    fs::write(
        dir.join("sum_count.py"),
        "def f(xs):\n    return sum(1 for x in xs if x > 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("generator_len.py"),
        "def f(xs):\n    return len(x for x in xs if x > 0)\n",
    )
    .unwrap();
    fs::write(
        dir.join("set_len.py"),
        "def f(xs):\n    return len({x % 2 for x in xs})\n",
    )
    .unwrap();
    fs::write(
        dir.join("list_mod_len.py"),
        "def f(xs):\n    return len([x % 2 for x in xs])\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    assert!(
        family_contains_all(&json, &["list_len.py", "sum_count.py"]),
        "proof-backed list comprehension cardinality should still converge with a count reduction: {json}"
    );
    for pair in [
        ["generator_len.py", "list_len.py"],
        ["generator_len.py", "sum_count.py"],
        ["set_len.py", "list_mod_len.py"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "unsupported Python comprehension cardinality must stay closed for {pair:?}: {json}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

/// Generator expression construction is lazy. Its body must not be treated like
/// an eager list comprehension body for exception timing.
#[test]
fn scan_mode_semantic_respects_python_generator_lazy_exception_timing() {
    let dir = std::env::temp_dir().join(format!(
        "nose_py_generator_lazy_exception_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("eager_list.py"),
        "def f():\n    try:\n        return [1 / 0 for x in [1]]\n    except ZeroDivisionError:\n        return 7\n",
    )
    .unwrap();
    fs::write(
        dir.join("lazy_generator.py"),
        "def f():\n    try:\n        return (1 / 0 for x in [1])\n    except ZeroDivisionError:\n        return 7\n",
    )
    .unwrap();

    let json = scan_json(&run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "1",
        "--min-lines",
        "1",
    ]));
    assert!(
        !family_contains_all(&json, &["eager_list.py", "lazy_generator.py"]),
        "generator construction must not inherit eager list-comprehension exception timing: {json}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scan_human_hides_generated_header_families() {
    let dir = make_generated_header_project("human");

    let out = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "12",
    ]);
    assert!(
        out.contains("0 semantic clone families"),
        "generated-header families should not be top-level human findings: {out}"
    );
    assert!(
        out.contains("omitted 1 family from default output (1 generated-code)"),
        "human report should explain the omitted generated family: {out}"
    );
    assert!(
        !out.contains("a/f.py") && !out.contains("b/f.py"),
        "generated-header families must not expose report locations: {out}"
    );

    let json = run(&[
        "scan",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "--top",
        "0",
        "--min-size",
        "12",
    ]);
    assert!(
        !scan_families(&scan_json(&json)).is_empty(),
        "full JSON should retain generated-header families for diagnostics: {json}"
    );
    let fail = Command::new(bin())
        .args([
            "scan",
            dir.to_str().unwrap(),
            "--mode",
            "semantic",
            "--min-size",
            "12",
            "--fail-on",
            "any",
        ])
        .output()
        .expect("run");
    assert!(
        fail.status.success(),
        "generated-header families should not trip the default CI gate: stdout={} stderr={}",
        String::from_utf8_lossy(&fail.stdout),
        String::from_utf8_lossy(&fail.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}
