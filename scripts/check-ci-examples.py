#!/usr/bin/env python3
"""Validate copyable CI workflow examples in docs/examples/ci."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EXAMPLE_DIR = ROOT / "docs" / "examples" / "ci"
OBSERVE = EXAMPLE_DIR / "divergent-edit-observe-only.yml"
ENFORCE = EXAMPLE_DIR / "divergent-edit-enforcing.yml"
CI_DOC = ROOT / "docs" / "continuous-integration.md"
CHANGELOG = ROOT / "CHANGELOG.md"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"ci example check failed: {message}")


def read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as err:
        raise SystemExit(f"ci example check failed: read {path}: {err}") from err


def latest_changelog_release() -> str:
    text = read(CHANGELOG)
    match = re.search(
        r"^## \[(\d+\.\d+\.\d+)\] - \d{4}-\d{2}-\d{2}$",
        text,
        re.MULTILINE,
    )
    require(match is not None, f"{CHANGELOG} must contain a dated latest release heading")
    return f"v{match.group(1)}"


def check_nose_version_pin(path: Path, text: str, latest_release: str) -> None:
    matches = re.findall(r"^\s+NOSE_VERSION:\s*(v\d+\.\d+\.\d+)\s*$", text, re.MULTILINE)
    require(len(matches) == 1, f"{path} must pin exactly one concrete NOSE_VERSION")
    require(
        matches[0] == latest_release,
        f"{path} pins NOSE_VERSION {matches[0]}, expected latest release {latest_release}",
    )


def check_workflow(path: Path, *, enforcing: bool) -> None:
    text = read(path)
    require("pull_request_target" not in text, f"{path} must not use pull_request_target")
    for needle in [
        "pull_request:",
        "contents: read",
        "security-events: write",
        "actions: read",
        "actions/checkout@v7",
        "fetch-depth: 0",
        "persist-credentials: false",
        "nose capabilities > nose-capabilities.json",
        "schemas.query_json=8",
        "query.output_formats",
        "base_divergence",
        "query_base_json_v8",
        "query_base_gate_fail_default",
        "query_base_sarif",
        "structured_ignores",
        "query_base_structured_ignores",
        'git rev-parse --verify --quiet "${BASE_REF}^{commit}"',
        '--mode syntax,semantic --format json top=0',
        '--mode syntax,semantic --format sarif top=0',
        "GITHUB_STEP_SUMMARY",
        "gate.fail_default",
        "github/codeql-action/upload-sarif@v4",
        "category: nose-divergent-edit",
    ]:
        require(needle in text, f"{path} missing `{needle}`")
    require("fire_eligible" not in text, f"{path} must not decide from fire_eligible")
    require("summary.strict" not in text, f"{path} must not decide from summary.strict")

    upload_at = text.find("- name: Upload divergent-edit SARIF")
    fail_at = text.find("- name: Fail on strict divergent edits")
    if enforcing:
        require(fail_at != -1, f"{path} missing final fail step")
        require(upload_at != -1 and upload_at < fail_at, f"{path} must upload SARIF before failing")
    else:
        require(fail_at == -1, f"{path} must stay observe-only")


def extract_python_block(workflow: str, step_name: str) -> str:
    marker = f"- name: {step_name}"
    start = workflow.find(marker)
    require(start != -1, f"missing step `{step_name}`")
    heredoc = workflow.find("python3 - <<'PY'", start)
    require(heredoc != -1, f"missing Python heredoc under `{step_name}`")
    code_start = workflow.find("\n", heredoc) + 1
    code_end = workflow.find("\n          PY", code_start)
    require(code_end != -1, f"unterminated Python heredoc under `{step_name}`")
    return textwrap.dedent(workflow[code_start:code_end])


def run_python_block(code: str, data: dict, *, expect_exit: int = 0) -> str:
    with tempfile.TemporaryDirectory(prefix="nose-ci-example-") as tmp:
        tmp_path = Path(tmp)
        (tmp_path / "nose-divergence.json").write_text(json.dumps(data), encoding="utf-8")
        summary = tmp_path / "summary.md"
        env = os.environ.copy()
        env["GITHUB_STEP_SUMMARY"] = str(summary)
        script = tmp_path / "block.py"
        script.write_text(code, encoding="utf-8")
        result = subprocess.run(
            [sys.executable, str(script)],
            cwd=tmp_path,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        require(
            result.returncode == expect_exit,
            f"Python block exited {result.returncode}, expected {expect_exit}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        return summary.read_text(encoding="utf-8") if summary.exists() else ""


def sample_doc(*, fail_default: bool) -> dict:
    return {
        "schema_version": 8,
        "view": "base",
        "summary": {
            "changed_files": 4,
            "divergences": 3,
            "shown_divergences": 3,
        },
        "items": [
            {"tier": "strict", "lane": "base-divergence", "gate": {"fail_default": fail_default}},
            {"tier": "review", "lane": "base-divergence", "gate": {"fail_default": False}},
            {"tier": "report-only", "lane": "new-copy", "gate": {"fail_default": False}},
        ],
    }


def check_summary_block(path: Path) -> None:
    workflow = read(path)
    code = extract_python_block(workflow, "Write divergent-edit summary")
    summary = run_python_block(code, sample_doc(fail_default=True))
    for needle in [
        "changed files: 4",
        "findings: 3",
        "shown findings: 3",
        "default-failing findings: 1",
        "strict: 1",
        "review: 1",
        "report-only: 1",
        "new-copy advisory: 1",
        "gate.fail_default=true",
    ]:
        require(needle in summary, f"{path} summary block missing `{needle}`")


def check_enforcing_block() -> None:
    workflow = read(ENFORCE)
    code = extract_python_block(workflow, "Fail on strict divergent edits")
    _ = run_python_block(code, sample_doc(fail_default=False), expect_exit=0)
    _ = run_python_block(code, sample_doc(fail_default=True), expect_exit=1)


def check_docs_link_examples() -> None:
    text = read(CI_DOC)
    for needle in [
        "docs/examples/ci/divergent-edit-observe-only.yml",
        "docs/examples/ci/divergent-edit-enforcing.yml",
        "pull_request_target",
        "github/codeql-action/upload-sarif@v4",
        "security-events: write",
        "actions: read",
        "gate.fail_default",
        "top=0",
    ]:
        require(needle in text, f"{CI_DOC} missing `{needle}`")


def main() -> None:
    latest_release = latest_changelog_release()
    for path in [OBSERVE, ENFORCE]:
        check_nose_version_pin(path, read(path), latest_release)
    check_workflow(OBSERVE, enforcing=False)
    check_workflow(ENFORCE, enforcing=True)
    check_summary_block(OBSERVE)
    check_summary_block(ENFORCE)
    check_enforcing_block()
    check_docs_link_examples()
    print("validated 2 divergent-edit GitHub Actions example(s)")


if __name__ == "__main__":
    main()
