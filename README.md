# nose

**Find code clone families across Python, JavaScript, TypeScript (incl. JSX/TSX),
Go, Rust, Java, C, and Ruby — plus the `<script>` logic inside Vue, Svelte, and
HTML** — in one self-contained Rust binary.

*The name: a **nose** sniffs out code smells — the duplication worth refactoring away, even
when it's been renamed or restructured (and, occasionally, rewritten in another language).*

## See it

These three functions compute the same thing — **sum a list** — in three languages and three
styles. A token-based copy-paste detector sees three unrelated files.
**nose reports them as one clone family**, because it matches on *what the code computes*:

```python
# Python — explicit loop
def total(xs):
    s = 0
    for x in xs:
        s += x
    return s
```
```javascript
// JavaScript — reduce
const total = xs => xs.reduce((a, b) => a + b, 0);
```
```go
// Go — indexed loop
func Total(xs []int) int {
    n := 0
    for i := 0; i < len(xs); i++ { n += xs[i] }
    return n
}
```

Same logic, three languages, **zero shared tokens**. nose sees through renamed variables,
reordered statements, and loop ↔ `reduce` ↔ comprehension rewrites — so it finds the
duplicated *logic* worth refactoring, not just literal copy-paste. (Cross-*language*
matches like this one are real but uncommon in practice; the everyday win is the same
logic renamed or restructured **within** a language.) And the match isn't a guess: an
equal fingerprint is **sound by intent** — *fingerprint-equal ⟹ behavior-equal* (see
[How it works](#how-it-works)).

## How it works

nose parses each language with tree-sitter and lowers it into a single normalized
**intermediate language (IL)** designed so that semantically-equivalent code converges
to (near-)identical structure: identifiers alpha-renamed, loops unified, surface sugar
desugared, operators/idioms canonicalized, plus a hash-consed **value graph** (GVN) that
captures *what the code computes* (invariant to temporaries, statement order, and common
subexpressions). On top of the IL it detects clones and ranks **design-level refactoring
opportunities**. Every IL node carries inline provenance, so every match traces back to
its source span.

The value graph is **sound by intent** — two fragments sharing a fingerprint should
compute the same thing. That contract is checked by a differential interpreter oracle
(it interprets the *pre-canonicalization* IL so a rewrite can't mask its own bug) and by
machine-checked **Lean proofs** of the core canonicalizations (`formal/`). It is a design
contract and an internal test discipline, not a whole-pipeline proof. See
[docs/architecture.md](docs/architecture.md) and Experiments §AJ/§AX.

## Clone types

Against the standard taxonomy (Roy, Cordy & Koschke, 2009):

- **Type-1** (identical except whitespace/comments) — caught by the `syntax` CPD floor.
- **Type-2** (renamed identifiers/types/literals) — identifiers/types converge in the
  normalized unit channels; literal changes are handled by `near`.
- **Type-3** (statements added / removed / changed) — opt in with `--mode near`.
- **Type-4** (same computation, different syntax) — exact modeled equivalence in `semantic`
  (loop ↔ reduce ↔ comprehension, control-flow forms, commutativity, cross-language), sound by
  intent — **not** arbitrary algorithmic equivalence.

See [docs/clone-types.md](docs/clone-types.md) for the precise per-type scope and limits.

## What it does

`nose scan <paths>` finds clone families across files, modules, and languages and
ranks them by **how cleanly each one extracts** into a shared helper, so you review
the duplication you can actually act on first (`--sort value` for raw volume instead).
By default it runs `syntax,semantic`: CPD-style syntax copy-paste plus exact
value-fingerprint Type-4 clones. If you pass `--mode`, that replaces the default
with exactly the channels you list: `syntax`, `semantic`, `near`, or a comma-list
such as `--mode syntax,semantic,near`.

## Install

```sh
# Homebrew (macOS / Linux):
brew install corca-ai/tap/nose

# Or the install script (downloads a prebuilt binary):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/corca-ai/nose/releases/latest/download/nose-cli-installer.sh | sh
```

Prebuilt binaries for macOS (Apple Silicon + Intel) and Linux (x86_64 + arm64)
are attached to every [release](https://github.com/corca-ai/nose/releases).

## Quick start

```sh
# Build from source (requires the Rust toolchain):
cargo build --release

# Default: syntax CPD floor + exact semantic Type-4 clones:
./target/release/nose scan path/to/project

# Markdown report (for a PR / issue):
./target/release/nose scan src --format markdown > REFACTOR.md

# JSON (machine-readable), top 50 families:
./target/release/nose scan src --format json --top 50

# jscpd-style copy-paste gate:
./target/release/nose scan src --mode syntax --fail-on any

# Exact semantic-only audit:
./target/release/nose scan src --mode semantic

# Include Type-3 near-duplicates (near takes an inline acceptance threshold):
./target/release/nose scan src --mode syntax,semantic,near:0.70
```

### Example

```
$ nose scan examples --min-size 8
scanned 3 files · go 1 · python 1 · typescript 1
1 clone family, ranked by extractability (cleanest to fold into one helper)  ·  ~30 duplicated lines  (showing 1)

#1  id b658f483dcc2b097 · 6 copies · same logic in 3 languages (go, python, typescript) · ~30 lines removable
    → local duplication — extract a helper (cross-language)
    examples/sum.go:3-9    SumFor
    examples/sum.go:11-17  SumRange
    examples/sum.py:1-7    sum_while
    examples/sum.py:10-14  sum_for
    examples/sum.ts:1-7    sumFor
    examples/sum.ts:9-15   sumOf
```

(Runs on the repo's own `examples/` — six tiny sum routines across three languages and
several loop forms, all one family. `--min-size 8` because the demo functions are tiny;
the default minimum is 24 IL tokens.)

Each family is described in plain language — how many copies, how much is actually
shared vs varies, how many lines you could remove — and **every site is listed** so you
can go act on it. The one-line **hint** (`→`) is grounded in the facts (a shared symbol
name, cross-language spread, how many modules it touches), never a guess about
semantics. For same-language families the description reads `N of M lines identical, K
spots differ`; add `--show proposal` to see the extracted helper and `--show diff` to see
exactly what varies.

Each **family** is one refactoring decision (extract a shared helper / base class /
data table). By default families are ranked by **extractability** — the *invariant*
lines you'd actually fold into one helper (`N/M shared`), weighted by how much of each
copy is invariant and dampened by how many parameters that helper needs — so a tight,
cleanly-extractable pair outranks a big block whose copies merely look similar.
`--sort hazard` is an experimental *divergence-propensity* view (see
[hazard-ranking](docs/hazard-ranking.md)); `--sort value` ranks by raw duplicated
volume; `--sort sites` by copy count.

## Pipeline

```
source ──tree-sitter──▶ raw IL ──normalize──▶ canonical IL ──▶ units + features
                                                                      │
                                       MinHash + LSH candidate gen ◀──┘
                                                  │
                          structural + value-graph scoring ──▶ clusters ──▶ ranked families
```

Normalization (the `normalize` stage above) is covered in [How it works](#how-it-works)
and [docs/normalization.md](docs/normalization.md). **Detection** (`nose-detect`) then
runs three selectable channels feeding one ranking:

- *Syntax* — a Rabin-Karp scan over each file's IL token stream that finds duplicated
    runs regardless of unit boundaries: the jscpd-style Type-1/2 floor.
  - *Semantic* — exact value-fingerprint matches: modeled Type-4 equivalence with the
    fingerprint-equal ⇒ behavior-equal contract.
  - *Near* — shape-candidate Type-3 near-duplicates scored by structural/value overlap
    and RANSAC alignment; opt in with `--mode near` or a comma-list containing `near`.

## CLI

- `nose scan <paths…>` — ranked clone families.
  - rank/filter/shape: `--sort extractability|value|sites|hazard` (default
    `extractability`), `--top N`, `--min-members N`, `--min-value V`,
    `--min-size N`,
    `--mode syntax|semantic|near` (comma-list/repeatable; omitted = `syntax,semantic`),
    `--exclude <glob>` (gitignore-syntax; `.gitignore` is respected automatically,
    even outside a git repo).
  - review: `--show diff|proposal|hotspots` (repeatable/comma-list) — `diff` shows each
    family as a unified diff of its two copies, `proposal` the shared skeleton with varying
    spots as `⟨param N⟩`, `hotspots` directories ranked by duplicated lines;
    `--format human|json|markdown|sarif`.
  - workflow: `nose.toml` config (`[scan]`), `--baseline <file>`,
    `--write-baseline`, `--fail-on any|new` (CI gate),
    `--cache-dir <dir>` (fast re-runs).
  - inline `// nose-ignore` marks a clone as intentionally kept.
- `nose il <file> [--normalized] [--no-cfg-norm] [--format sexpr|json]` — inspect the IL.
- `nose stats <paths…>` — IL lowering coverage per language.

A `detect` command (raw clone pairs/groups) and `eval`/`ceiling` (benchmark
scoring against a gold set) also exist as the strict/research surface; they're
hidden from `--help` because `scan` is the command for everyday use.

## Documentation

New to nose? **[Getting started](docs/getting-started.md)** walks through your first
scan and how to read the report. The full [`docs/`](docs/home.md) wiki is grouped by
what you're here to do:

- **Using nose** — [Usage](docs/usage.md), [Configuration](docs/configuration.md),
  [Continuous-Integration](docs/continuous-integration.md),
  [Structured-Ignores](docs/structured-ignores.md),
  [Clone-Types](docs/clone-types.md), [Languages](docs/languages.md).
- **Integrating nose** — [Capabilities](docs/capabilities.md),
  [Scan-JSON](docs/scan-json.md).
- **Contributing** — [Architecture](docs/architecture.md),
  [Normalization](docs/normalization.md), [Experiments](docs/experiments.md),
  [Benchmark](docs/benchmark.md), [Field-Evaluation](docs/field-evaluation.md),
  [Dogfooding](docs/dogfooding.md).

## Crates

| crate | role |
|---|---|
| `nose-il` | arena IL model, provenance spans, interner, serialization |
| `nose-frontend` | tree-sitter parse + per-language CST→IL lowering |
| `nose-normalize` | normalization passes + value graph (GVN) |
| `nose-detect` | fingerprints, LSH, scoring, clustering, refactor ranking |
| `nose-eval` | benchmark scoring (precision/recall, pooled, stratified) |
| `nose-cli` | the `nose` binary |

## Status

Pre-1.0. Languages: Python, JavaScript, TypeScript (with JSX/TSX), Go, Rust, Java, C, Ruby,
and the embedded `<script>` of Vue/Svelte/HTML (IL lowering coverage ≈ 99.99% — Raw-node
ratio < 0.01% on the vendored corpus). Output is **deterministic** — byte-identical
across runs, thread counts, *and* machines. The pipeline is parallel and frontend-bound
(parse+lower scales ~11.6× across cores); per-file throughput is corpus-dependent —
reproduce with `NOSE_TIME=1 nose scan <path>` (≈19.5k files/sec warm; see
[experiments](docs/experiments.md) §T).

Correctness is anchored by **cross-language convergence tests**: the same algorithm written
in different languages (and equivalent forms — `for`/`while`, ternary/`switch`, comprehension/`.map`,
f-string/template/interpolation, guard clauses, De Morgan) must normalize to one IL hash, while
behaviorally different code (sum vs product) must not. See `docs/experiments.md` for the
methodology and the bugs this discipline caught (§S).

## Quality gates

`./scripts/check-ci-local.sh --fast` is the PR/push preflight: rustfmt, clippy
(`-D warnings`), `nose-cli` tests, and docs wiki lint. `./scripts/check.sh` is the
full local CI mirror, including release build/tests, supply-chain checks, MSRV,
Lean proofs, and the self-hosted duplication gate. Lint policy lives once in
`[workspace.lints]`. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

MIT
