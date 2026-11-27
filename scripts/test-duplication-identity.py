#!/usr/bin/env python3
"""Regression checks for content-bound duplication approval identities."""
from __future__ import annotations

import contextlib
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from duplication_identity import REVIEW_SCHEMA, fingerprint, review_record

SPEC = importlib.util.spec_from_file_location("duplication_check", Path(__file__).with_name("check-duplication-baseline.py"))
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def family() -> dict:
    return {
        "id": "old-navigation-id", "scope": "test", "witness": "subdag", "surface": "default",
        "locations": [
            {"file": f"crates/{name}.rs", "name": "fixture", "start": 10, "end": 30,
             "region_key": digit * 64,
             "region": {"start_byte": 100, "end_byte": 500, "source_digest": "a" * 64},
             "shared_subdag": [15, 20],
             "boundary": {"enclosing_unit": {"file": f"crates/{name}.rs", "name": "Owner", "kind": "Class", "start_line": 1, "end_line": 40, "unit_key": "coordinates:1-40"}}}
            for name, digit in [("first", "1"), ("second", "2")]
        ],
    }


class IdentityTests(unittest.TestCase):
    def test_unrelated_line_insertions_do_not_change_approval(self) -> None:
        before = family()
        after = copy.deepcopy(before)
        after["id"] = "new-navigation-id"
        for loc in after["locations"]:
            loc["start"] += 12
            loc["end"] += 12
            loc["shared_subdag"] = [n + 12 for n in loc["shared_subdag"]]
            loc["region"].update(start_byte=300, end_byte=700, source_digest="b" * 64)
            loc["boundary"]["enclosing_unit"].update(start_line=13, end_line=52, unit_key="coordinates:13-52")
        self.assertEqual(review_record(before)["fingerprint"], review_record(after)["fingerprint"])

    def test_body_edit_with_identical_coordinates_requires_review(self) -> None:
        before = family()
        after = copy.deepcopy(before)
        after["locations"][0]["region_key"] = "3" * 64
        self.assertEqual(before["id"], after["id"])
        self.assertNotEqual(review_record(before)["fingerprint"], review_record(after)["fingerprint"])

    def test_member_permutation_is_irrelevant_but_multiplicity_is_not(self) -> None:
        before = family()
        reordered = copy.deepcopy(before)
        reordered["locations"].reverse()
        self.assertEqual(review_record(before)["fingerprint"], review_record(reordered)["fingerprint"])
        reordered["locations"].append(copy.deepcopy(reordered["locations"][0]))
        self.assertNotEqual(review_record(before)["fingerprint"], review_record(reordered)["fingerprint"])

    def test_ownership_scope_witness_and_selected_subdag_remain_bound(self) -> None:
        before = family()
        cases = []
        for key, value in [("file", "crates/moved.rs"), ("name", "other_symbol")]:
            changed = copy.deepcopy(before)
            changed["locations"][0][key] = value
            cases.append(changed)
        changed = copy.deepcopy(before)
        changed["locations"][0]["boundary"]["enclosing_unit"]["name"] = "OtherOwner"
        cases.append(changed)
        changed = copy.deepcopy(before)
        changed["locations"][0]["shared_subdag"][1] += 1
        cases.append(changed)
        for key, value in [("scope", "prod"), ("witness", "exact")]:
            changed = copy.deepcopy(before)
            changed[key] = value
            cases.append(changed)
        for changed in cases:
            with self.subTest(changed=changed):
                self.assertNotEqual(review_record(before)["fingerprint"], review_record(changed)["fingerprint"])

    def test_missing_content_provenance_and_ambiguity_fail_closed(self) -> None:
        changed = family()
        del changed["locations"][0]["region_key"]
        with self.assertRaisesRegex(ValueError, "content key"):
            review_record(changed)
        a = family()
        b = copy.deepcopy(a)
        b["id"] = "different-live-id"
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            CHECK.current_family_keys([a, b], {"schema_version": "0.2.0"})


class GateTests(unittest.TestCase):
    def baseline(self, approved: list[dict], **changes) -> Path:
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        data = {"schema_version": "0.2.0", "fingerprint_schema": REVIEW_SCHEMA,
                "accepted_families": [review_record(f) for f in approved], "budget": 20,
                "min_value": 40, "mode": "near", "recommended_surface": "default"}
        data.update(changes)
        path = Path(root.name)/"baseline.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def check(self, baseline: Path, current: list[dict]) -> tuple[int, str]:
        output = io.StringIO()
        with patch.object(CHECK, "current_families", return_value=current), contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
            result = CHECK.check(Path("unused-nose"), baseline)
        return result, output.getvalue()

    def test_live_id_churn_passes_but_new_content_fails(self) -> None:
        before = family()
        baseline = self.baseline([before])
        after = copy.deepcopy(before)
        after["id"] = "new-live-id"
        self.assertEqual(self.check(baseline, [after])[0], 0)
        after["locations"][0]["region_key"] = "4" * 64
        status, message = self.check(baseline, [after])
        self.assertEqual(status, 1)
        self.assertIn("unreviewed current content fingerprints", message)

    def test_missing_approved_relation_still_requires_review(self) -> None:
        status, message = self.check(self.baseline([family()]), [])
        self.assertEqual(status, 1)
        self.assertIn("accepted content fingerprints no longer reported", message)

    def test_budget_failure_is_preserved(self) -> None:
        status, message = self.check(self.baseline([], budget=0), [family()])
        self.assertEqual(status, 1)
        self.assertIn("exceeds budget", message)

    def test_baseline_fingerprint_is_verified_against_its_evidence(self) -> None:
        path = self.baseline([family()])
        value = json.loads(path.read_text())
        value["accepted_families"][0]["members"][0]["region_key"] = "5" * 64
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(SystemExit, "does not match"):
            CHECK.load_baseline(path)

    def test_legacy_baselines_keep_existing_id_semantics(self) -> None:
        before = family()
        path = self.baseline([], schema_version="0.1.0", accepted_family_ids=[before["id"]])
        self.assertEqual(self.check(path, [before])[0], 0)
        before["id"] = "new-live-id"
        self.assertEqual(self.check(path, [before])[0], 1)


@unittest.skipUnless(os.environ.get("NOSE_BIN"), "set NOSE_BIN for the live CLI regression")
class LiveIdentityTests(unittest.TestCase):
    def test_actual_line_movement_and_same_span_body_edit(self) -> None:
        nose = str(Path(os.environ["NOSE_BIN"]).resolve())
        with tempfile.TemporaryDirectory(prefix="nose-duplication-identity-") as directory:
            root = Path(directory)
            source = "def compute(x):\n    a = x * x\n    b = a + 7\n    c = b // 3\n    return c\n"
            for name in ["first", "second"]:
                (root/f"{name}.py").write_text(f"# {name} fixture\n{source}")

            def observed() -> dict:
                output = subprocess.run(
                    [nose, "query", ".", "all", "top=0", "--mode", "near", "--min-size", "1", "--min-lines", "1", "--format", "json"],
                    cwd=root, capture_output=True, text=True, check=True, timeout=30,
                    env=dict(os.environ, RAYON_NUM_THREADS="2"),
                )
                matches = [row for row in json.loads(output.stdout)["families"]
                           if len(row["locations"]) == 2
                           and all(loc.get("name") == "compute" and loc.get("boundary", {}).get("unit_kind") == "Function" for loc in row["locations"])]
                self.assertEqual(len(matches), 1, output.stdout)
                return matches[0]

            before = observed()
            first = root/"first.py"
            first.write_text("# unrelated declaration\n"+first.read_text())
            moved = observed()
            self.assertNotEqual(before["id"], moved["id"])
            self.assertEqual(review_record(before)["fingerprint"], review_record(moved)["fingerprint"])
            for name in ["first", "second"]:
                path = root/f"{name}.py"
                path.write_text(path.read_text().replace("+ 7", "+ 8"))
            edited = observed()
            self.assertEqual(moved["id"], edited["id"])
            self.assertNotEqual(review_record(moved)["fingerprint"], review_record(edited)["fingerprint"])


if __name__ == "__main__":
    unittest.main()
