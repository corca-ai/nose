use super::*;

#[test]
fn query_mode_semantic_rejects_cross_receiver_field_state() {
    let project = TempProject::new("field_place");
    project.write(
        "read_other.py",
        "def f(a, b):\n    a.x = 7\n    return b.x\n",
    );
    project.write(
        "read_written.py",
        "def f(a, b):\n    a.x = 7\n    return a.x\n",
    );

    let json = project.query_json("semantic", &["--min-size", "1", "--min-lines", "1"]);
    assert!(
        query_families(&json).is_empty(),
        "same-named fields on different receivers must not report as exact semantic clones: {json}"
    );
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
fn query_mode_semantic_matches_wildcard_imported_java_empty_collection_constructors() {
    let project = TempProject::new("java_wildcard_ctor");
    project.write(
        "A.java",
        "import java.util.*;\nclass A {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new ArrayList<>();\n    r.add(a);\n    r.add(b);\n    return r;\n  }\n}\n",
    );
    project.write(
        "B.java",
        "import java.util.*;\nclass B {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new LinkedList<>();\n    r.add(a);\n    r.add(b);\n    return r;\n  }\n}\n",
    );

    let json = project.query_semantic_json();
    assert!(
        family_contains_all(&json, &["A.java", "B.java"]),
        "wildcard-imported empty java.util collection constructors with identical appends must form one semantic family: {json}"
    );
}

/// Soundness guard for the regression fix above: making the wildcard constructor
/// exact-safe must not over-merge. Two wildcard-imported builders that append the same
/// elements in a DIFFERENT order are not behaviorally equivalent and must not form a
/// semantic family.
#[test]
fn query_mode_semantic_rejects_wildcard_java_collections_with_divergent_append_order() {
    let project = TempProject::new("java_wildcard_neg");
    project.write(
        "A.java",
        "import java.util.*;\nclass A {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new ArrayList<>();\n    r.add(a);\n    r.add(b);\n    return r;\n  }\n}\n",
    );
    project.write(
        "B.java",
        "import java.util.*;\nclass B {\n  List<Object> build(Object a, Object b) {\n    List<Object> r = new LinkedList<>();\n    r.add(b);\n    r.add(a);\n    return r;\n  }\n}\n",
    );

    let json = project.query_semantic_json();
    assert!(
        !family_contains_all(&json, &["A.java", "B.java"]),
        "builders appending the same elements in different order must not be exact semantic clones: {json}"
    );
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
fn query_mode_semantic_rejects_reassigned_param_with_stale_collection_domain() {
    let project = TempProject::new("stale_domain");
    // `y` reassigned to a list: `e in y` is list element membership.
    project.write(
        "list_membership.py",
        "def memb(e, y: list[int], z: list[int]):\n    y = z\n    return e in y\n",
    );
    // `y` reassigned to a str: `e in y` is substring membership — NOT equivalent.
    project.write(
        "substring_membership.py",
        "def memb(e, y: list[int], z: str):\n    y = z\n    return e in y\n",
    );

    let json = project.query_semantic_json();
    assert!(
        !family_contains_all(&json, &["list_membership.py", "substring_membership.py"]),
        "a reassigned parameter's declared domain is not proof of the current receiver's domain: list membership and substring membership must not merge: {json}"
    );
}

/// Soundness (semantic-kernel async protocol boundary): `await x` is not
/// equivalent to `x` until a language/protocol contract proves that erasure.
/// The old lowering stripped `await`, which made a sync function and an async
/// function form an exact semantic family even though Promise/thenable
/// scheduling and error propagation have different observable semantics.
#[test]
fn query_mode_semantic_rejects_unproven_js_await_sync_convergence() {
    let project = TempProject::new("js_await_boundary");
    project.write("sync.js", "function id(x) {\n  return x + 1;\n}\n");
    project.write(
        "async.js",
        "async function idAsync(x) {\n  return await x + 1;\n}\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["sync.js", "async.js"]),
        "await must not be erased into a sync exact semantic family without protocol evidence: {json}"
    );
}

/// `async function` itself is a Promise-producing protocol boundary even when
/// its body has no explicit `await`. Scheduling, rejection, and thenable
/// assimilation obligations must be proven before it can converge with a
/// synchronous return.
#[test]
fn query_mode_semantic_rejects_unproven_js_async_function_sync_convergence() {
    let project = TempProject::new("js_async_function_boundary");
    project.write("sync.js", "function id(x) {\n  return x + 1;\n}\n");
    project.write(
        "async_function.js",
        "async function id(x) {\n  return x + 1;\n}\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["sync.js", "async_function.js"]),
        "async functions must not merge with sync returns without Promise protocol evidence: {json}"
    );
}

/// Promise protocol surfaces stay closed across producer, continuation, and
/// aggregate shapes until the kernel has dependency-closed scheduling,
/// rejection-channel, settled-value, and receiver/callback demand/effect proof.
#[test]
fn query_mode_semantic_rejects_unproven_js_promise_protocol_convergence() {
    let project = TempProject::new("js_promise_protocol_boundary");
    project.write("sync_value.js", "function value(x) {\n  return x + 1;\n}\n");
    project.write(
        "promise_executor.js",
        "function value(x) {\n  return new Promise(resolve => resolve(x + 1));\n}\n",
    );
    project.write(
        "promise_resolve.js",
        "function value(x) {\n  return Promise.resolve(x + 1);\n}\n",
    );
    project.write(
        "promise_then.js",
        "function value(x) {\n  return Promise.resolve(x).then(v => v + 1);\n}\n",
    );
    project.write(
        "custom_then.js",
        "function value(p) {\n  return p.then(v => v + 1);\n}\n",
    );
    project.write(
        "promise_all.js",
        "function aggregate(xs) {\n  return Promise.all(xs);\n}\n",
    );
    project.write(
        "promise_race.js",
        "function aggregate(xs) {\n  return Promise.race(xs);\n}\n",
    );
    project.write(
        "promise_all_settled.js",
        "function aggregate(xs) {\n  return Promise.allSettled(xs);\n}\n",
    );
    project.write(
        "promise_any.js",
        "function aggregate(xs) {\n  return Promise.any(xs);\n}\n",
    );

    let json = project.query_semantic_min_json();
    for pair in [
        ["sync_value.js", "promise_executor.js"],
        ["sync_value.js", "promise_resolve.js"],
        ["sync_value.js", "promise_then.js"],
        ["promise_then.js", "custom_then.js"],
        ["promise_all.js", "promise_race.js"],
        ["promise_all.js", "promise_all_settled.js"],
        ["promise_race.js", "promise_any.js"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "Promise protocol boundary must not form an exact semantic family for {pair:?}: {json}"
        );
    }
}

/// A narrow Promise recovery is allowed only where the Promise factory boundary
/// stays explicit. `Promise.resolve` over a proven non-thenable scalar can
/// converge with the same Promise factory shape, but the Promise boundary itself
/// must remain visible so synchronous payload code does not join the family.
#[test]
fn query_mode_semantic_matches_safe_promise_resolve_factories_only_inside_promise_boundary() {
    let project = TempProject::new("js_promise_resolve_safe_boundary");
    project.write(
        "promise_a.ts",
        "function value(x: number) {\n  return [Promise.resolve(x), Promise.resolve(1)];\n}\n",
    );
    project.write(
        "promise_b.ts",
        "function value(y: number) {\n  return [Promise.resolve(y), Promise.resolve(1)];\n}\n",
    );
    project.write(
        "sync_value.ts",
        "function value(z: number) {\n  return [z, 1];\n}\n",
    );
    project.write(
        "possible_thenable.js",
        "function value(p) {\n  return [Promise.resolve(p), Promise.resolve(1)];\n}\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        family_contains_all(&json, &["promise_a.ts", "promise_b.ts"]),
        "Promise.resolve over proven non-thenable scalar arguments should form an exact family that preserves Promise factory boundaries: {json}"
    );
    for pair in [
        ["promise_a.ts", "sync_value.ts"],
        ["promise_a.ts", "possible_thenable.js"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "safe Promise.resolve recovery must not erase Promise boundaries or admit possible thenables for {pair:?}: {json}"
        );
    }
}

/// Same async protocol boundary for Python: `await x` is a coroutine protocol
/// operation, not a plain value read unless a future contract proves it.
#[test]
fn query_mode_semantic_rejects_unproven_python_await_sync_convergence() {
    let project = TempProject::new("py_await_boundary");
    project.write("sync.py", "def id(x):\n    return x + 1\n");
    project.write(
        "async.py",
        "async def id_async(x):\n    return await x + 1\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["sync.py", "async.py"]),
        "await must not be erased into a sync exact semantic family without protocol evidence: {json}"
    );
}

/// Rust `.await` and `async {}` are Future protocol operations, not plain
/// wrappers around the body. Exact sync/async convergence requires future
/// protocol proof that is not modeled yet.
#[test]
fn query_mode_semantic_rejects_unproven_rust_await_sync_convergence() {
    let project = TempProject::new("rs_await_boundary");
    project.write("sync.rs", "fn id(x: i32) -> i32 { x + 1 }\n");
    project.write(
        "async.rs",
        "async fn id_async(x: i32) -> i32 { async move { x + 1 }.await }\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["sync.rs", "async.rs"]),
        "Rust async/await must not be erased into a sync exact semantic family without future protocol evidence: {json}"
    );
}

/// Swift `await` is a task/protocol boundary. It must not be erased into a
/// synchronous expression until the kernel has scheduling and effect proof for
/// the relevant Swift async protocol slice.
#[test]
fn query_mode_semantic_rejects_unproven_swift_await_sync_convergence() {
    let project = TempProject::new("swift_await_boundary");
    project.write("sync.swift", "func id(_ x: Int) -> Int { return x + 1 }\n");
    project.write(
        "async.swift",
        "func idAsync(_ x: Int) async -> Int { return await x + 1 }\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["sync.swift", "async.swift"]),
        "Swift await must not be erased into a sync exact semantic family without async protocol evidence: {json}"
    );
}

/// Go concurrency and channel operations have synchronization/scheduling
/// semantics. They must not be erased into ordinary calls or value reads until
/// a language protocol contract proves the required demand/effect obligations.
#[test]
fn query_mode_semantic_rejects_unproven_go_concurrency_protocol_convergence() {
    let project = TempProject::new("go_protocol_boundary");
    project.write(
        "direct_call.go",
        "package p\nfunc direct(x int) { record(x) }\n",
    );
    project.write(
        "goroutine.go",
        "package p\nfunc goroutine(x int) { go record(x) }\n",
    );
    project.write(
        "deferred.go",
        "package p\nfunc deferred(x int) { defer record(x) }\n",
    );
    project.write(
        "plain_value.go",
        "package p\nfunc plain(ch int) int { return ch }\n",
    );
    project.write(
        "channel_receive.go",
        "package p\nfunc receive(ch chan int) int { return <-ch }\n",
    );
    project.write(
        "channel_status.go",
        "package p\nfunc status(ch chan int) bool { _, ok := <-ch; return ok }\n",
    );
    project.write(
        "constant_status.go",
        "package p\nfunc constant(ch chan int) bool { return false }\n",
    );
    project.write(
        "send_a.go",
        "package p\nfunc sendA(ch chan int, x int) { ch <- x }\n",
    );
    project.write(
        "send_b.go",
        "package p\nfunc sendB(ch chan int, x int) { ch <- x }\n",
    );
    project.write(
        "select_receive.go",
        "package p\nfunc selectReceive(ch chan int) int { select { case v := <-ch: return v; default: return 0 } }\n",
    );
    project.write(
        "if_receive.go",
        "package p\nfunc ifReceive(ch chan int) int { v := <-ch; if v != 0 { return v }; return 0 }\n",
    );

    let json = project.query_semantic_min_json();
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
}

/// Python comprehension source surfaces are not interchangeable. A list
/// comprehension is eager and materialized, a generator expression is lazy and
/// one-shot, and a set comprehension deduplicates and is unordered.
#[test]
fn query_mode_semantic_rejects_unproven_python_comprehension_surface_convergence() {
    let project = TempProject::new("py_comprehension_boundary");
    project.write(
        "list_value.py",
        "def f(xs):\n    return [x * x for x in xs]\n",
    );
    project.write(
        "generator_value.py",
        "def f(xs):\n    return (x * x for x in xs)\n",
    );
    project.write(
        "set_value.py",
        "def f(xs):\n    return {x * x for x in xs}\n",
    );

    let json = project.query_semantic_min_json();
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
}

/// Terminal consumers may reopen supported list/generator count reductions, but
/// `len(generator)` is a TypeError and `len(set_comprehension)` observes
/// deduplication rather than iteration count.
#[test]
fn query_mode_semantic_respects_python_comprehension_cardinality_boundaries() {
    let project = TempProject::new("py_comprehension_cardinality");
    project.write(
        "list_len.py",
        "def f(xs):\n    return len([x for x in xs if x > 0])\n",
    );
    project.write(
        "sum_count.py",
        "def f(xs):\n    return sum(1 for x in xs if x > 0)\n",
    );
    project.write(
        "generator_len.py",
        "def f(xs):\n    return len(x for x in xs if x > 0)\n",
    );
    project.write(
        "set_len.py",
        "def f(xs):\n    return len({x % 2 for x in xs})\n",
    );
    project.write(
        "list_mod_len.py",
        "def f(xs):\n    return len([x % 2 for x in xs])\n",
    );

    let json = project.query_semantic_min_json();
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
}

/// Generator expression construction is lazy. Its body must not be treated like
/// an eager list comprehension body for exception timing.
#[test]
fn query_mode_semantic_respects_python_generator_lazy_exception_timing() {
    let project = TempProject::new("py_generator_lazy_exception");
    project.write(
        "eager_list.py",
        "def f():\n    try:\n        return [1 / 0 for x in [1]]\n    except ZeroDivisionError:\n        return 7\n",
    );
    project.write(
        "lazy_generator.py",
        "def f():\n    try:\n        return (1 / 0 for x in [1])\n    except ZeroDivisionError:\n        return 7\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["eager_list.py", "lazy_generator.py"]),
        "generator construction must not inherit eager list-comprehension exception timing: {json}"
    );
}

#[test]
fn query_mode_semantic_respects_python_iterator_materializer_identity() {
    let project = TempProject::new("py_iterator_materializer_identity");
    project.write(
        "list_map.py",
        "def f(xs: list[int]):\n    return list(map(lambda x: x + 1, xs))\n",
    );
    project.write(
        "list_comp.py",
        "def f(xs: list[int]):\n    return [x + 1 for x in xs]\n",
    );
    project.write(
        "tuple_map.py",
        "def f(xs: list[int]):\n    return tuple(map(lambda x: x + 1, xs))\n",
    );
    project.write(
        "set_map.py",
        "def f(xs: list[int]):\n    return set(map(lambda x: x + 1, xs))\n",
    );

    let json = project.query_semantic_min_json();
    for pair in [
        ["list_map.py", "tuple_map.py"],
        ["list_map.py", "set_map.py"],
        ["tuple_map.py", "set_map.py"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "Python lazy iterator materializers must not merge across list/tuple/set boundaries for {pair:?}: {json}"
        );
    }
}

#[test]
fn query_mode_semantic_rejects_receiver_hof_records_with_effectful_callbacks() {
    let project = TempProject::new("receiver_hof_effectful_callback");
    project.write(
        "map_effect.ts",
        "declare function audit(x: number): number;\nfunction f(xs: number[]): number[] {\n  return xs.map(x => audit(x));\n}\n",
    );
    project.write(
        "loop_effect.ts",
        "declare function audit(x: number): number;\nfunction g(xs: number[]): number[] {\n  const out: number[] = [];\n  for (const x of xs) {\n    out.push(audit(x));\n  }\n  return out;\n}\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["map_effect.ts", "loop_effect.ts"]),
        "receiver-method HOF LibraryApi records must not surface in query results when callback-effect admission rejects the call: {json}"
    );
}

#[test]
fn query_human_hides_generated_header_families() {
    let dir = make_generated_header_project("human");

    let out = run(&[
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--min-size",
        "12",
    ]);
    assert!(
        out.contains("0 duplicated-code families"),
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
        "query",
        dir.to_str().unwrap(),
        "--mode",
        "semantic",
        "--format",
        "json",
        "top=0",
        "--min-size",
        "12",
    ]);
    assert!(
        !query_families(&query_json(&json)).is_empty(),
        "full JSON should retain generated-header families for diagnostics: {json}"
    );
    let fail = Command::new(bin())
        .args([
            "query",
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
