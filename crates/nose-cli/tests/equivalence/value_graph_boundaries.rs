use super::*;

#[test]
fn value_graph_distinguishes_boolean_literals() {
    // `True` and `False` are behavior-defining (like `0`≠`1`): a predicate
    // `if x>0: return True else False` and its negation (booleans swapped) compute
    // opposite results and must not collapse. The bool *value* was abstracted away.
    let i = Interner::new();
    let p = "def f(x: int):\n    if x > 0:\n        return True\n    return False\n";
    let q = "def g(x: int):\n    if x > 0:\n        return False\n    return True\n";
    assert_ne!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, q, Lang::Python),
        "a predicate and its boolean-swapped negation must not fingerprint identically"
    );
    // Cross-language: the same integer predicate in Java converges with Python.
    let java = "class C { static boolean f(int x) { if (x > 0) { return true; } return false; } }";
    assert_eq!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, java, Lang::Java),
        "same integer predicate should converge across languages"
    );
    // TypeScript `number` keeps its NaN comparison boundary closed.
    let ts = "function f(x: number): boolean { if (x > 0) { return true; } return false; }";
    assert_ne!(
        value_fp(&i, p, Lang::Python),
        value_fp(&i, ts, Lang::TypeScript),
        "TS number predicates must not merge with integer-only predicates"
    );
}

#[test]
fn value_graph_distinguishes_free_callees() {
    // Calls to DIFFERENT global functions must not collapse: alpha-renaming assigned
    // free names a positional cid by occurrence, so `foo(x)`/`bar(x)` became
    // identical IL. `max(a,b)`/`min(a,b)` must also remain distinct after their
    // scalar-choice builtin canonicalization.
    let i = Interner::new();
    let foo = "def f(x):\n    return foo(x)\n";
    let bar = "def f(x):\n    return bar(x)\n";
    assert_ne!(
        value_fp(&i, foo, Lang::Python),
        value_fp(&i, bar, Lang::Python),
        "calls to different globals foo(x) vs bar(x) must differ"
    );
    let mx = "def f(a, b):\n    return max(a, b)\n";
    let mn = "def f(a, b):\n    return min(a, b)\n";
    assert_ne!(
        value_fp(&i, mx, Lang::Python),
        value_fp(&i, mn, Lang::Python),
        "max(a,b) vs min(a,b) must differ"
    );
    // …but the SAME callee in two alpha-renamed copies must still converge.
    let g1 = "def f(items):\n    return helper(items)\n";
    let g2 = "def g(seq):\n    return helper(seq)\n";
    assert_eq!(
        value_fp(&i, g1, Lang::Python),
        value_fp(&i, g2, Lang::Python),
        "same callee with renamed locals must still converge"
    );
}

#[test]
fn value_graph_distinguishes_slice_bounds() {
    // `a[1:]` (drop first) and `a[:1]` (keep first) are different slices — collecting
    // only the slice's named children collapsed both to `Seq(1)`, losing whether the
    // `1` is the start or the stop. They must not fingerprint identically.
    let i = Interner::new();
    let drop1 = "def f(a):\n    return a[1:]\n";
    let keep1 = "def g(a):\n    return a[:1]\n";
    assert_ne!(
        value_fp(&i, drop1, Lang::Python),
        value_fp(&i, keep1, Lang::Python),
        "different slice bounds (a[1:] vs a[:1]) must not fingerprint identically"
    );
}

#[test]
fn value_graph_distinguishes_slice_bounds_go_rust() {
    // Same slice-position bug in Go's `a[1:]`/`a[:1]` and Rust's `&a[1..]`/`&a[..1]`,
    // plus Rust inclusivity `1..2` vs `1..=2`.
    let i = Interner::new();
    let g1 = "package p\nfunc f(a []int) []int {\n\treturn a[1:]\n}\n";
    let g2 = "package p\nfunc f(a []int) []int {\n\treturn a[:1]\n}\n";
    assert_ne!(
        value_fp(&i, g1, Lang::Go),
        value_fp(&i, g2, Lang::Go),
        "Go a[1:] vs a[:1] must differ"
    );
    let r1 = "fn f(a: &[i64]) -> &[i64] {\n    &a[1..]\n}\n";
    let r2 = "fn f(a: &[i64]) -> &[i64] {\n    &a[..1]\n}\n";
    let r3 = "fn f(a: &[i64]) -> &[i64] {\n    &a[1..2]\n}\n";
    let r4 = "fn f(a: &[i64]) -> &[i64] {\n    &a[1..=2]\n}\n";
    assert_ne!(
        value_fp(&i, r1, Lang::Rust),
        value_fp(&i, r2, Lang::Rust),
        "Rust &a[1..] vs &a[..1] must differ"
    );
    assert_ne!(
        value_fp(&i, r3, Lang::Rust),
        value_fp(&i, r4, Lang::Rust),
        "Rust 1..2 (exclusive) vs 1..=2 (inclusive) must differ"
    );
}

#[test]
fn value_graph_distinguishes_while_stride() {
    // `while i<len: s+=a[i]; i+=1` sums every element; `i+=2` sums every other. A
    // non-unit stride visits a subset, so `a[i]` is NOT `Elem(a)` — the two must not
    // fingerprint identically (the while-loop analog of the range-start bug).
    let i = Interner::new();
    let all = "def f(a):\n    s=0\n    i=0\n    while i < len(a):\n        s += a[i]\n        i += 1\n    return s\n";
    let evn = "def g(a):\n    s=0\n    i=0\n    while i < len(a):\n        s += a[i]\n        i += 2\n    return s\n";
    assert_ne!(
        value_fp(&i, all, Lang::Python),
        value_fp(&i, evn, Lang::Python),
        "a strided while-loop must not fingerprint identically to a unit-stride one"
    );
}

#[test]
fn value_graph_distinguishes_early_break() {
    // A loop that `break`s early (sum until acc>100) computes a PREFIX sum — a
    // different value than the full sum. They must not fingerprint identically;
    // `break` cannot be treated as a no-op.
    let i = Interner::new();
    let full = "def f(xs):\n    acc = 0\n    for x in xs:\n        acc += x\n    return acc\n";
    let brk = "def g(xs):\n    acc = 0\n    for x in xs:\n        acc += x\n        if acc > 100:\n            break\n    return acc\n";
    assert_ne!(
        value_fp(&i, full, Lang::Python),
        value_fp(&i, brk, Lang::Python),
        "an early-break loop must not fingerprint identically to a full-iteration loop"
    );
}

#[test]
fn statically_false_loop_guard_skips_dead_body() {
    let i = Interner::new();
    let exact = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; for (int j = 0; !found && j < size; j++) if (vertices[offset + j] != vertex[j]) found = false; if (found) return (long)i; } return -1; } }";
    let epsilon = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices, float epsilon) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; for (int j = 0; !found && j < size; j++) if ((vertices[offset + j] > vertex[j] ? vertices[offset + j] - vertex[j] : vertex[j] - vertices[offset + j]) > epsilon) found = false; if (found) return (long)i; } return -1; } }";
    let executes_from_false = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = false; for (int j = 0; !found && j < size; j++) if (vertices[offset + j] == vertex[j]) found = true; if (found) return (long)i; } return -1; } }";
    let positive_guard = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; for (int j = 0; found && j < size; j++) if (vertices[offset + j] != vertex[j]) found = false; if (found) return (long)i; } return -1; } }";
    let reassigned_guard = "class C { static long f(float[] vertex, int strideInBytes, float[] vertices, int numVertices) { final int size = strideInBytes / 4; for (int i = 0; i < numVertices; i++) { final int offset = i * size; boolean found = true; found = vertices == vertex; for (int j = 0; !found && j < size; j++) if (vertices[offset + j] != vertex[j]) found = false; if (found) return (long)i; } return -1; } }";
    let fp = value_fp(&i, exact, Lang::Java);
    assert_eq!(
        fp,
        value_fp(&i, epsilon, Lang::Java),
        "a loop guarded by !found after found=true has an unreachable body"
    );
    assert_ne!(fp, value_fp(&i, executes_from_false, Lang::Java));
    assert_ne!(fp, value_fp(&i, positive_guard, Lang::Java));
    assert_ne!(fp, value_fp(&i, reassigned_guard, Lang::Java));
}

#[test]
fn java_low_bit_toggle_parity_converges_with_xor() {
    let i = Interner::new();
    let even_branch = "class C { static int f(int edgeId) { return edgeId % 2 == 0 ? edgeId + 1 : edgeId - 1; } }";
    let xor = "class C { static int g(int edgeKey) { return edgeKey ^ 1; } }";
    let odd_branch = "class C { static int h(int edgeId) { return edgeId % 2 != 0 ? edgeId - 1 : edgeId + 1; } }";
    let reversed = "class C { static int r(int edgeId) { return edgeId % 2 == 0 ? edgeId - 1 : edgeId + 1; } }";
    let xor_two = "class C { static int x(int edgeId) { return edgeId ^ 2; } }";
    let positive_one = "class C { static int p(int edgeId) { return edgeId % 2 == 1 ? edgeId - 1 : edgeId + 1; } }";
    let wrong_delta = "class C { static int w(int edgeId) { return edgeId % 2 == 0 ? edgeId + 1 : edgeId - 2; } }";

    let fp = value_fp(&i, even_branch, Lang::Java);
    assert_eq!(
        fp,
        value_fp(&i, xor, Lang::Java),
        "Java even/odd +/-1 reverse-edge idiom should converge with low-bit xor"
    );
    assert_eq!(
        fp,
        value_fp(&i, odd_branch, Lang::Java),
        "the equivalent != 0 branch order should also converge"
    );
    assert_ne!(fp, value_fp(&i, reversed, Lang::Java));
    assert_ne!(fp, value_fp(&i, xor_two, Lang::Java));
    assert_ne!(fp, value_fp(&i, positive_one, Lang::Java));
    assert_ne!(fp, value_fp(&i, wrong_delta, Lang::Java));
}

#[test]
fn c_u16_big_endian_byte_pack_converges_with_boundaries() {
    let i = Interner::new();
    let add_casted = r#"
typedef unsigned char u8;
unsigned int f(const u8 *a) {
    return (((unsigned int)a[0]) << 8) + ((unsigned int)a[1]);
}
"#;
    let add_uncasted = r#"
typedef unsigned char u8;
int g(u8 *p) {
    return (p[0] << 8) + p[1];
}
"#;
    let bit_or = r#"
unsigned int h(unsigned char *a) {
    return (a[0] << 8) | a[1];
}
"#;
    let uint8_or = r#"
unsigned int j(const uint8_t *a) {
    return (a[0] << 8) | a[1];
}
"#;
    let wrong_order = r#"
typedef unsigned char u8;
unsigned int r(const u8 *a) {
    return (a[1] << 8) | a[0];
}
"#;
    let overlapping_lane = r#"
typedef unsigned char u8;
unsigned int o(const u8 *a) {
    return (a[0] << 4) | a[1];
}
"#;
    let wrong_second_byte = r#"
typedef unsigned char u8;
unsigned int w(const u8 *a) {
    return (a[0] << 8) | a[2];
}
"#;
    let unproven_alias = r#"
typedef unsigned short u8;
unsigned int u(const u8 *a) {
    return (a[0] << 8) | a[1];
}
"#;
    let int_pointer = r#"
unsigned int q(const int *a) {
    return (a[0] << 8) | a[1];
}
"#;

    let fp = value_fp(&i, add_casted, Lang::C);
    assert!(
        fp.len() >= 4,
        "C u16 byte-pack fingerprint must stay large enough for exact query buckets: {} atoms",
        fp.len()
    );
    assert_eq!(fp, value_fp(&i, add_uncasted, Lang::C));
    assert_eq!(fp, value_fp(&i, bit_or, Lang::C));
    assert_eq!(fp, value_fp(&i, uint8_or, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_order, Lang::C));
    assert_ne!(fp, value_fp(&i, overlapping_lane, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_second_byte, Lang::C));
    assert_ne!(fp, value_fp(&i, unproven_alias, Lang::C));
    assert_ne!(fp, value_fp(&i, int_pointer, Lang::C));
}

#[test]
fn c_u32_big_endian_byte_pack_requires_unsigned_high_lane() {
    let i = Interner::new();
    let add_casted_alias = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 f(const u8 *a) {
    return (((u32)a[0]) << 24) + (((u32)a[1]) << 16) + (((u32)a[2]) << 8) + ((u32)a[3]);
}
"#;
    let or_casted_alias = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 g(u8 *a) {
    return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;
    let unsigned_int_cast = r#"
unsigned int h(unsigned char *a) {
    return ((unsigned int)a[0] << 24) + ((unsigned int)a[1] << 16) + ((unsigned int)a[2] << 8) + (unsigned int)a[3];
}
"#;
    let high_lane_uncasted = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 u(const u8 *a) {
    return (a[0] << 24) | (a[1] << 16) | (a[2] << 8) | a[3];
}
"#;
    let wrong_order = r#"
typedef unsigned char u8;
typedef unsigned int u32;
u32 r(const u8 *a) {
    return ((u32)a[1] << 24) | ((u32)a[0] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;
    let wrong_alias = r#"
typedef unsigned char u8;
typedef signed int u32;
u32 s(const u8 *a) {
    return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;
    let int_pointer = r#"
typedef unsigned int u32;
u32 q(const int *a) {
    return ((u32)a[0] << 24) | ((u32)a[1] << 16) | ((u32)a[2] << 8) | ((u32)a[3]);
}
"#;

    let fp = value_fp(&i, add_casted_alias, Lang::C);
    assert!(
        fp.len() >= 4,
        "C u32 byte-pack fingerprint must stay large enough for exact query buckets: {} atoms",
        fp.len()
    );
    assert_eq!(fp, value_fp(&i, or_casted_alias, Lang::C));
    assert_eq!(fp, value_fp(&i, unsigned_int_cast, Lang::C));
    assert_ne!(fp, value_fp(&i, high_lane_uncasted, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_order, Lang::C));
    assert_ne!(fp, value_fp(&i, wrong_alias, Lang::C));
    assert_ne!(fp, value_fp(&i, int_pointer, Lang::C));
}

#[test]
fn value_graph_cross_language_reorder() {
    // Statement order disappears within a runtime domain, but Python `int` and TypeScript
    // IEEE-754 `number` cannot share an exact arithmetic fingerprint.
    let i = Interner::new();
    let py = "def f(a: int, b: int):\n    p = a * b\n    q = a + b\n    return p + q + p\n";
    let ts = "function g(a: number, b: number): number { const q = a + b; const p = a * b; return p + q + p; }";
    assert_ne!(
        value_fp(&i, py, Lang::Python),
        value_fp(&i, ts, Lang::TypeScript),
        "integer arithmetic must stay split from IEEE-754 arithmetic"
    );
}
