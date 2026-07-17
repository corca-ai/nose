use crate::cli_args::BatteryKind;
use crate::path_utils::{paths_as_refs, warn_if_empty};
use anyhow::Result;
use nose_il::{Corpus, Interner};
use std::path::PathBuf;

mod report;
use report::{gate_units, print_gate_report, tally_gate, GateManifest};

/// Deterministic input battery for an `arity`-parameter function. The parameters range
/// over a fixed pool of small int-lists and scalars; for small arity the pool is
/// enumerated *combinatorially* (mixed-radix), so e.g. a 2-arg comparison sees `a<b`,
/// `a>b`, and `a==b` rather than a few coincidental diagonal pairs — the difference
/// between trusting the completeness signal and not. All units of the same arity run on
/// identical inputs (comparable); a list where a scalar is expected (or vice-versa)
/// yields `Err`, itself part of the behavior signature.
/// A fixed input *width* used for every unit regardless of its arity: a function
/// binds the first `arity` values and ignores the rest, so all units run the same
/// number of rows (the behavior-vector length must be arity-independent — two
/// fingerprint-equal units can differ in arity, e.g. constant functions).
const VERIFY_WIDTH: usize = 4;
/// Verify is a bounded oracle, not a stress test for every generated/parser-vendored
/// mega-function. Cap per-unit battery work before building the value fingerprint or
/// interpreting rows; units above the cap are reported as `battery-bail` and excluded.
pub(super) const VERIFY_BATTERY_NODE_ROW_BUDGET: usize = 384_000; // 2k IL nodes * 192 standard rows.

pub(super) fn verify_battery_over_budget(tokens: usize, battery_rows: usize) -> bool {
    tokens.saturating_mul(battery_rows) > VERIFY_BATTERY_NODE_ROW_BUDGET
}

pub(super) fn verify_battery(probes: &[nose_normalize::Value]) -> Vec<Vec<nose_normalize::Value>> {
    use nose_normalize::Value;
    let l = |xs: &[i64]| Value::List(xs.iter().copied().map(Value::Int).collect());
    let pool = [
        l(&[1, 2, 3, 4]),
        Value::Int(3),
        Value::Int(0),
        Value::Int(-1),
        l(&[5, 1, 4, 2, 8]),
        Value::Int(7),
        l(&[]),
        Value::Int(2),
    ];
    let n = pool.len();
    // Part 1: combinatorial (mixed-radix) over the pool, width-VERIFY_WIDTH rows — a
    // 2-arg function's first two slots see `a<b`/`a>b`/`a==b`.
    const COUNT: usize = 64;
    let mut battery: Vec<Vec<Value>> = (0..COUNT)
        .map(|e| {
            (0..VERIFY_WIDTH)
                .map(|j| {
                    let radix = n.saturating_pow(j as u32).max(1);
                    pool[(e / radix) % n].clone()
                })
                .collect()
        })
        .collect();
    // Part 2: literal probes. For each value the corpus actually branches on (a mined
    // string/int constant), inject it at each position — so a value-keyed branch
    // (`fdNumber === 'ipc'`) is exercised instead of always falling through, which is
    // what makes two such functions look coincidentally equal. Row count stays fixed.
    let fill = pool[0].clone();
    for v in probes {
        for p in 0..VERIFY_WIDTH {
            let mut row = vec![fill.clone(); VERIFY_WIDTH];
            row[p] = v.clone();
            battery.push(row);
        }
    }
    // Part 3: ORDER-SENSITIVITY rows for non-commutative `+` (string / list CONCAT).
    // The combinatorial pool is int/list-only and the probes inject ONE string at a
    // time, so two DISTINCT strings (or two distinct lists) are never bound to two
    // params at once — the only input on which `a+b` and `b+a` differ under concat.
    // Without these rows the order-sensitive `Str`/`List` model (interp.rs) is starved,
    // and the oracle reads SOUND while the detector reorders untyped `+` (#283-C). Each
    // slot gets a distinct token so every adjacent operand pair differs.
    //
    // These rows are kept hand-curated DELIBERATELY: see docs/oracle-value-model.md
    // (§"Why the battery is not broadened by naive enumeration") — feeding broader typed
    // inputs (equal strings, bool/null) to slots a typed array/index param would consume
    // manufactures impossible inputs (a string as an array index) on which the
    // canonicalizer legitimately differs, producing spurious canon-preservation
    // violations. A sound broad distinguishing search needs type-domain-aware feeding.
    let s = |t: u64| Value::Str(vec![t]);
    let distinct: [[Value; VERIFY_WIDTH]; 2] = [
        [s(0xC0DE01), s(0xC0DE02), s(0xC0DE03), s(0xC0DE04)],
        [l(&[1, 1]), l(&[2, 2]), l(&[3, 3]), l(&[4, 4])],
    ];
    for row in &distinct {
        battery.push(row.to_vec());
        let mut rev = row.to_vec();
        rev.reverse();
        battery.push(rev);
    }
    // Part 4: in-place ELEMENT-MUTATION rows (#337). The combinatorial pool binds slot ≥2 of a
    // ≥3-arg function to a list (radix `n^2` exceeds COUNT), so a `swap(a,i,j)`/`clobber(a,i,j)`
    // never sees a list base with TWO distinct int indices — the only input on which in-place
    // element mutation is observable. Without these rows the value graph's element-write
    // forwarding (and the interpreter's in-place store) is starved and `swap` reads identical to
    // `clobber`. A list base + small int indices is the NORMAL array shape (unlike a string used
    // as an index), so it does not manufacture canonicalizer-divergent impossible inputs.
    battery.push(vec![
        l(&[1, 2, 3, 4]),
        Value::Int(0),
        Value::Int(1),
        Value::Int(2),
    ]);
    battery.push(vec![
        l(&[5, 1, 4, 2, 8]),
        Value::Int(2),
        Value::Int(0),
        Value::Int(3),
    ]);
    // Part 5: float NON-ASSOCIATIVITY rows (#342). The pool is int/list only, so a fully-untyped
    // `(a+b)+c` vs `a+(b+c)` never sees float inputs and the i64 oracle reads them associative.
    // These rows feed FLOATS of adversarial magnitude (`1e16` ± `1e16` loses the small term to
    // rounding), so `(a+b)+c != a+(b+c)`: `assoc_l(1e16,-1e16,1.0) = 1.0` but `assoc_r = 0.0`.
    // With the value graph holding such chains unassociated (see `proven_float`/`chain_has_float`
    // for untyped params in dynamically-typed languages), the oracle now WITNESSES the split.
    let f = |x: f64| Value::Float(nose_normalize::F64(x));
    battery.push(vec![f(1e16), f(-1e16), f(1.0), f(2.0)]);
    battery.push(vec![f(1.0), f(1e16), f(-1e16), f(2.0)]);
    // Part 6: int32-WRAP rows (#344). The pool is all small ints (`int32(x) == x`), so a JS
    // bitwise `a & b` is indistinguishable from an arbitrary-precision one. These rows carry
    // values whose HIGH bits (≥ 2^32) overlap, so `a & b` differs between int32 (JS) and i64
    // (Python/etc): `0xF_0000_0003 & 0xF_0000_0005` is `1` under int32 but `0xF_0000_0001` as
    // bigint. With the oracle now executing JS bitwise as int32, this WITNESSES the split the
    // `ToInt32` floor fingerprints (#283-D).
    battery.push(vec![
        Value::Int(0xF_0000_0003),
        Value::Int(0xF_0000_0005),
        Value::Int(0xA_0000_00FF),
        Value::Int(7),
    ]);
    battery
}

/// Mine the literal constants the corpus branches on — the top string-literal hashes
/// and small integers, as interpreter values — to seed the battery's probe inputs.
pub(super) fn verify_probes(corpus: &Corpus) -> Vec<nose_normalize::Value> {
    use nose_il::Payload;
    use nose_normalize::Value;
    use std::collections::HashMap;
    let (mut strs, mut ints): (HashMap<u64, u32>, HashMap<i64, u32>) =
        (HashMap::new(), HashMap::new());
    for il in &corpus.files {
        for node in &il.nodes {
            match node.payload {
                Payload::LitStr(h) => *strs.entry(h).or_default() += 1,
                Payload::LitInt(v) => *ints.entry(v).or_default() += 1,
                _ => {}
            }
        }
    }
    fn top<K: Ord + Copy>(m: HashMap<K, u32>, k: usize) -> Vec<K> {
        let mut v: Vec<(K, u32)> = m.into_iter().collect();
        v.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(k);
        v.into_iter().map(|(key, _)| key).collect()
    }
    let mut probes: Vec<Value> = top(strs, 16)
        .into_iter()
        .map(|h| Value::Str(vec![h]))
        .collect();
    probes.extend(top(ints, 16).into_iter().map(Value::Int));
    probes
}

/// Leap-3 "wide" battery: a much larger structured input domain than [`verify_battery`].
/// Bounded equivalence checking is "interpret on enough inputs that two functions which
/// differ anywhere differ HERE": more scalars (large, negative, boundary), more lists
/// (sorted/reversed/duplicate/negative/singleton/empty), a wider arity slot, and more
/// combinatorial rows. The leap-3 hypothesis: a finite battery merges some non-equivalent
/// pairs (the §AK risk); a wider domain should drive those false merges toward zero while
/// keeping the true positives. (Still not a proof — that is the SMT extension — but a much
/// stronger bounded checker.)
fn wide_battery(probes: &[nose_normalize::Value]) -> Vec<Vec<nose_normalize::Value>> {
    use nose_normalize::Value;
    let l = |xs: &[i64]| Value::List(xs.iter().copied().map(Value::Int).collect());
    let pool = [
        l(&[1, 2, 3, 4]),
        Value::Int(3),
        Value::Int(0),
        Value::Int(-1),
        l(&[5, 1, 4, 2, 8]),
        Value::Int(7),
        l(&[]),
        Value::Int(2),
        // wide additions: boundary/large/negative scalars and adversarial lists
        Value::Int(-7),
        Value::Int(100),
        Value::Int(1),
        l(&[2, 2, 2]),        // all-equal (separates min/max/dedup-sensitive)
        l(&[0]),              // singleton zero (separates *-fold from +-fold, presence)
        l(&[-3, -1, -2]),     // all-negative (separates abs/sign, min/max direction)
        l(&[4, 3, 2, 1]),     // reversed (separates order-sensitive from order-free)
        l(&[10, -10, 5, -5]), // mixed sign, zero-sum (separates sum from sum-abs)
    ];
    let n = pool.len();
    const WIDTH: usize = 5;
    const COUNT: usize = 243; // 3^5 — dense mixed-radix coverage over a low-entropy slice
    let mut battery: Vec<Vec<Value>> = (0..COUNT)
        .map(|e| {
            (0..WIDTH)
                .map(|j| {
                    let radix = n.saturating_pow(j as u32).max(1);
                    pool[(e / radix) % n].clone()
                })
                .collect()
        })
        .collect();
    let fill = pool[0].clone();
    for v in probes {
        for p in 0..WIDTH {
            let mut row = vec![fill.clone(); WIDTH];
            row[p] = v.clone();
            battery.push(row);
        }
    }
    battery
}

/// Trailing `sources/<id>/<file>` key shared by the corpus path and the manifest path,
/// so an interpreted unit can be matched to its manifest entry regardless of the prefix
/// the corpus was analyzed under.
fn manifest_key(path: &str) -> String {
    match path.rfind("sources/") {
        Some(i) => path[i..].to_string(),
        None => path.to_string(),
    }
}

pub(crate) fn cmd_behavioral_gate(
    paths: Vec<PathBuf>,
    manifest: PathBuf,
    battery_kind: BatteryKind,
) -> Result<()> {
    let refs = paths_as_refs(&paths);
    let corpus = nose_frontend::lower_corpus_many(&refs);
    warn_if_empty(&corpus, &paths);
    let battery = match battery_kind {
        BatteryKind::Standard => verify_battery(&verify_probes(&corpus)),
        BatteryKind::Wide => wide_battery(&verify_probes(&corpus)),
    };
    let units = gate_units(&corpus, &battery);
    let m: GateManifest = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
    let outcome = tally_gate(&m, &units);
    print_gate_report(battery_kind, battery.len(), &outcome);
    Ok(())
}

/// Index every `Func` in `il` by its source byte span, so a fully-normalized unit can
/// be matched (by span) to the same function in the pre-canon core IL.
pub(super) fn func_span_index(
    il: &nose_il::Il,
) -> std::collections::HashMap<(u32, u32), nose_il::NodeId> {
    let mut index = std::collections::HashMap::new();
    let mut stk = vec![il.root];
    while let Some(x) = stk.pop() {
        if il.kind(x) == nose_il::NodeKind::Func {
            let s = il.node(x).span;
            index.entry((s.start_byte, s.end_byte)).or_insert(x);
        }
        stk.extend(il.children(x).iter().copied());
    }
    index
}

/// Interpret `root` on every battery row (under the unit's pointer-length contracts);
/// `None` when any input fails to run — the unit is not interpretable on this battery.
/// A row whose execution forks on symbolic If/ternary conditions contributes one
/// behavior per explored path (#244, deterministic order, assumptions recorded in
/// each trace); `path_cap` reports a fail-closed bail on the per-execution
/// symbolic-site cap so the census can distinguish it from other bails.
pub(super) fn run_battery(
    il: &nose_il::Il,
    interner: &Interner,
    root: nose_il::NodeId,
    battery: &[Vec<nose_normalize::Value>],
    contracts: &[(u32, u32)],
    path_cap: &mut bool,
) -> Option<Vec<nose_normalize::Behavior>> {
    match run_battery_diagnostic_with_oracle_proofs(il, interner, root, battery, contracts, true) {
        Ok(behaviors) => Some(behaviors),
        Err(blocker) => {
            if blocker.capability_id == "budget.symbolic-branch-sites" {
                *path_cap = true;
            }
            None
        }
    }
}

pub(super) fn run_battery_diagnostic_with_oracle_proofs(
    il: &nose_il::Il,
    interner: &Interner,
    root: nose_il::NodeId,
    battery: &[Vec<nose_normalize::Value>],
    contracts: &[(u32, u32)],
    include_immutable_module_strings: bool,
) -> Result<Vec<nose_normalize::Behavior>, nose_normalize::InterpreterBlocker> {
    let mut beh = Vec::with_capacity(battery.len());
    let interpreter =
        nose_normalize::PreparedInterpreter::new(il, interner, include_immutable_module_strings);
    for inputs in battery {
        let row = apply_contracts(inputs, contracts);
        beh.extend(interpreter.run_paths_diagnostic(root, &row)?);
    }
    Ok(beh)
}

/// Fragment counterpart to the whole-unit diagnostic runner, retaining whether every path returned
/// from the enclosing function or fell through the fragment. The ordinary whole-function gate
/// intentionally keeps its historical value-only control convention. The module-string flag is
/// an explicit Soundness Lab ablation; ordinary verification always selects the final tranche.
pub(super) fn run_fragment_battery_diagnostic_with_oracle_proofs(
    il: &nose_il::Il,
    interner: &Interner,
    root: nose_il::NodeId,
    battery: &[Vec<nose_normalize::Value>],
    contracts: &[(u32, u32)],
    include_immutable_module_strings: bool,
) -> Result<
    (Vec<nose_normalize::Behavior>, Vec<nose_normalize::UnitExit>),
    nose_normalize::InterpreterBlocker,
> {
    let mut behaviors = Vec::with_capacity(battery.len());
    let mut exits = Vec::with_capacity(battery.len());
    let interpreter =
        nose_normalize::PreparedInterpreter::new(il, interner, include_immutable_module_strings);
    for inputs in battery {
        let row = apply_contracts(inputs, contracts);
        for (behavior, exit) in interpreter.run_paths_observing_exit_diagnostic(root, &row)? {
            behaviors.push(behavior);
            exits.push(exit);
        }
    }
    Ok((behaviors, exits))
}

/// Trivial behavior (constant / all-Err) is coincidental, never evidence of a
/// clone — exclude it from behavioral merging.
pub(super) fn is_trivial_behavior(beh: &[nose_normalize::Behavior]) -> bool {
    use nose_normalize::Value;
    let distinct: std::collections::HashSet<&Value> = beh.iter().map(|b| &b.ret).collect();
    distinct.len() < 2
        || beh
            .iter()
            .all(|b| matches!(b.ret, Value::Null | Value::Err))
}

/// Stable hash of a behavior battery (equal hash ⟺ behaviorally equal on the battery).
pub(super) fn behavior_hash(beh: &[nose_normalize::Behavior]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    beh.hash(&mut h);
    h.finish()
}

pub(super) fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

/// Rewrite a battery row to honor a unit's pointer-length contracts: set each length-param
/// slot to the length of its array-param slot, so the oracle interprets `f(xs, n)` under
/// `n = len(xs)` — the same convention the value graph used to merge it. Only applies when
/// the array slot is actually a list (else the unit Errs identically regardless). Returns
/// the row unchanged when there are no contracts (zero cost for the common case).
fn apply_contracts(
    row: &[nose_normalize::Value],
    contracts: &[(u32, u32)],
) -> Vec<nose_normalize::Value> {
    use nose_normalize::Value;
    let mut out = row.to_vec();
    // A length param shared by several arrays (aligned `f(a, b, n)`) is the SHARED logical
    // length: bind it to the MIN of those arrays' lengths, matching the `zip`-based form
    // (`sum(x*y for x,y in zip(a,b))` stops at the shorter). For a single array this is just
    // its length. Group contracts by length-position so the shared case is a min, not a
    // last-write race.
    let mut by_len: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for &(arr_pos, len_pos) in contracts {
        by_len
            .entry(len_pos as usize)
            .or_default()
            .push(arr_pos as usize);
    }
    for (len_pos, arrs) in by_len {
        if len_pos >= out.len() {
            continue;
        }
        // If EVERY contracted array slot is a list, bind `n` to the MIN of their lengths (the
        // shared logical length). If any slot is NOT a list, `len` is undefined — bind `n =
        // Null` so `i < n` Errs and the unit Errs exactly as the `len(non-list)` form does,
        // instead of running an empty loop and returning the init value.
        let mut shared: Option<i64> = Some(i64::MAX);
        for arr_pos in arrs {
            match out.get(arr_pos) {
                Some(Value::List(xs)) => {
                    let l = xs.len() as i64;
                    shared = shared.map(|s| s.min(l));
                }
                _ => shared = None,
            }
        }
        out[len_pos] = match shared {
            Some(l) if l != i64::MAX => Value::Int(l),
            _ => Value::Null,
        };
    }
    out
}
