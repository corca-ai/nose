# Declaration-only type contracts (#843)

Issue #843 moves mechanically non-actionable type-contract duplication off the bare
default without deleting a family or teaching nose a worthiness judgment. It implements
the `declaration-only-type.v1` lever frozen by the [#841 dev
taxonomy](default-head-failure-taxonomy-841.md).

## Product contract

A family reports `surface: "declaration"` only when **every** member is one complete,
unsliced type declaration with all of this language-neutral `UnitOrigin` evidence:

- the location is a non-fragment `Class` unit;
- `source_granularity` is `whole-unit`;
- `type-contract` is the only domain;
- the subkind is interface/trait/protocol, type alias, or defined type;
- `body_kind` is `declaration-only`; and
- both `declaration-only` and `type-only` evidence flags are present.

Runtime, data, style, and implementation domains fail open. So do enums, schemas,
extensions, partial or narrowed connected witnesses, mixed bodies, unknown origin, and
any runtime-value, validation, reusable/default-body, protocol/concrete/constrained
extension, or Java default/static/private-method evidence. These extra typed guards are
the sound product form of #841's abstract predicate: they prevent a sliced block from
borrowing its enclosing type's origin and map the taxonomy's abstract runtime/data/body
boundaries onto the existing IL vocabulary. There is no repository, file, symbol, path,
or language allowlist.

The classification is presentation-only. It preserves family IDs and the existing fold
forest, so baselines, ignores, direct `id=` lookup, and `all top=0` remain stable. The
default human report, Markdown, SARIF, and `--fail-on any` omit these families and explain
the count as `declaration-only-type-contract`. JSON keeps the existing machine contract:
the reason is `surface: "declaration"`, recoverable with `surface=declaration`. No schema
version or new machine field is needed; the more specific human subtype maps to that
existing surface.

## Existing producer facets, tightened at unsafe boundaries

The classifier consumes the existing `UnitOrigin` vocabulary. Source-backed tests prove
one shared contract across Java interfaces and annotation types, TypeScript interfaces
and type aliases, Rust traits, and Swift protocols.

Independent pre-implementation review found a few producer cases that could falsely
claim `declaration-only`. The frontends now express those cases using existing facets:

- Java interface or annotation fields with initializers carry `mixed` plus
  `runtime-value`; an annotation element's `default` value remains a declaration;
- Rust trait const/type defaults, method bodies, macro invocations, and outer/inner
  attributes carry the existing implementation/default-body evidence. This deliberately
  treats inert attributes such as `cfg`, `allow`, and `doc` as uncertainty too: excluding
  a safe declaration is preferable to hiding a contract whose attribute may synthesize
  behavior; and
- a parser-recovered Swift protocol member body carries mixed implementation and
  reusable/default-body evidence.

These are truthfulness corrections, not a frontend redesign or vocabulary expansion.
They also explain the only intentional non-surface JSON drift: five expanded Java
families expose an added `runtime-value` flag, and one semantic Netty family corrects its
body/flags. Their IDs, membership, fingerprints, and surfaces do not change.

## Frozen cohorts and dev behavior

All five #841 positives move to `declaration`: the ANTLR head family plus the four
independently audited ANTLR, Prettier, and Zustand deep families. All eight source-bound
hard negatives remain `default`, covering missing/partial origin and reusable
implementation bodies.

The exact immediate-parent comparison covers all 66 pinned dev repositories and keeps
held-out closed:

| expanded default-mode result | count |
|---|---:|
| families before / after | 54,754 / 54,754 |
| ordered family IDs changed | 0 |
| `default -> declaration` | 91 |
| `shallow -> declaration` | 44 |
| repositories with a surface transition | 14 |

The default top 30 removes exactly the five frozen positives and promotes five following
rows in ANTLR, Prettier, and Zustand. Across the established semantic-only command, all
9,850 family IDs and surfaces remain stable; only the source-truthfulness origin record
described above changes in Netty. Every expanded result is byte-identical across a repeat
and one/four Rayon threads.

The uncached v7 dev evaluation keeps full-universe worthy recall exactly
`2716/2849 = 95.33%`, with zero recovered or regressed worthy IDs against #842. Among
currently labeled default positions, measured P@10 rises from `382/658 = 58.05%` to
`382/647 = 59.04%`; the worthy-hit count itself remains 382. Coverage becomes
`647/658 = 98.33%` because 11 replacement positions are not yet in the precision overlay,
so this is an interim dev measurement rather than a closed precision claim. Closing that
residual label gap belongs to #845/#846, not to broadening this mechanical predicate.

The checked [behavior
artifact](../bench/labels/declaration_type_contract_behavior_2026_07_14.dev.v1.json) binds
per-repository output hashes, ordered-ID and non-surface projections, exact origin
corrections, default replacements, cohorts, and repeat/thread determinism. The checked
[product-quality
artifact](../bench/labels/declaration_type_contract_product_quality_2026_07_14.dev.v1.json)
binds the binary, source, labelset, corpus, uncached queries, confidence intervals, and
zero-regression comparison.

## Official-v0.19.0 performance

The baseline is the published Darwin arm64 v0.19.0 binary, SHA-256
`0f73ea…e0f3`, not a source rebuild. The harness distinguishes the annotated tag object
`54f8a674…` from peeled commit `0985e696…` and pairs every official/current report with a
current/current control.

The 66-repository r3 comparison is `15,143.67 -> 15,207.32 ms` (+63.66 ms, +0.42%).
The control is +10.74 ms, so the adjusted result is +52.92 ms / +0.35%: below the
material threshold of both 5% and 5 ms. Short-run repository/stage signals were not
waived. The checker required exactly 25 repositories at r9, six at r21, and Delve plus
Git at r40. The final two-repository slice is `529.75 -> 530.94 ms` raw; its +4.85 ms
control makes the adjusted result -3.65 ms / -0.69%. At r40, Delve and Git
`normalize+extract` are respectively -0.35 and -7.20 ms adjusted, while `query_surface`
is +0.05 and -0.10 ms. The complete 3 -> 9 -> 21 -> 40 chain therefore passes with no
material aggregate, repository, or stage regression.

## Validation

```sh
cargo test -p nose-frontend type_contract -- --nocapture
cargo test -p nose-cli type_contract --lib -- --nocapture
cargo test -p nose-cli --test cli declaration_surfaces -- --nocapture
python3 bench/labels/declaration_type_contract_behavior.py --self-test
python3 bench/labels/declaration_type_contract_behavior.py validate
python3 bench/labels/declaration_type_contract_closeout.py --self-test
python3 bench/labels/declaration_type_contract_closeout.py
./scripts/check-duplication.sh
./scripts/check-ci-local.sh --fast
./scripts/check-docs.sh
```

The #845 residual-ranking work and #846 one-time held-out/fresh-repository closeout still
own the epic-level precision, coverage, and blind-generalization gates.
