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
    # CLI consolidation removed the standalone --threshold flag; the near channel now
    # carries its acceptance threshold inline as `near:T`. Rewrite a bare `near` token.
    mode_eff = ",".join(
        f"near:{threshold}" if ch == "near" else ch for ch in mode.split(",")
    )
    cmd = [nose, "scan", target, "--mode", mode_eff, "--format", "json", "--top", "0"]
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


def code_at(repo, sha, file, s, e, pad=0):
    r = sh(["git", "-C", repo, "show", f"{sha}:{file}"])
    if r.returncode != 0:
        return ""
    lines = r.stdout.split("\n")
    return "\n".join(lines[max(s - 1 - pad, 0):e + pad])


def member_diff(repo, a, b, file, s, e, pad=8):
    """Unified diff of `file` (a..b) restricted to hunks near the member span [s,e]."""
    r = sh(["git", "-C", repo, "diff", f"-U2", "--no-color", a, b, "--", file])
    out, keep = [], False
    for line in r.stdout.splitlines():
        m = HUNK.match(line)
        if m:
            lo = int(m.group(1))
            n = int(m.group(2)) if m.group(2) is not None else 1
            hi = lo + max(n - 1, 0)
            keep = not (hi < s - pad or lo > e + pad)
        if keep:
            out.append(line)
    return "\n".join(out)


BUGFIX = re.compile(r"\b(fix(e[sd])?|bug(fix)?|defect|hotfix|patch|resolve[sd]?)\b", re.I)


def member_commits(repo, a, b, file, name, n=6):
    """Subjects of commits that touched the function `name` in `file` during (a, b]."""
    r = sh(["git", "-C", repo, "log", "-L", f":{re.escape(name)}:{file}",
            "--no-patch", "--pretty=format:%s", f"{a}..{b}"])
    return [l for l in r.stdout.splitlines() if l.strip()][:n]


def member_bugfixed(repo, sha_a, sha_b, file, name):
    """True if a bug-fix-message commit modified the *function* `name` in `file` during
    (sha_a, sha_b]. Function-level (git's -L:funcname) — drift-robust, far tighter than
    file-level attribution. Mockus & Votta message heuristic."""
    r = sh(["git", "-C", repo, "log", "-L", f":{re.escape(name)}:{file}",
            "--no-patch", "--pretty=format:%x01%s", f"{sha_a}..{sha_b}"])
    for line in r.stdout.splitlines():
        if line.startswith("\x01") and BUGFIX.search(line[1:]):
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
    ap.add_argument("--g1-evidence", default="",
                    help="if set, dump rich evidence (member code + diff + messages) per G1")
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

    counts = {"G1": 0, "G0c": 0, "G0s": 0, "G2": 0}
    ev = open(a.g1_evidence, "w") if a.g1_evidence else None
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
                    # G2 = harmful divergence: a changed sibling's *function* was modified by
                    # a bug-fix commit while lagging siblings were not (a fix that did not
                    # propagate). Function-level check — only runs for G1 (a few git calls).
                    g2 = label == "G1" and any(
                        flags[i] and member_bugfixed(
                            a.repo, psha, sha, fam["members"][i][0], fam["members"][i][1])
                        for i in range(nmem))
                    counts[label] += 1
                    if g2:
                        counts["G2"] += 1
                    fout.write(json.dumps({
                        "repo": repo_name, "fam_key": fam["key"], "nose_ver": nose_ver,
                        "from": psha, "to": sha, "date": iso[:10],
                        "label": label, "g2": g2, "k_changed": k, "n_members": nmem,
                        "feats": fam["feats"],
                    }) + "\n")
                    if ev is not None and label == "G1":
                        members = fam["members"]
                        changed = [m for m, fl in zip(members, flags) if fl]
                        lagging = [m for m, fl in zip(members, flags) if not fl]
                        cf, cn, cs, ce = changed[0]
                        lf, ln, ls, le = lagging[0]
                        ev.write(json.dumps({
                            "repo": repo_name, "fam_key": fam["key"],
                            "from": psha, "to": sha, "g2": g2,
                            "n_members": nmem, "n_changed": k,
                            "changed_member": {
                                "file": cf, "name": cn,
                                "code": code_at(a.repo, psha, cf, cs, ce)[:1800],
                                "diff": member_diff(a.repo, psha, sha, cf, cs, ce)[:1800]},
                            "lagging_member": {
                                "file": lf, "name": ln,
                                "code": code_at(a.repo, psha, lf, ls, le)[:1800]},
                            "commit_subjects": member_commits(a.repo, psha, sha, cf, cn),
                            "feats": fam["feats"],
                        }) + "\n")
            prev = (sha, fams)
    if ev is not None:
        ev.close()
    print(f"[mine] DONE {repo_name}: G1={counts['G1']} (G2={counts['G2']}) "
          f"G0c={counts['G0c']} G0s={counts['G0s']} -> {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
