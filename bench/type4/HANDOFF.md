# Type-4 Coevolution Handoff

Date: 2026-06-05
Updated: 2026-06-05, continued through exact-semantic shape-feature skip

This records where the adversarial Type-4 coevolution work stopped and how to resume it.
The work was intentionally paused after a real-repository evaluation pass. Do not start
another autonomous frontier loop unless the user explicitly resumes it.

## Current State

- Branch: `main`.
- This handoff is current through code commit
  `cc812fc perf(type4): skip shape features for exact semantic scans`.
- Last committed semantic frontier change: `0b63ad9 feat(type4): prove python module membership`.
- The strict frontier loop itself is still paused after the Python module membership batch.
  No new semantic frontier axis has been accepted after loop 406.
- Separate performance-enabling passes were started after that pause and committed as
  `93475b4 perf(type4): speed semantic value extraction` and
  `58ff3ed perf(type4): cap broad semantic block units`, then continued in
  `095bd62 perf(type4): compact semantic extraction hotspots`,
  `fe143db perf(type4): skip gated units before value extraction`, and
  `cc812fc perf(type4): skip shape features for exact semantic scans`.
- Installed baseline used for real-repo comparison: `/opt/homebrew/bin/nose`, version `0.2.0`.
- Current candidate used for comparison: `target/release/nose`, version `0.4.0`.
- Worktree at this pause: no uncommitted tracked code changes before this handoff update;
  the pre-existing untracked `.claude/` directory remains intentionally untouched.

The exact stop line is:

```text
loop 406 completed -> generated gates clean -> real-repo sample compared ->
performance pass opened -> core hotspots fixed -> 105-repo sweep exposed raylib/sympy/sqlite ->
shared subtree hash cache implemented -> large AC expression fast path implemented ->
iterative AC flatten implemented -> weakly-justified flatten cache removed ->
shadowed callback collection recursion fixed -> final validation and representative timing rerun ->
committed as 93475b4 -> block-unit cap moved before value extraction and lowered to 160 ->
validation and representative timing rerun -> committed as 58ff3ed ->
sympy/raylib profiled again -> class-container cap added at 8k preorder tokens ->
validation and representative timing rerun -> sqlite walChecksumBytes profiled ->
coupled loop recurrence compacted into a keyed Recurrence atom ->
validation and representative timing rerun -> very large generated AC expressions compacted
into keyed Formula atoms -> representative timing and output checks rerun -> committed as
095bd62 -> raylib/sympy/sqlite/vim/zod reprofiled -> file-level unit summary added for
distributed unit costs -> non-dense block/class size gate moved before strict/value
extraction -> signed +/- generated formulas compacted into keyed Formula atoms ->
representative timing and output checks rerun -> full tests and smoke passed ->
committed as fe143db -> 105-repo semantic sweep rerun -> raylib identified as the
remaining largest normalize/extract target -> exact semantic scan stopped building
syntactic shape/linear features unless near mode needs them -> representative timing and
output checks rerun -> full tests and smoke passed -> committed as cc812fc -> pause to
record the handoff
```

There is no half-implemented semantic frontier or uncommitted performance patch to finish.
The work paused at a clean performance checkpoint that supports the coevolution workflow
by making real-repo semantic audits cheaper.

Committed code state:

- `cargo fmt` and `cargo build --release -p nose-cli` passed after the latest code edits.
- `cargo test -p nose-normalize`, `cargo test -p nose-detect`, `cargo test -p nose-cli`,
  and `GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh` passed
  after the latest logic change.
- `git diff --check` passed.
- The committed block-unit cap change intentionally alters some real-repo report surfaces:
  SQLite keeps the same 74 family count with only metadata-level change in one family;
  Raylib drops several broad vendored C header block families and adds one narrower
  replacement, going 223 -> 219 families.
- The committed class-container cap preserves representative outputs:
  Sympy is byte-identical to the pre-class-cap run; Zod is byte-identical to the
  post-block-cap/class-cap run; SQLite, Raylib, and Vim keep the same families and are
  identical after dropping only `sem`/`mean_sem` metadata.
- The committed coupled-recurrence and generated-formula compactions intentionally reduce
  `sem` metadata in some families because large raw value DAGs no longer expose every
  intermediate atom. The family locations and counts are preserved in the representative
  set.
- The committed early size-gate pass is output-preserving by construction: non-function
  units that fail the existing syntactic gate were already discarded, and the dense
  semantic escape remains function/method-only. The representative real-repo outputs are
  byte-identical.
- The committed signed-formula compaction extends the generated-formula path to mixed
  `+`/`-` arithmetic chains. It preserves the existing subtraction-as-addition-of-negation
  semantics for large generated formulas and keeps changed signs distinct.
- Exact semantic `scan` no longer builds syntactic shape features (`shapes`,
  `shape_minhash`, `linear`) unless `near` mode needs them. The exact semantic detector
  uses the value graph for candidate generation and scoring, so the representative outputs
  remain byte-identical while extraction does less work. Cache signatures include this
  option so semantic-only cache entries cannot be reused for near-capable scans.
- The briefly added `flatten_cache` in `value_graph.rs` was removed because SQLite
  improved from the iterative flatten rewrite itself, while the extra cache did not
  materially improve the measured scan.
- A `prettier` stack overflow was fixed by rejecting local collection initializer
  inlining when the initializer RHS contains the same canonical id. In the current flat
  cid model this is the conservative strict proof boundary: the same id may be a true
  self-reference or a shadowed nested callback parameter, and this helper cannot
  distinguish those scopes safely.

## Current Performance Pass

This pass was started because real-repo semantic scans became too slow for the intended
adversarial loop cadence. The goal is not a new Type-4 proof rule; it is to make future
real-repo audits cheap enough to run inside the loop.

Baseline release timings before the performance changes:

| repo | wall time | `normalize+extract` | semantic families |
|---|---:|---:|---:|
| `bench/repos/zod` | 6.748s | 6207ms | 9 |
| `bench/repos/zstd` | 5.679s | 5574ms | 45 |
| `bench/repos/regex` | 1.024s | 945ms | 22 |
| `bench/repos/sqlalchemy` | 57.693s | 57345ms | 447 |
| `bench/repos/clap` | >120s timeout | not completed | not completed |

Sixteen performance/design changes are present across the committed passes:

1. Numeric-method recognition now checks the method shape before recursively evaluating
   the receiver. This fixed a pathological fluent-chain case in `clap_builder`:

   ```text
   bench/repos/clap/clap_builder/src/builder/arg.rs
   before: >45s timeout for one file
   after:  0.67s, normalize+extract 74ms
   full clap scan after: 1.78s, 6 families
   ```

   The root cause was `eval_proven_numeric_method_call` evaluating receivers for
   non-numeric chains like `.field(...).field(...)` before confirming that the method was
   one of `abs`, `min`, or `max`.

2. Value fingerprinting now has a file-level context that reuses module seed facts and
   function-binding hashes across all units from the same file. This avoids repeating
   top-level assignment scans, mutation scans, unit-symbol checks, and safe-function hash
   collection for every block unit.

3. Loop reduction recognition now uses a per-loop `ReductionCache` for both reduction
   classification and value-DAG reference checks. This fixed the remaining `zstd`
   bottleneck, where a single large C CLI parser caused repeated recursive
   `as_reduction` and `references` walks while most rayon workers were idle.

4. `StrictFacts::collect_immutable_bindings` now computes top-level assignment counts,
   candidate module names, top-level membership, and mutated bindings once per file
   instead of rescanning every node for every candidate binding. This preserves the old
   direct-receiver mutation semantics:

   - `Append(receiver, value)` marks only a direct receiver symbol as mutated;
   - mutating field calls mark only a direct receiver symbol as mutated;
   - non-top-level assignment left-hand sides still use recursive symbol containment.

5. Module value seeding now uses a per-file `ModuleSeedContext` and seeds only bindings
   required by the current unit plus the dependency closure of those bindings. This keeps
   module/global constant capture strict, while avoiding repeated evaluation of unrelated
   top-level tables for every block unit.

6. Shared structural subtree hashes are now available through `ValueFingerprintContext`.
   Contextual per-unit builders use a shared `OnceLock<Vec<u64>>` instead of recomputing
   `crate::subtree_hashes(il, interner)` for every unit in the same file. This was the
   direct follow-up from the raylib profile.

7. Very large associative/commutative source expressions now have a fast path. For
   same-operator binary expression trees with at least 64 operands, the builder collects
   source operands first, evaluates leaves once, and canonicalizes once. This targets
   generated or symbolic algebra style expressions without changing the small-expression
   path; the current large-formula representation is described in item 13.

8. AC value flattening is now iterative instead of recursive and no longer clones each
   nested node's argument vector while flattening. A per-builder `flatten_cache` was tried
   and then removed because it added state without a measured benefit.

9. Local collection binding proof now refuses to inline an initializer whose RHS contains
   the same cid. This fixed a stack overflow on Prettier's
   `scripts/clean-cspell.js`, where a callback parameter shadowed the outer `words`
   binding and `words.includes(...)` recursively re-entered the initializer proof. A
   focused CLI regression test was added:
   `scan_mode_semantic_handles_shadowed_callback_collection_name`.

10. Block units are now capped at 160 preorder tokens and the cap is applied immediately
    after `collect_pre`, before strict-safety and value fingerprint extraction. This makes
    block extraction match its purpose: keep fragment-sized sub-function regions, while
    leaving broad nested bodies to their enclosing function/class units. The change
    removes repeated value extraction for large overlapping blocks in SQLite and Vim.

11. Very large class container units are capped at 8,000 preorder tokens through the same
    container-cap policy helper. Ordinary class/type clones remain eligible, but generated
    or monolithic class bodies are delegated to their method/function units. This removes
    duplicated semantic fingerprint work for Sympy's 49k-token `PolyQuintic` class while
    preserving the 502 reported Sympy families byte-for-byte.

12. Coupled non-reduction loop recurrences now compact into a keyed `Recurrence` value
    atom when a loop-carried assignment depends on another loop-carried value or an
    already compacted recurrence. Clean reductions (`acc += elem`, guarded reductions,
    min/max selection reductions) still reach `Reduce`; only raw coupled updates such as
    `s1 += f(s2); s2 += g(s1)` stop expanding into a large expression DAG. A focused C
    equivalence regression verifies the atom set stays compact and a changed recurrence
    remains distinct.

13. Very large generated associative/commutative formulas now compact into a keyed
    `Formula` atom after operand collection and canonicalization. The key includes the
    operator, operand count, and canonical operand value hashes, so reordered additions
    remain equivalent while changed terms remain distinct. A focused Python equivalence
    regression covers compactness, operand-order canonicalization, and changed-term
    separation.

14. The syntactic size gate now runs before strict-safety and value-fingerprint extraction
    for non-function units. Function/method units still get the dense semantic escape for
    compressed one-liners, but blocks/classes that fail the existing line/token gate are
    skipped before the expensive value path. A CLI `features` regression keeps this
    contract explicit: dense small functions remain extractable, while small blocks stay
    behind the syntactic gate.

15. Large generated `+`/`-` formula chains now collect signed source operands before value
    evaluation and compact them into one keyed `Formula` atom. This covers generated
    polynomial code where subtraction broke the previous same-operator AC fast path into
    many smaller expressions. A focused Python equivalence regression verifies signed
    operand canonicalization, sign-change separation, and compact fingerprints.

16. Exact semantic scans now skip structural shape feature construction unless `near`
    scoring is enabled. `scan --mode semantic` and the default exact semantic channel do
    not need `shapes`, `shape_minhash`, or `linear`; exact semantic candidates and scores
    are value-graph based. Hidden detector/default library options still build shape
    features, and CLI cache signatures include the new option so semantic-only caches do
    not contaminate near-capable scans.

Representative measured state after the first three core fixes:

| repo | baseline `normalize+extract` | final measured `normalize+extract` | output check |
|---|---:|---:|---|
| `bench/repos/zod` | 6207ms | 177ms | byte-identical |
| `bench/repos/zstd` | 5574ms | 74-127ms | byte-identical |
| `bench/repos/regex` | 945ms | 52ms | byte-identical |
| `bench/repos/sqlalchemy` | 57345ms | 572-839ms | byte-identical |
| `bench/repos/clap` | >120s timeout before the first fix | 27-33ms after all fixes | byte-identical to the post-timeout baseline |

Latest measured state after the performance continuation:

| repo | latest measured `normalize+extract` | wall time | output check | note |
|---|---:|---:|---|---|
| `bench/repos/raylib` | 1787-1851ms | 2.55-2.79s | byte-identical | shared subtree cache fixed the distributed per-unit cost |
| `bench/repos/sqlalchemy` | 146.7ms | 0.44s | byte-identical | remained stable after shared context work |
| `bench/repos/sympy` | 1214.0ms | 2.16s | byte-identical | large AC fast path fixed `eqs_165x165` style formulas |
| `bench/repos/sqlite` | 3093.6ms | 3.94s | byte-identical | iterative flatten helped; cache later removed |
| `bench/repos/netty` | 225.2ms | 0.81s | byte-identical | large AC path removed the main outlier cost |
| `bench/repos/zstd` | 88.5ms | not recorded here | byte-identical | no longer a priority target |
| `bench/repos/clap` | 34.2ms | not recorded here | byte-identical | keep as a regression target |
| `bench/repos/zod` | 41.3ms | not recorded here | byte-identical | no longer a priority target |
| `bench/repos/regex` | 54.0ms | not recorded here | byte-identical | no longer a priority target |

Final rerun after removing `flatten_cache` and fixing the Prettier stack overflow:

| repo | `normalize+extract` | wall time | output check | note |
|---|---:|---:|---|---|
| `bench/repos/zod` | 77.4ms | 0.63s | byte-identical | JS/TS guard regression target |
| `bench/repos/raylib` | 1835.7ms | 2.91s | byte-identical | still useful as many-unit regression target |
| `bench/repos/sqlite` | 3416.0ms | 4.16s | byte-identical | still the best next hotspot |
| `bench/repos/prettier` | 47.6ms | 1.06s | previously crashed | stack overflow fixed |

Follow-up rerun after lowering and moving the block-unit cap:

| repo | before `normalize+extract` | after `normalize+extract` | after wall time | output effect |
|---|---:|---:|---:|---|
| `bench/repos/sqlite` | 3218.3ms | 950.3ms | 1.71s | 74 families preserved; one `sem` metadata value changed |
| `bench/repos/vim` | 1545.2ms | 264.4ms | 0.75s | 19 families preserved |
| `bench/repos/raylib` | 1835.7ms | 1778.4ms | 2.58s | broad vendored block families reduced, 223 -> 219 |
| `bench/repos/zod` | 77.4ms | 73.3ms | 0.17s | byte-identical |
| `bench/repos/sympy` | 1023-1214ms | 1085.3ms | 1.69s | 502 families preserved |

Follow-up rerun after adding the class-container cap:

| repo | before `normalize+extract` | after `normalize+extract` | after wall time | output effect |
|---|---:|---:|---:|---|
| `bench/repos/sympy` | 1101.9ms | 633.6ms | 1.73s | byte-identical; 502 families |
| `bench/repos/raylib` | 1740.8ms | 1771.7ms | 2.86s | byte-identical; class cap has no direct effect |
| `bench/repos/sqlite` | 950.3ms | 1015.1ms | 1.36s | 74 families preserved; only `sem` metadata differs from the previous post-block run |
| `bench/repos/vim` | 264.4ms | 200.7ms | 0.69s | byte-identical; 19 families |
| `bench/repos/zod` | 73.3ms | 69.6ms | 0.36s | byte-identical; 9 families |

Follow-up rerun after adding coupled-recurrence compaction:

| repo | before `normalize+extract` | after `normalize+extract` | after wall time | output effect |
|---|---:|---:|---:|---|
| `bench/repos/sqlite` | 1214.7ms profile / 1015.1ms representative | 218.8ms representative | 0.47s | 74 families preserved; only `sem` metadata differs |
| `bench/repos/sympy` | 633.6-697.6ms | 730.5ms | 1.71s | byte-identical/no-sem-identical; 502 families |
| `bench/repos/raylib` | 1771.7-1790.3ms | 1826.2ms | 2.84s | 219 families preserved; only `sem` metadata differs |
| `bench/repos/vim` | 200.7ms | 207.9ms | 0.54s | 19 families preserved; only `sem` metadata differs |
| `bench/repos/zod` | 69.6ms | 43.9ms | 0.14s | byte-identical; 9 families |

Follow-up rerun after adding generated-formula compaction:

| repo | before `normalize+extract` | after `normalize+extract` | after wall time | output effect |
|---|---:|---:|---:|---|
| `bench/repos/sympy` | 636.3ms | 655.3ms | 1.81s | 502 families preserved; no-sem-identical |
| `bench/repos/sqlite` | 355.9ms | 419.3ms | 1.35s | 74 families preserved; no-sem-identical |
| `bench/repos/raylib` | 1932.9ms | 1736.3ms | 3.06s | 219 families preserved; no-sem-identical |

The generated-formula pass is a compactness/frontier-safety improvement more than a
clear wall-time win on this sample. In Sympy, `eqs_165x165` went from `54372` value atoms
to `167`, while measured value time moved from `71.4ms` to `84.3ms` in this run. The
remaining Sympy hotspot after this commit was the generated `PolyQuintic` methods
(`b`, `o`, `c`), because those formulas mixed `+` and `-` and therefore were not
addressed by the same-operator AC formula path.

Follow-up rerun after early size-gating and signed generated-formula compaction:

| repo | before `normalize+extract` | after `normalize+extract` | after wall time | output effect |
|---|---:|---:|---:|---|
| `bench/repos/raylib` | 1709.5ms | 663.3ms | 1.10s | byte-identical; 219 families |
| `bench/repos/sympy` | 555.0ms | 203.5ms | 0.72s | byte-identical; 502 families |
| `bench/repos/sqlite` | 198.1ms | 144.1ms | 0.40s | byte-identical; 74 families |
| `bench/repos/vim` | 180.8ms | 134.7ms | 0.41s | byte-identical; 19 families |
| `bench/repos/zod` | 33.2ms after early size gate | 27.9ms | 0.08s | byte-identical; 9 families |

`NOSE_TIME_UNIT_SUMMARY=1` showed raylib's remaining cost was distributed across many
small units, not one giant function. Moving the non-function size gate ahead of strict
and value extraction cut the distributed cost without changing reports. On Sympy, signed
formula compaction reduced the `polyquinticconst.py` method aggregate from about
`475.5ms` value time to below the old hotspot threshold; in a final unit-timed sample,
`Method b` had `2307` value atoms and `36.1ms` value time, while `o` and `c` no longer
appeared over the 10ms unit log threshold. `eqs_165x165` remained compact at `167` value
atoms.

Follow-up rerun after exact semantic scans stopped building shape features:

| repo | before `normalize+extract` | after `normalize+extract` | after wall time | output effect |
|---|---:|---:|---:|---|
| `bench/repos/raylib` | 676.9ms | 593.4ms | 1.06s | byte-identical to `fe143db`; 219 families |
| `bench/repos/sympy` | 209.8ms | 178.7ms | 0.69s | byte-identical to `fe143db`; 502 families |
| `bench/repos/sqlite` | 129.2ms | 109.4ms | 0.35s | byte-identical to `fe143db`; 74 families |
| `bench/repos/vim` | 129.9ms | 121.9ms | 0.41s | byte-identical to `fe143db`; 19 families |
| `bench/repos/zod` | 29.4ms | 21.7ms | 0.08s | byte-identical to `fe143db`; 9 families |

This optimization is intentionally exact-semantic-only. `near` mode still builds the
structural features it needs for fuzzy scoring.

The original `sqlalchemy` output mismatch is resolved. The cause was semantic drift in the
new file-level mutation summary: it recursively collected symbols from `Append` and
mutating-field receivers, while the old proof checked only direct receiver symbols for
those cases.

The broader 105-repo sweep after the core fixes wrote temporary artifacts under
`/tmp/nose-perf-sweep/`. The top outliers from that sweep, and their current status, are:

| repo | wall time | semantic families | status |
|---|---:|---:|---|
| `raylib` | 83.88s | 223 | reduced to about 1.06s wall / 593ms normalize; broad vendored block families reduced to 219 families |
| `sympy` | 6.57s | 502 | reduced to 0.69s wall / 179ms normalize after signed generated-formula and shape-skip passes |
| `sqlite` | 4.59s | 74 | reduced to about 0.35s wall / 109ms normalize after recurrence, size-gate, and shape-skip compaction |
| `vim` | 2.28s | 19 | reduced to about 0.41s wall / 122ms normalize after block/size-gate and shape-skip follow-ups |
| `nats-server` | 1.91s | 48 | reduced to about 0.47s wall / 226ms normalize |
| `netty` | 1.76s | 182 | reduced to 0.81s with large AC fast path |
| `sqlalchemy` | 0.91s | 447 | reduced to 0.44s; acceptable after fixes |

`prettier` originally failed during the sweep with `rc=134` and:

```text
thread '<unknown>' (...) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

That failure is now fixed. It narrowed to one file:
`bench/repos/prettier/scripts/clean-cspell.js`. The trigger was a shadowed callback
parameter named `words` inside the initializer for an outer `words` collection. The local
collection binding proof tried to inline that initializer while proving
`words.includes(...)`, re-entering the same initializer recursively. The fix is the
conservative self-cid RHS guard described above. Full Prettier semantic scan now
completes in about 1.06s wall time.

Raylib investigation after the sweep:

| state | wall time | output check |
|---|---:|---|
| after core fixes, before raylib-specific work | 83.88s | baseline for this phase |
| after selective module binding seeding | 61.75s | byte-identical to `/tmp/nose-perf-sweep/raylib.full.json` |
| after `StrictFacts` mutation-summary rewrite | 5.11s | byte-identical to `/tmp/nose-perf-sweep/raylib.full.json` |
| after shared subtree-hash cache | 2.79s | byte-identical to `/tmp/nose-perf-sweep/raylib.full.json` |
| after large AC fast path | 2.55s | byte-identical to previous current output |

`NOSE_TIME=1` on raylib after the `StrictFacts` rewrite showed:

```text
discover 4.7ms
parse+lower 307ms
lower 311.9ms
normalize+extract 4077.7ms
candidates 11ms
```

`NOSE_TIME_UNITS=1` produced no individual unit over the 10ms reporting threshold, so the
remaining raylib cost looks distributed across many units rather than one giant unit.
The latest sample, `/tmp/nose-raylib-current.sample.txt`, showed repeated
`nose_normalize::commutative::subtree_hashes` calls from
`Builder::build_unit_with_context` / `process_stmt` / `eval`.

The shared subtree-hash patch was implemented as a lazy `OnceLock<Vec<u64>>`, not as an
eager vector. That was the right shape: files that never need structural subtree hashes
still do not pay the whole-file pass, while contextual builders for large files share the
cost when `Raw` or `Lambda` paths need it.

Additional outlier notes:

- `sympy` was dominated by a giant generated arithmetic expression:
  `eqs_165x165` spent about 5889ms in value extraction before the large-AC fast path.
  After the patch, the full repo `normalize+extract` time was 1214ms.
- `sympy` then exposed a duplicated container-unit cost: `Class PolyQuintic` spent about
  430ms in value extraction while its large methods were already extracted as method
  units, and no reported family used `polyquinticconst.py`. The class-container cap
  reduces full-repo `normalize+extract` to 633.6ms with byte-identical output.
- `sqlite` was dominated by repeated extraction around `walChecksumBytes` and nested
  blocks in the same region. The follow-up block cap removed the repeated nested-block
  value extraction and reduced full-repo `normalize+extract` from 3218.3ms to 950.3ms.
- After the block cap, `sqlite` exposed a single-function coupled recurrence hotspot:
  `walChecksumBytes` spent 1076.7ms in value extraction and produced 6830 value atoms
  from a 334-token C function. The recurrence compaction removed it from the unit timing
  log and reduced a representative full-repo scan to 218.8ms `normalize+extract`.
- `netty` improved from 1421.4ms `normalize+extract` after subtree-cache work to 225.2ms
  after the large-AC path.
- `vim` previously spent 1545.2ms in `normalize+extract`, including several broad C block
  units. The block cap follow-up reduced it to 264.4ms and preserved 19 reported
  families.
- `nats-server` is no longer a priority after the latest rerun: about 0.47s wall time and
  226.2ms `normalize+extract`.

Hotspots found and improved in this pass:

- non-numeric fluent chains were evaluated by the numeric method recognizer before method
  filtering;
- top-level module statement lists were rebuilt once per unit;
- module assignment counts were recomputed once per unit;
- stable module binding mutation scans were repeated per candidate and per unit;
- direct mutating-method classification and interner resolution were repeated inside
  those mutation scans;
- unit-name shadow checks were repeated per module assignment and per unit;
- safe function-binding proofs were repeated once per unit;
- literal-sensitive function subtree hashes were rebuilt once per unit;
- ordinary structural subtree hashes were rebuilt once per unit in some `Raw` and
  `Lambda` paths; this was fixed with shared file-level subtree hashes;
- giant generated associative/commutative expressions repeatedly flattened and rebuilt
  nested pairs; this was fixed with a large-AC expression fast path;
- recursive AC flattening cloned nested argument vectors; this was reduced by iterative
  flattening;
- local collection binding proof could recursively re-enter an initializer when callback
  parameters shadowed the outer collection cid; this is fixed by refusing self-cid RHS
  inlining;
- broad nested block units repeated value extraction for regions already covered by
  enclosing function/class units; this is reduced by applying a 160-token block cap before
  strict/value extraction;
- very large class container units repeated value extraction for method bodies already
  covered by primary method/function units; this is reduced by applying an 8k-token class
  cap before strict/value extraction;
- coupled non-reduction loop recurrences expanded raw accumulator expressions across
  mutually dependent loop-carried variables; this is reduced by compacting the full RHS
  value hash into a `Recurrence` atom while keeping changed recurrences distinct;
- non-function units below the existing syntactic size gate still paid strict-safety and
  value-fingerprint extraction; this is fixed by applying that gate before the expensive
  paths while preserving the dense function/method exception;
- large generated `+`/`-` arithmetic chains were not covered by the same-operator large
  AC path, leaving Sympy `PolyQuintic` methods as single-unit hotspots; this is fixed by
  collecting signed add/sub source operands and compacting their canonical hashes;
- reduction recognition recomputed the same `(value, loopv)` classifications inside
  nested `Phi` branches;
- reduction recognition repeatedly walked the same value DAGs for `references(value,
  loopv)`;
- one pathological unit caused practical rayon underuse: most workers idled while a
  single large function monopolized value extraction.

The hidden unit instrumentation is intentionally kept in `units.rs`. `NOSE_TIME_UNITS=1`
prints individual units above the 10ms threshold; `NOSE_TIME_UNIT_SUMMARY=1` prints
per-file/per-kind aggregates above the 10ms file threshold. The summary mode is useful
when a repo like raylib has no single slow unit but still spends significant time across
many small units.

## Resume The Performance Pass

If the next session continues performance work, resume from the committed
`cc812fc` baseline before starting any new Type-4 frontier:

1. Confirm the worktree state:

   ```text
   git status --short
   ```

   At this pause, the only expected untracked item is `.claude/`. There is no open
   performance patch to finish.

2. Rebuild current:

   ```text
   cargo fmt
   cargo build --release -p nose-cli
   ```

3. Do not assume the next hotspot is still value extraction. After `cc812fc`, representative
   timings show `normalize+extract` is much smaller and `parse+lower` is becoming visible
   on some repos. Choose the next target from current measurements rather than from the
   pre-performance memory.

4. Rerun the small representative set:

   ```text
   NOSE_TIME=1 target/release/nose scan bench/repos/raylib --mode semantic --format json --top 0 > /tmp/nose-perf-next/raylib.json
   NOSE_TIME=1 target/release/nose scan bench/repos/sympy --mode semantic --format json --top 0 > /tmp/nose-perf-next/sympy.json
   NOSE_TIME=1 target/release/nose scan bench/repos/sqlite --mode semantic --format json --top 0 > /tmp/nose-perf-next/sqlite.json
   NOSE_TIME=1 target/release/nose scan bench/repos/vim --mode semantic --format json --top 0 > /tmp/nose-perf-next/vim.json
   NOSE_TIME=1 target/release/nose scan bench/repos/zod --mode semantic --format json --top 0 > /tmp/nose-perf-next/zod.json
   ```

   The `cc812fc` representative outputs were byte-identical to the `fe143db` outputs for
   raylib, sympy, sqlite, vim, and zod.

5. Re-run the core validation set after any new code change:

   ```text
   cargo test -p nose-normalize
   cargo test -p nose-detect
   cargo test -p nose-cli
   GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
   ```

6. Re-run a small top-outlier sweep before picking another optimization. The next likely
   candidates are:

   - `bench/repos/raylib`, still the largest representative normalize/extract target at
     about 593ms normalize / 1.06s wall;
   - a broader 105-repo sweep, because the former top outliers have shifted enough that
     the next best target should be selected from current data rather than memory.

   For each outlier, use both global timing and unit timing:

   ```text
   NOSE_TIME=1 NOSE_TIME_UNITS=1 NOSE_TIME_UNIT_SUMMARY=1 target/release/nose scan <repo> --mode semantic --format json --top 0 > /tmp/nose-next-profile.json 2> /tmp/nose-next-profile.err
   ```

   `zstd`, `clap`, `regex`, `zod`, `netty`, `nats-server`, `prettier`, and `sqlalchemy`
   are no longer the best profiling targets unless they regress.

7. Use modern hardware deliberately in the next pass. File-level rayon parallelism works
   well after the per-file fixed costs are gone, but single huge units can still cause
   load imbalance. Do not add coarse parallelism blindly; first identify whether the next
   outlier is a many-file fixed-cost problem, a single-unit DAG problem, or candidate
   generation/scoring work.

Validation already run before the raylib-specific continuation:

```text
cargo build --release -p nose-cli
cargo test -p nose-normalize
cargo test -p nose-detect
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
```

Final smoke result:

```text
positive recall: 613/613
hard-negative false merges: 0/1201
Raw nodes: 0/66123
```

Additional validation after the raylib-specific and later changes:

```text
cargo build --release -p nose-cli
cargo test -p nose-normalize
cargo test -p nose-detect
cargo test -p nose-cli
GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
cmp /tmp/nose-perf-sweep/raylib.full.json /tmp/nose-perf-sweep/raylib.after-binding-filter.json
cmp /tmp/nose-perf-sweep/raylib.full.json /tmp/nose-perf-sweep/raylib.after-strictfacts.json
cmp /tmp/nose-perf-sweep/raylib.full.json /tmp/nose-perf-sweep/raylib.after-subtree-cache.json
cmp /tmp/nose-perf-after-subtree/sympy.json /tmp/nose-perf-after-subtree/sympy.after-large-ac.json
cmp /tmp/nose-perf-after-subtree/sqlite.json /tmp/nose-perf-final/sqlite.after-guard.json
```

Latest validation passed after the final large-AC, flatten, cache-removal, Prettier
guard, block-cap, early size-gate, signed-formula, and exact-semantic shape-skip
changes. The block-cap rerun used `/tmp/nose-perf-continue/*.after-block-gate.*`;
the final shape-skip rerun used `/tmp/nose-perf-after-shape-skip-seq/*`.

## Last Completed Coevolution Loop

The last completed strict-frontier loop was `Batch-3 Python module collection membership`
recorded in `bench/type4/ITERATIONS.md` as loops 402-406.

The batch opened:

- `axis_membership_module_python_tuple_identity`;
- `axis_membership_module_python_set_identity`;
- `axis_membership_module_python_mutated_boundary`.

The detector change:

- canonicalized Python module-level tuple/set literal bindings as strict collection
  membership values only after stable-binding proof;
- rejected normalized `@Append(receiver, value)` mutations for module/local bindings,
  closing a false merge where `VALUES.append(...)` bypassed the old field-method scanner.

Final validation:

```text
focused:                  4/4 positives, 0/6 false merges
literal-membership core:  175/175 positives, 0/424 false merges
compact all-cross core:   613/613 positives, 0/1201 false merges
```

Afterwards, a broader generated smoke run also showed:

```text
same-surface/full smoke: 1089/1089 positives, 0/2010 false merges
```

## Where Work Stopped

After the last completed loop, the next candidate was explored but not implemented.

The likely next synthetic frontier candidate was `map_default_lookup` for Ruby
`Hash#fetch(key, default)` on a proven dynamic receiver. A probe showed Ruby fetch lowers
to this normalized IL shape:

```lisp
(call
  (field "fetch"
    (var v0))
  (var v2)
  (lit int=0))
```

No files were changed for that candidate. It is only a candidate note.

Do not treat this as an accepted next axis. It lacks:

- real-repo frequency evidence;
- a mined positive family showing useful refactoring value;
- hard negatives for custom `fetch`-like methods or mutated receivers;
- a focused generated batch;
- a before/after detector measurement.

## Real-Repo Evaluation Pass

Before continuing synthetic work, a real-repo sample comparison was run as requested:
two repos per supported language by actual file extension where possible, comparing
installed `nose 0.2.0` against current `nose 0.4.0` with:

```text
nose scan <repo> --mode semantic --format json --top 0
```

Results were written to:

```text
/tmp/nose-real-compare2/summary.json
```

The `/tmp` artifacts existed at handoff time, but they are intentionally not durable.
If they are missing, rerun the sample before making a resume decision.

Final sampled repos:

| language | repos |
|---|---|
| C | `tmux`, `zstd` |
| Go | `chi`, `gin` |
| Java | `gson`, `jsoup` |
| JavaScript | `axios`, `marked` |
| Python | `scrapy`, `sqlalchemy` |
| Ruby | `rspec-core`, `pry` |
| Rust | `regex`, `alacritty` |
| TypeScript | `zod`, `trpc` |

The initially selected Rust repo `clap` timed out on the then-current binary after 90s and
was replaced by `alacritty`. The subsequent committed performance pass addressed that
specific `clap` bottleneck; keep `clap` as a regression target, not the next profiling
priority.

Aggregate real-repo sample result:

```text
semantic families: 2021 -> 635  (delta -1386)
prod families:      944 -> 260  (delta -684)
test families:     1030 -> 346  (delta -684)
dup lines:        29265 -> 5047 (delta -24218)
value sum:        43916.2 -> 7661.5
added families:     256
removed families:  1642
```

Interpretation:

- The current detector is much stricter than the installed version. It removes many broad
  semantic families that are not proven strict Type-4.
- The current detector still adds some new useful strict families, especially in Python,
  C legacy-version code, Rust shared utility code, and TypeScript helper predicates.
- This is a good direction for strict Type-4, but the result also shows that synthetic
  recall alone is no longer the right success metric.

Examples that looked useful:

- `zstd`: repeated legacy-version blocks across `zstd_v02`-`zstd_v07`.
- `regex`: duplicated `mkwordset` logic across `regex-automata` and `regex-lite`.
- `scrapy`: repeated `from_crawler` / setup-family methods.
- `sqlalchemy`: repeated test setup/mapping patterns.
- `trpc`: small repeated predicate/helper functions.

Examples with low refactoring value:

- one-line `axios` test callbacks;
- short constructor boilerplate in `gson`/`jsoup`;
- short `expecting` helpers in `alacritty`.

## What To Do Differently Next

1. Do not keep running only synthetic batch loops.

   The generated strict suite is currently clean. More synthetic batches can still widen
   the frontier, but the marginal value is lower unless the new proof invariant is backed
   by real-repo evidence.

2. Make real-repo useful yield part of the loop gate.

   A future loop should pass all of these:

   - focused generated batch improves recall or closes a false merge;
   - axis-core and compact all-cross remain at zero false merges;
   - installed-vs-current real-repo sample produces at least a few human-useful added
     families, or demonstrably removes unsafe installed-version families;
   - runtime does not regress badly on representative repos.

3. Preserve batch-3, but choose batches from one invariant.

   Good batch shape:

   - two or three positives sharing the same proof rule;
   - one hard-negative boundary sharing the same coordinates;
   - one focused baseline before implementation;
   - one axis-core and one compact all-cross after implementation.

   Bad batch shape:

   - mixing unrelated language features;
   - adding examples just because generator coverage is easy;
   - broadening ambiguous receiver methods without type/import/mutation proof.

4. Before Ruby `fetch`, investigate real examples.

   If resuming the Ruby `Hash#fetch(key, default)` idea, first mine `rubocop`,
   `fastlane`, `rspec-core`, and `pry` for concrete repeated fetch-default patterns.
   Only implement it if receiver/key/default coordinates can be proven without trusting
   arbitrary `fetch` methods.

5. Treat performance as a first-class loop constraint.

   The coevolution loop only works if generated gates and real-repo audits are cheap
   enough to run repeatedly. The open performance pass has been committed through
   `cc812fc`; keep a small top-outlier timing set in the loop gate so broadening the
   strict frontier does not silently make audits unusable.

6. Persist important real-repo audit outputs outside `/tmp`.

   The generated benchmark artifacts can stay temporary, but real-repo deltas that affect
   frontier selection should be copied or summarized under `bench/type4/` before pausing.
   Otherwise the next session has to reconstruct too much context.

7. Separate three kinds of success.

   A loop should report them independently:

   - strict-frontier coverage: generated positives found, generated hard negatives clean;
   - product usefulness: real-repo families that a human would plausibly refactor;
   - operational cost: scan/build/gate runtime, especially on large real repos.

   Do not let a clean generated smoke hide weak real-repo usefulness or a serious runtime
   regression.

8. Prefer corpus-driven all-language axes.

   Recent improvements included language-specific proof work, but the broader objective is
   not to bias the detector toward one language family. Use
   `bench/type4/prioritize_frontier.py` to find proof invariants that appear across the
   pinned multi-language corpus, then implement language-specific lowerings only as needed
   to express that common invariant.

9. Keep the accelerated loop shape, but cap each batch by one invariant.

   The 3-item batch shape worked for loops 392-406 because each batch shared one proof
   invariant. Continue grouping two or three related surfaces per loop, but split the work
   when the proof story changes. Faster batching is useful only while false-merge
   diagnosis remains simple.

10. Add a short qualitative review before merging more frontier code.

    For each real-repo comparison, inspect representative added and removed families.
    Mark each as `useful`, `low-value`, `unsafe/removed`, or `unclear`. A strict detector
    can still produce low-value clones; those should steer ranking and reporting even when
    they are technically exact.

## Suggested Resume Sequence

1. Start from the committed `cc812fc` performance baseline.

   There is no open performance patch to finish. First confirm the clean tracked
   worktree, rebuild, and rerun a small timing set so the next optimization is chosen from
   current data.

2. Rebuild current binary:

   ```text
   cargo fmt
   cargo build --release -p nose-cli
   ```

3. Re-run a small strict generated smoke to make sure the baseline is still clean:

   ```text
   GATE=core CROSS=all NOSE=target/release/nose ./scripts/type4-smoke.sh
   ```

4. Re-run the real-repo sample if `/tmp/nose-real-compare2/summary.json` is gone.

5. Refresh frontier priorities from the pinned corpus:

   ```text
   python3 bench/type4/prioritize_frontier.py \
     --cache /tmp/nose-frontier-priorities.cache.json \
     --json-out /tmp/nose-frontier-priorities.json \
     --md-out bench/type4/FRONTIER_PRIORITIES.md
   ```

6. Pick the next frontier only after reviewing real examples. Ruby
   `Hash#fetch(key, default)` under `map_default_lookup` is a plausible note, but it is
   not committed to the plan. It should lose to any higher-yield corpus-backed invariant.

7. For the selected frontier, run the loop in this order:

   - mine real examples and write the proof invariant;
   - add two or three generated positives plus matching hard negatives;
   - measure focused baseline against the previous release/current baseline;
   - implement the smallest detector/lowering change that proves the invariant;
   - run focused, axis-core, and compact all-cross gates;
   - run a small real-repo installed-vs-current or before-vs-after comparison;
   - qualitatively review added/removed families before deciding whether to keep it.

8. If implementing a new batch, record it in `bench/type4/ITERATIONS.md` and keep this
   handoff file updated at the end of the session.

## Resume Decision Checklist

Before starting another autonomous run, answer these questions in the next session notes:

- Which proof invariant is being widened?
- Which languages express the same invariant, and which are intentionally deferred?
- What real-repo samples justify the axis?
- What hard negatives prevent over-merging?
- Which generated gate is expected to fail before the detector change?
- Which real-repo audit will decide whether the loop was useful?
- What runtime budget is acceptable for that audit?

If any answer is missing, spend the next turn on mining or profiling rather than detector
code.
