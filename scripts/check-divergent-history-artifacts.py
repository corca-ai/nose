#!/usr/bin/env python3
"""Validate checked divergent-history evidence artifacts."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
HISTORY_ARTIFACT = ROOT / "bench/divergent_history/issue-687-cli-tests-2026-07-06.v1.json"
PILOT_ARTIFACT = ROOT / "bench/divergent_history/issue-687-maintainer-pilot-2026-07-06.v1.json"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    require(path.exists(), f"missing artifact: {path.relative_to(ROOT)}")
    return json.loads(path.read_text())


def main() -> int:
    subprocess.run(
        [
            sys.executable,
            "scripts/divergent-history-mining.py",
            "--check-artifact",
            HISTORY_ARTIFACT.relative_to(ROOT).as_posix(),
        ],
        cwd=ROOT,
        check=True,
    )

    history = load_json(HISTORY_ARTIFACT)
    pilot = load_json(PILOT_ARTIFACT)
    require(
        pilot.get("schema") == "nose.divergent_gate_pilot.v1",
        "pilot schema mismatch",
    )
    require(
        pilot.get("bounds", {}).get("pr_ci_required") is False,
        "pilot must not be a required PR CI claim",
    )
    require(
        pilot.get("maintainer_disposition", {}).get("default_on_readiness_claim") is False,
        "pilot must not claim default-on readiness",
    )
    require(
        pilot.get("source_policy", {}).get("raw_query_tracked") is False,
        "pilot raw query must stay outside git",
    )
    recorded_history = pilot.get("history_artifact", {})
    require(
        recorded_history.get("sha256") == sha256_file(HISTORY_ARTIFACT),
        "pilot history artifact sha256 is stale",
    )
    require(
        recorded_history.get("summary") == history.get("summary"),
        "pilot history summary is stale",
    )
    raw_query = pilot.get("observe_only_run", {}).get("raw_query", {})
    require(
        raw_query.get("tracked") is False
        and str(raw_query.get("path", "")).startswith("target/"),
        "pilot raw query path must be an untracked target/ artifact",
    )
    print("divergent history checked artifacts OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
