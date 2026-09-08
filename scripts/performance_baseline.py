"""Verify an explicitly adopted capability baseline before executing its binary."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "bench/release/0.21.0/performance-baseline.v1.json"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(manifest: Path, binary: Path | None = None) -> dict:
    """Bind the local artifact to its adopted source, platform and sealed evidence."""
    data = json.loads(manifest.read_text())
    if data.get("schema") != "nose.capability_performance_baseline/v1":
        raise ValueError("unsupported capability performance baseline schema")
    source = data["source_sha"]
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("baseline source must be a full Git commit")
    artifact = data["artifact"]
    if (artifact["system"], artifact["machine"]) != (platform.system(), platform.machine()):
        raise ValueError("baseline artifact is not registered for this host")
    binary = (binary or ROOT / artifact["path"]).resolve()
    if digest(binary) != artifact["sha256"]:
        raise ValueError("baseline binary checksum mismatch")
    for path, expected in data["product_objects"].items():
        actual = subprocess.check_output(
            ["git", "rev-parse", f"{source}:{path}"], cwd=ROOT, text=True
        ).strip()
        if actual != expected:
            raise ValueError(f"baseline source object mismatch: {path}")
    for record in [*data["evidence"], data["compatibility_baseline"]]:
        if digest(ROOT / record["path"]) != record["sha256"]:
            raise ValueError(f"baseline evidence checksum mismatch: {record['path']}")
    return {
        "id": data["id"], "manifest": str(manifest.resolve()),
        "manifest_sha256": digest(manifest), "source_sha": source,
        "binary": str(binary), "binary_sha256": artifact["sha256"],
        "kind": artifact["kind"], "target": artifact["target"],
    }


def apply(args) -> dict | None:
    if args.performance_baseline_manifest is None:
        args.baseline_source_ref = args.baseline_source_ref or "origin/main"
        return None
    receipt = verify(args.performance_baseline_manifest, args.baseline_binary)
    for supplied in (args.baseline_source_sha, args.baseline_source_ref):
        if supplied is not None and supplied != receipt["source_sha"]:
            raise ValueError("explicit baseline source conflicts with adopted manifest")
    args.baseline_binary = Path(receipt["binary"])
    args.baseline_source_ref = args.baseline_source_sha = receipt["source_sha"]
    return receipt


def run_self_test() -> None:
    # Exercise rejection before measurement without needing the local release binary.
    from argparse import Namespace
    data = json.loads(DEFAULT_MANIFEST.read_text())
    # A shallow CI checkout need not contain the adopted historical commit.
    data["source_sha"] = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    data["product_objects"] = {
        path: subprocess.check_output(
            ["git", "rev-parse", f"HEAD:{path}"], cwd=ROOT, text=True
        ).strip()
        for path in data["product_objects"]
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        binary = root / "nose"
        binary.write_bytes(b"baseline fixture, never executed")
        data["artifact"].update(system=platform.system(), machine=platform.machine(), sha256=digest(binary))
        manifest = root / "baseline.json"
        manifest.write_text(json.dumps(data))
        receipt = verify(manifest, binary)
        assert receipt["source_sha"] == data["source_sha"]
        args = Namespace(performance_baseline_manifest=manifest, baseline_binary=binary,
                         baseline_source_ref=None, baseline_source_sha="0" * 40)
        try:
            apply(args)
        except ValueError as error:
            assert "conflicts" in str(error)
        else:
            raise AssertionError("conflicting source accepted")
        for change, message in (
            (lambda: binary.write_bytes(b"different executable"), "binary checksum"),
            (lambda: data["product_objects"].update(crates="0" * 40), "source object"),
            (lambda: data["evidence"][0].update(sha256="0" * 64), "evidence checksum"),
            (lambda: data["artifact"].update(machine="unregistered"), "host"),
        ):
            original = json.loads(json.dumps(data))
            binary.write_bytes(b"baseline fixture, never executed")
            change()
            manifest.write_text(json.dumps(data))
            try:
                verify(manifest, binary)
            except ValueError as error:
                assert message in str(error), str(error)
            else:
                raise AssertionError(f"accepted {message} mismatch")
            data = original
    print("capability performance baseline self-test passed")
