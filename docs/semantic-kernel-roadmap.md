# Semantic kernel roadmap

Back to [semantic-kernel](semantic-kernel.md). Current code shape is recorded in
[semantic-kernel-snapshot](semantic-kernel-snapshot.md).

This page tracks decisions, history, and remaining work for the semantic kernel
and pack ecosystem.

## Decisions

1. **All language and library semantics should eventually enter through packs.**
   First-party languages are not special at the API boundary; they may be compiled
   into nose, but they should use the same pack contracts as external languages.

2. **nose certifies only first-party packs.** External pack providers own their
   semantic claims. Users own the decision to enable them. nose owns the extension
   contract, validation of pack structure, fail-closed execution, and provenance
   reporting.

3. **Packs emit evidence, not verdicts.** A pack can emit facts, contracts, and
   protocol operations. It cannot mint fingerprints, bypass laws, or approve exact
   clones.

4. **Language core and library semantics are separate layers.** Evaluation order,
   truthiness, overloadability, and exception behavior belong to language core.
   `sum`, `Iterator::map`, `Array.prototype.map`, RxJS `Observable.map`, and NumPy
   vector operations belong to stdlib/library packs mapped onto protocols.

5. **Demand and effect are first-class.** Lazy evaluation, short-circuiting,
   iterators, async/futures, and observables cannot be accurately modeled with a
   simple purity flag. Exact laws need demand and effect preconditions.

6. **Unknown stays fail-closed.** Missing type, receiver, symbol, version,
   shadowing, or effect evidence must block exact semantic acceptance. It may
   still inform `near` scoring.

7. **Selectors are not proof.** A function or method name is only a selector.
   Exact contracts must also declare and check the language, symbol/namespace,
   arity, receiver/protocol, shadowing, import, version, overload, demand, and
   effect obligations that make that selector mean the claimed operation.

## History

- The original architecture lowered every supported language into one shared IL,
  then normalized toward common fingerprints.
- The value graph became the behavioral fingerprint substrate, separating exact
  semantic matching from fuzzy structural candidate generation.
- The independent interpreter oracle was added to test fingerprint-equal units
  against concrete behavior and catch behavior-changing canonicalizations.
- Lean proof obligations were added for proof-sensitive rules.
- Exact fragments gained explicit contracts, effect classifications, and
  fail-closed receiver/place boundaries.
- Dogfooding surfaced repeated per-language frontend shapes; safe common helpers
  moved into `lower.rs`, while grammar-specific parallelism remained explicit.
- Documentation review in PR #89 clarified the current limits: exact Type-4 is a
  modeled subset, not arbitrary semantic equivalence.
- The semantic-kernel direction was chosen to make language and library semantics
  an explicit extension boundary rather than scattered engine code.
- The first internal facade landed as `nose-semantics`, wrapping first-party
  language/profile predicates and API contracts while the rest of the pipeline is
  migrated.
- Name-only contracts were narrowed: JS/TS `Map`/`Set` constructors, JS-like
  `.then`, untyped JS collection methods, and Rust iterator adapters now require
  explicit proof or remain exact-closed.
- Additional call surfaces moved behind proof-gated contracts: JS/TS
  `filter(...).length`, Rust `get(key).is_some()`, Java `keySet().contains`,
  and Java `Stream.count()` require receiver/protocol or map proof. Ruby untyped
  `Enumerable` surfaces, including `.each`/`.each_with_index` block loops, and
  scalar/array numeric helpers remain closed until comparable proof facts exist.
- New value-graph rewrites began moving into named `rules/*` modules with
  mechanical formal-obligation pairing; `clamp` is the current proof-backed
  example.
- Per-semantic parameter recognizers were folded into `is_param_value`, making
  `ParamSemantic` the current internal vocabulary for receiver/domain proof
  facts until packs provide versioned evidence records.
- Rust scalar integer methods (`abs`, `min`, `max`, `clamp`) now consume a
  language-, signature-, and integer-domain-constrained first-party contract
  instead of a bare method-name recognizer. Float/NaN-sensitive methods remain a
  separate future contract.
- Exact fragment IL-surface proofs for Java `this.field`, Java `return this`,
  non-overloadable C/Go/Java index assignment, and single-item builder append
  calls moved into `nose-semantics`, so predicate and contract paths no longer
  duplicate those language/API gates.
- The first receiver-domain evidence facade landed as `DomainEvidence`.
  Frontend `ParamSemantic` facts still provide the current evidence source, but
  normalize and detect exact gates now consume the kernel-facing domain
  vocabulary so pack-provided evidence can replace the source fact later.
- Rust stdlib path contracts for `Some`/`Option::Some`,
  `None`/`Option::None`, `Option::and_then`, and `Vec::new` moved into the
  kernel facade with explicit shadow-root obligations. The caller still proves
  local shadow safety, and the Rust frontend no longer lowers bare `None`
  directly to null before that proof.
- Java collection/map factory selectors, Python free-name/imported collection
  factories, Rust std collection/map factory paths, and Ruby `Set.new` moved
  behind shared `nose-semantics` contracts. Normalize, strict exact gates, and
  corpus import proof now consume the same selector source while keeping local
  import, require, shadow, mutation, and entry-shape proof at the caller.
- Membership and map-key membership recognition now uses language-scoped method
  contracts before normalization or strict exact matching assigns containment
  semantics. This intentionally closes old name-only paths such as JavaScript
  `.contains(...)`, which had no first-party JS membership contract.
- Java stream source adapters are now proof-gated: receiver `.stream()` requires
  exact iterable evidence, and static `Arrays.stream(xs)` requires the
  `java.util.Arrays` import binding with no local `Arrays` type shadow.
- Cross-file immutable import replacement now copies import-binding dependencies
  required by the exported literal expression, preserving provider-side stdlib
  proofs such as `java.util.Map` for Java static imports.
- JS-like `Map`/`Set` constructors are now represented as explicit closed
  contracts requiring construct-syntax proof; they remain exact-closed until the
  frontend/kernel can distinguish `new Map(...)` from plain `Map(...)`.
- Map key-view recognition moved behind contracts that distinguish collection
  views from iterator views. JS-like `Map.keys()` now requires an
  `Array.from(...)` wrapper before exact membership can consume it.
- Go composite map literal/default-zero lookup recognition moved behind a shared
  contract for literal/entry tags and supported zero-default payload classes.
- Map `get(key)` lookup surfaces for Java, Rust, and JS-like typed/proven maps
  moved behind an explicit map-get contract. Defaulting surfaces continue through
  the existing `GetOrDefault` method contract.
- JS-like static array `indexOf`/`findIndex` membership and their accepted
  threshold comparisons moved behind shared semantic contracts.
- Channel eligibility and pack trust were split: first-party/default status is
  provenance and enablement policy, not a semantic channel.
- Newly migrated selector contracts started carrying explicit receiver/proof
  requirements so extension APIs do not look like name-only semantic guesses.
- Python `math.prod` product-reduction recognition moved behind an imported
  namespace function contract with missing-import and overwritten-binding hard
  negatives.
- Java `Math.abs`/`Math.min`/`Math.max` moved out of frontend text-only lowering
  and into method contracts that require an unshadowed `Math` receiver.
- JS-like `undefined` moved from unconditional frontend null lowering to an
  unshadowed-global nullish contract, preserving shadowed binding hard negatives.
- Strict exact gates now consume the same nullish-global proof, so temp-bound
  JS/TS `Map.get(...)` defaulting remains exact-eligible only when `undefined`
  is the unshadowed JS-like sentinel.

## Phase 0: documentation and vocabulary

- Define semantic-kernel goals, non-goals, responsibility model, and pack kinds.
- Record the current implementation snapshot separately from the roadmap.
- Link the direction from home, architecture, languages, and formal-soundness.
- Keep docs honest: this is planned architecture, not current user-facing
  capability.

## Phase 1: kernel facade and fail-closed migration

- Add a `nose-semantics` crate or module with stable internal types for language
  profile, semantic facts, evaluation rules, effect summaries, protocol ops, API
  contracts, law ids, and proof status.
- Implement first-party built-in profiles by wrapping existing `Lang` matches.
- Replace direct semantic `Lang` checks with named predicates where behavior is
  already sound.
- Tighten old name-only recognizers when the required proof does not exist yet,
  even when this changes old convergence behavior.
- Add tests proving language/arity/shadowing/receiver obligations for the facade.
- Keep parser/lowering dispatch unchanged in this phase.

## Phase 2: shared contracts for duplicated gates

- Move primitive comparison gates behind `OperatorSemantics`.
- Expand the exact fragment facade from first-party helper functions into
  versioned pack-facing effect/place evidence records.
- Move collection/map factory recognition into `LibraryApiContract` records.
- Make value-graph and strict exact gates consume the same contract source.
- Replace the current `DomainEvidence` facade with versioned, pack-facing
  receiver/domain evidence records while preserving the current precision gates.
- Turn named value-graph rule modules into LawPack-facing law ids/contracts while
  retaining formal-obligation metadata as the first-party proof boundary.
- Add construct/call distinction facts so constructor-only contracts such as
  JS/TS `new Map` and `new Set` can be reopened safely.
- Add receiver/place facts so field read/write and property contracts are not
  field-name-only.
- Add provenance fields internally before exposing them in scan JSON.

## Phase 3: first-party packs

- Convert Python, JavaScript/TypeScript, Go, Rust, Java, C, Ruby, and embedded
  JS/TS containers into first-party compiled packs.
- Split stdlib knowledge into first-party `StdlibPack`s.
- Define conformance manifests for each pack: positive convergence cases, hard
  negatives, Raw coverage expectations, oracle coverage, and proof obligations.
- Ensure existing docs and capabilities are generated from or checked against pack
  metadata.

## Phase 4: external pack contract

- Define a versioned pack manifest schema.
- Start with data-only external packs for simple APIs.
- Add restricted recognizer hooks only after the manifest path is stable.
- Require pack metadata: provider, license, version range, supported analysis
  channels, evidence status, conformance commands, and semantic provenance ids.
- Document the pack conformance checklist as part of the extension schema; make
  clear that conformance evidence is provider/user responsibility unless the pack
  is first-party.
- Add user configuration for enabling external packs explicitly.
- Report which external packs influenced `near` candidates and exact matches.

The external schema must make proof obligations first-class. For example, a pack
claiming `pkg.Foo.map` maps to the `Map` protocol must say how `pkg.Foo` is
resolved, which versions it covers, how callback demand works, whether effects
are delayed or eager, and which hard negatives distinguish it from same-named but
different APIs.

## Phase 5: demand-aware semantics

- Model child demand: always, never, conditional, per-element pull,
  short-circuit-until, maybe repeated, and call-by-need memoized.
- Model effect visibility under demand: skipped effects, delayed effects,
  per-element callback effects, async scheduling, yields, and stream emissions.
- Refactor oracle and value graph to consume demand rules instead of local
  hard-coded evaluation behavior.
- Add lazy iterator/generator hard negatives before enabling new exact laws.

## Phase 6: ecosystem packs

- Add high-value first-party packs only when their contracts are narrow and
  testable.
- Keep community packs external and opt-in unless nose explicitly adopts them as
  first-party/default packs with project-owned gates.
- Candidate areas: Lodash, RxJS, NumPy, pandas, Java Streams/Guava, Rust Iterator
  ecosystem helpers, Tokio futures, Rails ActiveSupport collection helpers.
- Keep exact eligibility narrow. Many APIs should stay `near-only` because
  versioning, mutability, callback effects, or dynamic dispatch make exact
  equivalence too risky.

## Open questions

- How much of a pack should be data-only, and when is a restricted recognizer hook
  justified?
- Should external recognizers run as compiled Rust, WASM, or a sandboxed DSL?
- What is the minimum provenance that scan JSON must expose without making reports
  noisy?
- How should users pin pack versions in CI?
- How should conflicting packs or overlapping API contracts be resolved?
- What conformance score is enough for a first-party pack to enter the default
  exact channel?
- Should a pack be able to express language-specific proof-producing lowering
  extensions before the general construct/import/type fact model is complete?

## Near-term acceptance criteria

The first implementation PR should be considered successful if it:

- introduces the semantic-kernel vocabulary and first compiled facade;
- replaces proof-sensitive `Lang`/name matches with named semantic predicates or
  fail-closed contracts;
- records intentional old-behavior changes where missing evidence blocks exact
  convergence;
- keeps tests and docs checks green;
- documents the first-party/external responsibility boundary;
- makes it easier to explain why an exact match was accepted.
