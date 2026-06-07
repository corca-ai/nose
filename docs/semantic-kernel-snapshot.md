# Semantic kernel snapshot

Back to [semantic-kernel](semantic-kernel.md). This page records the current
implementation shape; planned work and decision history live in
[semantic-kernel-roadmap](semantic-kernel-roadmap.md).

Snapshot date: 2026-06-07, `feature/semantic-kernel-migration` worktree against
`origin/main`.

## What exists today

nose now has a first internal semantic-kernel facade, but most of the engine is
still being migrated toward it.

- `nose-il` defines a compact shared IL, `Lang`, `Builtin`, `HoFKind`, operators,
  literals, source spans, units, and parameter semantic facts.
- `nose-semantics` defines the first-party semantic profile facade: language,
  operator, effect, fragment, module, stdlib, builtin, method-call, property,
  async, iterator-adapter, builder-append, and factory contracts.
- `nose-frontend` owns tree-sitter parsing, per-language lowering, embedded
  `<script>` extraction, import facts, and Raw-node coverage.
- `nose-normalize` owns desugaring, alpha-renaming, recursion normalization,
  dataflow, CFG/algebra normalization, type-gated value-graph rules, and the
  interpreter oracle.
- proof-sensitive value-graph laws are starting to move into named rule modules
  under `crates/nose-normalize/src/value_graph/rules/`; `clamp` and
  `promise_then` are the current examples.
- `nose-detect` owns unit extraction, exact fragment contracts, effect fragments,
  value/shape features, candidate generation, clustering, and ranking.
- `formal/obligations` records proof obligations for proof-sensitive rules.

The current model already enforces the main product principle: exact semantic
matches must be fail-closed and false merges are bugs.

## Implemented facade contracts

The current facade is compiled Rust, not an external manifest schema. It is
intended to make the future pack extension boundary explicit while behavior is
migrated.

- Free-function builtin contracts are language- and arity-constrained and require
  unshadowed builtin/global proof before exact lowering.
- Method contracts carry receiver obligations such as exact collection, exact
  protocol, exact option, exact string, exact primitive integer, exact map literal,
  imported namespace, or unshadowed global.
- Property builtin contracts are language-constrained; a selector such as
  `length` is not enough without receiver proof. JS/TS `filter(...).length`
  is admitted only after the receiver has already entered a proven collection/HOF
  value.
- Promise `.then` has a JS-like surface contract, but exact beta-reduction is
  closed until a pack/frontend can prove a Promise-like receiver.
- Rust iterator identity adapters (`iter`, `into_iter`, `collect`, `to_vec`,
  `copied`, `cloned`) are language-, arity-, and receiver-proof constrained.
- Builder append contracts are separate from arbitrary method calls: Java `add`
  and Rust `push` are admitted only for active builder proofs.
- Exact fragment surface proofs for Java `this.field`, Java `return this`,
  non-overloadable C/Go/Java index assignment, and single-item builder append
  calls are now shared through `nose-semantics`; predicate and contract paths
  consume the same IL-level proof helpers.
- Collection reductions such as Rust `Iterator::count()` and Java
  `Stream.count()` are admitted through exact protocol receiver contracts, not
  through a bare method-name check.
- Map-key membership contracts now require map receiver proof. Examples include
  Java `Map.containsKey`, Java `keySet().contains`, Rust `contains_key`, Rust
  `get(key).is_some()`, and TypeScript `Array.from(map.keys()).includes(key)`
  when the receiver is a typed/proven map.
- JS/TS `new Map(...)` and `new Set(...)` remain closed because lowering does not
  yet retain a constructor proof distinct from ordinary `Map(...)`/`Set(...)`.

## Scattered semantic knowledge

Semantic knowledge still appears in several forms outside the facade:

- direct `Lang` checks and local recognizers in strict exact gates and value-graph
  rules that have not yet been expressed as shared contracts;
- factory recognizers such as Python collection factories, Ruby `Set.new`, Rust
  `Vec::new`, Java `List.of`, and Java/Rust map factories;
- module/import proof logic for immutable sibling-module literal bindings;
- type facts and coarse type inference used to gate numeric and collection laws;
- named value-graph rule modules that still consume internal `Builder` facts
  instead of versioned `LawPack` records;
- hard-coded oracle evaluation rules for eager calls, short-circuit operators,
  HOFs, nullish defaulting, recursion, and effect traces;
- duplicated receiver/domain and library/API proof gates in desugaring,
  idiom lowering, value-graph, and strict exact paths.

These are valuable, but they do not yet share one complete semantic contract
language.

## Current strengths

- Exact matching is conservative by design.
- The value graph already separates behavioral fingerprints from fuzzy candidate
  structure.
- The oracle models return values, ordered effects, final field state, `Err`
  behavior, short-circuit `and`/`or`, `any`/`all`, HOFs, recursion, and selected
  interprocedural calls.
- Proof-sensitive normalization already has named rule modules and a Lean
  obligation registry.
- Raw-node coverage gives a practical measure of lowering gaps.
- Convergence tests and hard negatives catch many semantic boundary mistakes.

## Current limits

- Language semantics are not first-class. Many rules ask "which language is this?"
  instead of "which semantic capability has been proven?"
- Library semantics are embedded in engine code rather than declared as versioned
  API contracts.
- Evaluation strategy is not a shared model. Eager, short-circuit, pull-lazy,
  call-by-need, async, and observable behavior are not represented by a common
  demand/effect abstraction.
- External extension points do not exist. New languages and libraries must be
  added inside the main crates.
- Report output does not yet expose semantic provenance such as pack id, contract
  id, law id, or proof status.
- First-party and external responsibility boundaries are not represented because
  there are no loadable external packs yet.

## Current fail-closed choices

Several older convergence expectations are intentionally disabled or narrowed in
this worktree because the required evidence is not yet modeled:

- JS-like `.then(lambda)` does not converge with `await` code until Promise-like
  receiver proof exists.
- Plain JS/TS `Map` and `Set` constructor semantics do not enter exact matching
  until constructor-vs-call proof exists.
- Untyped JS/TS array method chains do not enter exact higher-order contracts
  unless the receiver is a literal/proven collection surface.
- Nested element method chains such as `xs.map(...)` inside a flat-map callback
  stay closed unless the nested element collection proof is available. Explicit
  nested builder loops can still converge with identity flat-map when their loop
  structure proves the emitted elements.
- Ruby untyped `Enumerable` methods, Ruby scalar/array `abs`/`min`/`max`, and C
  `fmin`/`fmax` remain closed until the relevant receiver, stdlib, and overload
  facts are modeled as contracts.
- Rust scalar `.abs`, `.min`, `.max`, and `.clamp` are admitted only for the
  current first-party primitive-integer domain. Rust float methods need a separate
  float/NaN contract and proof before they can enter exact matching.

These reduce recall in affected cases, but they are the correct precision trade
until packs can emit the missing facts.

## Known migration targets

The first high-value targets for semantic-kernel extraction are:

- field/place identity for field reads and writes, replacing field-name-only
  state with receiver-aware proof;
- constructor facts for JS/TS `new Map` and `new Set`;
- resolved symbol facts for Java/Rust stdlib factories instead of path/name
  heuristics;
- nested collection element proofs for iterator chains and builder convergence;
- Promise/future/thenable receiver facts;
- receiver/domain evidence records to replace `ParamSemantic` as the internal
  proof vocabulary for collection, map, option, string, integer, and byte-array
  domains;
- demand/protocol contracts that distinguish eager arrays, lazy iterators,
  streams, callbacks, futures/promises, and call-by-need thunks;
- demand/error contracts for language-core oracle behavior such as non-iterable
  `for`/`foreach` evaluation;
- LawPack-facing ids for named value-graph rules, with the existing formal
  obligation metadata kept as the first-party proof boundary;
- module export visibility, path resolution, and mutation proof;
- provenance fields in scan JSON for pack id, contract id, law id, and evidence
  status.
