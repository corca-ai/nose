#!/usr/bin/env python3
"""Price putting the near channel on the default scan surface.

The experiment compares the current CLI default (`syntax,semantic`) with opt-in
`near` arms on the frozen v5 refactoring-family labelset. It reports:

* P@10 in nose's native JSON order (`extractability`)
* worthy-label recall
* default-surface family-count deltas by family `scope`

Example:

    python3 bench/labels/near_default_surface_experiment.py \
      --repos-root /Users/ak/prjs/cc/nose/bench/repos \
      --cache-dir /tmp/nose-near-default-cache \
      --output bench/labels/near_default_surface_2026_06_10.json
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import random
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NOSE = ROOT / "target" / "release" / "nose"
SURFACES = ("default", "review", "hidden", "debug")
SCOPES = ("prod", "test", "mixed", "unknown")
ARMS = (
    ("default", None),
    ("near", "syntax,semantic,near"),
    ("near:0.80", "syntax,semantic,near:0.8"),
    ("near:0.85", "syntax,semantic,near:0.85"),
)

spec = importlib.util.spec_from_file_location("ev", ROOT / "bench/labels/eval_by_language.py")
ev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ev)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--repos-root",
        type=Path,
        default=ROOT / "bench" / "repos",
        help="checkout root containing one directory per corpus repo",
    )
    p.add_argument("--cache-dir", type=Path, help="forwarded to nose scan --cache-dir")
    p.add_argument(
        "--output",
        type=Path,
        default=ROOT / "bench/labels/near_default_surface_2026_06_10.json",
        help="aggregate JSON result path",
    )
    p.add_argument("--bootstrap", type=int, default=2000, help="bootstrap resamples per CI")
    p.add_argument("--timeout", type=int, default=300, help="per-repo scan timeout in seconds")
    p.add_argument("--limit-repos", type=int, help="smoke-test with the first N labeled repos")
    return p.parse_args()


def load_inputs() -> tuple[list[dict], dict[str, dict], dict[str, list[dict]]]:
    labels = json.loads((ROOT / "bench/labels/refactoring_families.v5.json").read_text())["families"]
    corpus = {
        r["id"]: r
        for r in json.loads((ROOT / "bench/goldens/corpus.json").read_text())["repositories"]
    }
    by_repo: dict[str, list[dict]] = defaultdict(list)
    for label in labels:
        by_repo[label["repo"]].append(label)
    return labels, corpus, by_repo


def scan(repo: Path, mode: str | None, cache_dir: Path | None, timeout: int) -> dict:
    cmd = [str(NOSE), "scan", str(repo), "--format", "json", "--top", "0"]
    if mode:
        cmd += ["--mode", mode]
    if cache_dir:
        cmd += ["--cache-dir", str(cache_dir)]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=timeout)
    r.check_returncode()
    payload = json.loads(r.stdout or "{}")
    if isinstance(payload, list):
        return {"ranking": {}, "families": payload}
    return payload


def overlap_count(family: dict, label: dict) -> int:
    return sum(
        1
        for site in ev.mlocs(family)
        for member in label["members"]
        if ev.ov(site, member)
    )


def p10_flags(families: list[dict], labels: list[dict]) -> list[int]:
    out = []
    for family in families[:10]:
        best = None
        best_overlap = 0
        for label in labels:
            overlap = overlap_count(family, label)
            if overlap > best_overlap:
                best = label
                best_overlap = overlap
        if best:
            out.append(1 if best["worthy"] else 0)
    return out


def recall_flags(families: list[dict], labels: list[dict]) -> list[int]:
    out = []
    for label in labels:
        if not label["worthy"]:
            continue
        hit = any(overlap_count(family, label) > 0 for family in families)
        out.append(1 if hit else 0)
    return out


def surface_counter(payload: dict) -> Counter:
    counts = Counter()
    reported = payload.get("ranking", {}).get("surface_counts") or {}
    for surface in SURFACES:
        counts[surface] = int(reported.get(surface, 0) or 0)
    return counts


def surface_scope_counter(families: list[dict]) -> Counter:
    counts = Counter()
    for family in families:
        surface = family.get("recommended_surface") or "unknown"
        scope = family.get("scope") or "unknown"
        counts[(surface, scope)] += 1
    return counts


def display_path(path: Path) -> str:
    text = str(path)
    marker = "bench/repos"
    idx = text.find(marker)
    return text[idx:] if idx >= 0 else text


def ci(flags: list[int], bootstrap: int, rng: random.Random) -> tuple[float, float]:
    if not flags:
        return (0.0, 0.0)
    n = len(flags)
    samples = []
    for _ in range(bootstrap):
        samples.append(sum(flags[rng.randrange(n)] for _ in range(n)) / n)
    samples.sort()
    return (samples[int(0.025 * bootstrap)] * 100, samples[int(0.975 * bootstrap)] * 100)


def metric(flags: list[int], bootstrap: int, rng: random.Random) -> dict:
    pct = (sum(flags) / len(flags) * 100) if flags else 0.0
    lo, hi = ci(flags, bootstrap, rng)
    return {
        "pct": round(pct, 2),
        "ci95": [round(lo, 2), round(hi, 2)],
        "n": len(flags),
        "hits": sum(flags),
    }


def evaluate(args: argparse.Namespace) -> dict:
    _labels, corpus, by_repo = load_inputs()
    repo_ids = sorted(by_repo)
    if args.limit_repos:
        repo_ids = repo_ids[: args.limit_repos]

    acc = {
        arm: defaultdict(lambda: {"p10": [], "recall": [], "worthy": 0, "labels": 0})
        for arm, _mode in ARMS
    }
    noise = {
        arm: {"surfaces": Counter(), "surface_scope": Counter(), "repos": 0}
        for arm, _mode in ARMS
    }
    missing = []

    for rid in repo_ids:
        repo = args.repos_root / rid
        if not repo.is_dir():
            missing.append(rid)
            continue
        labs = by_repo[rid]
        meta = corpus[rid]
        keys = [(meta["primary_language"], meta["split"]), ("OVERALL", meta["split"])]
        for arm, mode in ARMS:
            payload = scan(repo, mode, args.cache_dir, args.timeout)
            families = payload.get("families", [])
            for key in keys:
                acc[arm][key]["p10"].extend(p10_flags(families, labs))
                acc[arm][key]["recall"].extend(recall_flags(families, labs))
                acc[arm][key]["worthy"] += sum(1 for label in labs if label["worthy"])
                acc[arm][key]["labels"] += len(labs)
            noise[arm]["surfaces"].update(surface_counter(payload))
            noise[arm]["surface_scope"].update(surface_scope_counter(families))
            noise[arm]["repos"] += 1

    rng = random.Random(1)
    metrics = {}
    for arm, values in acc.items():
        metrics[arm] = {}
        for (lang, split), data in sorted(values.items()):
            metrics[arm].setdefault(split, {})[lang] = {
                "labels": data["labels"],
                "worthy_labels": data["worthy"],
                "p_at_10": metric(data["p10"], args.bootstrap, rng),
                "worthy_recall": metric(data["recall"], args.bootstrap, rng),
            }

    noise_out = {}
    baseline_scope = noise["default"]["surface_scope"]
    baseline_surfaces = noise["default"]["surfaces"]
    for arm, data in noise.items():
        surface_scope = data["surface_scope"]
        default_by_scope = {
            scope: int(surface_scope[("default", scope)])
            for scope in SCOPES
            if surface_scope[("default", scope)] or scope != "unknown"
        }
        delta_default_by_scope = {
            scope: int(surface_scope[("default", scope)] - baseline_scope[("default", scope)])
            for scope in SCOPES
            if surface_scope[("default", scope)] or baseline_scope[("default", scope)] or scope != "unknown"
        }
        surfaces = {surface: int(data["surfaces"][surface]) for surface in SURFACES}
        noise_out[arm] = {
            "repos": data["repos"],
            "surfaces": surfaces,
            "surface_delta_vs_default": {
                surface: int(data["surfaces"][surface] - baseline_surfaces[surface])
                for surface in SURFACES
            },
            "default_surface_by_scope": default_by_scope,
            "default_surface_delta_by_scope": delta_default_by_scope,
        }

    return {
        "schema_version": 1,
        "generated_by": "bench/labels/near_default_surface_experiment.py",
        "labelset": "refactoring_families.v5.json",
        "arms": [{"name": name, "mode": mode or "(CLI default syntax,semantic)"} for name, mode in ARMS],
        "repos_root": display_path(args.repos_root),
        "missing_repos": missing,
        "metrics": metrics,
        "noise": noise_out,
    }


def fmt_metric(data: dict) -> str:
    lo, hi = data["ci95"]
    return f'{data["pct"]:.1f}% [{lo:.1f}-{hi:.1f}] n={data["n"]}'


def markdown(result: dict) -> str:
    lines = []
    lines.append("## Near default-surface experiment")
    lines.append("")
    lines.append("P@10 uses nose's native JSON order (`extractability`); worthy-recall is over worthy v5 labels.")
    for split in ("dev", "heldout"):
        lines.append("")
        lines.append(f"### {split}")
        lines.append("")
        lines.append("| arm | language | worthy labels | P@10 | worthy recall |")
        lines.append("|---|---:|---:|---:|---:|")
        for arm, split_map in result["metrics"].items():
            for lang in sorted(split_map.get(split, {}), key=lambda x: (x != "OVERALL", x)):
                data = split_map[split][lang]
                lines.append(
                    f"| {arm} | {lang} | {data['worthy_labels']}/{data['labels']} | "
                    f"{fmt_metric(data['p_at_10'])} | {fmt_metric(data['worthy_recall'])} |"
                )
    lines.append("")
    lines.append("### default-surface family deltas")
    lines.append("")
    lines.append("| arm | default | delta | prod delta | test delta | mixed delta | review delta | hidden delta |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
    for arm, data in result["noise"].items():
        lines.append(
            f"| {arm} | {data['surfaces']['default']} | {data['surface_delta_vs_default']['default']:+d} | "
            f"{data['default_surface_delta_by_scope'].get('prod', 0):+d} | "
            f"{data['default_surface_delta_by_scope'].get('test', 0):+d} | "
            f"{data['default_surface_delta_by_scope'].get('mixed', 0):+d} | "
            f"{data['surface_delta_vs_default']['review']:+d} | "
            f"{data['surface_delta_vs_default']['hidden']:+d} |"
        )
    return "\n".join(lines)


def main() -> None:
    args = parse_args()
    result = evaluate(args)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(markdown(result))
    print(f"\nwrote {args.output}")


if __name__ == "__main__":
    main()
