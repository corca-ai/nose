# Semantic pack adoption

Status: adoption and release-gate policy for builtin semantic packs.

This page defines how a semantic pack moves from external provider ownership to
nose-owned builtin support. The semantic vocabulary stays the same across lanes:
promotion changes ownership, trust, default enablement, CI gates, and rollback
responsibility. It must not create a second implementation path.

## Lanes

| lane | owner | default | meaning |
|---|---|---|---|
| `external-opt-in` | provider/user | off | Local manifests are explicit opt-ins and metadata-only until influence gates exist. |
| `builtin-optional` | nose | off | Shipped with nose, but not default-enabled until product risk is accepted. |
| `builtin-default` | nose | on | Shipped with nose and enabled by default. |

The same evidence vocabulary, contract rows, law rows, fixture refs, proof
obligations, and provenance concepts apply in every lane. Pack ids may stay the
same only through explicit ownership transfer. Otherwise promotion needs a
documented replacement or composition mapping, because builtin ids are reserved
and local external manifests cannot claim shipped nose ownership. Kernel
admission must continue to require dependency-backed evidence and fail-closed
conflict handling.
Pack compatibility across manifest API versions, installed nose versions, and
kernel vocabulary changes is governed by
[semantic-pack-compatibility](semantic-pack-compatibility.md).

## External To Builtin Optional

Promote an `external-opt-in` pack to `builtin-optional` only when nose is ready to
own the support surface.

Requirements:

- The pack has a stable id, version policy, owner, and changelog entry.
- The manifest or descriptor passes structural conformance.
- Exact-capable rows have positive fixtures and hard negatives for every
  admitted contract/law family.
- Exact-capable rows have executable conformance or oracle gates that exist,
  run in CI, and pass before they can influence exact results. A plan is enough
  to track adoption work, not enough to open exact influence.
- Language packs additionally name parser/lowering ownership, language-version
  coverage, embedded-region boundaries where relevant, parse/lower failure
  behavior, and corpus fixtures before official support is advertised.
- Required symbol/import/source/effect/demand/version/shadowing/arity
  obligations are explicit enough for the kernel to fail closed.
- Conflicts with existing builtin rows are resolved by composition or replacement
  policy, not provider priority.
- Query-regression output drift and runtime drift are measured against `main`.
  Use the [Product Behavior Gate](semantic-pack-architecture.md#product-behavior-gate)
  and [Performance Gate](semantic-pack-architecture.md#performance-gate)
  thresholds.
- User-facing docs explain the support boundary and unsupported cases.
- A nose maintainer is named for release responsibility, regression response,
  and user-visible support decisions.
- Capabilities/query JSON changes are versioned or additive according to their
  documented contracts.

Promotion does not imply default enablement. A newly builtin-optional pack may
still have recall gaps or ecosystem-version risk, but its influence path must be
owned and testable by nose.

Implementation PR checklist:

- Name the pack id, owner, version policy, support boundary, and unsupported
  cases.
- Link structural conformance or builtin inventory output.
- List exact-capable producers, contracts, laws, or aliases changed by the PR.
- Attach positive fixtures and hard negatives for the changed exact-capable
  rows.
- Classify product behavior drift as unchanged metadata, precision improvement,
  measured recall change, or bug fix.
- Report runtime drift against `main` when the PR changes analysis behavior.
- State the rollback action: demote pack, disable row, tighten admission, or
  revert.

## Builtin Optional To Builtin Default

Promote `builtin-optional` to `builtin-default` only after corpus evidence shows
the default surface is useful and safe.

Requirements:

- Query-regression shows no unexplained family-output drift.
- False-merge risk has hard negatives and a rollback path.
- Default-surface noise has been reviewed on representative real repositories.
- Runtime stays within the pack-architecture budget or has an accepted product
  decision.
- Exact acceptance never widens without dependency-backed evidence and hard
  negatives.
- Docs, examples, and capabilities describe the default behavior.
- Release notes name the new default support and any known unsupported cases.

Default enablement is a product decision layered on top of the same kernel
admission checks. It must not bypass external/builtin parity rules.

Implementation PR checklist:

- Include `nose semantic-pack adoption-gates --format json` output or a summary
  of its blocker status for the changed pack.
- Include `nose semantic-pack inventory --format json` output or a summary of
  exact-capable coverage for the changed rows.
- Attach query-regression output against representative repositories, or explain
  why the change is descriptor/reporting-only.
- Attach runtime measurements against `main`; use the pack architecture
  performance threshold.
- Note docs, examples, capabilities, and release-note changes for the new
  default behavior.
- Name the smallest rollback path if false merges, noise, or runtime regression
  appears after release.

## Rollback

If a builtin pack causes false merges, unacceptable noise, or runtime regression,
prefer the smallest reversible action:

- Disable the pack by default by moving it from `builtin-default` to
  `builtin-optional`.
- Disable only the risky contract/law row when the rest of the pack remains
  useful.
- Tighten admission requirements with additional dependency or hard-negative
  checks.
- Revert the descriptor/producer change if the issue is systemic.

Rollback must preserve provenance. Reports should still make it clear which pack
or row was responsible for the disabled behavior, so users and maintainers can
track the fix.

## Adoption Gate Report

Builtin packs have a lightweight gate report:

```sh
nose semantic-pack adoption-gates
nose semantic-pack adoption-gates --format json
```

The report reads compiled builtin descriptors through the same static inventory
surface as `nose semantic-pack inventory`. It does not run queries, parse
external manifests, execute provider code, or enable external influence.

JSON schema version 1 reports:

- current builtin lane counts;
- optional/default PR checklist requirements;
- rollback actions;
- per-pack trust lane, default enablement, exact-capable coverage status, and
  blockers.

The checklist and `required_evidence` arrays are static policy requirements for
PR authors. They are not proof that a PR has attached the evidence; the
mechanical pass/fail signal is `status`, `totals.blocked_packs`, and each
pack's `blockers`.

Important fields:

| Field | Values |
|---|---|
| `schema_version` | `1` |
| `status` | `ok` or `needs-evidence` |
| `totals.builtin_packs` | Number of compiled builtin packs inspected. |
| `totals.builtin_default_packs` | Builtin packs in the `builtin-default` trust lane. |
| `totals.builtin_optional_packs` | Builtin packs in the `builtin-optional` trust lane. |
| `totals.exact_capable_packs` | Builtin packs with exact-capable inventory coverage. |
| `totals.packs_needing_coverage` | Exact-capable builtin packs whose inventory audit needs coverage. |
| `totals.blocked_packs` | Packs with lane, enablement, or inventory blockers. |
| `policy.scope` | `compiled-builtin` |
| `policy.default_lane` | `builtin-default` |
| `policy.optional_lane` | `builtin-optional` |
| `policy.external_influence` | `metadata-only` |
| `policy.product_behavior_gate` | `required-for-builtin-default-promotion` |
| `policy.performance_gate` | `required-for-builtin-default-promotion` |
| `packs[].adoption_status` | `default-gated`, `optional-gated`, `blocked`, or `tracked` |
| `packs[].coverage_status` | `covered`, `needs-coverage`, or `tracked-no-exact-rows` |
| `packs[].blockers[]` | `builtin-default-not-enabled`, `builtin-optional-enabled-by-default`, `unexpected-trust-lane`, `exact-capable-coverage-gap`, or `inventory-audit-gap` |
| `packs[].required_evidence[]` | Static checklist labels for the pack's trust lane. |
| `packs[].rollback_actions[]` | Static rollback action labels maintainers may choose from. |

`status: "ok"` means the current compiled builtin packs satisfy the mechanical
lane checks: `builtin-default` packs are enabled by default,
`builtin-optional` packs are not, and exact-capable packs are covered by the
builtin inventory audit. It is not a product decision by itself; promotion to
`builtin-default` still requires the behavior, runtime, docs, release-note, and
rollback evidence above.

## Implementation Rule

Enablement is trust/default metadata plus gates:

- `source`: how the pack is loaded, such as `compiled-builtin` or
  `local-manifest`;
- `trust`: who owns the risk, such as `external-opt-in`, `builtin-optional`, or
  `builtin-default`;
- `enabled_by_default`: whether product analysis uses the pack by default;
- `influence`: whether admitted evidence/contracts can affect analysis.

Do not fork semantics between "external implementation" and "official
implementation." If a pack is promoted, reuse the same evidence and contract
vocabulary with stronger ownership, conformance, and release gates.

## Related

- [semantic-pack-architecture](semantic-pack-architecture.md)
- [semantic-pack-conformance](semantic-pack-conformance.md)
- [semantic-pack-compatibility](semantic-pack-compatibility.md)
- [semantic-pack-extension-api-v0](semantic-pack-extension-api-v0.md)
- [semantic-pack-loading](semantic-pack-loading.md)
