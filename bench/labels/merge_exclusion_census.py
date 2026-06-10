#!/usr/bin/env python3
"""Merge per-repo `nose verify --exclusion-census` shards into one corpus inventory.

The oracle-completeness campaign measures, per IL construct, how much
fingerprint-merge mass carries no behavioral verification (see
`crates/nose-cli/src/verify_census.rs` and experiments §BL). The corpus pass is
run per repo — matching how the product actually scans, so fingerprint-merge
pairs are counted within a repo, never across repos — and merged here:

    ls bench/repos | grep -v '^raylib$' | xargs -P 6 -I {} sh -c \\
      './target/release/nose verify bench/repos/{} \\
         --exclusion-census /tmp/census_shards/{}.json \\
         --leads /tmp/leads_shards/{}.json > /dev/null 2>&1'
    python3 bench/labels/merge_exclusion_census.py /tmp/census_shards \\
      bench/labels/oracle_exclusion_census_<date>.json

(`raylib` is excluded until #208 — verify does not finish on it in useful time.)

Deterministic: output sorted on stable keys; no timestamps.
"""

import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main() -> None:
    shard_dir = Path(sys.argv[1])
    out_path = sys.argv[2]
    nose_version = subprocess.run(
        [str(ROOT / "target" / "release" / "nose"), "--version"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

    units_total = interpretable = 0
    excluded_by_reason: dict[str, int] = defaultdict(int)
    merge = {"total": 0, "verified": 0, "unverified": 0}
    tags: dict[str, dict] = defaultdict(
        lambda: {
            "interpretable_units": 0,
            "excluded_units": 0,
            "unverified_pairs": 0,
            "example_excluded": [],
        }
    )
    shards = sorted(shard_dir.glob("*.json"))
    for p in shards:
        d = json.loads(p.read_text())
        units_total += d["units_total"]
        interpretable += d["interpretable_units"]
        for k, v in d["excluded_by_reason"].items():
            excluded_by_reason[k] += v
        for k in merge:
            merge[k] += d["merge_pairs"][k]
        for row in d["tags"]:
            t = tags[row["tag"]]
            t["interpretable_units"] += row["interpretable_units"]
            t["excluded_units"] += row["excluded_units"]
            t["unverified_pairs"] += row["unverified_pairs"]
            t["example_excluded"] += [f"{p.stem}:{e}" for e in row["example_excluded"]]

    rows = []
    for tag, t in tags.items():
        t["example_excluded"] = sorted(t["example_excluded"])[:3]
        rows.append({"tag": tag, **t})
    rows.sort(key=lambda r: (-r["unverified_pairs"], -r["excluded_units"], r["tag"]))

    out = {
        "schema_version": "0.1.0",
        "nose_version": nose_version,
        "sharding": "per-repo (merge pairs counted within repo, matching per-repo scans)",
        "excluded_repos": ["raylib (verify does not finish, #208)"],
        "shards": len(shards),
        "units_total": units_total,
        "interpretable_units": interpretable,
        "excluded_by_reason": dict(sorted(excluded_by_reason.items())),
        "merge_pairs": merge,
        "tags": rows,
    }
    Path(out_path).write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")

    ex = units_total - interpretable
    print(
        f"units {units_total} | interpretable {interpretable} "
        f"({100 * interpretable / units_total:.1f}%) | excluded {ex} "
        f"{dict(sorted(excluded_by_reason.items()))}"
    )
    print(
        f"merge pairs: {merge['total']} total, {merge['verified']} verified, "
        f"{merge['unverified']} unverified "
        f"({100 * merge['unverified'] / max(1, merge['total']):.1f}%)"
    )
    print("\ntop constructs by unverified merge mass:")
    for r in rows[:14]:
        tot_units = r["interpretable_units"] + r["excluded_units"]
        print(
            f"  {r['tag']:<34} unverified={r['unverified_pairs']:<8} "
            f"excluded_units={r['excluded_units']:<7} "
            f"(excl-share {100 * r['excluded_units'] / max(1, tot_units):.0f}%)"
        )


if __name__ == "__main__":
    main()
