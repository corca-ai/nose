# Development and evidence index

This page collects current internal design notes, proof contracts, operating
processes, and active roadmaps used to develop nose. You do not need these
documents to use the tool. Start at the
[documentation home](home.md) for the user guide or
[contributing](contributing.md) to change the code.

Issue-numbered closeouts, dated audits, release evidence, and append-only
ledgers are retained in the [historical records index](historical-records.md).
The [documentation lifecycle](documentation-lifecycle.md) defines the checked
classification, ownership, and freshness policy for every wiki page.

## Fundamentals

- [design & direction](design.md) — the *why* behind the roadmap: the sound core as the moat, the two (non-human) consumers, and what that decides. **Check roadmap calls against this.**
- [architecture](architecture.md) — the crates and the lower → normalize → detect → rank pipeline.
- [normalization](normalization.md) — the passes that make behaviorally-equivalent code converge (the hard part).
- [refactoring-ratchets](refactoring-ratchets.md) — repository quality ratchets for incremental design cleanup, including Rust file-length and the CLI prelude guard.
- [repository gate inventory](repository-gates.md) — authoritative named-gate ownership, lane selection, worktree effects, and timing protocol.
- [auxiliary development tools](tooling.md) — checked non-workspace tool pins,
  read-only diagnosis, explicit bootstrap, hosted consumption, and update
  procedure.
- [evidence artifact lifecycle](evidence-artifact-lifecycle.md) — lifecycle classes, exact inventory drift checks, receipt/seal/baseline bindings, and conservative retention policy.
- [semantic-regression-smoke](semantic-regression-smoke.md) — base/head semantic output and runtime tripwire, exact intentional-drift declarations, focused reruns, and pinned evidence.

## Repository workflow

- [contributing](contributing.md) — contributor workflow, local quality gates,
  repository automation, conventions, changelog discipline, and release steps.
- [agent-instructions](agent-instructions.md) — repository-specific instructions for coding agents, including documentation workflow and docs checks.
- [documentation lifecycle](documentation-lifecycle.md) — page kinds, owners,
  verification windows, append-only retention, and the exact-inventory gate.

## Channels, witnesses & proofs

- [graded-witness](graded-witness.md) — the anti-unification grade for near families: "equal except *k* holes", each hole a candidate parameter, with the soundness-relevant referent check.
- [fragment-contracts](fragment-contracts.md) — how exact sub-function fragments are modeled: classification, contract, the wrapper-synthesis behavior oracle, the effect algebra, and fail-closed receiver identity.
- [reinvented-helper implementation and evidence](reinvented-helpers-internals.md) — the containment proof, exclusions, surface policy, and field measurements behind the user-facing channel.
- [divergent-edit policy and qualification](divergent-edits-policy.md) — the gate evidence model, schema, measured policy, and v2/v3 qualification record.
- [oracle-value-model](oracle-value-model.md) — the verify oracle's value model (Int/Bool/Str-monoid/List/Float/Sym), what it witnesses, and the outcomes that closed the #283 false-merge cluster (C string/`+`-non-assoc, D-int32 width, D-div float) plus the `--falsify` search.
- [recall-loss-diagnostics](recall-loss-diagnostics.md) — the local `nose verify --recall-loss-report` artifact for keeping exact semantic admission strict while measuring under-merges, oracle exclusions, and structured admission rejections.
- [formal-soundness](formal-soundness.md) — Lean 4 proof-obligation registry for proof-sensitive IL, normalization, fragment, and oracle contracts.
- [Soundness Lab](soundness-lab.md) — the frozen v0.19.0 exact-claim cohort,
  risk-weighted non-gameable scorecard, published-asset replay discrepancy, and
  reproduction gate for 0.20 soundness work.

## Semantic kernel & packs

### Current architecture and operating process

- [semantic-kernel](semantic-kernel.md) — semantic-kernel and pack architecture: language/library semantics, extension boundaries, responsibility model, and exact-channel eligibility.
- [semantic-pack-architecture](semantic-pack-architecture.md) — the #473 migration rulebook for builtin/external pack terminology, kernel-vs-pack ownership, behavior gates, and performance gates.
- [evidence-records](evidence-records.md) — the internal pack-facing evidence substrate for source, domain, import, symbol, type, guard, place/effect, library API, and sequence-surface facts.
- [source-facts](source-facts.md) — source-origin evidence for semantic contracts: construct syntax, async/generator/error boundaries, literal/operator provenance, pack boundaries, and fail-closed exact admission.
- [demand-effect-semantics](demand-effect-semantics.md) — the internal demand/effect contract model for eager, lazy, short-circuit, async, generator, and channel boundaries.
- [recall-loss-recovery-loop](recall-loss-recovery-loop.md) — checked-in baseline summaries, report diff workflow, five-cycle recovery/attribution loop, and the full-corpus priority census for semantic-kernel work.
- [semantic-pack-adoption](semantic-pack-adoption.md) — promotion, rollback, and adoption-gate reports for moving external or optional packs into official builtin support without forking semantic vocabulary.
- [semantic-pack-compatibility](semantic-pack-compatibility.md) — manifest API, installed-version, kernel-vocabulary, and fail-closed external-influence compatibility policy for semantic packs.
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md) — versioned v0 schema and provider-facing extension API for language/library semantic packs.
- [semantic-pack-extension-api-v1](semantic-pack-extension-api-v1.md) — closed typed Java/Maven package-API grammar, deterministic compiler, local dependency/occurrence evidence, bounded locked near influence, and receipt-backed collection-factory exact claims.
- [semantic-pack-reference-vavr](semantic-pack-reference-vavr.md) — shipped, disabled-by-default Vavr `List.of` reference pack, pinned lock/receipt, measured value, boundaries, and rollback.
- [semantic-pack-project-lock](semantic-pack-project-lock.md) — local content-pinned v1 authorization, row/channel selection, dependency/receipt pins, path containment, deterministic conflict rejection, and evidence input boundary.
- [semantic-pack-conformance](semantic-pack-conformance.md) — provider/user workflow for structural and bounded kernel source checks, receipt output, and builtin inventory audits.
- [semantic-pack-loading](semantic-pack-loading.md) — local pack manifest loading, explicit opt-in trust policy, and fail-closed near/external-claim-exact boundaries.

### Planning and pricing

- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md) — narrow-slice candidate matrix for future large-ecosystem builtin packs such as Guava, Lodash, NumPy, and RxJS.
- [semantic-pack-candidate-pricing](semantic-pack-candidate-pricing.md) — corpus-backed pricing loop for deciding which narrow semantic-pack rows are ready, blocked, or unpriced before implementation.

## Type-4, hazard & measurement

- [benchmark](benchmark.md) — the gold set, methodology, and the headline precision/recall numbers.
- [0.20 caller-provided generated paths](caller-generated-path-provenance.md) — the
  root-anchored CLI/config assertion contract, canonical containment and fail-open rules,
  all-member recovery semantics, and machine-readable trust boundary.
- [type4-benchmark](type4-benchmark.md) — the evidence-carrying synthetic Type-4 benchmark factory.
- [type4-adversarial-coverage](type4-adversarial-coverage.md) — focused Type-4 cases, target-packet task cards, and verifier-lead draft workflow.
- [frontier-platform](frontier-platform.md) — corpus-balanced evidence platform that ranks the next Type-4 axis by presence breadth (not raw count) and separates the queue signal from human-verified evidence.
- [proof-carrying frontier](proof-carrying-frontier.md) — target-packet admission report that requires linked evidence, proof invariants, hard negatives, blockers, and co-evolution guardrail context before exact Type-4 admission opens.
- [Type-4 semantic pattern loop](type4-semantic-pattern-loop.md) — repeatable process for turning one admitted language surface into a language-neutral semantic pattern with proof facts, capability matrix, and reusable hard-negative templates.
- [adversarial-coevolution](adversarial-coevolution.md) — the cross-axis campaign runbook: a white-box attacker derives structurally-missed patterns, an assessor prices them, a defender ships the largest sound generalization.
- [hazard-ranking](hazard-ranking.md) — the evidence base for the experimental `sort=hazard` (a divergence-*propensity* signal; **not** a validated harm ranker — it ranks actual harm near chance) and the honest evaluation trail.
- [hazard-benchmark](hazard-benchmark.md) — the evaluation criteria and labeled dataset hazard is measured against (repo selection, graded labels, quantitative sufficiency).
- [hazard-release-checklist](hazard-release-checklist.md) — what to do for the hazard ranking on every new nose release (one-page runbook: refresh the dataset, re-tune, re-validate).
- [runtime triage](runtime-triage.md) — reproducible query-runtime regression triage: harness, classification policy, timing knobs, and when not to optimize.
- [order-aware performance controls](order-aware-performance-controls.md) — the preregistered paired-block estimator, one-sided same-binary correction, exact sign-test decision rule, and frozen #892 decision ledger.
- [portable cache artifacts](portable-cache-artifacts.md) — the #873 SHA-256 layered CAS, #874
  dependency-aware source/raw/resolved reuse, and #877 bounded direct warm-leaf restoration with
  exact invalidation closure and fail-safe reporting.

## Current field workflow

- [dogfooding](dogfooding.md) — the current nose-on-nose duplication gate and baseline workflow.
- [historical records](historical-records.md) — dated field audits, release
  qualifications, issue closeouts, performance records, experiment results, and
  dogfooding decisions retained without presenting them as current contracts.

The root [CONTRIBUTING](../CONTRIBUTING.md) is a short entry point to the
[contributing](contributing.md) workflow page; release history is in [CHANGELOG](../CHANGELOG.md).
