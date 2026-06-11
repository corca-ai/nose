# Adversarial co-evolution packet ledger

`packets.v1.json` is the machine-readable record of every target packet across
campaign series — surface, persona, mode, claim, verdict, and defense pointer.
The narrative per series lives in [experiments.md](../../docs/experiments.md)
(§BZ, §CA, …); the protocol is
[docs/adversarial-coevolution.md](../../docs/adversarial-coevolution.md).

Uses: attacker no-resubmission lists, persona-precision tracking, assessor dedup.
Series 1–2 entries are condensed backfill from the experiments sections; series 3
onward records packets as submitted.

Verdicts: `violation-fixed` · `refuted` · `recorded-low-prevalence` ·
`deferred-issue` · `green-confirmed`.
