# Clone types — what nose covers

The standard clone taxonomy is from Roy, Cordy & Koschke, *"Comparison and evaluation of
code clone detection techniques and tools: A qualitative approach"*, Science of Computer
Programming (2009) — <https://www.sciencedirect.com/science/article/pii/S0167642309000367>.
The four types:

- **Type-1** — identical fragments except whitespace, layout, and comments.
- **Type-2** — syntactically identical except identifiers, literals, and types (plus Type-1
  variations).
- **Type-3** — copied fragments with statements changed, added, or removed (plus Type-2
  variations).
- **Type-4** — fragments that perform the same computation but are implemented by different
  syntactic variants (semantic clones).

This page states what nose does for each — including where it stops. The scan modes are
detector channels, not perfect taxonomy buckets: the default combines `syntax` and
`semantic`, and `near` is the opt-in fuzzy Type-3 surface. The engine is described in
[architecture](architecture.md).

## Type-1 — fully

Whitespace, layout, and comments never enter the IL, so Type-1 fragments produce identical
fingerprints. They are caught by the same-language `syntax` CPD floor and by the unit
fingerprints.

## Type-2 — identifiers and types fully; literals on a two-axis split

- **Identifiers** are alpha-renamed to canonical ids in the normalized unit channels, so
  renamed copies can converge under `semantic` or `near`.
- **Types** are erased during normalization.
- **Literal values** are handled deliberately on two axes. The *behavioral* fingerprint
  RETAINS behavior-defining literals (`0` ≠ `1`, `true` ≠ `false`, distinct strings/floats) —
  different literals are different behavior. The *structural* fingerprint abstracts them to
  their class. So a Type-2 clone that differs only in literal values is matched by
  `--mode near`, but deliberately kept distinct by exact `--mode semantic`.

`--mode syntax` is intentionally the CPD floor: it is excellent for same-language copied
runs that a token detector should catch, including runs that cross function boundaries, but
it is not the renamed/literal-varied Type-2 story by itself. Use the default (which
includes `near`) when renamed or literal-varied copies matter.

The hidden experimental `abstraction` mode narrows one slice of this space for tools:
it starts from low-threshold `near` candidates, then reports only same-language pairs
whose normalized IL differs by exactly one supported literal leaf. The output is a typed
template witness, not a claim that the copies are behavior-equivalent.

## Type-3 — near-duplicate via similarity (the primary use)

Unit pairs are scored by value-graph + shape similarity and structural alignment, and
accepted above a threshold (0.70 by default in `--mode near`). A copy with added/removed/changed
statements scores below 1.0 but above the threshold, so it surfaces as a near-duplicate
family. How much divergence still matches is bounded by the threshold (raise it for tighter
matches, lower it for more recall).

## Type-4 — a modeled subset, not arbitrary equivalence

This is nose's distinguishing capability, but it is **bounded, not total** — arbitrary
semantic equivalence is undecidable. nose converges the equivalence classes its IL, value
graph, and canonicalizations actually model:

- loop ↔ `reduce`/`sum` ↔ comprehension ↔ proof-backed `.append`/`.map` builder loop;
  nested list builders ↔ Python multi-clause comprehensions ↔ proven flat-map
  surfaces; inner method chains such as `xs.map(...)` require nested element
  collection proof before they enter exact matching; an `any`/`all` loop ↔ the
  functional form; pure aggregate consumers over flat-map streams ↔ nested
  reduction loops; a `reduce`-lambda min/max ↔ `max`/`min`;
  `len([… if p]) ↔ Σ(p?1:0)`; a dict-building loop `d={}; for x: d[k]=v` ↔ the dict
  comprehension `{k: v for x in xs}`;
- literal map-default lookup through proven map/key/fallback coordinates, including
  Python `dict.get(key, fallback)` and Ruby literal `Hash#fetch(key, fallback)` or
  zero-arg pure fallback blocks such as `Hash#fetch(key) { fallback }`, plus Python
  sibling-module `from module import LOOKUP` literal bindings, Java static imports from
  class `Map.of` fields, and Rust `use` imports of const entry arrays consumed by
  `HashMap::from`/`BTreeMap::from` when the provider binding is unique, immutable, and
  unambiguous;
- `filter q (filter p) ↔ filter (p∧q)` (and the filtered comprehension / `.filter().filter()`
  chain / filtered builder loop), plus the direct Rust `filter_map` / guarded-builder slice
  where `None` means absence and emitted falsey values remain values;
- guard-clause ↔ nested-if, ternary ↔ early-return, min/max and proof-backed integer clamp
  idioms, commutativity +
  associativity (AC-canonical operands), distribution `a·c + b·c ≡ (a+b)·c` (numeric),
  `a − b ≡ a + (−b)`, De Morgan, short-circuit `and`/`or`;
- the same algorithm written in a **different language** when the relevant API
  contracts and receiver domains are proven (for example Rust
  `it.iter().filter(p).sum()` ↔ Python `sum(x for x in xs if p)` under a collection
  proof).

For the classes it captures, equal fingerprint -> equal behavior is the design invariant:
guarded by the hidden `nose verify` oracle, regression tests, and Lean obligations for the
core canonicalizations, but not a per-scan or whole-pipeline proof. See
[normalization](normalization.md) for the full pass list.

### What nose does *not* do (no overclaim)

- **Different algorithms with the same result** — e.g. bubble sort vs quicksort, or two
  different primality tests — are **not** recognized. Only the modeled transformations
  converge; nose does not search for arbitrary input/output equivalence.
- **General collection flattening** — depth-parameterized flattening such as
  `Array.prototype.flat(depth)` is not canonicalized. The modeled flattening is the
  one-level flat-map shape (`FlatMap[A, λa. Map[B, λb. e]]`) used by Python multi-clause
  comprehensions, proof-backed JS `.flatMap`, and equivalent nested builder loops.
- **Name-only library semantics** are not accepted. A method named `map`, `then`,
  `collect`, or `contains` is exact only when the language, symbol/receiver,
  shadowing/import, arity, and effect/demand obligations for that API are proven.
- **Unproven method calls share an opaque identity keyed by the method name.** An
  unresolved `obj.foo(x)` is modeled as one opaque operation keyed by `foo`, so two
  callers of same-named methods on *different* receiver types — or whose `foo` bodies
  differ but nose cannot resolve which is called — converge. Resolving them needs
  receiver-type analysis the value model erases. A consequence: a behavior-changing
  decorator on a method (`@cached`, `@retry`) is invisible to its callers' fingerprints
  (the decorator is dropped in lowering and the call is name-keyed) — see
  [experiments §CG](experiments.md). Free-function calls do NOT share this hazard:
  decorated function definitions fail closed (no `DirectFunction` evidence).
- **Recursion ↔ iteration** is partially modeled — a bounded subset converges (see
  [normalization](normalization.md)); general recursion↔iteration remains out of scope.
- The behavioral *proof* (`nose verify`) covers only interpretable (≈ pure) units; detection
  itself runs on every unit via the fingerprint, but pairs outside that interpretable slice
  carry no per-pair behavioral proof.
- Type-4 coverage is a **growing set of modeled equivalences**, not a guarantee about any
  given pair of semantically-equal fragments.
- **JS loose equality is fail-closed outside the null-ish contract**: `a == b` over
  non-null operands does not merge with `a === b`, because the value model does not
  implement JS coercions such as `false == 0`, `"0" == 0`, or `[] == 0`. Loose
  equality still converges with the same loose surface, including symmetric operand
  order. Null-ish checks are handled precisely — `x == null` remains the modeled
  nullish check that can converge with `x ?? d`, while `x === null` never merges
  with `x == null` / `x ?? d`, and `null` and `undefined` spellings stay distinct
  in strict checks.
- **JS `instanceof` is exact-closed rather than equality-modeled**: `x instanceof C`
  and `!(x instanceof C)` can converge with the same membership surface, but they
  do not merge with `x === C` or `x !== C`. The value model does not attempt to
  prove prototype-chain type membership equivalent to other predicates.
- **JS-family relational comparisons need numeric proof**: untyped `<`, `<=`, `>`,
  and `>=` stay exact-closed because JS compares two strings lexicographically but
  otherwise coerces toward numeric comparison (`"2" < "10"` differs from `2 < 10`).
  Scalar/literal/operator-proven numeric TypeScript comparisons still use the
  primitive comparison laws. TypeScript `number[]` callback or loop elements are
  not yet treated as numeric proof, so those higher-order relational predicates
  stay closed until element-domain facts are modeled. Negated order comparisons
  such as `!(a < b)` and branch-swapped order conditionals lower to the apparent
  dual (`a >= b`) only with integer-domain proof, because `NaN` makes the two differ.
- **Float-sensitive min/max APIs stay exact-closed**: the modeled Min/Max value
  nodes represent integer/ternary selection idioms. JS-family `Math.min`/`Math.max`
  and Go `math.Min`/`math.Max` return `NaN` if any argument is `NaN`; Java
  `Math.min`/`Math.max` lower only with unshadowed `Math` plus integer-domain
  proof for their value arguments. Free `min`/`max` lowering needs integer-domain
  proof before it can use the modeled Min/Max node.
- **Float-sensitive abs APIs stay exact-closed**: JS-family `Math.abs` and Go
  `math.Abs` differ from ternary absolute-value idioms on signed zero; Java
  `Math.abs` likewise lowers only with unshadowed `Math` plus integer-domain
  proof. Python free `abs(...)` and sign-test ternary absolute-value idioms also
  need integer-domain proof before they use the modeled Abs node. Float overloads
  need a signed-zero-aware numeric model before entering exact matching.
- Sub-function semantic coverage is intentionally bounded: nose extracts control-flow
  blocks and exact-safe single-statement fragments (return/throw expressions and simple
  conditional return/throw/effect guards, including bare returns, explicit empty no-op
  branches, and nested branches whose only non-empty statement is another exact
  conditional, a single exact ForEach effect loop, or exactly two ordered exact effect
  items drawn from ForEach effect loops, append effects, conditional direct effects, and
  non-overloadable C/Go/Java index assignments, plus branches that assign one local
  temporary and immediately consume it in a direct return/throw/effect statement or
  assign two local temporaries as a linear chain and immediately consume the final
  temporary in such a statement or a non-overloadable C/Go/Java index assignment whose
  receiver does not depend on the temporary, plus ForEach loops whose only loop-body
  effects are appends or non-overloadable C/Go/Java index assignments that depend on the
  iteration binding,
  optionally preceded by one loop-local temporary assignment or a two-temporary linear
  chain whose first RHS depends on the iteration binding and whose final value is
  immediately consumed by that effect, plus exact conditional branches containing two or
  three ordered single-item append effects where each item is direct or immediately
  consumes a branch-local temp/temp-chain, plus exact conditional branches containing two or
  three ordered non-overloadable C/Go/Java index-assignment effects where each assignment is direct or
  immediately consumes a branch-local temp/temp-chain, plus Java
  `this.field = value` self-field assignments and all-self-field Java function-body blocks
  with the receiver fixed to `this`, optionally ending in `return this`), not arbitrary
  statement windows with unmodeled free-variable, live-out, receiver-overload, or effect
  boundaries. Multiple statement-level effects are ordered through control-flow-aware
  sink tags; swapping appends/emits on one execution path is a behavior change, not a
  Type-4 clone.
Exact fragment proof is not the same thing as user-facing refactorability. Fragment
locations carry stable proof metadata (`is_fragment`, `fragment_kind`, `reason_code`,
span size, and `enclosing_unit` when recoverable), but product placement is decided
separately with `recommended_surface`: default, review, or hidden. See
[fragment-contracts](fragment-contracts.md) for the exact-fragment contract and
[scan-json](scan-json.md#fragment-metadata) for the stable output fields.

## Scan modes, and cross-language

Each type maps to a detection channel by evidence surface: **Type-1 and token-level
copy floors → `syntax`**, identifier/type-normalized **Type-2 → `semantic` or `near`**,
literal-varied Type-2 and **Type-3 → `near`** (fuzzy; the threshold rides on the mode,
`near:0.70` by default), and exact **Type-4 → `semantic`**. The experimental
`abstraction[:T]` mode is a weak family-wide witness layer over a narrow `near` subset;
it does not feed `semantic` or `verify`. The default is `syntax,semantic,near`; see
[usage → Scan modes](usage.md#scan-modes) for the full table and how to compose channels.

The taxonomy is usually stated within a single language; because every language lowers to
one shared IL, nose applies Type-1–4 **across languages** as well — though in practice
rarely, since cross-language fragments seldom converge to the same fingerprint.
