# #891 checked-in generated artifact provenance

Issue #891 follows the fresh-repository audit from #846. Picocli's bare default was
dominated by checked-in generated HTML, while Pydantic's mypy snapshots and Check's
Autoconf Archive pages exposed no producer-independent proof that nose could safely use.
This tranche deliberately solves only the mechanically proven class.

## Product behavior

A file has self-declared generated-document provenance when its first 64 KiB contains a
complete HTML document with an explicit `doctype`, `html`, and `head`, and that head has a
real `meta` element whose `name` is `generator` and whose `content` is non-empty. Attribute
order, quoting, whitespace, and ASCII case do not matter. The value is not matched against
an Asciidoctor, Javadoc, repository, language, directory, or filename allowlist.

Comments, quoted/escaped examples, `script`, `style`, and `template` contents, lookalike
attribute names, and empty declarations do not qualify. A family moves to
`surface: "generated"` only when every member file already has generated provenance or
passes this rule. Missing, unreadable, partial, or mixed evidence fails open. The rule is
surface-only: it does not alter helper selection, family construction, witnesses, ranking,
or overlap folding. The human default omits the family and reports `generated-code`; users
and integrations can recover it with `all top=0` or `surface=generated`.

## Frozen development result

The [development cohort](../bench/labels/generated_artifact_provenance_891_dev_cohort_2026_07_19.v1.json)
was frozen before implementation from the already sealed #846 field discoveries. The
candidate was compared byte-for-byte with its immediate pre-implementation binary.

| repository | expanded families | surface transitions | bare top-10 effect |
|---|---:|---:|---|
| Picocli | 3,256 | 599 | 9 generated families leave; the mixed docinfo family remains |
| Pydantic | 1,498 | 0 | byte-identical |
| Check | 148 | 0 | byte-identical |

Picocli's transitions are exactly 280 `default -> generated`, 261
`hidden -> generated`, and 58 `shallow -> generated`. All 3,256 ordered IDs and every
non-`surface` field are unchanged. The 599-entry ID/before/after ledger is embedded in the
checked [closeout artifact](../bench/labels/generated_artifact_provenance_891_closeout_2026_07_19.v1.json).
All 66 existing dev repositories (54,758 families) are byte-identical, preserving the
full-universe worthy set and adding zero family merges.

## Held-out confirmation and honest limit

PlantUML, mypy, and Autoconf Archive identities and commits were recorded in the
pre-implementation [sealed held-out selection](../bench/labels/generated_artifact_provenance_891_heldout_selection_2026_07_19.v1.json), then opened only after commit `1b33953e`.
Their 4,537 expanded families and bare top tens
are byte-identical before and after, so the packet found no false demotion. None contained
a positive family for the new contract, however; this held-out result confirms precision,
not positive sensitivity. The predicate and sample were not changed after reveal.

Pydantic and Check therefore remain an explicit no-go for detector-side inference:
Pydantic's observed families mix maintained inputs with analysis outputs, while the Check
pages expose only Autoconf-specific titles, links, and asset names. Recognizing either
would require a path/tool/ecosystem exception. The result-dependent next step is a separate
caller-provided generated-provenance or path-control API in
[#925](https://github.com/corca-ai/nose/issues/925), not more built-in heuristics.

## Determinism and published-release price

Candidate JSON is byte-identical at `RAYON_NUM_THREADS=1` and `4` across all 66 dev
repositories and all three held-out repositories (59,295 total families, zero mismatch).
The official performance baseline is the published Darwin arm64 v0.19.0 binary, SHA-256
`0f73ea54…e0f3`; the same-binary control uses the candidate on both sides.

| three-iteration all-dev run | baseline | current | delta |
|---|---:|---:|---:|
| published v0.19.0 vs candidate | 15,840.43 ms | 15,807.42 ms | -33.02 ms / -0.21% |
| candidate vs candidate control | 15,316.13 ms | 15,368.85 ms | +52.73 ms / +0.34% |
| control-adjusted | — | — | -85.74 ms / -0.54% |

The material-regression gate requires a control-adjusted increase greater than both 5%
and 5 ms, so no focused escalation was requested. Raw runs and binary/corpus provenance
are checked in under `bench/recall_loss/issue-891-*`.

## Validation

```sh
cargo test -p nose-cli --lib declared_generator -- --nocapture
cargo test -p nose-cli --test cli \
  query_declared_generator_provenance_is_reason_coded_and_recoverable -- --nocapture
cargo clippy -p nose-cli --all-targets -- -D warnings
./scripts/check-ci-local.sh --fast
./scripts/check-docs.sh
```
