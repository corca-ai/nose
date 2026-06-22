# Semantic pack boundary review, 2026-06-22

Status: pre-release review for the builtin semantic-pack operating model after
the #484 stabilization tracker. This review does not bump the nose version.

Reviewed base commit before this docs-only PR:
`442668ab2fdb7f6c74ad73d9cfef20bef507625b`.

## Verdict

The current semantic kernel and builtin semantic-pack split is suitable for the
next minor release, with one important caveat: builtin packs are compiled Rust
descriptors and in-tree evidence producers today, not an external plugin runtime.
That is the intended builtin-first stage. The important safety boundary holds:
external manifests are metadata-only, and exact analysis still crosses
kernel-owned admission checks.

No release-blocking kernel/pack boundary issue was found.

## Reviewed Surfaces

Docs reviewed:

- [semantic-kernel](semantic-kernel.md)
- [semantic-pack-architecture](semantic-pack-architecture.md)
- [semantic-pack-adoption](semantic-pack-adoption.md)
- [semantic-pack-compatibility](semantic-pack-compatibility.md)
- [semantic-pack-conformance](semantic-pack-conformance.md)
- [semantic-pack-loading](semantic-pack-loading.md)
- [capabilities](capabilities.md)
- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md)

Code reviewed:

- `crates/nose-semantics/src/packs.rs`
- `crates/nose-semantics/src/packs/compiled.rs`
- `crates/nose-semantics/src/packs/loading.rs`
- `crates/nose-semantics/src/packs/validation.rs`
- `crates/nose-semantics/src/packs/external.rs`
- `crates/nose-cli/src/query_dataset.rs`
- `crates/nose-cli/src/query_commands.rs`
- `crates/nose-cli/src/query_semantic_packs.rs`
- `crates/nose-cli/src/semantic_pack.rs`
- `crates/nose-cli/src/semantic_pack/inventory.rs`
- `crates/nose-cli/src/semantic_pack/adoption_gates.rs`
- `crates/nose-cli/src/semantic_pack/compatibility.rs`
- `crates/nose-cli/src/capabilities.rs`
- semantic-pack CLI tests for external metadata-only behavior and preflight
  blockers

## Boundary Assessment

The split is desirable because the kernel owns policy and admission, while packs
own semantic knowledge.

Kernel-owned responsibilities currently include:

- manifest API parsing and compatibility validation;
- trust lane and default-enable policy;
- reserved builtin id rejection for local manifests;
- evidence, contract, law, channel, and proof-status vocabulary;
- external row conflict detection;
- row-level external influence preflight blockers;
- query JSON and capabilities reporting;
- exact-channel admission through dependency-backed builtin evidence and
  contract/law resolvers.

Builtin-pack-owned responsibilities currently include:

- stable pack ids and descriptor metadata;
- language, stdlib, protocol, library, and law ownership boundaries;
- evidence producer ids, source-fact producer ids, contract ids, and law ids;
- fixture/conformance refs and coverage counts;
- supported language/package metadata;
- builtin trust lane and default enablement.

The implementation is not a pure inversion-of-control plugin boundary yet.
`packs.rs` imports many builtin constants, and parser/lowering implementation
remains in-tree. That is acceptable for the current stage because builtin packs
are compiled, nose-owned, and released with the binary. It would not be
acceptable as the external-pack influence design.

## Evidence

Local external manifests load through `SemanticPackSet::new_local`, but their
summary is forced to `source = local-manifest` and `influence = metadata-only`.
Their producer, contract, and law rows are stored as external rows for reporting
and preflight only.

External influence preflight always keeps these blockers before influence can
open:

- `data-only-registration`
- `dependency-backed-evidence-unavailable`
- `explicit-influence-trust-gate-missing`

Executable conformance can clear only `executable-conformance-unavailable`; it
does not clear the data-only, dependency, or trust blockers.

`nose query` resolves semantic packs for metadata reporting, but detection still
runs from the lowered corpus and detect options, not from external rows. The
`base=` divergent-edit view rejects configured or CLI semantic packs, avoiding a
second pack-enabled divergence path.

Compiled builtin descriptors report `source = compiled-builtin` and
`influence = evidence-and-contracts`. They are the only pack rows allowed to
affect analysis in this release line.

Current diagnostic evidence on the reviewed commit:

- `nose semantic-pack inventory --format json`: `status=ok`,
  `builtin_packs=43`, `exact_capable_packs=33`, `packs_needing_coverage=0`;
- `nose semantic-pack adoption-gates --format json`: `status=ok`,
  `blocked_packs=0`;
- `nose semantic-pack compatibility --format json`: `status=ok`,
  `external_pack_influence=metadata-only`,
  `external_pack_execution=none`, and `external_metadata_only=true`.
- `nose capabilities`: `schema_version=4`, `query_json=6`,
  `semantic_pack_conformance=2`, `semantic_pack_inventory=1`,
  `semantic_pack_adoption_gates=1`, and `semantic_pack_compatibility=1`.

## Non-Blocking Follow-Ups

These are not blockers for the next minor release, but they should be handled
before broad ecosystem work or external influence:

- Add checked golden fixtures for the machine-readable semantic-pack reports so
  docs and schema examples cannot drift from CLI output again.
- Before bumping from `0.14.x` to `0.15.0`, update the example manifests and
  tests whose `compatibility.nose` ranges intentionally mention
  `>=0.14.0 <0.15.0`.
- Keep reducing the broad `nose.first_party` compatibility surface. It is
  acceptable as a v0 compatibility descriptor, but it should not regain active
  semantic ownership.
- Expand the builtin inventory audit from pack-level fixture counts to row-level
  fixture coverage in a future schema version.
- Run the release query-regression and corpus-verify gates on the final release
  candidate. If detection output changes, follow the
  [hazard-release-checklist](hazard-release-checklist.md).

## Release Readiness Checklist

Before cutting the next minor release, keep the remaining unchecked items as
release-candidate gates. Checked items below were verified during this boundary
review and should still be re-run on the final release candidate.

- [x] `CHANGELOG.md` has Unreleased notes for semantic-pack user-visible changes.
- [ ] Example manifest `compatibility.nose` ranges match the release version.
- [x] `nose capabilities` advertises the intended schema versions.
- [x] `nose semantic-pack inventory --format json` reports no coverage gaps.
- [x] `nose semantic-pack adoption-gates --format json` reports no blocked packs.
- [x] `nose semantic-pack compatibility --format json` still reports external
      influence as metadata-only and execution as none.
- [x] `nose semantic-pack check docs/examples/semantic-packs/v0 --format json`
      passes.
- [ ] `scripts/check-ci-local.sh --full` or equivalent CI has passed.
- [ ] Query-regression/product-output and runtime drift have been recorded for
      the final release candidate.

## Related

- [semantic-pack-architecture](semantic-pack-architecture.md)
- [semantic-pack-adoption](semantic-pack-adoption.md)
- [semantic-pack-compatibility](semantic-pack-compatibility.md)
- [semantic-pack-conformance](semantic-pack-conformance.md)
- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md)
