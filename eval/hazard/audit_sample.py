#!/usr/bin/env python3
"""Reconstruct evidence packets for every G2 (gold) event so a judge can audit the label.

For each G2 event we re-scan the `from` snapshot, locate the family by fam_key, find the
changed member and the bug-fix commit that touched its function, and capture the code of
a changed member and a lagging (unchanged) sibling plus the commit message. The output
feeds an LLM-judge precision audit (docs/hazard-benchmark.md, the "human-audited gold
subset" — here an LLM stands in for the human).

Usage: audit_sample.py all-events.jsonl out-evidence.jsonl [--limit N]
"""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mine import sh, scan, families, span_changed, changed_ranges, BUGFIX  # noqa: E402

NOSE = os.environ.get("NOSE", "/Users/ak/prjs/cc/nose/target/release/nose")
WORK = os.environ.get("WORK", "/tmp/hazard-mine")
SUBDIR = {"kafka": "clients", "pandas": "pandas", "grpc": "src", "terraform": "internal"}


def code_at(repo, sha, file, s, e):
    r = sh(["git", "-C", repo, "show", f"{sha}:{file}"])
    if r.returncode != 0:
        return ""
    lines = r.stdout.split("\n")
    return "\n".join(lines[max(s - 1, 0):e])


def bugfix_subject(repo, a, b, file, name):
    r = sh(["git", "-C", repo, "log", "-L", f":{re.escape(name)}:{file}",
            "--no-patch", "--pretty=format:%x01%s", f"{a}..{b}"])
    for line in r.stdout.splitlines():
        if line.startswith("\x01") and BUGFIX.search(line[1:]):
            return line[1:]
    return None


def main():
    events = [json.loads(l) for l in open(sys.argv[1])]
    out_path = sys.argv[2]
    limit = int(sys.argv[sys.argv.index("--limit") + 1]) if "--limit" in sys.argv else None
    g2 = [e for e in events if e.get("g2")]
    if limit:
        g2 = g2[:limit]
    print(f"[audit] reconstructing evidence for {len(g2)} G2 events", file=sys.stderr)

    n_ok = 0
    with open(out_path, "w") as fout:
        for i, e in enumerate(g2):
            repo = e["repo"]
            dir_ = f"{WORK}/{repo}"
            subdir = SUBDIR.get(repo, "")
            jdoc = scan(dir_, e["from"], NOSE, subdir, "semantic,near", 0.7)
            if not jdoc:
                continue
            fams = families(jdoc, os.path.abspath(dir_))
            fam = next((f for f in fams if f["key"] == e["fam_key"]), None)
            if fam is None:
                continue  # family not reproduced at re-scan (rare)
            ranges = changed_ranges(dir_, e["from"], e["to"])
            changed, lagging = [], []
            for (f, n, s, en) in fam["members"]:
                (changed if span_changed(ranges, f, s, en) else lagging).append((f, n, s, en))
            # the bug-fixed changed member (the fix that did not propagate)
            fixed = None
            for (f, n, s, en) in changed:
                subj = bugfix_subject(dir_, e["from"], e["to"], f, n)
                if subj:
                    fixed = (f, n, s, en, subj)
                    break
            if fixed is None or not lagging:
                continue
            ff, fn, fs, fe, subj = fixed
            lf, ln, ls, le = lagging[0]
            fout.write(json.dumps({
                "id": i, "repo": repo, "stratum": e.get("stratum"),
                "n_members": len(fam["members"]), "n_changed": len(changed),
                "bugfix_subject": subj,
                "fixed_member": {"file": ff, "name": fn,
                                 "code": code_at(dir_, e["from"], ff, fs, fe)[:2000]},
                "lagging_member": {"file": lf, "name": ln,
                                   "code": code_at(dir_, e["from"], lf, ls, le)[:2000]},
            }) + "\n")
            n_ok += 1
            if (i + 1) % 20 == 0:
                print(f"[audit] {i+1}/{len(g2)} ({n_ok} packets)", file=sys.stderr)
    print(f"[audit] DONE: {n_ok} evidence packets -> {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
