#!/usr/bin/env python3
"""Replay selected real-frontier Type-4 evidence with nose query.

The replay manifest is stable planning metadata: it links a target packet to a
real_frontier case and says what today's detector should observe. The status
artifact is checked in so proof-carrying packets can cite an executed perimeter.

When external corpus checkouts are unavailable, live replay reports
``unavailable`` for those entries instead of pass/fail. Metadata checks still
fail stale links, stale checked-in status, and failing available replays.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

SCHEMA_VERSION = 1
TOOL_VERSION = "real-frontier-replay/1"
DEFAULT_MANIFEST = HERE / "real_frontier_replay.v1.json"
DEFAULT_STATUS = HERE / "real_frontier_replay_status.v1.json"
DEFAULT_TARGET_PACKETS = HERE / "frontier_target_packets.v1.json"
DEFAULT_REAL_FRONTIER = HERE / "real_frontier.v1.json"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"

EXPECTED_OBSERVATIONS = {"same-family", "split"}
REPLAY_STATUSES = {"passed", "failed", "unavailable"}


class ReplayError(RuntimeError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise ReplayError(f"missing artifact: {repo_rel(path)}") from exc
    except json.JSONDecodeError as exc:
        raise ReplayError(f"invalid JSON in {repo_rel(path)}: {exc}") from exc


def repo_rel(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def command_path(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def artifact_ref(path: Path) -> dict[str, Any]:
    return {
        "path": repo_rel(path),
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def artifact_refs(
    manifest_path: Path,
    target_packets_path: Path,
    real_frontier_path: Path,
) -> dict[str, Any]:
    return {
        "manifest": artifact_ref(manifest_path),
        "target_packets": artifact_ref(target_packets_path),
        "real_frontier": artifact_ref(real_frontier_path),
    }


def query_families(payload: object) -> list[dict[str, Any]]:
    if isinstance(payload, list):
        return [family for family in payload if isinstance(family, dict)]
    if isinstance(payload, dict) and isinstance(payload.get("families"), list):
        return [family for family in payload["families"] if isinstance(family, dict)]
    raise ValueError("nose query JSON must be a list or an object with a families array")


def default_nose() -> Path:
    if os.environ.get("NOSE_BIN"):
        return Path(os.environ["NOSE_BIN"])
    debug = ROOT / "target" / "debug" / "nose"
    if debug.exists():
        return debug
    return ROOT / "target" / "release" / "nose"


def entries_by_id(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        raise ReplayError("real_frontier_replay.v1.json must contain entries")
    result: dict[str, dict[str, Any]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ReplayError("replay manifest entries must be objects")
        replay_id = entry.get("replay_id")
        if not replay_id:
            raise ReplayError("replay manifest entry missing replay_id")
        if replay_id in result:
            raise ReplayError(f"duplicate replay_id: {replay_id}")
        result[replay_id] = entry
    return result


def packet_by_id(packet_doc: dict[str, Any]) -> dict[str, dict[str, Any]]:
    packets = packet_doc.get("packets")
    if not isinstance(packets, list):
        raise ReplayError("frontier target packets must contain packets")
    result: dict[str, dict[str, Any]] = {}
    for packet in packets:
        packet_id = packet.get("packet_id")
        if not packet_id:
            raise ReplayError("target packet missing packet_id")
        if packet_id in result:
            raise ReplayError(f"duplicate target packet: {packet_id}")
        result[packet_id] = packet
    return result


def real_case_by_id(real_frontier: dict[str, Any]) -> dict[str, dict[str, Any]]:
    items = real_frontier.get("items")
    if not isinstance(items, list):
        raise ReplayError("real_frontier.v1.json must contain items")
    result: dict[str, dict[str, Any]] = {}
    for item in items:
        case_id = item.get("case_id")
        if not case_id:
            raise ReplayError("real frontier item missing case_id")
        if case_id in result:
            raise ReplayError(f"duplicate real frontier case: {case_id}")
        result[case_id] = item
    return result


def validate_manifest(
    manifest: dict[str, Any],
    packet_doc: dict[str, Any],
    real_frontier: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise ReplayError("real frontier replay manifest schema_version must be 1")
    if manifest.get("tool_version") != TOOL_VERSION:
        raise ReplayError(f"real frontier replay manifest tool_version must be {TOOL_VERSION}")
    if packet_doc.get("schema_version") != SCHEMA_VERSION:
        raise ReplayError("frontier target packets schema_version must be 1")
    if real_frontier.get("schema_version") != SCHEMA_VERSION:
        raise ReplayError("real_frontier.v1.json schema_version must be 1")

    entries = entries_by_id(manifest)
    packets = packet_by_id(packet_doc)
    cases = real_case_by_id(real_frontier)
    referenced_by_packet: dict[str, set[str]] = {
        packet_id: set(packet.get("real_frontier_replay_ids") or [])
        for packet_id, packet in packets.items()
    }
    for packet_id, replay_ids in referenced_by_packet.items():
        if not isinstance(packets[packet_id].get("real_frontier_replay_ids"), list):
            raise ReplayError(f"packet {packet_id} real_frontier_replay_ids must be a list")
        for replay_id in replay_ids:
            if replay_id not in entries:
                raise ReplayError(f"packet {packet_id} references unknown replay {replay_id}")

    for replay_id, entry in entries.items():
        packet_id = entry.get("packet_id")
        case_id = entry.get("case_id")
        if packet_id not in packets:
            raise ReplayError(f"replay {replay_id} references unknown packet {packet_id}")
        if case_id not in cases:
            raise ReplayError(f"replay {replay_id} references unknown real frontier case {case_id}")
        packet = packets[packet_id]
        if case_id not in packet.get("evidence_case_ids", []):
            raise ReplayError(
                f"replay {replay_id} case {case_id} is not linked by packet {packet_id}"
            )
        if replay_id not in referenced_by_packet.get(packet_id, set()):
            raise ReplayError(
                f"replay {replay_id} is not listed in packet {packet_id} real_frontier_replay_ids"
            )
        if cases[case_id].get("status") != "real-miss":
            raise ReplayError(f"replay {replay_id} must link a real-miss case")
        if entry.get("expect") not in EXPECTED_OBSERVATIONS:
            raise ReplayError(f"replay {replay_id} has unknown expect: {entry.get('expect')}")
        if not isinstance(entry.get("sources"), list) or not entry["sources"]:
            raise ReplayError(f"replay {replay_id} must list sources")
        if not isinstance(entry.get("members"), list) or len(entry["members"]) < 2:
            raise ReplayError(f"replay {replay_id} must list at least two members")
        query = entry.get("query")
        if not isinstance(query, dict):
            raise ReplayError(f"replay {replay_id} query must be an object")
    return entries


def resolve_source(source: dict[str, Any], repos_root: Path) -> tuple[Path | None, list[str]]:
    kind = source.get("kind")
    if kind == "workspace":
        rel = source.get("path")
        if not isinstance(rel, str) or not rel:
            raise ReplayError("workspace source needs path")
        path = ROOT / rel
        if not path.exists():
            raise ReplayError(f"workspace replay source is stale or missing: {repo_rel(path)}")
        return path, []
    if kind == "repo":
        repo = source.get("repo")
        rel = source.get("path")
        if not isinstance(repo, str) or not repo:
            raise ReplayError("repo source needs repo")
        if not isinstance(rel, str) or not rel:
            raise ReplayError("repo source needs path")
        repo_root = repos_root / repo
        if not repo_root.exists():
            return None, [repo_rel(repo_root)]
        path = repo_root / rel
        if not path.exists():
            raise ReplayError(f"replay source is stale or missing: {repo_rel(path)}")
        return path, []
    raise ReplayError(f"unknown replay source kind: {kind}")


def resolve_sources(entry: dict[str, Any], repos_root: Path) -> tuple[list[Path], list[str]]:
    paths: list[Path] = []
    unavailable: list[str] = []
    for source in entry["sources"]:
        path, missing = resolve_source(source, repos_root)
        if path is None:
            unavailable.extend(missing)
        else:
            paths.append(path)
    return paths, unavailable


def member_path(member: dict[str, Any], repos_root: Path) -> Path:
    kind = member.get("kind")
    if kind == "workspace":
        return ROOT / member["file"]
    if kind == "repo":
        return repos_root / member["repo"] / member["file"]
    raise ReplayError(f"unknown replay member kind: {kind}")


def loc_file(loc: dict[str, Any]) -> Path | None:
    value = loc.get("file")
    if not isinstance(value, str):
        return None
    path = Path(value)
    if not path.is_absolute():
        path = ROOT / path
    return path.resolve()


def loc_bounds(loc: dict[str, Any]) -> tuple[int, int] | None:
    start = loc.get("start_line", loc.get("start"))
    end = loc.get("end_line", loc.get("end"))
    if start is None or end is None:
        return None
    return int(start), int(end)


def overlaps(a: tuple[int, int], b: tuple[int, int]) -> bool:
    return not (a[1] < b[0] or b[1] < a[0])


def member_matches_loc(member: dict[str, Any], loc: dict[str, Any], repos_root: Path) -> bool:
    if loc_file(loc) != member_path(member, repos_root).resolve():
        return False
    if member.get("name") and loc.get("name") != member["name"]:
        return False
    if "start_line" in member or "end_line" in member:
        if "start_line" not in member or "end_line" not in member:
            return False
        bounds = loc_bounds(loc)
        if bounds is None:
            return False
        expected = (int(member["start_line"]), int(member["end_line"]))
        if not overlaps(expected, bounds):
            return False
    return True


def family_matches_members(
    family: dict[str, Any], members: list[dict[str, Any]], repos_root: Path
) -> bool:
    locations = family.get("locations", [])
    if not isinstance(locations, list):
        return False
    return all(
        any(
            isinstance(loc, dict) and member_matches_loc(member, loc, repos_root)
            for loc in locations
        )
        for member in members
    )


def matching_family(
    families: list[dict[str, Any]], members: list[dict[str, Any]], repos_root: Path
) -> dict[str, Any] | None:
    for family in families:
        if family_matches_members(family, members, repos_root):
            return family
    return None


def summarize_family(family: dict[str, Any]) -> dict[str, Any]:
    return {
        "value": family.get("value"),
        "witness": family.get("witness"),
        "locations": [
            {
                "file": loc.get("file"),
                "start": loc.get("start_line", loc.get("start")),
                "end": loc.get("end_line", loc.get("end")),
                "name": loc.get("name"),
            }
            for loc in family.get("locations", [])
            if isinstance(loc, dict)
        ],
    }


def run_query(nose: Path, entry: dict[str, Any], roots: list[Path]) -> list[dict[str, Any]]:
    query = entry.get("query") or {}
    mode = query.get("mode", "semantic")
    min_size = str(query.get("min_size", 1))
    min_lines = str(query.get("min_lines", 1))
    top = str(query.get("top", 0))
    cmd = [str(nose), "query"]
    for root in roots:
        cmd.extend(["-r", command_path(root)])
    cmd.extend(
        [
            "all",
            f"top={top}",
            "--mode",
            mode,
            "--format",
            "json",
            "--min-size",
            min_size,
            "--min-lines",
            min_lines,
        ]
    )
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
    if proc.returncode != 0:
        raise ReplayError(
            f"replay {entry['replay_id']} query failed with {proc.returncode}: "
            f"{proc.stderr.strip() or proc.stdout.strip()}"
        )
    try:
        return query_families(json.loads(proc.stdout or "[]"))
    except (json.JSONDecodeError, ValueError) as exc:
        raise ReplayError(f"replay {entry['replay_id']} produced invalid query JSON") from exc


def query_cache_key(entry: dict[str, Any], roots: list[Path]) -> tuple[str, ...]:
    query = entry.get("query") or {}
    return (
        *(command_path(root) for root in roots),
        f"mode={query.get('mode', 'semantic')}",
        f"min_size={query.get('min_size', 1)}",
        f"min_lines={query.get('min_lines', 1)}",
        f"top={query.get('top', 0)}",
    )


def unavailable_result(entry: dict[str, Any], missing: list[str]) -> dict[str, Any]:
    return {
        "replay_id": entry["replay_id"],
        "packet_id": entry["packet_id"],
        "case_id": entry["case_id"],
        "expect": entry["expect"],
        "status": "unavailable",
        "observed": None,
        "ok": None,
        "availability": {
            "status": "unavailable",
            "missing": sorted(missing),
        },
        "matching_family": None,
    }


def result_for(
    entry: dict[str, Any],
    families: list[dict[str, Any]],
    repos_root: Path,
) -> dict[str, Any]:
    match = matching_family(families, entry["members"], repos_root)
    observed = "same-family" if match is not None else "split"
    status = "passed" if observed == entry["expect"] else "failed"
    return {
        "replay_id": entry["replay_id"],
        "packet_id": entry["packet_id"],
        "case_id": entry["case_id"],
        "expect": entry["expect"],
        "status": status,
        "observed": observed,
        "ok": status == "passed",
        "availability": {"status": "available", "missing": []},
        "matching_family": summarize_family(match) if match is not None else None,
    }


def run_replays(
    nose: Path,
    manifest_path: Path,
    target_packets_path: Path,
    real_frontier_path: Path,
    repos_root: Path,
    only_replay: str | None = None,
    nose_label: str | None = None,
) -> dict[str, Any]:
    manifest = load_json(manifest_path)
    packet_doc = load_json(target_packets_path)
    real_frontier = load_json(real_frontier_path)
    entries = validate_manifest(manifest, packet_doc, real_frontier)
    if only_replay:
        if only_replay not in entries:
            raise ReplayError(f"unknown replay id: {only_replay}")
        selected = [entries[only_replay]]
    else:
        selected = [entries[replay_id] for replay_id in sorted(entries)]

    query_cache: dict[tuple[str, ...], list[dict[str, Any]]] = {}
    results = []
    query_count = 0
    for entry in selected:
        roots, missing = resolve_sources(entry, repos_root)
        if missing:
            results.append(unavailable_result(entry, missing))
            continue
        key = query_cache_key(entry, roots)
        if key not in query_cache:
            query_cache[key] = run_query(nose, entry, roots)
            query_count += 1
        results.append(result_for(entry, query_cache[key], repos_root))

    counts = Counter(result["status"] for result in results)
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "input_artifacts": artifact_refs(
            manifest_path,
            target_packets_path,
            real_frontier_path,
        ),
        "nose_binary": nose_label or str(nose),
        "entry_count": len(results),
        "query_count": query_count,
        "passed": counts.get("passed", 0),
        "failed": counts.get("failed", 0),
        "unavailable": counts.get("unavailable", 0),
        "results": results,
    }


def validate_status_report(
    report: dict[str, Any],
    manifest: dict[str, Any],
    expected_artifacts: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    if report.get("schema_version") != SCHEMA_VERSION:
        raise ReplayError("replay status schema_version must be 1")
    if report.get("tool_version") != TOOL_VERSION:
        raise ReplayError(f"replay status tool_version must be {TOOL_VERSION}")
    if report.get("input_artifacts") != expected_artifacts:
        raise ReplayError("real frontier replay status input_artifacts are stale")
    results = report.get("results")
    if not isinstance(results, list):
        raise ReplayError("replay status must contain results")
    entries = entries_by_id(manifest)
    result_by_id: dict[str, dict[str, Any]] = {}
    for result in results:
        if not isinstance(result, dict):
            raise ReplayError("replay status results must be objects")
        replay_id = result.get("replay_id")
        if replay_id in result_by_id:
            raise ReplayError(f"duplicate replay status result: {replay_id}")
        entry = entries.get(replay_id)
        if entry is None:
            raise ReplayError(f"extra replay status result: {replay_id}")
        result_by_id[replay_id] = result
        for field in ("packet_id", "case_id", "expect"):
            if result.get(field) != entry[field]:
                raise ReplayError(f"replay status result {replay_id} field {field} is stale")
        status = result.get("status")
        if status not in REPLAY_STATUSES:
            raise ReplayError(f"replay status result {replay_id} has unknown status {status}")
        observed = result.get("observed")
        if status == "unavailable":
            if observed is not None or result.get("ok") is not None:
                raise ReplayError(f"unavailable replay {replay_id} must not carry ok/observed")
        else:
            if observed not in EXPECTED_OBSERVATIONS:
                raise ReplayError(f"replay status result {replay_id} has unknown observed")
            expected_ok = status == "passed" and observed == result.get("expect")
            expected_ok = expected_ok or (status == "failed" and observed != result.get("expect"))
            if not expected_ok:
                raise ReplayError(f"replay status result {replay_id} status/observed drifted")
            if bool(result.get("ok")) != (status == "passed"):
                raise ReplayError(f"replay status result {replay_id} ok flag drifted")
    missing = sorted(set(entries) - set(result_by_id))
    if missing:
        raise ReplayError(f"replay status missing results: {missing}")
    counts = Counter(result["status"] for result in results)
    if report.get("entry_count") != len(results):
        raise ReplayError("replay status entry_count is stale")
    if report.get("passed") != counts.get("passed", 0):
        raise ReplayError("replay status passed count is stale")
    if report.get("failed") != counts.get("failed", 0):
        raise ReplayError("replay status failed count is stale")
    if report.get("unavailable") != counts.get("unavailable", 0):
        raise ReplayError("replay status unavailable count is stale")
    if counts.get("failed", 0):
        failed = sorted(r["replay_id"] for r in results if r["status"] == "failed")
        raise ReplayError(f"checked-in replay status contains failing results: {failed}")
    return result_by_id


def compare_available_replays(
    current: dict[str, Any],
    checked: dict[str, dict[str, Any]],
) -> list[str]:
    mismatches = []
    for result in current["results"]:
        replay_id = result["replay_id"]
        if result["status"] == "unavailable":
            continue
        expected = checked[replay_id]
        for field in ("status", "observed", "expect"):
            if result.get(field) != expected.get(field):
                mismatches.append(
                    f"{replay_id} {field}: checked={expected.get(field)!r} "
                    f"current={result.get(field)!r}"
                )
    return mismatches


def print_human(report: dict[str, Any]) -> None:
    failed = report["failed"]
    prefix = "FAIL" if failed else "ok"
    print(
        f"{prefix}: {report['passed']}/{report['entry_count']} real-frontier "
        f"replays passed; {report['unavailable']} unavailable; "
        f"{report['query_count']} query runs"
    )
    for result in report["results"]:
        if result["status"] == "unavailable":
            missing = ", ".join(result["availability"]["missing"])
            print(f"  skip {result['replay_id']}: unavailable ({missing})")
        else:
            status = "ok" if result["status"] == "passed" else "FAIL"
            print(
                f"  {status} {result['replay_id']}: "
                f"expected {result['expect']}, observed {result['observed']}"
            )


def selftest() -> None:
    family = {
        "locations": [
            {"file": str(ROOT / "a.py"), "name": "a", "start": 1, "end": 4},
            {"file": str(ROOT / "b.py"), "name": "b", "start": 10, "end": 12},
        ]
    }
    members = [
        {"kind": "workspace", "file": "a.py", "name": "a"},
        {"kind": "workspace", "file": "b.py", "start_line": 9, "end_line": 11},
    ]
    assert family_matches_members(family, members, ROOT)
    assert not family_matches_members(
        family,
        [
            {"kind": "workspace", "file": "a.py", "name": "a"},
            {"kind": "workspace", "file": "c.py", "name": "c"},
        ],
        ROOT,
    )
    manifest = {
        "schema_version": 1,
        "tool_version": TOOL_VERSION,
        "entries": [
            {
                "replay_id": "r",
                "packet_id": "p",
                "case_id": "c",
                "expect": "split",
                "sources": [{"kind": "workspace", "path": "bench/type4/real_frontier_replay.py"}],
                "members": [
                    {"kind": "workspace", "file": "a.py"},
                    {"kind": "workspace", "file": "b.py"},
                ],
                "query": {"mode": "semantic", "min_size": 1, "min_lines": 1},
            }
        ],
    }
    packet_doc = {
        "schema_version": 1,
        "packets": [
            {
                "packet_id": "p",
                "evidence_case_ids": ["c"],
                "real_frontier_replay_ids": ["r"],
            }
        ],
    }
    real_frontier = {"schema_version": 1, "items": [{"case_id": "c", "status": "real-miss"}]}
    assert list(validate_manifest(manifest, packet_doc, real_frontier)) == ["r"]
    report = {
        "schema_version": 1,
        "tool_version": TOOL_VERSION,
        "input_artifacts": {"manifest": {}, "target_packets": {}, "real_frontier": {}},
        "entry_count": 1,
        "passed": 1,
        "failed": 0,
        "unavailable": 0,
        "results": [
            {
                "replay_id": "r",
                "packet_id": "p",
                "case_id": "c",
                "expect": "split",
                "status": "passed",
                "observed": "split",
                "ok": True,
            }
        ],
    }
    checked = validate_status_report(report, manifest, report["input_artifacts"])
    assert checked["r"]["status"] == "passed"
    stale = json.loads(json.dumps(report))
    stale["results"][0]["expect"] = "same-family"
    try:
        validate_status_report(stale, manifest, report["input_artifacts"])
        raise AssertionError("stale replay status was not detected")
    except ReplayError:
        pass
    print("selftest OK")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--status", type=Path, default=DEFAULT_STATUS)
    parser.add_argument("--target-packets", type=Path, default=DEFAULT_TARGET_PACKETS)
    parser.add_argument("--real-frontier", type=Path, default=DEFAULT_REAL_FRONTIER)
    parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--nose", type=Path, default=default_nose())
    parser.add_argument("--replay", help="run one replay id")
    parser.add_argument("--json", action="store_true", help="print JSON report")
    parser.add_argument("--json-out", type=Path, help="write JSON report")
    parser.add_argument("--stable-report", action="store_true", help="use 'nose' as binary label")
    parser.add_argument("--require-available", action="store_true", help="fail on unavailable replays")
    parser.add_argument("--check", action="store_true", help="fail stale checked-in replay metadata")
    parser.add_argument("--selftest", action="store_true", help="run helper self-test")
    args = parser.parse_args()

    try:
        if args.selftest:
            selftest()
            return 0
        manifest = load_json(args.manifest)
        packet_doc = load_json(args.target_packets)
        real_frontier = load_json(args.real_frontier)
        validate_manifest(manifest, packet_doc, real_frontier)
        expected_artifacts = artifact_refs(args.manifest, args.target_packets, args.real_frontier)

        if not args.nose.exists():
            raise ReplayError(f"nose binary not found: {args.nose}")

        nose_label = "nose" if args.stable_report else None
        report = run_replays(
            args.nose,
            args.manifest,
            args.target_packets,
            args.real_frontier,
            args.repos_root,
            args.replay,
            nose_label,
        )
        if args.require_available and report["unavailable"]:
            unavailable = [
                result["replay_id"]
                for result in report["results"]
                if result["status"] == "unavailable"
            ]
            raise ReplayError(f"required replays unavailable: {unavailable}")
        if report["failed"]:
            failed = [
                result["replay_id"]
                for result in report["results"]
                if result["status"] == "failed"
            ]
            raise ReplayError(f"real-frontier replay failed: {failed}")

        if args.check:
            checked_report = load_json(args.status)
            checked = validate_status_report(checked_report, manifest, expected_artifacts)
            mismatches = compare_available_replays(report, checked)
            if mismatches:
                raise ReplayError(
                    "checked-in replay status is stale for available replays: "
                    + "; ".join(mismatches)
                )
        if args.json_out:
            args.json_out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        if args.json:
            print(json.dumps(report, indent=2, sort_keys=True))
        elif not args.check:
            print_human(report)
    except ReplayError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
