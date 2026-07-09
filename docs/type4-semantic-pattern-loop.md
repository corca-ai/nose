# Type-4 semantic pattern loop

This page is the operating loop for scaling Type-4 admission from one proven
language surface into reusable semantic patterns. Use it when a frontier packet
has taught us a general law, but the next step should be "attach another
language surface" rather than "invent another language-specific detector rule."

The loop sits above the [frontier platform](frontier-platform.md), focused
cases in [Type-4 adversarial coverage](type4-adversarial-coverage.md), and the
[proof-carrying frontier](proof-carrying-frontier.md). It must still obey the
[design](design.md) constraint: exact semantic admission protects the
zero-false-merge contract before recall.

## Core rule

Repeat by **semantic pattern**, not by language spelling.

A language surface may be the first evidence for a pattern, but the reusable
unit is the invariant:

- bad repeat unit: "Python `all(...)` plus a loop";
- good repeat unit: "universal quantifier equals a pure counterexample loop
  over the same iteration source, with vacuous truth preserved."

Language-specific code belongs in evidence producers, frontend lowering, and
surface fixtures. Detector consumers should prefer neutral proof facts,
admitted resolvers, or kernel contracts. If exact admission logic mentions a
language name after the first proof slice, treat that as a design smell.

## Pattern card

Write a pattern card before opening detector behavior:

```yaml
pattern_id: quantifier.universal.counterexample-loop
law: >
  all(P(x) for x in xs) is equivalent to a loop that returns false on
  not P(x) and true after exhausting xs.
required_facts:
  - quantifier.vacuous-truth
  - quantifier.universal.counterexample-loop
  - iteration.same-source-identity
  - effect.pure-predicate
  - boolean.demorgan.proven-bool-operands
hard_negative_templates:
  - loop.changed-empty-result
  - loop.iterator-identity
  - effect.observed-predicate-effect
  - effect.helper-call-without-purity-proof
  - boolean.value-context
  - boolean.changed-predicate
language_surfaces:
  python:
    status: admitted
    evidence: python-loop-demorgan-all-2026-07-07
  ruby:
    status: open
    surface: Enumerable#all?
  rust:
    status: open
    surface: Iterator::all
  typescript:
    status: admitted
    surface: dense-literal one-argument Array.prototype.every
  javascript:
    status: open
    surface: Array.prototype.every
```

Keep the card short enough to review in a PR. The detailed evidence still lives
in `real_frontier.v1.json`, `frontier_target_packets.v1.json`, focused cases,
proof fact artifacts, generated PCF/readiness outputs, and the checked pattern
catalog at `bench/type4/semantic_pattern_cards.v1.json`.

## Capability matrix

Every reusable pattern should have a small matrix that distinguishes facts from
surfaces:

| fact | Python | TypeScript | JavaScript | Ruby | Rust |
|---|---|---|---|---|---|
| `quantifier.vacuous-truth` | modeled-controlled; packet admitted | admitted for dense-literal one-arg every/for-of | open | open | open |
| `quantifier.universal.counterexample-loop` | modeled-controlled; packet admitted | admitted for dense-literal one-arg every/for-of; number[] param and callback extra args stay split | open | open | open |
| `iteration.same-source-identity` | modeled-controlled; packet admitted | admitted for same dense-literal source | open | open | open |
| `effect.pure-predicate` | modeled-controlled; packet admitted | admitted for pure comparison predicates | open | open | open |
| `boolean.demorgan.proven-bool-operands` | modeled-controlled; packet admitted | admitted for boolean comparison results; value-returning && remains closed | open | open | open |

Use vocabulary like `open`, `modeled-controlled`, `admitted`, and
`not-applicable`. Do not mark a language surface admitted because another
language has the same-looking syntax. A language cell becomes admitted only
after executable replay and PCF/readiness agree.

## Open-surface audit

Use the checked open-surface audit before choosing the next admission target:

```sh
python3 bench/type4/open_surface_admission_audit.py --check
```

The generated
[`open_surface_admission_audit.md`](../bench/type4/open_surface_admission_audit.md) artifact
groups open language surfaces by semantic pattern, proof fact, language,
current status, evidence level, focused-case support, and likely blocker. It is
the queueing view for this loop: start with `proof-fact-ready` or
`probe-to-focused-candidate` rows when choosing an admission target. Treat
`needs-surface-focused-perimeter` rows as focused-fixture setup work: the
neutral facts are modeled, but the exact language surface still needs positives,
adjacent hard negatives, and executable expectations. Leave
`blocked-by-unmodeled-facts` rows open until their neutral facts become
modeled-controlled.

When an epic selects multiple audit rows, freeze that selection in the audit
artifact itself. The `Epic #778 Audit Slice` section records the chosen
in-scope rows, the rows intentionally left out because they need new neutral
facts, and the issue numbers that own each step. Later PRs should update that
section by regenerating `open_surface_admission_audit.py` as rows leave the
open audit instead of hand-editing the Markdown or re-triaging the whole queue.

## Eight-step loop

1. Pick a frontier candidate from corpus evidence.
   Start from `frontier_platform.py`, `real_frontier.v1.json`, `nose verify
   --leads`, or a README/user-facing claim. Prevalence is only a queue signal;
   it is not proof.

2. Name the semantic law.
   Write the pattern in language-neutral terms first. If the law cannot be
   stated without language/API spelling, keep it as a local target packet until
   the invariant is clearer.

3. Define proof facts and boundaries.
   Reuse existing proof fact vocabulary when possible. Add new neutral facts
   only when they will apply to more than the first surface. Keep first-surface
   fact IDs as compatibility aliases or packet-local evidence sources when
   renaming would churn checked artifacts.

4. Instantiate focused positives and hard negatives.
   Use templates: vacuous truth, iterator identity, observed effects, helper
   calls without purity proof, value-returning boolean operands, changed
   predicate, wrong literal set, API identity, and domain/precondition
   boundaries. Every positive needs adjacent negatives.

5. Emit or cite source evidence.
   Frontend/per-language modules may inspect syntax to emit source, domain,
   guard, place/effect, sequence, or API evidence. Semantic consumers should
   require admitted evidence, a resolver, or a kernel contract before assigning
   meaning.

6. Open detector behavior only from the neutral fact set.
   The detector should consume the proof facts and shared value-graph law. A
   second language surface should usually add evidence production and fixtures,
   not a new detector branch.

7. Replay the perimeter.
   Refresh executable expectations, real-frontier replay, target packets,
   proof-carrying frontier artifacts, readiness artifacts, and docs. The
   positive may flip to `same-family`; hard negatives must stay `split`.

8. Let CI and dogfooding price the change.
   Run the Type-4 checks, docs checks, focused Rust/CLI tests, and self-query
   duplication gate. If a stronger detector surfaces new nose-on-nose families,
   either dedupe them or record the reviewed decision in
   [dogfooding history](dogfooding-history.md) and the duplication baseline.

## When to stop generalizing

Stop and keep the work packet-local when:

- the proof fact depends on one language's value semantics and has no clear
  equivalent elsewhere;
- the hard-negative template cannot be expressed for another surface;
- the proposed neutral fact would hide a product/runtime boundary such as
  laziness, exception timing, mutation, iterator invalidation, overloading, or
  API identity;
- the second language needs a different semantic law, not just a different
  evidence producer.

The goal is not to erase language differences. The goal is to make every
cross-language admission explicit about which differences have proof evidence
and which stay closed.

## Seed: Python loop/De Morgan

Issue #745 is the first vocabulary cleanup from this loop. PR #744 admitted the
Python surface, and #745 promotes its packet-local facts into the neutral
registry names that future language surfaces should cite:

- positive: `all(x != 0 and x != 1 for x in xs)` and an early-return loop that
  returns `False` on `x == 0 or x == 1`;
- admitted behavior: the pair replays as `same-family`;
- boundaries: vacuous truth, observed effects, helper calls, value-returning
  boolean operands, changed predicates, and iterator mismatch remain `split`.

The retired packet-local aliases are:

- `python-loop-demorgan.boolean-demorgan`;
- `python-loop-demorgan.universal-short-circuit`;
- `python-loop-demorgan.iterator-identity`;
- `python-loop-demorgan.effect-safety`;

The canonical neutral facts are:

- `boolean.demorgan.proven-bool-operands`;
- `quantifier.universal.counterexample-loop`;
- `quantifier.vacuous-truth`;
- `iteration.same-source-identity`;
- `effect.pure-predicate`.

Use those canonical facts before adding another language surface for the same
pattern.

Back to [documentation home](home.md).
