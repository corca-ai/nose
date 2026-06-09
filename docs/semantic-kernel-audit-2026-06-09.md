# Semantic kernel audit, 2026-06-09

Back to [semantic-kernel](semantic-kernel.md). Current implementation status is
in [semantic-kernel-snapshot](semantic-kernel-snapshot.md); remaining work is
tracked in [semantic-kernel-roadmap](semantic-kernel-roadmap.md).

This audit closes issue #150. It assumes PR #147 is merged and asks whether any
remaining first-party semantic consumer still opens exact semantics from raw
shape, selector spelling, raw `Seq`/payload tags, or local
`LibraryApiContract` plus evidence recomposition instead of crossing the
`nose-semantics` admission boundary.

## Result

One high-risk consumer bypass was found and closed during this audit:
`value_graph/builders.rs::list_append_parts` no longer treats a raw
active-builder method selector plus a first-party language row as list-append
proof. Active-builder receiver context proves only the receiver role; append
semantics must come from exact append-effect evidence or admitted `LibraryApi`
occurrence evidence.

After that closure, the remaining raw-looking pockets are either:

- first-party producers that mint evidence records;
- test fixtures that construct evidence;
- semantic consumers that now call shared admitted resolvers or evidence
  contracts;
- intentionally separate opaque identity policy that preserves same-callee exact
  identity without assigning library semantics;
- future pack surfaces that need new vocabulary before they can safely reopen
  exact convergence.

The value-graph regression test
`builder_append_candidate_requires_contract_or_effect_and_seed_context` now
asserts that raw active-builder method selectors stay closed. Existing tests
also cover the relevant hard negatives for raw library payloads, raw HOFs, raw
`Seq` surfaces, raw import coordinates, raw callee spelling, and missing
`LibraryApi` occurrence evidence.

## Closed During This Audit

- `crates/nose-normalize/src/value_graph/builders.rs::list_append_parts` now
  accepts list-builder append semantics only through
  `builder_append_call_args(...)` exact effect evidence or
  `admitted_builder_append_method_call_args(...)` admitted occurrence evidence.
- `crates/nose-normalize/src/library_api_evidence.rs` now emits first-party
  `LibraryApi(MethodCall(Builtin(Append)))` occurrence evidence for
  language-scoped source method append rows only when the receiver has
  seed-backed collection-domain proof, so source builders keep recall without
  reopening selector-only consumer fallback.
- `crates/nose-semantics/src/effects.rs::contracted_builder_append_method_call_args`
  was removed so semantic consumers cannot reopen method semantics from
  selector spelling plus a local language effect row.
- `crates/nose-normalize/src/value_graph/tests.rs::builder_append_candidate_requires_contract_or_effect_and_seed_context`
  covers the hard negative for a raw `push` method on an active builder seed and
  keeps the positive cases for admitted API evidence and asserted append-effect
  evidence.

## Audit Method

The audit searched production and test code for:

- direct `Lang` checks in semantic consumers;
- raw selector parsing through `Payload::Name` and `Interner::resolve`;
- raw `Seq`, `Payload::Builtin`, and `Payload::HoF` admission paths;
- local construction or inspection of `LibraryApiEvidenceKind::Contract`;
- direct calls to `library_*_contract(...)` outside `nose-semantics`;
- consumer-side calls to `library_api_contract_evidence_for_*`;
- places where exact matching remains open through same-name callee identity.

The important distinction is producer versus consumer. First-party lowering and
normalization producers may inspect raw source/IL shape to emit evidence; exact
consumers must require admitted evidence or a kernel contract before assigning
semantic meaning.

## Inventory

### Accepted first-party producers and helpers

- `crates/nose-semantics/src/library_api.rs` and
  `crates/nose-semantics/src/library_api/*`
  own first-party contract rows, evidence dependency validation, hash-to-row
  registry helpers, and admitted occurrence resolvers. Raw selector parsing here
  is the current compiled first-party kernel facade, not a consumer bypass.
- `crates/nose-normalize/src/library_api_evidence.rs::run` and its
  `record_*_library_api` helpers emit first-party `LibraryApi` and dependent
  `Domain` evidence after normalization. These helpers still inspect
  call/field/var shape and look up first-party rows, but they are producers.
  They are the main internal precursor to pack-provided occurrence emission.
  Builder append method occurrence evidence depends on receiver collection proof
  from existing domain evidence or an evidence-backed local collection seed.
- `crates/nose-frontend/src/lower.rs`, per-language frontend modules, and
  `crates/nose-frontend/src/module_imports.rs` emit source, domain, import,
  symbol, guard, place/effect, sequence-surface, and selected API evidence from
  source syntax. They remain first-party producers until language and stdlib
  packs exist.
- `crates/nose-normalize/src/effect_evidence.rs` records first-party effect and
  place evidence such as call mutation, self-receiver, assignment, and canonical
  builder append facts. These are producers; semantic consumers still need
  asserted evidence, admitted occurrence evidence, or kernel contracts before
  assigning exact meaning.
- `crates/nose-semantics/src/type_domain.rs` still recognizes first-party
  type-domain surfaces from source text for arrays, collections,
  iterable/iterator facts, records, options/results, promise/future-like facts,
  strings, booleans, integer/float/number distinctions, maps, sets, and byte
  arrays where the language-specific annotation surface is exact enough. This is
  accepted as a first-party producer; it is not yet a parsed/versioned external
  pack surface. Alias lifecycle and shadowing remain the frontend or pack
  producer's responsibility.
- `crates/nose-semantics/src/module_exports.rs` and import helpers consume
  evidence-backed provider/import facts. Raw import-coordinate `Seq` values may
  still carry lowered structure, but they no longer prove import identity by
  themselves.

### Test and fixture only

- Direct `library_*_contract(...)` calls and manual
  `LibraryApiEvidenceKind::Contract` construction in
  `crates/nose-normalize/src/idioms.rs`, `crates/nose-normalize/src/interp.rs`,
  `crates/nose-normalize/src/effect_evidence.rs`,
  `crates/nose-normalize/src/value_graph/tests*.rs`,
  `crates/nose-detect/src/strict_exact.rs`, and
  `crates/nose-detect/src/units.rs` are under test modules or local test helpers.
  They build asserted evidence fixtures and hard negatives; they are not
  production admission paths.

### Migrated semantic consumers

- `crates/nose-normalize/src/desugar.rs` consumes property builtins, HOF
  receiver proof, and method HOF admission through `nose-semantics` admitted
  resolvers and domain contracts.
- `crates/nose-normalize/src/idioms.rs` production consumers for free-function
  builtins, generic method calls, map get, map-key views, Rust `Some`, iterator
  adapters, and Java static adapters now call admitted resolvers.
- `crates/nose-normalize/src/value_graph/builders.rs::list_append_parts` now
  accepts list-builder append only through exact append-effect evidence or an
  admitted append method occurrence. Row-only method-effect fallback has been
  removed.
- `crates/nose-normalize/src/value_graph/{collections,stdlib,eval}.rs` use
  admitted call, node, or span resolvers for producer-covered factory, method,
  map, property, Option, static-index, and scalar-integer surfaces.
- `crates/nose-normalize/src/value_graph/output.rs::seq_tag` still dispatches on
  `record_guard` and `own_property_guard` raw tags, but the tag is not proof:
  the value tag is emitted only when the corresponding guard evidence helper
  admits the node. Other `Seq` surfaces go through `seq_surface_contract_for_node`
  and otherwise become untagged.
- `crates/nose-detect/src/strict_exact.rs` consumes admitted resolvers for
  builtin payloads, HOFs, collection/map factories, Java constructors, static
  JS-like helpers, map get/key views, regex `.test`, iterator adapters, Rust
  `Vec`/`Option`, and static index membership. `Seq` admission uses
  `seq_surface_contract_for_node`; Go zero-map literal/default lookup uses
  Go map literal and entry contracts before accepting the shape.

### Intentionally separate opaque identity policy

- `crates/nose-detect/src/strict_exact.rs::strict_exact_callee_identity` keeps
  exact same-callee calls eligible when the callee itself is exact-safe or when a
  concrete `CallTarget` fact proves a direct local, imported function, or
  imported member target. This is deliberately not library/API semantics: the
  call only remains an opaque same-call value, and cross-language or builtin
  convergence must still use admitted semantic contracts. Direct method records
  also require exact receiver identity; dynamic-dispatch records do not prove a
  single concrete target by themselves.

## Remaining Backlog

These are not #150 fixes; they are the next independent work items.

| priority | owner | remaining pocket | why it remains |
|---|---|---|---|
| P0 | #151 | Pack extension API v0 | First-party producers still call compiled Rust helper functions directly. The external boundary needs stable fact, contract, dependency, and channel-eligibility schemas before those producers can be expressed as packs. |
| P0 | #153 | Demand/effect substrate | Lazy, repeated, async, generator, channel, observable, and callback effect visibility cannot be represented by the current first-party demand profiles. Exact laws for those surfaces must remain closed. |
| P0 | #155 | Call-target and dispatch evidence | The shared vocabulary and resolver now cover direct functions, direct methods, imported functions/members, and dynamic-dispatch facts. Remaining work is broader producer coverage for method/imported/module targets, trait/interface dispatch, and overload-specific target proof. |
| P0 | #156 | Type/domain evidence expansion | The shared vocabulary now covers richer scalar, protocol, record, future/result, and nominal domains. Remaining work is broader producer coverage for constructor result domains, field/property domains, protocol receiver facts with demand/effect obligations, and versioned library domains. |
| P1 | #154 | Async and Promise receiver proof | `crates/nose-normalize/src/value_graph/rules/promise_then.rs` has a JS-like `.then` contract but returns fail-closed until Promise-like receiver proof exists. This should depend on #153 and #156. |
| P1 | #152 | Pack loading, provenance, and opt-in trust | The internal provenance/trust model exists, but there is no loadable pack runtime, manifest validation, user opt-in path, or report-level pack provenance. |
| P1 | #157 | Pack conformance harness and ecosystem workflow | The first-party regression suite covers many hard negatives, but external pack providers need a defined conformance fixture layout and workflow. |
| P2 | #153/#156 | Guard, sequence, and aggregate surfaces | JS/TS record/own-property guards and selected sequence surfaces are evidence-backed where covered, but richer guard clauses, nested aggregate surfaces, iterator materialization, map-entry variants, and protocol-specific aggregate facts need versioned records. |
| P2 | #153/#157 | LawPack-facing value laws | Named value-graph rules and formal-obligation metadata exist, but reduction, parity/toggle, byte-pack, and ecosystem laws remain local first-party code rather than pack-facing law contracts. |

## Follow-up Guidance

Start #151 and #153 first. #151 defines the extension shape that producers will
target; #153 defines the semantic axes that prevent lazy, async, and callback
APIs from being squeezed into unsafe API rows. #155 and #156 should then expand
the evidence vocabulary consumed by both. #154 should stay closed until those
receiver/demand pieces exist. #152 and #157 should follow the API shape from
#151 and feed conformance requirements back into it early.
