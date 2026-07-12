# Type-4 #791 Closeout

Closeout issue: #799

Status: complete with controlled admissions and explicit real-replay gaps.

## Summary

The #791 neutral-fact epic is closed at its proof perimeter. All seven rows frozen as
`blocked-by-unmodeled-facts` have left the current open audit, and the twelve reusable
facts they required are now `modeled-controlled`.

- current open audit rows: 0
- audit groups: 7, including the final closeout group
- modeled neutral-fact groups: 6
- frozen rows resolved or promoted: 7/7
- frozen rows still blocked: 0
- unexpected blocked rows or validation errors: 0
- executable expectations: 358/358 across 41 query runs
- real-frontier replay: 29/29 across 11 query runs
- target packets: 7 unchanged
- proof-carrying frontier: `no-exact-admission-ready-packets`

## In-Scope Disposition

| issue | surface | disposition | new real replay |
|---|---|---|---|
| #793 | Ruby nil/option presence | controlled focused admission | none available in the checked frontier |
| #793 | Swift Optional presence/defaulting | controlled focused admission | none available in the checked frontier |
| #795 | Swift `compactMap` option emission | controlled focused admission | none available in the checked frontier |
| #796 | Swift one-level `flatMap` | controlled focused admission | none available in the checked frontier |
| #797 | Java `Stream.flatMap` aggregate reduction | controlled focused admission | none available in the checked frontier |
| #797 | Swift `flatMap` / `allSatisfy` aggregate reduction | controlled focused admission | none available in the checked frontier |
| #798 | Swift `Dictionary` default subscript | controlled focused admission | none available in the checked frontier |

The checked real-frontier inventory has 31 items and 29 executable replay entries, but
none is a new source-backed pair for these seven language surfaces. The closeout therefore
keeps replay at 29/29 instead of relabeling focused fixtures or probes as real-corpus
evidence.

## Proof-Fact Disposition

Six implementation groups modeled twelve shared facts; the audit's seventh group is this
closeout and does not introduce another fact:

- option absence-channel identity;
- HOF callback purity;
- filter-map drop and emitted-value coordinates;
- one-level flat-map depth, traversal order, and emitted-value coordinates;
- flat-map aggregate guard coordinates;
- map absence fallback, receiver identity, key/fallback coordinates, and mutation closure.

The semantic cards and proof registry admit only the controlled slices backed by those
facts. Unsupported callback effects, dispatch, receiver identity, mutation, channel
coordinates, and deeper flattening remain explicit boundaries rather than residual open
rows from this epic.

## Frontier Decision

The seven pre-existing target packets remain unchanged. Readiness reports no non-admitted
packet to queue and the proof-carrying frontier has zero exact-admission-ready packets.
That is a completed closeout state, not a reason to manufacture another follow-up issue:
future work should begin only when new source-backed evidence identifies a concrete fact
gap outside this frozen slice.

The machine-readable snapshot and its cross-artifact validation command are recorded in
`issue_791_closeout.v1.json`.
