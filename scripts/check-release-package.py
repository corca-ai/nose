#!/usr/bin/env python3
"""Check and exercise a cargo-dist archive on its native build runner."""

import argparse
import hashlib
import io
import json
import platform
import subprocess
import tarfile
import tempfile
from pathlib import Path, PurePosixPath


def unpack(archive: Path, destination: Path) -> Path:
    checksum = archive.with_name(archive.name + ".sha256").read_text().split()
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if len(checksum) != 2 or checksum[0] != digest or checksum[1].lstrip("*") != archive.name:
        raise ValueError("archive checksum or checksum filename mismatch")
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            relative = PurePosixPath(member.name)
            if relative.is_absolute() or ".." in relative.parts or not (member.isfile() or member.isdir()):
                raise ValueError(f"unsupported archive member: {member.name}")
            target = destination.joinpath(*relative.parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as content:
                    target.write_bytes(content.read())
                target.chmod(member.mode & 0o777)
    binaries = list(destination.rglob("nose"))
    if len(binaries) != 1 or not binaries[0].is_file():
        raise ValueError("archive must contain exactly one nose executable")
    return binaries[0]


def native_target() -> str:
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64"}[platform.machine()]
    suffix = {"Darwin": "apple-darwin", "Linux": "unknown-linux-gnu"}[platform.system()]
    return f"{arch}-{suffix}"


def exercise(binary: Path, version: str, root: Path) -> dict:
    def run(*args: str) -> bytes:
        return subprocess.run([str(binary), *args], cwd=root, check=True, capture_output=True).stdout

    if run("--version").decode().strip() != f"nose {version}":
        raise ValueError("packaged binary version differs from release manifest")
    capabilities = json.loads(run("capabilities"))
    if "query" not in capabilities or "schemas" not in capabilities:
        raise ValueError("packaged capabilities are incomplete")
    fixture = root / "fixture"
    fixture.mkdir()
    body = "\n".join(f"    x = x + {n}" for n in range(1, 12))
    for name in ("first", "second"):
        (fixture / f"{name}.py").write_text(f"def {name}(x):\n{body}\n    return x\n")
    args = ("query", "fixture", "all", "top=0", "--mode", "syntax", "--format", "json")
    clean = run(*args)
    result = json.loads(clean)
    if not result.get("families"):
        raise ValueError("packaged query did not find the duplicate fixture")
    for _ in range(2):
        if run(*args, "--cache-dir", str(root / "cache")) != clean:
            raise ValueError("packaged clean/cold/warm query output differs")
    run("query", "fixture", "--save-analysis", "capture.json", "--mode", "syntax")
    saved = json.loads(run("query", "--before", "capture.json", "--after", "capture.json", "--format", "json"))
    if not saved.get("view"):
        raise ValueError("packaged saved-analysis query is missing its view")
    return {"query_sha256": hashlib.sha256(clean).hexdigest(), "families": len(result["families"]),
            "clean_cold_warm_equal": True, "saved_analysis_view": saved["view"]}


def self_test() -> None:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        for name in ("package/nose", "../escape", "/absolute"):
            archive = root / "package.tar.xz"
            with tarfile.open(archive, "w:xz") as target:
                member = tarfile.TarInfo(name)
                member.size = 1
                target.addfile(member, io.BytesIO(b"x"))
            checksum = archive.with_name(archive.name + ".sha256")
            checksum.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
            try:
                unpack(archive, root / "unpacked")
            except ValueError:
                assert name != "package/nose"
            else:
                assert name == "package/nose"
            if name == "package/nose":
                checksum.write_text("0" * 64 + f"  {archive.name}\n")
                try:
                    unpack(archive, root / "bad-checksum")
                except ValueError:
                    pass
                else:
                    raise AssertionError("corrupt archive was accepted")
    print("package smoke self-test passed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--target")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if not all((args.manifest, args.archive, args.target, args.output)):
        parser.error("--manifest, --archive, --target and --output are required")
    if args.target != native_target():
        raise ValueError("package smoke must run on the native target")
    manifest = json.loads(args.manifest.read_text())
    releases = [item for item in manifest["releases"] if item["app_name"] == "nose-cli"]
    if len(releases) != 1 or args.archive.name not in releases[0]["artifacts"]:
        raise ValueError("archive is not part of the selected release")
    version = releases[0]["app_version"]
    with tempfile.TemporaryDirectory(prefix="nose-package-smoke-") as temporary:
        root = Path(temporary)
        binary = unpack(args.archive, root / "unpacked")
        checks = exercise(binary, version, root)
        report = {"status": "passed", "version": version, "target": args.target,
                  "archive_sha256": hashlib.sha256(args.archive.read_bytes()).hexdigest(),
                  "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "checks": checks}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
