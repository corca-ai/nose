# Demand and effect semantics

Back to [semantic-kernel](semantic-kernel.md). The implemented code shape is
summarized in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); remaining
work is tracked in [semantic-kernel-roadmap](semantic-kernel-roadmap.md).

Demand/effect contracts describe how an already-admitted semantic operation
evaluates its children, invokes callbacks, and exposes effects. They do not
admit a source API by name. API admission still requires source, symbol, import,
receiver, domain, and `LibraryApi` evidence.

## Current substrate

`nose-semantics::demand` now exposes a shared `DemandEffectProfile` with these
axes:

- operation class: eager, fold reduction, short-circuit quantifier, append
  mutation, nullish default, per-element HOF, pull-lazy HOF, call-by-need thunk,
  async continuation, generator suspension, channel operation, or protocol
  boundary;
- evaluation order: source order, short-circuit, per-element source order,
  deferred until observation, runtime scheduled, or protocol-defined;
- child demand: always, never, conditional, short-circuit-until-known,
  per-element-pull, maybe repeated, call-by-need memoized, suspended until
  observed, async continuation, channel boundary, or protocol boundary;
- callback demand, when present: per-element callback, fold step, or async
  continuation, with argument/result roles;
- effect visibility: immediate, only-if-demanded, delayed-until-pull,
  memoized-first-demand, async boundary, yield boundary, channel boundary, or
  protocol boundary.

This is a contract model for admitted operations, not an evidence record family.
Source protocol facts such as `Source::Protocol(Await)` and
`Source::Protocol(Yield)` are proof anchors. The demand/effect profile says what
a contract would need to prove before exact consumers may use that anchor.

## Implemented profiles

Builtins have demand/effect profiles for:

- eager operations such as `len`, `sum`, `min`, `max`, `range`, `zip`, `keys`,
  and `get-or-default` after their API occurrence is admitted;
- explicit fold reduction;
- `any`/`all` short-circuit quantifiers;
- append mutation;
- nullish/default fallback, where the fallback child is conditional.

Higher-order forms have per-element callback profiles for `map`, `flat_map`,
`filter_map`, `filter`, and `reduce`, but a raw HOF kind does not choose eager
or lazy timing. Timing comes from an explicit demand source. Python
list/dict-comprehension surfaces use eager per-element demand where modeled.
Python generator-expression surfaces use pull-lazy demand: callback errors and
effects are delayed until a terminal consumer pulls an element. Library/API HOF
rows need their own admitted demand timing before exact consumers may observe
callback effects; admitted HOF identity alone is not enough.

Promise `.then` now carries an async-continuation demand/effect profile in its
contract row. That does not open exact beta-reduction by itself. The value-graph
rule still requires explicit Promise-like receiver proof, so arbitrary
selector-only `.then(...)` calls remain closed.

Source protocol boundaries have internal profiles for future contracts:

- `await` and Promise continuations are async boundaries;
- `async {}` is suspended until observation;
- `yield` is a generator suspension boundary;
- Go channel/select surfaces are channel boundaries;
- Go goroutine/defer surfaces are protocol boundaries, not channel operations;
- Rust `?` is a conditional short-circuit boundary.

## Current consumers

The interpreter oracle consumes builtin demand/effect profiles for admitted
builtin calls instead of branching on local demand enums. This preserves current
behavior while giving the oracle a single semantic contract source.

The value graph consumes HOF demand/effect profiles for Python comprehension
exception timing. A list comprehension with a statically failing callback can
trigger the surrounding handler when the collection is known non-empty. A
generator expression with the same callback does not, because construction is
pull-lazy and the callback is not demanded until observation. Admitted library
HOFs currently stay closed for this exact timing path until their contract row
proves eager callback demand.

The Promise `.then` value-graph rule consumes the async-continuation contract and
then remains fail-closed until receiver proof is available.

## Exact-channel policy

Demand/effect profiles are necessary but not sufficient for exact semantics.
Exact consumers must still prove:

- the API occurrence or source protocol surface is admitted;
- receiver/protocol/domain obligations are satisfied;
- callback purity/effect obligations are satisfied where the law requires them;
- missing, ambiguous, conflicting, or dependency-broken evidence closes the
  exact path.

Selectors, raw `Payload::Builtin`, raw `Payload::HoF`, and source protocol facts
do not prove demand behavior by themselves.

## Remaining gaps

The substrate is intentionally broader than today's exact consumers. Remaining
work includes:

- Promise-like receiver proof and async/await convergence contracts (tracked by
  issue #154);
- pack-facing schema names for demand/effect rows (coordinated with issue #151);
- conformance fixtures that let pack authors prove demand/effect behavior
  without giving packs exact-clone authority (issue #157);
- richer iterator, generator, channel, call-by-need, observable, scheduling, and
  callback-effect contracts;
- report-level provenance for which demand/effect contract influenced an exact
  result.
