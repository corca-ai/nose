# Semantic kernel capability minimization

Status: issue #507 implementation record. This page turns the 20-iteration
semantic-pack pricing loop into a primitive census, blocker taxonomy, and
accept/reject matrix. Accepted kernel reductions are implemented in the same
change; rejected rows stay out of the kernel until a future pricing packet
brings evidence, fixtures, and a performance budget.

Source artifacts:

- [candidate_pricing.v1.json](../bench/semantic_pack/candidate_pricing.v1.json)
- [candidate_pricing.md](../bench/semantic_pack/candidate_pricing.md)
- [loop_reviews.v1.json](../bench/semantic_pack/loop_reviews.v1.json)
- [kernel_capability_matrix.v1.json](../bench/semantic_pack/kernel_capability_matrix.v1.json)

## Goal

The target is not to add more semantic-pack rows. The target is to keep the
kernel small enough that broad packs can share a few admission primitives, while
still having enough material to support meaningful external packs later.

The rule for this pass:

1. Start from the 20 priced candidate rows.
2. Group their blockers by repeated proof shape.
3. Check whether an existing primitive can be generalized or removed.
4. Implement every accepted reduction in this issue.
5. Reject anything that would add speculative vocabulary without occurrence
   proof, hard negatives, and runtime evidence.
6. Keep the query-regression subset within the semantic-pack performance gate:
   no more than 10% median runtime regression.

## Primitive Census

Implemented evidence families are:

| family | role |
|---|---|
| `Source` | source-syntax and construct provenance |
| `Domain` | proven value or receiver domain |
| `Import` | import and require facts |
| `Symbol` | unshadowed, imported, and qualified symbol identity |
| `Type` | nominal and C type-domain facts |
| `Guard` | shape and own-property guards |
| `Place` | exact receiver/place facts |
| `Effect` | mutation and escape facts |
| `LibraryApi` | occurrence proof for admitted library/API contracts |
| `CallTarget` | direct/imported/dynamic call target identity |
| `SequenceSurface` | aggregate and sequence surface proof |

Implemented contract substrates are receiver-domain dependency admission,
symbol/import occurrence proof, library API occurrence contracts, call-target
identity proof, demand/effect profiles, sequence-surface proof, manifest
package/version metadata, and conformance fixture gates.

The machine-readable census in
[`kernel_capability_matrix.v1.json`](../bench/semantic_pack/kernel_capability_matrix.v1.json)
also records the primitive-level rows that were checked before accepting or
rejecting changes:

| primitive | producer/consumer coverage | blocker cells |
|---|---|---|
| `domain_evidence` | type-domain, binding-domain, and factory result-domain producers feed receiver admission, value-domain inference, and strict-exact receiver gates | covers existing domain proof shape; still needs dtype/class/trait producers for candidate rows |
| `domain_requirement` | contract rows choose receiver/domain requirements consumed by semantic, normalize, and detect gates | accepted for dtype/domain, array-vs-scalar, and receiver-domain expression; does not cover Rust trait identity or map iteration order |
| `symbol_import_occurrence` | import and symbol producers feed library API and call-target admission | supports import-backed APIs but does not prove package/version occurrence |
| `library_api_occurrence` | compiled API recognizers feed canonical builtin admission, result-domain proof, and value-graph rewrites | supports row-specific API proof once a row exists; corpus misses remain pricing-only |
| `demand_effect_profile` | source/API demand rows feed value graph and oracle timing/effect-sensitive rewrites | already covers default, HOF, pull-lazy, async, generator, channel, and protocol classes; RxJS lifecycle/scheduler proof is still missing |
| `call_target_identity` | direct/imported call-target producers feed strict exact call identity and value-DAG referents | DirectMethod/DynamicDispatch vocabulary exists, but safe receiver/dispatch producers are not justified by this pricing packet |
| `manifest_package_metadata` | manifest loader feeds metadata, compatibility, and adoption reports | metadata exists, but package/version occurrence proof is missing |
| `corpus_pricing_signal` | pricing script feeds planning only | rejected as a semantic kernel primitive |

The only accepted primitive reduction in this pass is in receiver/domain
requirements. Before this issue, `DomainRequirement` mixed atomic evidence
domains with composite requirements such as collection-or-map, set-or-map, and
array-collection-or-set. After this issue, the primitive representation is:

```rust
DomainRequirement::Exact(DomainEvidence)
DomainRequirement::AnyOf(&'static [DomainEvidence])
```

Existing built-in contracts use named aliases such as
`DomainRequirement::ARRAY_COLLECTION_OR_SET`; those names are compatibility
conveniences over the two constructors, not separate primitives.

## Blocker Taxonomy

The pricing loop found one ready row, nine priced-but-blocked rows, and ten
unpriced rows. The blocker labels were mostly unique, but they collapse into
five proof shapes:

| proof shape | source blockers | candidate examples |
|---|---|---|
| version/package occurrence | Guava Optional version/package proof, ActiveSupport version proof, Go version proof | Guava Optional, Rails presence, Go maps helpers |
| receiver/domain/value boundary | dtype/domain proof, array-vs-scalar boundary, receiver class proof, Rust trait identity proof, map iteration order boundary | NumPy scalar ufuncs, Rails presence, Rust itertools, Go maps |
| demand/effect/lifecycle | default-demand/effect profile, Observable demand/effect substrate, stream lifecycle proof, callback effect proof, iterator demand/effect proof | Guava Optional, RxJS, itertools, Rust itertools |
| scheduler/platform/runtime | scheduler boundary, platform/cwd sensitivity, runtime/test boundary | RxJS adapters, Node path, Tokio |
| corpus/query evidence | external consumer corpus evidence, candidate-specific current miss/query evidence, zero corpus presence | Commons Lang, Lodash, Pandas, Rails helpers |

## Primitive x Blocker Matrix

This is the decision-relevant slice of the full JSON matrix. `Accepted` means
the primitive was changed in this issue. `Existing` means the primitive already
exists and the candidate row still needs occurrence proof, fixtures, or a row
producer. `Rejected` means adding vocabulary now would be speculative.

| blocker | primitive cell | decision |
|---|---|---|
| Guava Optional version/package proof | `manifest_package_metadata` is metadata-only; no lockfile/build occurrence producer exists | rejected |
| default-demand/effect profile | `demand_effect_profile` already has default/eager profiles; Guava needs API-specific proof | existing |
| dtype/domain proof | `domain_evidence` exists; `domain_requirement` now composes required domain sets | accepted for requirement composition, producer still missing |
| array-vs-scalar boundary | `domain_requirement::Exact` and `AnyOf` can express the exact domain boundary | accepted |
| Observable demand/effect substrate | generic demand/effect profiles do not prove observable lifecycle or subscription semantics | rejected |
| scheduler boundary | no scheduler/environment occurrence proof or hard negatives exist | rejected |
| stream lifecycle proof | current profiles do not model stream lifecycle ownership | rejected |
| callback effect proof | `Effect` plus demand profiles exist, but candidate-specific callback proof is missing | existing |
| ActiveSupport version proof | package metadata exists, occurrence-level version proof does not | rejected |
| receiver class proof | `DomainEvidence::Nominal` plus `DomainRequirement::Exact/AnyOf` can express it once a class producer exists | accepted for requirement composition, producer still missing |
| iterator demand proof | pull-lazy/eager profile classes exist, but source/API proof is missing | existing |
| source iteration effect proof | effect and demand primitives exist, but source-specific proof is missing | existing |
| Go version proof | language/runtime metadata exists, occurrence-level version proof does not | rejected |
| map iteration order boundary | operation-order semantics are not a receiver/domain requirement | rejected |
| external consumer corpus evidence | corpus evidence is a pricing signal, not semantic proof | rejected |
| candidate-specific current miss/query evidence | query evidence informs row selection only | rejected |
| Rust trait identity proof | call-target/type-domain vocabulary is insufficient without a safe trait producer | rejected |
| iterator consumption effect proof | demand/effect primitives exist, but consumption proof and fixtures are missing | existing |

## Decision Matrix

| candidate | decision | reason |
|---|---|---|
| Domain requirement composition | accepted and implemented | Domain-side blockers need receiver/domain boundary combinations. The old enum spent a primitive variant per combination; `Exact` plus `AnyOf` covers existing built-ins and the dtype/domain, array-vs-scalar, and receiver-class requirement cells without adding variants. It does not solve Rust trait identity or map iteration order. |
| Package/version occurrence anchor | rejected | Manifest package/version metadata already exists, but there is no lockfile/build-system occurrence producer. Adding a package anchor without occurrence proof would be speculative. |
| New demand/effect profile classes | rejected | `DemandEffectProfile` already covers eager, default, HOF, pull-lazy, async, generator, channel, and protocol boundaries. Current blockers need API-specific proof and hard negatives, not new primitive classes. |
| Observable scheduler/lifecycle substrate | rejected | RxJS scheduler and stream lifecycle semantics need dedicated occurrence proof and hard negatives. Generic demand/effect vocabulary would make exact admission too broad. |
| DirectMethod/DynamicDispatch producers | rejected | The vocabulary and consumers already exist. Safe producers require receiver type and dispatch proof that the pricing blockers did not supply. |
| Corpus presence primitive | rejected | Corpus presence is useful for pricing and queue ordering. It is not semantic evidence and must not admit exact contracts. |
| Platform/runtime environment contract | rejected | Platform, cwd, runtime, and test-environment sensitivity stays unsupported or hard-negative until an explicit environment proof exists. |

## Implementation

The accepted reduction is implemented in
[`crates/nose-semantics/src/evidence/domain.rs`](../crates/nose-semantics/src/evidence/domain.rs).
Built-in consumers in `nose-semantics`, `nose-normalize`, and `nose-detect` now
use named domain-requirement aliases backed by `Exact` or `AnyOf`.

The focused test additions are
`domain_requirements_compose_pack_boundaries_without_new_variants`, which shows
that a pack-style domain boundary can be expressed as an `AnyOf` requirement
without adding another enum variant, and
`named_domain_requirement_aliases_match_their_domain_sets`, which table-tests
every named alias against the expected accepted domain set. Existing
receiver-domain and library API predicate tests continue to assert built-in
behavior.

## Pricing Impact

This pass does not promote any blocked row to ready by itself. That is
intentional: the accepted reduction lowers the cost of representing
receiver/domain blockers, but it does not prove a NumPy dtype, Rails receiver
class, Rust trait identity, or Go map-order boundary for a concrete occurrence.

The pricing scanner was rerun with:

```sh
python3 bench/semantic_pack/pricing.py --nose ./target/release/nose --query-sample-repos 1
```

The rerun produced no diff to `candidate_pricing.v1.json` or
`candidate_pricing.md`, which matches the expectation: this issue changes the
kernel requirement representation, not the corpus pricing candidates.

Rows that become easier to implement later:

- `python.numpy.scalar_integer_ufuncs`: scalar-vs-array and dtype boundaries can
  be expressed without minting a new domain-requirement variant.
- `ruby.rails_active_support_presence`: receiver class/domain proof can reuse
  `Exact` or `AnyOf` requirements once the occurrence producer exists.
- `go.stdlib.maps_helpers`: map/set/collection boundaries reuse the same
  requirement model; map iteration order remains a separate semantic blocker.
- `rust.itertools_collect_vec`: iterator/collection result boundaries reuse the
  same requirement model; trait identity and consumption effects remain blocked.

Rows that remain blocked:

- version/package blockers, until package-manager or build metadata can prove
  occurrence-level package/version identity;
- RxJS scheduler, observable, and stream-lifecycle blockers, until a dedicated
  substrate exists;
- corpus-only blockers, until the pricing loop finds meaningful presence and
  row-specific current misses.

## Performance Gate

The issue uses the semantic-pack performance gate from
[semantic-pack-architecture](semantic-pack-architecture.md): the normal target is
within 5% median growth, and any greater than 10% median growth on the
query-regression subset blocks the PR unless explicitly accepted as a product
decision.

The measurement command is:

```sh
cargo build --release --bin nose
python3 bench/type4/query_regression/query_regression.py baseline \
  --nose ./target/release/nose \
  --repos-root bench/repos \
  --repeats 3 \
  --build-ref "issue507-pre@<sha>" \
  --out target/issue507/query-baseline-pre.json
python3 bench/type4/query_regression/query_regression.py compare \
  --nose ./target/release/nose \
  --repos-root bench/repos \
  --repeats 3 \
  --build-ref "issue507-post@<sha>" \
  --baseline target/issue507/query-baseline-pre.json \
  --summary target/issue507/query-compare-post.md
```

The first `--repeats 3` compare showed a small-repo phase trigger in `chi` on
`lower` and `parse+lower`; the changed code does not touch parser or lowering
paths. A fresh baseline binary from commit `768c102f` was then built in
`target/issue507/base-worktree` and compared with `--repeats 7`. That compare
showed no output drift and no HoF budget failure. It still reported
investigation triggers in `lower`/`parse+lower` for `boltons` and `ky`, again
outside the changed path.

Because those triggers were in unrelated small-repo phases, the runtime decision
uses paired alternating measurements between the baseline and current release
binaries:

| measurement | baseline | current | delta |
|---|---:|---:|---:|
| query-regression subset, sum of repo wall medians, alternating r9 | 1223.21 ms | 1223.15 ms | -0.0% |
| `gin`, alternating r31 wall median | 55.15 ms | 56.34 ms | +2.2% |
| `chi`, alternating r31 wall median | 34.30 ms | 34.01 ms | -0.9% |

The accepted implementation therefore stays under the 10% runtime-regression
limit. The phase-trigger root-cause note is measurement noise in parser/lower
stages that this issue did not modify; the relevant aggregate and focused
paired measurements are performance-neutral.

Back to [semantic-kernel](semantic-kernel.md).
