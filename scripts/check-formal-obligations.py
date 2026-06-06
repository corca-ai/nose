#!/usr/bin/env python3
"""Lint the Lean proof-obligation registry.

The registry is directory-shaped: every obligation lives at
`formal/obligations/<id path>/meta.toml`, and the id must be the dot-joined path.
Proof-sensitive Rust rule modules are also directory-shaped; for example
`crates/nose-normalize/src/value_graph/rules/factor_distribute.rs` must have a
matching `formal/obligations/normalize/value_graph/factor_distribute/meta.toml`.
Other proof-sensitive surfaces are registered below as required obligations so that
IL, fragment-oracle, recursion, and oracle-cutoff contracts cannot silently lose
their Lean evidence.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

try:
    import tomllib  # type: ignore[import-not-found]
except ModuleNotFoundError:  # Python < 3.11 on local developer machines.
    tomllib = None

ROOT = Path(__file__).resolve().parents[1]
OBLIGATION_ROOT = ROOT / "formal" / "obligations"
STATUSES = {
    "proven",
    "covered",
    "missing",
    "empirical-only",
    "rejected-counterexample",
}
RULE_ROOTS = {
    "normalize.value_graph": ROOT / "crates" / "nose-normalize" / "src" / "value_graph" / "rules",
}

RUST_ROOTS = (ROOT / "crates",)
MARKER_RE = re.compile(r"\bproof-obligation:\s*([A-Za-z0-9_.]+)\b")
DECL_RE = re.compile(r"^\s*(?:theorem|lemma|def)\s+([A-Za-z_][A-Za-z0-9_']*)\b")
NAMESPACE_RE = re.compile(r"^\s*namespace\s+([A-Za-z_][A-Za-z0-9_'.]*)\b")
END_RE = re.compile(r"^\s*end(?:\s+[A-Za-z_][A-Za-z0-9_'.]*)?\s*$")
SECTION_RE = re.compile(r"^\[([A-Za-z0-9_.]+)\]$")
ASSIGN_RE = re.compile(r"^([A-Za-z0-9_.]+)\s*=\s*(.+)$")


@dataclass(frozen=True)
class Obligation:
    id: str
    path: Path
    meta: dict[str, Any]


@dataclass(frozen=True)
class RequiredObligation:
    id: str
    rust_files: tuple[str, ...]
    rust_symbols: tuple[str, ...]


REQUIRED_OBLIGATIONS = (
    RequiredObligation(
        "il.arena.validity",
        ("crates/nose-il/src/lib.rs",),
        ("validate", "IlBuilder"),
    ),
    RequiredObligation(
        "il.arena.deep_copy",
        ("crates/nose-detect/src/fragment/oracle.rs",),
        ("copy_subtree", "synthesize_wrapper"),
    ),
    RequiredObligation(
        "normalize.recursion.tail",
        ("crates/nose-normalize/src/recursion/tail.rs",),
        ("recognize", "build_body"),
    ),
    RequiredObligation(
        "normalize.recursion.structural_fold",
        ("crates/nose-normalize/src/recursion/structural_fold.rs",),
        ("recognize", "build_body"),
    ),
    RequiredObligation(
        "detect.fragment.effect_place",
        ("crates/nose-detect/src/fragment/contract.rs",),
        ("Effect", "Place", "writes_proven"),
    ),
    RequiredObligation(
        "detect.fragment.free_inputs",
        ("crates/nose-detect/src/fragment/oracle.rs",),
        ("free_input_cids", "collect_bound_cids", "collect_binding_targets"),
    ),
    RequiredObligation(
        "detect.fragment.wrapper_synthesis",
        ("crates/nose-detect/src/fragment/oracle.rs",),
        ("fragment_behavior", "synthesize_wrapper", "run_unit"),
    ),
    RequiredObligation(
        "oracle.cutoff",
        ("crates/nose-normalize/src/lib.rs",),
        ("NormalizeOptions", "oracle", "recursion::run"),
    ),
)


def error(errors: list[str], message: str) -> None:
    errors.append(message)


def non_empty_str(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def as_list(value: Any, field: str, errors: list[str], where: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        error(errors, f"{where}: `{field}` must be a list of strings")
        return []
    return value


def lean_declarations(path: Path) -> set[str]:
    names: set[str] = set()
    namespaces: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if match := NAMESPACE_RE.match(line):
            namespaces.extend(part for part in match.group(1).split(".") if part)
            continue
        if END_RE.match(line):
            if namespaces:
                namespaces.pop()
            continue
        if match := DECL_RE.match(line):
            name = match.group(1)
            names.add(name)
            if namespaces:
                names.add(".".join([*namespaces, name]))
    return names


def obligation_id_for(meta_file: Path) -> str:
    rel = meta_file.parent.relative_to(OBLIGATION_ROOT)
    return ".".join(rel.parts)


def parse_meta(text: str) -> dict[str, Any]:
    if tomllib is not None:
        return tomllib.loads(text)

    root: dict[str, Any] = {}
    current = root
    pending_key: str | None = None
    pending_items: list[str] = []

    def finish_array() -> None:
        nonlocal pending_key, pending_items, current
        if pending_key is not None:
            current[pending_key] = pending_items
            pending_key = None
            pending_items = []

    def parse_scalar(raw: str) -> Any:
        value = raw.strip()
        if value in {"true", "false"}:
            return value == "true"
        if value.startswith('"') and value.endswith('"'):
            return value[1:-1]
        raise ValueError(f"unsupported TOML value: {value}")

    def parse_array_line(raw: str) -> list[str]:
        value = raw.strip()
        if value == "[]":
            return []
        if value.startswith("[") and value.endswith("]"):
            inner = value[1:-1].strip()
            if not inner:
                return []
            return [parse_scalar(item.strip()) for item in inner.split(",") if item.strip()]
        raise ValueError(f"unsupported TOML array: {value}")

    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if pending_key is not None:
            if line == "]":
                finish_array()
                continue
            item = line.rstrip(",").strip()
            if item:
                pending_items.append(parse_scalar(item))
            continue
        if match := SECTION_RE.match(line):
            finish_array()
            current = root
            for part in match.group(1).split("."):
                current = current.setdefault(part, {})
            continue
        match = ASSIGN_RE.match(line)
        if not match:
            raise ValueError(f"unsupported TOML line: {raw_line}")
        key, raw_value = match.group(1), match.group(2).strip()
        if raw_value == "[":
            pending_key = key
            pending_items = []
        elif raw_value.startswith("["):
            current[key] = parse_array_line(raw_value)
        else:
            current[key] = parse_scalar(raw_value)
    finish_array()
    return root


def load_obligations(errors: list[str]) -> dict[str, Obligation]:
    obligations: dict[str, Obligation] = {}
    for meta_file in sorted(OBLIGATION_ROOT.rglob("meta.toml")):
        where = str(meta_file.relative_to(ROOT))
        try:
            meta = parse_meta(meta_file.read_text(encoding="utf-8"))
        except Exception as exc:
            error(errors, f"{where}: invalid TOML: {exc}")
            continue
        obligation_id = meta.get("id")
        expected_id = obligation_id_for(meta_file)
        if not isinstance(obligation_id, str):
            error(errors, f"{where}: `id` must be a string")
            obligation_id = expected_id
        if obligation_id != expected_id:
            error(errors, f"{where}: `id` must match directory path `{expected_id}`")
        if obligation_id in obligations:
            error(errors, f"{where}: duplicate obligation id `{obligation_id}`")
        obligations[obligation_id] = Obligation(obligation_id, meta_file.parent, meta)
    return obligations


def collect_rust_markers() -> dict[str, set[str]]:
    markers: dict[str, set[str]] = {}
    for root in RUST_ROOTS:
        if not root.exists():
            continue
        for rust_file in sorted(root.rglob("*.rs")):
            rel = str(rust_file.relative_to(ROOT))
            for match in MARKER_RE.finditer(rust_file.read_text(encoding="utf-8")):
                markers.setdefault(match.group(1), set()).add(rel)
    return markers


def lint_meta(
    obligation: Obligation,
    all_ids: set[str],
    errors: list[str],
    rust_markers: dict[str, set[str]] | None = None,
) -> None:
    rel = obligation.path.relative_to(ROOT)
    where = str(rel / "meta.toml")
    meta = obligation.meta

    for field in ("status", "kind", "summary"):
        if not isinstance(meta.get(field), str) or not meta[field].strip():
            error(errors, f"{where}: `{field}` must be a non-empty string")

    status = meta.get("status")
    if isinstance(status, str) and status not in STATUSES:
        error(errors, f"{where}: unknown status `{status}`")

    rust = meta.get("rust", {})
    if not isinstance(rust, dict):
        error(errors, f"{where}: `[rust]` must be a table")
        rust = {}
    rust_files = as_list(rust.get("files", []), "rust.files", errors, where)
    for rust_file in rust_files:
        path = ROOT / rust_file
        if not path.exists():
            error(errors, f"{where}: rust file does not exist: {rust_file}")
    symbols = as_list(rust.get("symbols", []), "rust.symbols", errors, where)
    if rust_files and symbols:
        contents = "\n".join((ROOT / rust_file).read_text(encoding="utf-8") for rust_file in rust_files if (ROOT / rust_file).exists())
        for symbol in symbols:
            if re.search(rf"\b{re.escape(symbol)}\b", contents) is None:
                error(errors, f"{where}: rust symbol `{symbol}` was not found in listed files")
    markers = as_list(rust.get("markers", []), "rust.markers", errors, where)
    if rust_files and not markers:
        error(errors, f"{where}: rust-backed obligations must list at least one `rust.markers` entry")
    if rust_files and rust_markers is not None:
        for marker in markers:
            if marker != obligation.id:
                error(errors, f"{where}: rust marker `{marker}` must match obligation id `{obligation.id}`")
            found_files = rust_markers.get(marker, set())
            if not any(rust_file in found_files for rust_file in rust_files):
                error(errors, f"{where}: rust marker `{marker}` was not found in listed rust files")

    if rust.get("rule_module", False) is True:
        matching_prefix = next((prefix for prefix in RULE_ROOTS if obligation.id.startswith(prefix + ".")), None)
        if matching_prefix is None:
            error(errors, f"{where}: `rust.rule_module = true` has no configured rule root")
        else:
            rule_name = obligation.id.removeprefix(matching_prefix + ".")
            expected = RULE_ROOTS[matching_prefix] / f"{rule_name}.rs"
            if not expected.exists():
                error(errors, f"{where}: missing rule module `{expected.relative_to(ROOT)}`")
            elif str(expected.relative_to(ROOT)) not in rust_files:
                error(errors, f"{where}: `rust.files` must include `{expected.relative_to(ROOT)}`")

    lean = meta.get("lean", {})
    if not isinstance(lean, dict):
        error(errors, f"{where}: `[lean]` must be a table")
        lean = {}
    proof = lean.get("proof")
    theorems = as_list(lean.get("theorems", []), "lean.theorems", errors, where)
    if status == "proven":
        if not non_empty_str(proof):
            error(errors, f"{where}: proven obligations must name `lean.proof`")
        if not theorems:
            error(errors, f"{where}: proven obligations must list at least one `lean.theorems`")
    if proof is not None and not non_empty_str(proof):
        error(errors, f"{where}: `lean.proof` must be a non-empty string")
    if non_empty_str(proof):
        proof_path = obligation.path / proof
        if not proof_path.exists():
            error(errors, f"{where}: proof file does not exist: {proof}")
        else:
            decls = lean_declarations(proof_path)
            for theorem in theorems:
                if theorem not in decls:
                    error(errors, f"{where}: theorem `{theorem}` not found in {proof}")

    counterexamples = lean.get("counterexamples")
    counterexample_theorems = as_list(
        lean.get("counterexample_theorems", []),
        "lean.counterexample_theorems",
        errors,
        where,
    )
    if status == "rejected-counterexample":
        if not non_empty_str(counterexamples):
            error(
                errors,
                f"{where}: rejected-counterexample obligations must name `lean.counterexamples`",
            )
        if not counterexample_theorems:
            error(
                errors,
                f"{where}: rejected-counterexample obligations must list at least one "
                "`lean.counterexample_theorems`",
            )
    if counterexamples is not None and not non_empty_str(counterexamples):
        error(errors, f"{where}: `lean.counterexamples` must be a non-empty string")
    if non_empty_str(counterexamples):
        counterexample_path = obligation.path / counterexamples
        if not counterexample_path.exists():
            error(errors, f"{where}: counterexample file does not exist: {counterexamples}")
        else:
            decls = lean_declarations(counterexample_path)
            for theorem in counterexample_theorems:
                if theorem not in decls:
                    error(errors, f"{where}: counterexample theorem `{theorem}` not found in {counterexamples}")
        if not counterexample_theorems:
            error(errors, f"{where}: listed `lean.counterexamples` must name counterexample theorems")

    covered_by = as_list(meta.get("covered_by", []), "covered_by", errors, where)
    if status == "covered" and not covered_by:
        error(errors, f"{where}: covered obligations must list at least one `covered_by`")
    for covered_id in covered_by:
        if covered_id not in all_ids:
            error(errors, f"{where}: covered_by references unknown obligation `{covered_id}`")


def lint_rule_modules(obligations: dict[str, Obligation], errors: list[str]) -> None:
    for prefix, root in RULE_ROOTS.items():
        if not root.exists():
            continue
        for rule_file in sorted(root.glob("*.rs")):
            if rule_file.name == "mod.rs":
                continue
            obligation_id = f"{prefix}.{rule_file.stem}"
            if obligation_id not in obligations:
                error(
                    errors,
                    f"{rule_file.relative_to(ROOT)}: missing matching obligation "
                    f"`formal/obligations/{'/'.join(obligation_id.split('.'))}/meta.toml`",
                )
                continue
            rust = obligations[obligation_id].meta.get("rust", {})
            if not isinstance(rust, dict) or rust.get("rule_module") is not True:
                error(
                    errors,
                    f"{rule_file.relative_to(ROOT)}: matching obligation must set `rust.rule_module = true`",
                )


def lint_rust_marker_index(
    obligations: dict[str, Obligation],
    rust_markers: dict[str, set[str]],
    errors: list[str],
) -> None:
    for marker, marker_files in sorted(rust_markers.items()):
        if marker not in obligations:
            for marker_file in sorted(marker_files):
                error(errors, f"{marker_file}: marker references unknown obligation `{marker}`")
            continue
        rust = obligations[marker].meta.get("rust", {})
        rust_files = rust.get("files", []) if isinstance(rust, dict) else []
        if not isinstance(rust_files, list):
            rust_files = []
        for marker_file in sorted(marker_files):
            if marker_file not in rust_files:
                where = str(obligations[marker].path.relative_to(ROOT) / "meta.toml")
                error(errors, f"{where}: marker `{marker}` appears in `{marker_file}` but `rust.files` does not list it")


def lint_required_obligations(
    obligations: dict[str, Obligation],
    errors: list[str],
    required: tuple[RequiredObligation, ...] = REQUIRED_OBLIGATIONS,
) -> None:
    for item in required:
        if item.id not in obligations:
            error(
                errors,
                f"required proof-sensitive surface `{item.id}` is missing "
                f"`formal/obligations/{'/'.join(item.id.split('.'))}/meta.toml`",
            )
            continue

        obligation = obligations[item.id]
        where = str(obligation.path.relative_to(ROOT) / "meta.toml")
        rust = obligation.meta.get("rust", {})
        if not isinstance(rust, dict):
            error(errors, f"{where}: `[rust]` must be a table")
            continue

        rust_files = rust.get("files", [])
        if not isinstance(rust_files, list):
            rust_files = []
        rust_symbols = rust.get("symbols", [])
        if not isinstance(rust_symbols, list):
            rust_symbols = []
        rust_markers = rust.get("markers", [])
        if not isinstance(rust_markers, list):
            rust_markers = []

        for rust_file in item.rust_files:
            if rust_file not in rust_files:
                error(errors, f"{where}: required surface must list rust file `{rust_file}`")
        for symbol in item.rust_symbols:
            if symbol not in rust_symbols:
                error(errors, f"{where}: required surface must list rust symbol `{symbol}`")
        if item.id not in rust_markers:
            error(errors, f"{where}: required surface must list rust marker `{item.id}`")


def lint_lean_layout(errors: list[str]) -> None:
    for lean_file in sorted(OBLIGATION_ROOT.rglob("*.lean")):
        if lean_file.name not in {"Proof.lean", "Counterexamples.lean"}:
            error(errors, f"{lean_file.relative_to(ROOT)}: obligation Lean files must be named Proof.lean or Counterexamples.lean")
        if not (lean_file.parent / "meta.toml").exists():
            error(errors, f"{lean_file.relative_to(ROOT)}: missing sibling meta.toml")


def run_self_tests() -> int:
    target = ROOT / "target"
    target.mkdir(exist_ok=True)
    with TemporaryDirectory(prefix="formal-obligation-self-test-", dir=target) as temp:
        obligation_path = Path(temp) / "formal" / "obligations" / "self_test"
        obligation_path.mkdir(parents=True)
        (obligation_path / "Proof.lean").write_text(
            "namespace SelfTest\n\ntheorem ok : True := by\n  trivial\n\nend SelfTest\n",
            encoding="utf-8",
        )
        (obligation_path / "Counterexamples.lean").write_text(
            "namespace SelfTest\n\ntheorem bad : True := by\n  trivial\n\nend SelfTest\n",
            encoding="utf-8",
        )

        def base_meta(status: str) -> dict[str, Any]:
            return {
                "id": "self_test",
                "status": status,
                "kind": "self-test",
                "summary": "self-test obligation",
            }

        def lint(
            meta: dict[str, Any],
            all_ids: set[str] | None = None,
            rust_markers: dict[str, set[str]] | None = None,
        ) -> list[str]:
            errors: list[str] = []
            lint_meta(
                Obligation("self_test", obligation_path, meta),
                all_ids or {"self_test"},
                errors,
                rust_markers,
            )
            return errors

        def required_lint(obligations: dict[str, Obligation]) -> list[str]:
            errors: list[str] = []
            lint_required_obligations(
                obligations,
                errors,
                (
                    RequiredObligation(
                        "self_test",
                        ("required.rs",),
                        ("required_symbol",),
                    ),
                ),
            )
            return errors

        def marker_index_lint(
            obligations: dict[str, Obligation],
            rust_markers: dict[str, set[str]],
        ) -> list[str]:
            errors: list[str] = []
            lint_rust_marker_index(obligations, rust_markers, errors)
            return errors

        cases = [
            (
                "valid proven",
                {
                    **base_meta("proven"),
                    "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                },
                False,
                "",
            ),
            (
                "proven without theorem list",
                {
                    **base_meta("proven"),
                    "lean": {"proof": "Proof.lean", "theorems": []},
                },
                True,
                "proven obligations must list at least one `lean.theorems`",
            ),
            (
                "covered without covered_by",
                {**base_meta("covered"), "covered_by": []},
                True,
                "covered obligations must list at least one `covered_by`",
            ),
            (
                "rejected counterexample without theorem list",
                {
                    **base_meta("rejected-counterexample"),
                    "lean": {"counterexamples": "Counterexamples.lean", "counterexample_theorems": []},
                },
                True,
                "rejected-counterexample obligations must list at least one "
                "`lean.counterexample_theorems`",
            ),
            (
                "rust-backed obligation without marker list",
                {
                    **base_meta("proven"),
                    "rust": {"files": ["required.rs"], "symbols": []},
                    "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                },
                True,
                "rust-backed obligations must list at least one `rust.markers` entry",
            ),
            (
                "rust-backed obligation with missing marker",
                {
                    **base_meta("proven"),
                    "rust": {
                        "files": ["required.rs"],
                        "symbols": [],
                        "markers": ["self_test"],
                    },
                    "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                },
                True,
                "rust marker `self_test` was not found in listed rust files",
            ),
        ]

        required_cases = [
            (
                "missing required obligation",
                {},
                "required proof-sensitive surface `self_test` is missing",
            ),
            (
                "required obligation missing rust file",
                {
                    "self_test": Obligation(
                        "self_test",
                        obligation_path,
                        {
                            **base_meta("proven"),
                            "rust": {"files": [], "symbols": ["required_symbol"]},
                            "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                        },
                    )
                },
                "required surface must list rust file `required.rs`",
            ),
            (
                "required obligation missing rust symbol",
                {
                    "self_test": Obligation(
                        "self_test",
                        obligation_path,
                        {
                            **base_meta("proven"),
                            "rust": {"files": ["required.rs"], "symbols": []},
                            "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                        },
                    )
                },
                "required surface must list rust symbol `required_symbol`",
            ),
            (
                "required obligation missing rust marker",
                {
                    "self_test": Obligation(
                        "self_test",
                        obligation_path,
                        {
                            **base_meta("proven"),
                            "rust": {
                                "files": ["required.rs"],
                                "symbols": ["required_symbol"],
                                "markers": [],
                            },
                            "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                        },
                    )
                },
                "required surface must list rust marker `self_test`",
            ),
        ]

        marker_index_cases = [
            (
                "orphan rust marker",
                {},
                {"missing.obligation": {"crates/missing.rs"}},
                "crates/missing.rs: marker references unknown obligation `missing.obligation`",
            ),
            (
                "marker file not listed by meta",
                {
                    "self_test": Obligation(
                        "self_test",
                        obligation_path,
                        {
                            **base_meta("proven"),
                            "rust": {
                                "files": ["other.rs"],
                                "symbols": [],
                                "markers": ["self_test"],
                            },
                            "lean": {"proof": "Proof.lean", "theorems": ["SelfTest.ok"]},
                        },
                    )
                },
                {"self_test": {"required.rs"}},
                "marker `self_test` appears in `required.rs` but `rust.files` does not list it",
            ),
        ]

        failures = []
        for name, meta, should_fail, expected in cases:
            rust_marker_index = {"self_test": {"required.rs"}} if "missing marker" not in name else {}
            errors = lint(meta, rust_markers=rust_marker_index)
            joined = "\n".join(errors)
            if should_fail and expected not in joined:
                failures.append(f"{name}: expected `{expected}`, got {errors}")
            elif not should_fail and errors:
                failures.append(f"{name}: expected no errors, got {errors}")
        for name, obligations, expected in required_cases:
            errors = required_lint(obligations)
            joined = "\n".join(errors)
            if expected not in joined:
                failures.append(f"{name}: expected `{expected}`, got {errors}")
        for name, obligations, rust_markers, expected in marker_index_cases:
            errors = marker_index_lint(obligations, rust_markers)
            joined = "\n".join(errors)
            if expected not in joined:
                failures.append(f"{name}: expected `{expected}`, got {errors}")

    if failures:
        print("formal obligation self-test failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    print("formal obligation self-test passed")
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        return run_self_tests()
    if len(sys.argv) > 1:
        print("usage: scripts/check-formal-obligations.py [--self-test]", file=sys.stderr)
        return 2

    errors: list[str] = []
    obligations = load_obligations(errors)
    all_ids = set(obligations)
    rust_markers = collect_rust_markers()
    for obligation in obligations.values():
        lint_meta(obligation, all_ids, errors, rust_markers)
    lint_rule_modules(obligations, errors)
    lint_rust_marker_index(obligations, rust_markers, errors)
    lint_required_obligations(obligations, errors)
    lint_lean_layout(errors)

    if errors:
        print("formal obligation lint failed:", file=sys.stderr)
        for item in errors:
            print(f"  - {item}", file=sys.stderr)
        return 1
    print(f"formal obligation lint passed ({len(obligations)} obligations)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
