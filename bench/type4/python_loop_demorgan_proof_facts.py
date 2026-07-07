#!/usr/bin/env python3
"""Machine-check controlled Python loop/all proof-fact evidence.

The README-facing Python loop plus De Morgan packet is intentionally closed
until reusable proof facts exist. This tool supplies a small, source-level
controlled model for two facts:

* iterator identity: an ``all(...)`` generator and an explicit ``for`` loop
  consume the same local iterable binding;
* effect safety: the supported predicate/loop fragment has no calls,
  assignments, mutation, logging, ``yield``, ``await``, or other observable
  effects before the short-circuit decision.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

SCHEMA_VERSION = 1
TOOL_VERSION = "python-loop-demorgan-proof-facts/1"
FACT_ID_ITERATOR_IDENTITY = "python-loop-demorgan.iterator-identity"
FACT_ID_EFFECT_SAFETY = "python-loop-demorgan.effect-safety"
DEFAULT_JSON_OUT = HERE / "python_loop_demorgan_proof_facts.v1.json"

ITERATOR_OBSERVATIONS = {"same-iterator", "different-iterator", "unsupported-iterator"}
EFFECT_OBSERVATIONS = {
    "effect-safe",
    "effectful",
    "unsupported-effect-safety",
}
OBSERVATIONS = ITERATOR_OBSERVATIONS | EFFECT_OBSERVATIONS
MUTATING_METHODS = {
    "add",
    "append",
    "clear",
    "discard",
    "extend",
    "insert",
    "pop",
    "remove",
    "reverse",
    "sort",
    "update",
}

CONTROLLED_EVIDENCE = [
    {
        "check": "iterator-identity",
        "evidence_id": "python-loop-demorgan-iterator-positive-same-binding",
        "fact_id": FACT_ID_ITERATOR_IDENTITY,
        "case_id": "python_loop_demorgan_all_readme",
        "expectation_id": "python_loop_demorgan_positive_currently_split",
        "fixture": "bench/type4/adversarial/cases/python_loop_demorgan/positive.py",
        "all_function": "all_not_zero_or_one",
        "loop_function": "loop_no_zero_or_one",
        "expect": "same-iterator",
    },
    {
        "check": "iterator-identity",
        "evidence_id": "python-loop-demorgan-iterator-negative-different-binding",
        "fact_id": FACT_ID_ITERATOR_IDENTITY,
        "case_id": "python_loop_demorgan_iterator_identity_boundary",
        "expectation_id": "python_loop_demorgan_iterator_identity_stays_split",
        "fixture": "bench/type4/adversarial/cases/python_loop_demorgan/iterator_identity.py",
        "all_function": "all_not_zero_or_one",
        "loop_function": "loop_different_iterable",
        "expect": "different-iterator",
    },
    {
        "check": "effect-safety",
        "evidence_id": "python-loop-demorgan-effect-positive-pure-local-comparisons",
        "fact_id": FACT_ID_EFFECT_SAFETY,
        "case_id": "python_loop_demorgan_all_readme",
        "expectation_id": "python_loop_demorgan_positive_currently_split",
        "fixture": "bench/type4/adversarial/cases/python_loop_demorgan/positive.py",
        "all_function": "all_not_zero_or_one",
        "loop_function": "loop_no_zero_or_one",
        "expect": "effect-safe",
    },
    {
        "check": "effect-safety",
        "evidence_id": "python-loop-demorgan-effect-negative-observed-loop-effect",
        "fact_id": FACT_ID_EFFECT_SAFETY,
        "case_id": "python_loop_demorgan_side_effect_boundary",
        "expectation_id": "python_loop_demorgan_side_effect_stays_split",
        "fixture": "bench/type4/adversarial/cases/python_loop_demorgan/side_effect.py",
        "all_function": "all_not_zero_or_one",
        "loop_function": "loop_with_observed_effect",
        "expect": "effectful",
    },
    {
        "check": "effect-safety",
        "evidence_id": "python-loop-demorgan-effect-negative-helper-call",
        "fact_id": FACT_ID_EFFECT_SAFETY,
        "case_id": "python_loop_demorgan_helper_call_boundary",
        "expectation_id": "python_loop_demorgan_helper_call_stays_split",
        "fixture": "bench/type4/adversarial/cases/python_loop_demorgan/helper_call.py",
        "all_function": "all_with_helper_call",
        "loop_function": "loop_no_zero_or_one",
        "expect": "effectful",
    },
]


class ProofFactError(RuntimeError):
    pass


@dataclass(frozen=True)
class Binding:
    expr: str
    provenance: str
    supported: bool

    def to_json(self) -> dict[str, Any]:
        return {
            "expr": self.expr,
            "provenance": self.provenance,
            "supported": self.supported,
        }


@dataclass(frozen=True)
class IteratorShape:
    function: str
    kind: str
    iterable: Binding
    element: Binding
    diagnostics: tuple[str, ...]

    def to_json(self) -> dict[str, Any]:
        return {
            "diagnostics": list(self.diagnostics),
            "element": self.element.to_json(),
            "function": self.function,
            "iterable": self.iterable.to_json(),
            "kind": self.kind,
        }


@dataclass(frozen=True)
class EffectShape:
    function: str
    kind: str
    supported: bool
    effect_safe: bool
    diagnostics: tuple[str, ...]
    effects: tuple[str, ...]

    def to_json(self) -> dict[str, Any]:
        return {
            "diagnostics": list(self.diagnostics),
            "effect_safe": self.effect_safe,
            "effects": list(self.effects),
            "function": self.function,
            "kind": self.kind,
            "supported": self.supported,
        }


def repo_rel(path: Path) -> str:
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


def canonical_json(doc: dict[str, Any]) -> str:
    return json.dumps(doc, indent=2, sort_keys=True) + "\n"


def node_text(node: ast.AST) -> str:
    try:
        return ast.unparse(node)
    except Exception:
        return node.__class__.__name__


def dedupe(values: list[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


def effect_label(node: ast.AST) -> str | None:
    if isinstance(node, ast.Call):
        return f"call:{node_text(node.func)}"
    if isinstance(node, ast.Await):
        return "await"
    if isinstance(node, ast.Yield):
        return "yield"
    if isinstance(node, ast.YieldFrom):
        return "yield-from"
    if isinstance(node, ast.NamedExpr):
        return "assignment-expression"
    return None


def expression_effects(node: ast.AST | None) -> tuple[str, ...]:
    if node is None:
        return ()
    effects: list[str] = []
    for child in ast.walk(node):
        label = effect_label(child)
        if label is not None:
            effects.append(label)
    return dedupe(effects)


def statement_effects(stmt: ast.stmt) -> tuple[str, ...]:
    if isinstance(stmt, ast.Assign):
        return ("assignment",)
    if isinstance(stmt, ast.AnnAssign):
        return ("annotated-assignment",)
    if isinstance(stmt, ast.AugAssign):
        return ("augmented-assignment",)
    if isinstance(stmt, ast.Delete):
        return ("delete",)
    if isinstance(stmt, (ast.Import, ast.ImportFrom)):
        return ("import",)
    if isinstance(stmt, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
        return ("local-definition",)
    if isinstance(stmt, ast.Return):
        return expression_effects(stmt.value)
    if isinstance(stmt, ast.Expr):
        return expression_effects(stmt.value)
    if isinstance(stmt, ast.If):
        effects = list(expression_effects(stmt.test))
        for nested in [*stmt.body, *stmt.orelse]:
            effects.extend(statement_effects(nested))
        return dedupe(effects)
    if isinstance(stmt, (ast.For, ast.AsyncFor, ast.While, ast.With, ast.AsyncWith, ast.Try)):
        return (f"control:{stmt.__class__.__name__}",)
    if isinstance(stmt, (ast.Raise, ast.Assert, ast.Global, ast.Nonlocal)):
        return (stmt.__class__.__name__.lower(),)
    return dedupe([label for child in ast.walk(stmt) if (label := effect_label(child))])


def is_literal(node: ast.AST) -> bool:
    if isinstance(node, ast.Constant):
        return True
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub)):
        return isinstance(node.operand, ast.Constant) and isinstance(
            node.operand.value, (int, float, complex)
        )
    return False


def is_local_operand(node: ast.AST, allowed_names: set[str]) -> bool:
    return (
        isinstance(node, ast.Name)
        and node.id in allowed_names
        or is_literal(node)
    )


def is_bool_literal_return(stmt: ast.stmt) -> bool:
    return (
        isinstance(stmt, ast.Return)
        and isinstance(stmt.value, ast.Constant)
        and isinstance(stmt.value.value, bool)
    )


def is_pure_local_comparison(node: ast.AST, allowed_names: set[str]) -> bool:
    if isinstance(node, ast.BoolOp):
        return all(is_pure_local_comparison(value, allowed_names) for value in node.values)
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.Not):
        return is_pure_local_comparison(node.operand, allowed_names)
    if isinstance(node, ast.Compare):
        operands = [node.left, *node.comparators]
        pure_ops = all(
            isinstance(op, (ast.Eq, ast.NotEq, ast.Lt, ast.LtE, ast.Gt, ast.GtE))
            for op in node.ops
        )
        return pure_ops and all(is_local_operand(operand, allowed_names) for operand in operands)
    return False


def non_docstring_body(fn: ast.FunctionDef) -> list[ast.stmt]:
    body = list(fn.body)
    if (
        body
        and isinstance(body[0], ast.Expr)
        and isinstance(body[0].value, ast.Constant)
        and isinstance(body[0].value.value, str)
    ):
        return body[1:]
    return body


def arg_map(fn: ast.FunctionDef) -> dict[str, str]:
    result: dict[str, str] = {}
    for index, arg in enumerate(fn.args.args):
        result[arg.arg] = f"arg[{index}]:{arg.arg}"
    return result


def name_binding(node: ast.AST, args: dict[str, str]) -> Binding:
    if isinstance(node, ast.Name):
        return Binding(
            expr=node.id,
            provenance=args.get(node.id, f"local:{node.id}"),
            supported=True,
        )
    return Binding(
        expr=node_text(node),
        provenance=f"unsupported:{node_text(node)}",
        supported=False,
    )


def target_writes_name(node: ast.AST, name: str) -> bool:
    if isinstance(node, ast.Name):
        return node.id == name
    if isinstance(node, (ast.Tuple, ast.List)):
        return any(target_writes_name(item, name) for item in node.elts)
    if isinstance(node, ast.Subscript):
        return target_writes_name(node.value, name)
    if isinstance(node, ast.Attribute):
        return target_writes_name(node.value, name)
    return False


def function_mutates_name(fn: ast.FunctionDef, name: str) -> bool:
    for node in ast.walk(ast.Module(body=non_docstring_body(fn), type_ignores=[])):
        if isinstance(node, ast.Assign):
            if any(target_writes_name(target, name) for target in node.targets):
                return True
        elif isinstance(node, ast.AnnAssign):
            if target_writes_name(node.target, name):
                return True
        elif isinstance(node, ast.AugAssign):
            if target_writes_name(node.target, name):
                return True
        elif isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            if (
                isinstance(node.func.value, ast.Name)
                and node.func.value.id == name
                and node.func.attr in MUTATING_METHODS
            ):
                return True
    return False


def function_map(tree: ast.Module) -> dict[str, ast.FunctionDef]:
    functions: dict[str, ast.FunctionDef] = {}
    for node in tree.body:
        if isinstance(node, ast.FunctionDef):
            if node.name in functions:
                raise ProofFactError(f"duplicate function in fixture: {node.name}")
            functions[node.name] = node
    return functions


def extract_all_generator_shape(fn: ast.FunctionDef) -> IteratorShape:
    diagnostics: list[str] = []
    args = arg_map(fn)
    body = non_docstring_body(fn)
    if len(body) != 1 or not isinstance(body[0], ast.Return):
        diagnostics.append("all-form must be a single return statement")
        return unsupported_shape(fn, "all-generator", diagnostics)
    value = body[0].value
    if not (
        isinstance(value, ast.Call)
        and isinstance(value.func, ast.Name)
        and value.func.id == "all"
        and len(value.args) == 1
        and not value.keywords
    ):
        diagnostics.append("return expression must be all(<generator>)")
        return unsupported_shape(fn, "all-generator", diagnostics)
    gen = value.args[0]
    if not isinstance(gen, ast.GeneratorExp):
        diagnostics.append("all argument must be a generator expression")
        return unsupported_shape(fn, "all-generator", diagnostics)
    if len(gen.generators) != 1:
        diagnostics.append("generator must have exactly one comprehension")
        return unsupported_shape(fn, "all-generator", diagnostics)
    comp = gen.generators[0]
    if comp.is_async:
        diagnostics.append("async generator comprehension is outside this fact")
        return unsupported_shape(fn, "all-generator", diagnostics)
    if comp.ifs:
        diagnostics.append("filtered generator cardinality is outside this fact")
        return unsupported_shape(fn, "all-generator", diagnostics)
    if not isinstance(comp.target, ast.Name):
        diagnostics.append("generator target must be a simple local name")
        return unsupported_shape(fn, "all-generator", diagnostics)
    iterable = name_binding(comp.iter, args)
    if iterable.supported and function_mutates_name(fn, iterable.expr):
        diagnostics.append("all-form iterable is reassigned or mutated")
        return unsupported_shape(fn, "all-generator", diagnostics)
    return IteratorShape(
        function=fn.name,
        kind="all-generator",
        iterable=iterable,
        element=Binding(comp.target.id, f"element:{comp.target.id}", True),
        diagnostics=tuple(diagnostics),
    )


def extract_loop_shape(fn: ast.FunctionDef) -> IteratorShape:
    diagnostics: list[str] = []
    args = arg_map(fn)
    body = non_docstring_body(fn)
    loops = [stmt for stmt in body if isinstance(stmt, ast.For)]
    if len(loops) != 1:
        diagnostics.append("loop-form must contain exactly one top-level for loop")
        return unsupported_shape(fn, "for-loop", diagnostics)
    loop = loops[0]
    if not isinstance(loop.target, ast.Name):
        diagnostics.append("for-loop target must be a simple local name")
        return unsupported_shape(fn, "for-loop", diagnostics)
    if loop.orelse:
        diagnostics.append("for-loop else blocks are outside this fact")
        return unsupported_shape(fn, "for-loop", diagnostics)
    iterable = name_binding(loop.iter, args)
    if iterable.supported and function_mutates_name(fn, iterable.expr):
        diagnostics.append("loop iterable is reassigned or mutated")
        return unsupported_shape(fn, "for-loop", diagnostics)
    return IteratorShape(
        function=fn.name,
        kind="for-loop",
        iterable=iterable,
        element=Binding(loop.target.id, f"element:{loop.target.id}", True),
        diagnostics=tuple(diagnostics),
    )


def unsupported_shape(
    fn: ast.FunctionDef, kind: str, diagnostics: list[str]
) -> IteratorShape:
    unsupported = Binding("<unsupported>", "unsupported", False)
    return IteratorShape(
        function=fn.name,
        kind=kind,
        iterable=unsupported,
        element=unsupported,
        diagnostics=tuple(diagnostics),
    )


def observation(all_shape: IteratorShape, loop_shape: IteratorShape) -> str:
    if not all_shape.iterable.supported or not loop_shape.iterable.supported:
        return "unsupported-iterator"
    if all_shape.iterable.provenance == loop_shape.iterable.provenance:
        return "same-iterator"
    return "different-iterator"


def extract_all_effect_shape(fn: ast.FunctionDef) -> EffectShape:
    iterator_shape = extract_all_generator_shape(fn)
    diagnostics = list(iterator_shape.diagnostics)
    effects: list[str] = []
    if not iterator_shape.iterable.supported or not iterator_shape.element.supported:
        return EffectShape(
            function=fn.name,
            kind="all-generator-effect",
            supported=False,
            effect_safe=False,
            diagnostics=tuple(diagnostics),
            effects=(),
        )

    body = non_docstring_body(fn)
    gen = body[0].value.args[0]
    comp = gen.generators[0]
    allowed_names = {comp.target.id}
    effects.extend(expression_effects(gen.elt))
    if not is_pure_local_comparison(gen.elt, allowed_names):
        diagnostics.append(
            "all predicate must be local comparisons over the element and literals"
        )
    supported = not diagnostics
    return EffectShape(
        function=fn.name,
        kind="all-generator-effect",
        supported=supported,
        effect_safe=supported and not effects,
        diagnostics=tuple(diagnostics),
        effects=dedupe(effects),
    )


def analyze_loop_effect_statement(
    stmt: ast.stmt, allowed_names: set[str]
) -> tuple[list[str], list[str]]:
    diagnostics: list[str] = []
    effects = list(statement_effects(stmt))
    if isinstance(stmt, ast.If):
        if not is_pure_local_comparison(stmt.test, allowed_names):
            diagnostics.append(
                "loop if test must be local comparisons over the element and literals"
            )
        for nested in [*stmt.body, *stmt.orelse]:
            nested_diagnostics, nested_effects = analyze_loop_effect_statement(
                nested, allowed_names
            )
            diagnostics.extend(nested_diagnostics)
            effects.extend(nested_effects)
    elif isinstance(stmt, ast.Return):
        if not is_bool_literal_return(stmt):
            diagnostics.append("loop returns inside the fact must be literal booleans")
    elif effects:
        pass
    else:
        diagnostics.append(f"loop statement is outside this effect fact: {node_text(stmt)}")
    return diagnostics, effects


def extract_loop_effect_shape(fn: ast.FunctionDef) -> EffectShape:
    iterator_shape = extract_loop_shape(fn)
    diagnostics = list(iterator_shape.diagnostics)
    effects: list[str] = []
    if not iterator_shape.iterable.supported or not iterator_shape.element.supported:
        return EffectShape(
            function=fn.name,
            kind="for-loop-effect",
            supported=False,
            effect_safe=False,
            diagnostics=tuple(diagnostics),
            effects=(),
        )

    body = non_docstring_body(fn)
    loop = next(stmt for stmt in body if isinstance(stmt, ast.For))
    allowed_names = {loop.target.id}
    for stmt in body:
        if stmt is loop:
            for nested in loop.body:
                nested_diagnostics, nested_effects = analyze_loop_effect_statement(
                    nested, allowed_names
                )
                diagnostics.extend(nested_diagnostics)
                effects.extend(nested_effects)
            continue
        if is_bool_literal_return(stmt):
            continue
        effects.extend(statement_effects(stmt))
        diagnostics.append(
            f"top-level statement outside the loop is outside this effect fact: "
            f"{node_text(stmt)}"
        )

    supported = not diagnostics
    return EffectShape(
        function=fn.name,
        kind="for-loop-effect",
        supported=supported,
        effect_safe=supported and not effects,
        diagnostics=tuple(diagnostics),
        effects=dedupe(effects),
    )


def effect_observation(all_shape: EffectShape, loop_shape: EffectShape) -> str:
    if all_shape.effects or loop_shape.effects:
        return "effectful"
    if not all_shape.supported or not loop_shape.supported:
        return "unsupported-effect-safety"
    if all_shape.effect_safe and loop_shape.effect_safe:
        return "effect-safe"
    return "unsupported-effect-safety"


def result_for(entry: dict[str, str]) -> dict[str, Any]:
    fixture = ROOT / entry["fixture"]
    try:
        tree = ast.parse(fixture.read_text(), filename=repo_rel(fixture))
    except FileNotFoundError as exc:
        raise ProofFactError(f"missing fixture: {entry['fixture']}") from exc
    functions = function_map(tree)
    missing = [
        name
        for name in (entry["all_function"], entry["loop_function"])
        if name not in functions
    ]
    if missing:
        raise ProofFactError(
            f"fixture {entry['fixture']} missing functions: {', '.join(missing)}"
        )
    check = entry["check"]
    if check == "iterator-identity":
        all_shape = extract_all_generator_shape(functions[entry["all_function"]])
        loop_shape = extract_loop_shape(functions[entry["loop_function"]])
        observed = observation(all_shape, loop_shape)
    elif check == "effect-safety":
        all_shape = extract_all_effect_shape(functions[entry["all_function"]])
        loop_shape = extract_loop_effect_shape(functions[entry["loop_function"]])
        observed = effect_observation(all_shape, loop_shape)
    else:
        raise ProofFactError(f"unknown proof-fact check for {entry['evidence_id']}: {check}")
    expect = entry["expect"]
    if expect not in OBSERVATIONS:
        raise ProofFactError(f"unknown expectation for {entry['evidence_id']}: {expect}")
    return {
        "case_id": entry["case_id"],
        "check": check,
        "evidence_id": entry["evidence_id"],
        "expect": expect,
        "expectation_id": entry["expectation_id"],
        "fact_id": entry["fact_id"],
        "fixture": entry["fixture"],
        "observed": observed,
        "ok": observed == expect,
        "shapes": {
            "all": all_shape.to_json(),
            "loop": loop_shape.to_json(),
        },
    }


def build_report() -> dict[str, Any]:
    results = [result_for(entry) for entry in CONTROLLED_EVIDENCE]
    failed = [result for result in results if not result["ok"]]
    fixture_paths = sorted({ROOT / entry["fixture"] for entry in CONTROLLED_EVIDENCE})
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "fact_ids": sorted({entry["fact_id"] for entry in CONTROLLED_EVIDENCE}),
        "input_artifacts": {
            "tool": artifact_ref(Path(__file__).resolve()),
            "fixtures": [artifact_ref(path) for path in fixture_paths],
        },
        "evidence_count": len(results),
        "passed": len(results) - len(failed),
        "failed": len(failed),
        "results": results,
    }


def print_human(report: dict[str, Any]) -> None:
    prefix = "ok" if report["failed"] == 0 else "FAIL"
    print(
        f"{prefix}: {report['passed']}/{report['evidence_count']} "
        "Python loop/De Morgan proof-fact evidence checks passed"
    )
    for result in report["results"]:
        status = "ok" if result["ok"] else "FAIL"
        print(
            f"  {status} {result['evidence_id']}: "
            f"expected {result['expect']}, observed {result['observed']}"
        )


def check_report(report: dict[str, Any], json_out: Path) -> None:
    if report["failed"]:
        failed = ", ".join(
            result["evidence_id"] for result in report["results"] if not result["ok"]
        )
        raise ProofFactError(f"proof-fact evidence failed: {failed}")
    expected = canonical_json(report)
    if not json_out.exists() or json_out.read_text() != expected:
        raise ProofFactError(f"proof-fact evidence artifact is stale: {repo_rel(json_out)}")


def selftest() -> None:
    good_tree = ast.parse(
        """
def all_form(xs):
    return all(x != 0 for x in xs)

def loop_form(xs):
    for x in xs:
        if x == 0:
            return False
    return True
"""
    )
    good = function_map(good_tree)
    assert observation(
        extract_all_generator_shape(good["all_form"]),
        extract_loop_shape(good["loop_form"]),
    ) == "same-iterator"

    bad_tree = ast.parse(
        """
def all_form(xs, ys):
    return all(x != 0 for x in xs)

def loop_form(xs, ys):
    for x in ys:
        if x == 0:
            return False
    return True
"""
    )
    bad = function_map(bad_tree)
    assert observation(
        extract_all_generator_shape(bad["all_form"]),
        extract_loop_shape(bad["loop_form"]),
    ) == "different-iterator"

    derived_tree = ast.parse(
        """
def all_form(xs):
    return all(x != 0 for x in list(xs))

def loop_form(xs):
    for x in xs:
        if x == 0:
            return False
    return True
"""
    )
    derived = function_map(derived_tree)
    assert observation(
        extract_all_generator_shape(derived["all_form"]),
        extract_loop_shape(derived["loop_form"]),
    ) == "unsupported-iterator"

    mutated_tree = ast.parse(
        """
def all_form(xs):
    return all(x != 0 for x in xs)

def loop_form(xs):
    for x in xs:
        xs.append(x)
        if x == 0:
            return False
    return True
"""
    )
    mutated = function_map(mutated_tree)
    assert observation(
        extract_all_generator_shape(mutated["all_form"]),
        extract_loop_shape(mutated["loop_form"]),
    ) == "unsupported-iterator"

    assert effect_observation(
        extract_all_effect_shape(good["all_form"]),
        extract_loop_effect_shape(good["loop_form"]),
    ) == "effect-safe"

    effectful_loop_tree = ast.parse(
        """
def all_form(xs, seen):
    return all(x != 0 for x in xs)

def loop_form(xs, seen):
    for x in xs:
        seen.append(x)
        if x == 0:
            return False
    return True
"""
    )
    effectful_loop = function_map(effectful_loop_tree)
    assert effect_observation(
        extract_all_effect_shape(effectful_loop["all_form"]),
        extract_loop_effect_shape(effectful_loop["loop_form"]),
    ) == "effectful"

    helper_call_tree = ast.parse(
        """
def all_form(xs, seen):
    return all(allowed(x, seen) for x in xs)

def loop_form(xs, seen):
    for x in xs:
        if x == 0:
            return False
    return True

def allowed(x, seen):
    seen.append(x)
    return x != 0
"""
    )
    helper_call = function_map(helper_call_tree)
    assert effect_observation(
        extract_all_effect_shape(helper_call["all_form"]),
        extract_loop_effect_shape(helper_call["loop_form"]),
    ) == "effectful"

    unsupported_effect_tree = ast.parse(
        """
def all_form(xs):
    return all(x + 1 != 0 for x in xs)

def loop_form(xs):
    for x in xs:
        if x == 0:
            return False
    return True
"""
    )
    unsupported_effect = function_map(unsupported_effect_tree)
    assert effect_observation(
        extract_all_effect_shape(unsupported_effect["all_form"]),
        extract_loop_effect_shape(unsupported_effect["loop_form"]),
    ) == "unsupported-effect-safety"
    print("selftest OK")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print JSON report")
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_OUT)
    parser.add_argument("--check", action="store_true", help="fail if report artifact is stale")
    parser.add_argument("--selftest", action="store_true", help="run helper self-test")
    args = parser.parse_args()

    try:
        if args.selftest:
            selftest()
            return 0
        report = build_report()
        if args.check:
            check_report(report, args.json_out)
        else:
            args.json_out.write_text(canonical_json(report))
        if args.json:
            print(canonical_json(report), end="")
        else:
            print_human(report)
    except ProofFactError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
