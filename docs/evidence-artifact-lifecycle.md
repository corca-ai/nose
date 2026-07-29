# Evidence artifact lifecycle

The authoritative lifecycle catalog is
[`scripts/evidence/artifacts.json`](../scripts/evidence/artifacts.json). It
assigns every checked JSON, JSONL, and checksum artifact to one owning domain
with a producer, validator, consumers, and retention policy. It also records
exact size, SHA-256 identity, provenance, supersession, and a retention decision
for every checked file at least 1 MiB.

The catalog is maintenance policy, not a replacement for domain validators.
Soundness, label, Type-4, cache, semantic-pack, and release checks continue to
validate the meaning of their evidence.

## Lifecycle classes

`canonical-input`
: A versioned corpus or measurement input whose identity is part of a
  reproducible contract. Keep it self-contained while any supported result
  names it.

`gold-input`
: Human-adjudicated or otherwise authoritative truth. Removal requires a
  reviewed replacement, provenance migration, and replay of every dependent
  gate.

`sealed-evidence`
: Blind-review, pre-commitment, or reveal evidence. Preserve the payload and
  binder together; a newer result does not authorize rewriting the historical
  boundary.

`derived-artifact`
: A reproducible report, packet, census, or calibration. It needs a producer
  and validator, or a documented producer exception when the historical
  environment is intentionally frozen.

`receipt`
: A binding between inputs, outputs, commands, digests, or decisions. Keep it
  for at least as long as every artifact it binds.

`active-baseline`
: The current comparison point for a fail-closed gate. Replace it only with a
  named successor that passes the same validator and after all consumers move.

`historical-evidence`
: A prior result needed for audit, release reconstruction, or regression
  attribution. Lack of a current CI invocation is not evidence that it is
  disposable.

`superseded-output`
: A named predecessor that has no active consumers. Removal is still allowed
  only after proving it is reproducible or policy-approved and is not gold,
  sealed, binding, or needed for a published claim.

The machine-readable definitions and removal rules in the catalog are
authoritative if this summary and the catalog ever disagree.

## Coverage and drift contract

Each domain set has an exact artifact count and an inventory SHA-256 over sorted
path, byte-size, and content-digest rows. The catalog's own row is normalized to
a fixed self marker to avoid a self-referential digest. Consequently, adding,
removing, renaming, or changing any checked artifact requires an explicit
catalog review even when the file is below 1 MiB.

The #951 audit covers 511 checked JSON, JSONL, and checksum artifacts in 17
non-overlapping owning sets. The count includes the repository gate-timing
receipt, which is retained with repository policy so future reviews can
reconstruct the measured commit and environment.

The focused gate is:

```sh
./scripts/check-ci-local.sh --gate evidence-artifacts
```

It runs mutation self-tests and the live validation. It fails on:

- an unowned JSON, JSONL, or checksum artifact;
- overlapping owners or stale set count/inventory digests;
- a missing or stale large-file path, size, or SHA-256;
- invalid lifecycle, producer, validator, provenance, supersession, or
  retention metadata;
- a receipt, seal, checksum, or active baseline without a two-way catalog
  relation;
- a broken relation endpoint or supersession cycle.

Relations are stored once as `binder` → `bound` rows. The validator indexes
both endpoints, so either side is traceable and removing either side fails
closed. Domain-specific receipt validators still check the embedded digest and
semantic contract.

## Conservative retention audit for #951

The audit found 19 checked files at least 1 MiB:

| Domain | Count | Lifecycle result |
|---|---:|---|
| Product labels | 12 | 2 active baselines, 3 sealed packets, 5 derived records, and 2 superseded-but-retained historical label sets |
| Recall-loss evidence | 3 | Historical external-corpus and runtime-parity reports bound by default-head provenance or closeout |
| Soundness 0.19.0 | 4 | Canonical cohort, historical crates report, derived exclusion census, and gold exclusion ledger |

All 19 are retained. The derived and superseded files still support checked
decisions, provenance, release reconstruction, or published soundness claims.
The audit therefore found no deletion whose reproducibility and consumer
closure were strong enough to justify removal. In particular, the three blind
heldout packets, the soundness cohort and exclusion ledger, and both v5 label
sets remain durable historical evidence.

This no-deletion result is intentional. Repository size reduction is secondary
to auditability, and the policy does not infer removability merely from a lack
of a direct CI reference.

## Updating artifacts

When an artifact changes:

1. Run its domain producer and validator, preserving the source revision,
   corpus/baseline identity, and external tool identity in the domain receipt.
2. Update explicit metadata for a file at least 1 MiB, or the binding relation
   when a receipt, seal, checksum, or baseline changes.
3. Recompute the owning set's count and inventory SHA-256 as part of the
   reviewed catalog change.
4. Run the focused lifecycle gate and the domain gate.
5. For removal, record the successor or regeneration proof, verify all direct
   consumers and relations are clear, and retain gold, sealed, soundness, and
   published-claim evidence unless a reviewed migration preserves equivalent
   auditability.

Do not refresh a digest merely to make the gate pass. The content change and
retention decision are the review subject.
