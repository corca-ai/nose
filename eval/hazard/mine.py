#!/usr/bin/env python3
"""Mine divergent-edit (G1) and control (G0) clone-family events from a repo's history.

Tier-1 ground-truth pipeline from docs/hazard-benchmark.md, using nose as the
Type-4-aware clone *identifier*. At each monthly snapshot T we ask nose which sites
form a clone family; git tells us which of those members changed by T+1. The label is
Kim's Inconsistent-Change predicate, computed channel-agnostically from git:

  - G1  (divergent edit): 0 < (members changed) < (all members) — some siblings were
        edited, others were not. The unchanged siblings are the "missed" copies.
  - G0c (consistent change): every member changed together (propagated edit) — control.
  - G0s (stable): no member changed — control.

Each record carries the family's features at T (the forward-prediction state) so a
hazard score computed at T can be evaluated against the (T, T+1] outcome.

Usage:
  mine.py --repo /path/to/clone --nose /path/to/nose [--subdir src] \
          [--mode semantic,near] [--threshold 0.7] --max-months 60 --out events.jsonl
The repo dir is checked out (detached) repeatedly — pass a throwaway clone.
"""
import argparse, json, os, subprocess, sys, re


def sh(args, cwd=None):
    # errors="replace": some repos have non-UTF-8 bytes in diffs (binary/legacy files)
    return subprocess.run(args, cwd=cwd, capture_output=True, text=True, errors="replace")


def monthly_commits(repo, branch, max_months):
    """Newest commit per calendar month, oldest->newest, capped to the most recent max_months."""
    r = sh(["git", "-C", repo, "log", "--first-parent", "--pretty=%H|%cI", branch])
    if r.returncode != 0:
        sys.exit(f"git log failed: {r.stderr}")
    seen, picked = set(), []
    for line in r.stdout.splitlines():  # newest -> oldest
        sha, iso = line.split("|", 1)
        ym = iso[:7]
        if ym not in seen:
            seen.add(ym)
            picked.append((sha, iso))
    return list(reversed(picked[:max_months]))  # oldest -> newest


def scan(repo, sha, nose, subdir, mode, threshold):
    if sh(["git", "-C", repo, "checkout", "-q", "--detach", sha]).returncode != 0:
        return None
    target = repo if not subdir else f"{repo}/{subdir}"
    cmd = [nose, "scan", target, "--mode", mode, "--format", "json", "--top", "0"]
    if "near" in mode:
        cmd += ["--threshold", str(threshold)]
    r = sh(cmd)
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        return None


FEAT_KEYS = ("mean_sem", "members", "modules", "files", "languages", "mean_score",
             "mean_lines", "shared_weight", "params", "scope", "value", "dup_lines",
             "shared_lines")


def fam_key(members):
    """Stable cross-revision identity: hash of the sorted (file, name) member set."""
    sig = "\x00".join(sorted(f"{f}\x01{n}" for (f, n, _s, _e) in members))
    h = 1469598103934665603
    for b in sig.encode():
        h = ((h ^ b) * 1099511628211) & 0xFFFFFFFFFFFFFFFF
    return f"{h:016x}"


def families(jdoc, repo_abs):
    """Named semantic/near families (>=2 named members) with per-member spans + features.

    nose emits absolute file paths; normalize to repo-relative so they match git diff."""
    prefix = repo_abs.rstrip("/") + "/"

    def rel(p):
        return p[len(prefix):] if p.startswith(prefix) else p

    out = []
    for f in jdoc.get("families", []):
        locs = f["locations"]
        if any(not l.get("name") for l in locs) or len(locs) < 2:
            continue
        members = [(rel(l["file"]), l["name"], l["start_line"], l["end_line"]) for l in locs]
        out.append({"members": members, "key": fam_key(members),
                    "feats": {k: f[k] for k in FEAT_KEYS}})
    return out


HUNK = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+")


def changed_ranges(repo, sha_a, sha_b):
    """One whole-repo diff a..b -> {old_path: [(lo,hi), ...]} of changed old-side line ranges."""
    r = sh(["git", "-C", repo, "diff", "--unified=0", "--no-color", sha_a, sha_b])
    out = {}
    cur = None
    for line in r.stdout.splitlines():
        if line.startswith("--- "):
            p = line[4:]
            cur = None if p == "/dev/null" else (p[2:] if p.startswith("a/") else p)
        elif line.startswith("@@") and cur is not None:
            m = HUNK.match(line)
            if not m:
                continue
            a = int(m.group(1))
            n = int(m.group(2)) if m.group(2) is not None else 1
            lo, hi = (a, a) if n == 0 else (a, a + n - 1)
            out.setdefault(cur, []).append((lo, hi))
    return out


def span_changed(ranges, file, start, end):
    """True if any changed range in `file` overlaps [start, end]."""
    for lo, hi in ranges.get(file, ()):  # noqa: E741
        if not (hi < start or lo > end):
            return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--nose", required=True)
    ap.add_argument("--branch", default="HEAD")
    ap.add_argument("--subdir", default="")
    ap.add_argument("--mode", default="semantic,near")
    ap.add_argument("--threshold", type=float, default=0.7)
    ap.add_argument("--max-months", type=int, default=60)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    r = sh(["git", "-C", a.repo, "rev-parse", "--abbrev-ref", a.branch])
    branch = r.stdout.strip() if a.branch == "HEAD" and r.returncode == 0 else a.branch
    if sh(["git", "-C", a.repo, "rev-parse", branch]).returncode != 0:
        branch = "HEAD"

    # Stamp the nose version: features (mean_sem, params, ...) and the family set are
    # produced by nose, so the tuning is valid only for this detector version. Labels
    # (from git) are version-independent; re-mining a new nose version refreshes only
    # the features/families. See docs/hazard-benchmark.md "Versioning and refresh".
    nose_ver = sh([a.nose, "--version"]).stdout.strip() or "unknown"
    commits = monthly_commits(a.repo, branch, a.max_months)
    repo_name = a.repo.rstrip("/").split("/")[-1]
    print(f"[mine] {repo_name}: {len(commits)} monthly snapshots "
          f"{commits[0][1][:7]}..{commits[-1][1][:7]}", file=sys.stderr)

    counts = {"G1": 0, "G0c": 0, "G0s": 0}
    with open(a.out, "w") as fout:
        prev = None
        for sha, iso in commits:
            jdoc = scan(a.repo, sha, a.nose, a.subdir, a.mode, a.threshold)
            fams = families(jdoc, os.path.abspath(a.repo)) if jdoc else []
            print(f"[mine] {iso[:10]} {sha[:10]}: {len(fams)} named families", file=sys.stderr)
            if prev is not None:
                psha, pfams = prev
                ranges = changed_ranges(a.repo, psha, sha)  # one diff for the whole interval
                for fam in pfams:
                    flags = [span_changed(ranges, f, s, e)
                             for (f, n, s, e) in fam["members"]]
                    k = sum(flags)
                    nmem = len(flags)
                    if k == 0:
                        label = "G0s"
                    elif k == nmem:
                        label = "G0c"
                    else:
                        label = "G1"
                    counts[label] += 1
                    fout.write(json.dumps({
                        "repo": repo_name, "fam_key": fam["key"], "nose_ver": nose_ver,
                        "from": psha, "to": sha, "date": iso[:10],
                        "label": label, "k_changed": k, "n_members": nmem,
                        "feats": fam["feats"],
                    }) + "\n")
            prev = (sha, fams)
    print(f"[mine] DONE {repo_name}: G1={counts['G1']} G0c={counts['G0c']} "
          f"G0s={counts['G0s']} -> {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
