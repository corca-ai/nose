#!/usr/bin/env python3
"""Guard against reintroducing the retired nose-cli legacy prelude.

The compatibility prelude has been removed. This gate keeps top-level modules on
explicit owner imports and fails if the prelude file or imports come back.
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path


DEFAULT_ROOT = Path("crates/nose-cli/src")
DEFAULT_MAX_USERS = 0
DEFAULT_MAX_EXPORTS = 0
LEGACY_IMPORT_PREFIX = "use crate::legacy_prelude"
LEGACY_EXPORT_PREFIX = "pub(crate) use "


def legacy_prelude_users(root: Path) -> list[tuple[Path, int]]:
    if not root.exists():
        raise SystemExit(f"legacy prelude scan root does not exist: {root}")

    users: list[tuple[Path, int]] = []
    for path in sorted(root.glob("*.rs")):
        if not path.is_file():
            continue
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            if line.strip().startswith(LEGACY_IMPORT_PREFIX):
                users.append((path, lineno))
    return users


def legacy_prelude_exports(root: Path) -> list[tuple[Path, int]]:
    prelude = root / "legacy_prelude.rs"
    if not prelude.exists():
        return []
    lines = prelude.read_text(encoding="utf-8").splitlines()
    return [
        (prelude, lineno)
        for lineno, line in enumerate(lines, start=1)
        if line.strip().startswith(LEGACY_EXPORT_PREFIX)
    ]


def check(root: Path, max_users: int, max_exports: int, *, quiet: bool = False) -> int:
    users = legacy_prelude_users(root)
    exports = legacy_prelude_exports(root)
    prelude = root / "legacy_prelude.rs"
    failures: list[str] = []
    if len(users) > max_users:
        failures.append(
            f"{len(users)} top-level users exceeds user budget {max_users}"
        )
    if prelude.exists() and max_exports == 0:
        failures.append(f"{prelude} exists after the legacy prelude was retired")
    elif len(exports) > max_exports:
        failures.append(
            f"{len(exports)} prelude exports exceeds export budget {max_exports}"
        )

    if failures:
        if quiet:
            return 1
        print("legacy-prelude gate failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print("legacy-prelude users:", file=sys.stderr)
        for path, lineno in users:
            print(f"  - {path}:{lineno}", file=sys.stderr)
        print("legacy-prelude exports:", file=sys.stderr)
        for path, lineno in exports:
            print(f"  - {path}:{lineno}", file=sys.stderr)
        return 1

    if not quiet:
        print(
            "legacy-prelude guard: "
            f"{len(users)} top-level users / budget {max_users}; "
            f"{len(exports)} exports / budget {max_exports}"
        )
    return 0


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        (root / "nested").mkdir()
        first = root / "first.rs"
        second = root / "second.rs"
        prelude = root / "legacy_prelude.rs"
        nested = root / "nested" / "ignored.rs"
        first.write_text("fn a() {}\n", encoding="utf-8")
        second.write_text("fn b() {}\n", encoding="utf-8")
        nested.write_text(f"{LEGACY_IMPORT_PREFIX}::*;\nfn c() {{}}\n", encoding="utf-8")

        if check(root, 0, 0, quiet=True) != 0:
            print("self-test failed: clean retired prelude rejected", file=sys.stderr)
            return 1

        first.write_text(f"{LEGACY_IMPORT_PREFIX}::*;\nfn a() {{}}\n", encoding="utf-8")
        if check(root, 0, 0, quiet=True) == 0:
            print("self-test failed: top-level import accepted", file=sys.stderr)
            return 1
        first.write_text("fn a() {}\n", encoding="utf-8")

        prelude.write_text("// empty compatibility layer\n", encoding="utf-8")
        if check(root, 0, 0, quiet=True) == 0:
            print("self-test failed: retired prelude file accepted", file=sys.stderr)
            return 1

        prelude.write_text(f"{LEGACY_EXPORT_PREFIX}crate::a;\n", encoding="utf-8")
        if check(root, 0, 1, quiet=True) != 0:
            print("self-test failed: explicit export budget rejected", file=sys.stderr)
            return 1
        prelude.write_text(
            f"{LEGACY_EXPORT_PREFIX}crate::a;\n{LEGACY_EXPORT_PREFIX}crate::b;\n",
            encoding="utf-8",
        )
        if check(root, 0, 1, quiet=True) == 0:
            print("self-test failed: export growth accepted", file=sys.stderr)
            return 1

    print("legacy-prelude guard self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--max-users", type=int, default=DEFAULT_MAX_USERS)
    parser.add_argument("--max-exports", type=int, default=DEFAULT_MAX_EXPORTS)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.max_users < 0:
        raise SystemExit("--max-users must be non-negative")
    if args.max_exports < 0:
        raise SystemExit("--max-exports must be non-negative")
    if args.self_test:
        return self_test()
    return check(args.root, args.max_users, args.max_exports)


if __name__ == "__main__":
    sys.exit(main())
