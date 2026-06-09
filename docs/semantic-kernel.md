# Semantic kernel and packs

Back to [home](home.md). Current implementation status is in
[semantic-kernel-snapshot](semantic-kernel-snapshot.md); history and remaining
work are tracked in [semantic-kernel-roadmap](semantic-kernel-roadmap.md). The
post-PR #147 raw/local pocket audit is recorded in
[semantic-kernel-audit-2026-06-09](semantic-kernel-audit-2026-06-09.md), and the
#109 closeout is recorded in
[semantic-kernel-tranche-closeout-2026-06-09](semantic-kernel-tranche-closeout-2026-06-09.md).
The versioned provider-facing extension surface is defined in
[semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md). Source
origin evidence is detailed in [source-facts](source-facts.md); the shared
internal evidence substrate is described in
[evidence-records](evidence-records.md). The current demand/effect contract
model is described in [demand-effect-semantics](demand-effect-semantics.md).

## Context

nose's long-term moat is not "more parsers". It is a precise, extensible model of
what code means across many languages and libraries, strong enough to support exact
semantic clone detection while still practical enough to run on real repositories.

The current engine already has the right instincts: fail-closed exact matching,
language-specific lowering, a value graph, an interpreter oracle, hard negatives,
and Lean proof obligations for the most sensitive rewrites. The missing foundation
is a single semantic boundary. Today, facts about language and library behavior
are spread across frontends, normalization, fragment recognition, import proof
logic, and the oracle. That makes new language work harder than it should be and
makes soundness depend on scattered `Lang` checks.

The semantic kernel is the boundary that all exact semantic reasoning must cross.
The first internal facade now lives in `nose-semantics`; it is still a compiled
first-party implementation for evidence/contract execution. Local external
manifests can be loaded for metadata/provenance reporting, but they are not an
external producer runtime.
The external API design starts at
[semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md): it narrows
the current internal evidence and contract vocabulary into a manifest shape for
future language/library packs. The provider/user conformance workflow is in
[semantic-pack-conformance](semantic-pack-conformance.md).

Evidence records are one kernel input at that boundary. They preserve facts that
the IL deliberately abstracts away, but they do not approve semantic equivalence
by themselves. Source facts are one evidence class; domain, import, symbol,
type, guard, place/effect, selected library API occurrence, and
sequence-surface facts now use the same internal substrate. See
[evidence-records](evidence-records.md) for the record shape and
[source-facts](source-facts.md) for the source-origin vocabulary.

## Goal

Build a research-grade, practical semantic kernel that:

- represents language semantics explicitly: evaluation order, demand/laziness,
  truthiness, nullability, exceptions, mutation, operator dispatch, overloadability,
  and effect observability;
- represents library semantics through versioned packs, not ad hoc builtin
  recognizers;
- lets first-party and external packs add language and library knowledge through
  well-defined extension points;
- keeps the exact `semantic` channel fail-closed unless the required semantic facts
  and contracts are present;
- keeps fuzzy or uncertain knowledge useful in candidate generation and `near`
  ranking without letting it approve exact fingerprint equality;
- records why a semantic match was accepted: which pack, contract, semantic facts,
  laws, and proof or test status were involved.

The standard is: a pack contributes semantic evidence; the kernel validates the
evidence shape and decides whether it is admissible for a given analysis channel
under the user's configured trust policy.

## Non-goals

- Do not make packs arbitrary plugins that can generate fingerprints or approve
  exact equivalence directly.
- Do not require nose to certify every external pack. nose validates the pack
  format and runs fail-closed, but external pack correctness is the responsibility
  of the pack provider and the user who enables it.
- Do not merge language core semantics and library semantics into one table.
  Python call-by-value, JavaScript `Array.prototype.map`, Rust `Iterator::map`,
  RxJS `Observable.map`, and NumPy vector operations are different layers.
- Do not make all library knowledge exact by default. Unresolved symbols,
  monkey-patching, shadowing, overloads, dynamic imports, version uncertainty,
  or missing hard negatives must demote a contract to `near` or reject it.
- Do not hide semantic assumptions inside lowering. Lowering may emit facts, but
  exact semantic acceptance must be decided by the kernel and its contracts.

## Responsibility model

There are two responsibility classes.

**First-party packs** ship with nose and are maintained by the Corca/nose
project. Their exact contracts must be covered by the same quality gates as the
engine: regression tests, hard negatives, the interpreter oracle where
applicable, benchmark checks, and Lean obligations for proof-sensitive laws.

The first-party packs enabled by the default nose distribution are the **default
packs**; nose owns their review, validation, and release quality.

**External packs** are not approved or certified by nose. The provider owns their
semantic claims, version constraints, tests, and documentation. The user owns the
decision to enable them. nose's responsibility is to define the extension
contract, validate that a pack declares its claims in that contract, keep exact
analysis fail-closed when evidence is missing, and surface provenance in reports.

This is similar in spirit to a "DefinitelyTyped for semantics" ecosystem, but the
trust boundary is stricter: a type declaration can be wrong and still compile;
an exact semantic contract can create a false merge. External packs therefore need
clear conformance criteria, and users need visible provenance.

## Pack kinds

Every language and library, including the built-in ones, should eventually enter
through the same extension points.

| pack kind | purpose |
|---|---|
| `LanguagePack` | file detection, parser binding, surface lowering, core language semantics, and language-level proof facts |
| `StdlibPack` | standard library APIs for a specific language/version |
| `LibraryPack` | ecosystem APIs such as Lodash, RxJS, NumPy, Guava, Tokio, or Rails |
| `ProtocolPack` | language-neutral semantic protocols such as `Iterable`, `Iterator`, `Stream`, `Option`, `Result`, `Map`, `Set`, `Future`, `Promise`, `Observable`, and `Tensor` |
| `LawPack` | reusable semantic laws such as map fusion, filter fusion, monoid folds, short-circuit reductions, and nullish defaulting |

First-party packs may be compiled Rust. External packs should start as data-only
manifests for simple APIs, with restricted recognizer hooks added later for cases
that require code. In both cases, packs emit kernel-defined facts and contracts,
not private value-graph operations.

Current named value-graph rule modules such as `value_graph/rules/clamp.rs` are
the internal precursor to `LawPack` law ids. The first compiled first-party pilot,
`nose.value_graph.laws`, reports per-family provenance for numeric
factor-distribution and integer ordered-clamp laws. External packs may declare
evidence and API facts that make a law applicable, but they must not bypass the
first-party law registry or emit private canonical value-graph nodes.

## Extension boundary

Packs may:

- lower source constructs to kernel-defined surface IR;
- emit source and semantic facts such as construct syntax, literal/operator
  provenance, proven type domains, non-shadowed builtins, unique immutable
  imports, primitive array places, pure callbacks, pull iterators, or memoized
  thunks;
- declare API contracts that map resolved symbols to protocol operations;
- declare evaluation and demand behavior for constructs and APIs;
- declare effect summaries and exact/near eligibility;
- provide conformance fixtures, hard negatives, and proof-obligation links.

Packs may not:

- mint arbitrary value fingerprints;
- bypass the law registry;
- mark a pair of units as exact clones;
- assume symbol identity without the required resolution or shadowing proof;
- turn uncertain dynamic behavior into exact facts;
- mutate global kernel behavior outside their declared contracts.

## Contract shape

An API contract is not a name match. A contract must identify the surface it
claims and the evidence required to admit it.

For source-origin distinctions, that evidence should be represented as
kernel-defined source facts rather than recovered later from selectors or raw CST
text. For example, a constructor-only contract should require construct syntax
evidence; a regex-specific contract should require regex-literal or resolved
regex-receiver evidence; an equality-sensitive contract should require the
language-specific source operator kind.

At minimum, exact-capable contracts need:

- a language or language-family constraint;
- a symbol identity constraint: fully resolved symbol, imported namespace, proven
  receiver protocol/type, or a language-defined unshadowed builtin/global;
- signature constraints: arity, parameter roles, receiver position, overload
  identity, and call shape, including whether the operation is a function,
  method, constructor, property, operator, or macro-like surface;
- shadowing, import, receiver, version, and overload preconditions;
- argument demand, callback demand, effect summary, and return/protocol mapping;
- channel eligibility and provenance ids.

For example, "method named `map`" is not a semantic fact. `JavaScript
Array.prototype.map`, `Rust Iterator::map`, `Ruby Enumerable#map`, and RxJS
`Observable.map` have different demand and effect behavior. A pack may connect
one of those surfaces to the common `Map` protocol only after it proves the
surface is that API. If the current IL cannot prove the surface, the exact
channel stays closed until the pack or frontend adds the missing fact.

## Semantic axes

The kernel needs first-class axes instead of language-name checks.

### Evaluation and demand

Lazy evaluation and short-circuiting cannot be modeled as a boolean. The kernel
must represent when each child is demanded, how often it can be demanded, whether
results are memoized, and in which order effects become observable.

Examples:

- eager call-by-value arguments, usually left-to-right;
- conditional demand for `&&`, `||`, ternary, nullish defaulting, and exception
  handlers;
- pull-based iterator pipelines, where `map` and `filter` callbacks run only
  when a terminal consumer demands elements;
- call-by-need thunks with memoization;
- async, promise, future, and observable APIs whose construction and observation
  happen at different times.

The implemented first-party substrate now names these as
`DemandEffectProfile` contracts for admitted operations. The profiles cover
current eager, short-circuit, eager HOF, pull-lazy HOF/generator, async
continuation, generator suspension, channel-boundary, and protocol-boundary
classes. HOF timing must come from an explicit source or API demand source; the
node-level resolver distinguishes source comprehensions, eager JS-like/Ruby
library HOFs, and pull-lazy Rust/Java library HOFs. The profiles describe how an
already-proven operation is consumed; they do not prove that a selector or raw
source protocol anchor has that meaning.

### Values and domains

Algebraic laws are valid only inside the right value domain. Numeric `+`,
string/list concatenation, overloaded operators, floating NaN, integer overflow,
and tensor broadcasting cannot share one undifferentiated `Add`.

The kernel should model domains such as numeric, boolean, string/free-monoid,
sequence, map, set, option/nullish, result/error, future/promise, observable, and
domain-specific collection types. Unknown stays top: exact rewrites fire only
when a required domain is proven.

### Effects and observations

Behavior is not just a return value. The kernel must distinguish ordered effects,
final state by place, local-only mutation, emitted streams, exceptions, async
scheduling, yields, allocation identity when relevant, and opaque calls.

For example, swapping two appends is observable, while writing two distinct
constructor fields may be equivalent if the observable is final object state.
Those are different effect equivalence classes.

### Symbol and module facts

Library semantics require reliable symbol identity. A pack contract for `sum`,
`Array.prototype.map`, `Iterator::map`, or `Observable.map` is admissible only when
resolution, receiver type, version, and shadowing rules prove that the call is the
API the contract describes.

### Laws and proof status

Rewrites should be registered as semantic laws with explicit preconditions:
required domains, effect conditions, demand conditions, and proof status.

Proof statuses mirror [formal-soundness](formal-soundness.md): `proven`,
`covered`, `missing`, `empirical-only`, and `rejected-counterexample`. First-party
exact laws need project-owned evidence. External laws may declare evidence, but
nose does not certify it unless the law ships as first-party.

## Channel eligibility

The same semantic knowledge can be safe for one channel and unsafe for another.

| eligibility | use |
|---|---|
| `syntax-only` | parsing/lowering/reporting only |
| `near-only` | candidate generation and review-oriented scoring |
| `abstraction-witness` | weak refactoring-template witnesses over `near` candidates; never exact equivalence |
| `exact-empirical` | exact channel when required tests, hard negatives, and oracle coverage are present |
| `exact-proven` | exact channel with proof obligations for the core law |

Eligibility is not a trust class. Pack provenance and enablement are tracked
separately.

| trust policy | use |
|---|---|
| `default-first-party` | maintained, gated, and enabled by the nose project |
| `first-party-optional` | maintained and gated by nose, but not enabled by default |
| `external-opt-in` | provider/user responsibility; enabled only by explicit user choice |

External packs may declare their intended eligibility, but users choose whether
to trust that declaration. Today, explicitly loaded external packs are reported
as `metadata-only`; exact matching is enabled only by compiled first-party
evidence/contracts.

## Pack conformance

The extension contract must define what a pack provider is expected to publish.
Meeting these criteria is not nose certification; it is the minimum shape that
lets users and tools evaluate whether a pack is trustworthy enough for their use.

Every pack should declare:

- stable pack id, provider, license, source repository, and support contact;
- language, runtime, package, and version ranges it claims to model;
- supported channels: `syntax-only`, `near-only`, `exact-empirical`, or
  `exact-proven`;
- semantic contracts with stable ids, including symbol/receiver constraints,
  evaluation rules, effect summaries, return/protocol mapping, and exact
  eligibility;
- required facts for each exact contract, such as non-shadowing, resolved import,
  receiver type, immutable binding, pure callback, or primitive operator proof;
- conformance fixtures: positive convergence cases and hard negatives;
- known unsound or unsupported boundaries;
- proof status and links for any claimed `exact-proven` law;
- provenance labels that can appear in reports.

Packs that claim exact eligibility should also provide a reproducible conformance
command. First-party packs must run that command in nose CI. External packs
should ship it so users can run `nose semantic-pack check`, but passing the
structural harness is not nose approval of external semantic correctness unless
the pack is adopted as first-party.

## Practical architecture

The target shape is:

```text
source
  -> LanguagePack parser/lowering
  -> surface semantic IR + facts
  -> kernel validation
  -> core semantic IR
  -> protocol/law normalization
  -> value graph + oracle
  -> detection/ranking/report provenance
```

This does not require a flag-day rewrite. The migration can start with wrappers
around existing behavior: replace `Lang` checks with semantic predicates, then
move duplicated library recognizers and strict gates behind shared contracts.

## Design rule

If a future contributor asks "can this exact match be accepted?", the answer
should not be "because the file is Java" or "because this recognizer says so".
It should be:

1. which pack emitted the relevant facts;
2. which API or construct contract matched;
3. which semantic law applied;
4. which domains, demand rules, and effect conditions were proven;
5. which channel eligibility admitted the result.
