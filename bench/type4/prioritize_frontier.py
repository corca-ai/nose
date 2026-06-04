#!/usr/bin/env python3
"""Rank Type-4 frontier candidates from the pinned real-repo corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"

SKIP_DIRS = {
    ".git",
    ".hg",
    ".svn",
    ".cache",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    ".yarn",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "vendor",
}
LANG_BY_EXT = {
    ".c": "c",
    ".h": "c",
    ".go": "go",
    ".java": "java",
    ".js": "javascript",
    ".jsx": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".py": "python",
    ".rb": "ruby",
    ".rs": "rust",
    ".ts": "typescript",
    ".tsx": "typescript",
}
@dataclass(frozen=True)
class PatternSpec:
    pattern_id: str
    lang: str
    regex: re.Pattern[str]
    precision: str


@dataclass(frozen=True)
class ProbeSpec:
    probe_id: str
    lang: str
    regex: re.Pattern[str]


@dataclass(frozen=True)
class Candidate:
    candidate_id: str
    title: str
    scope: str
    implementation_cost: int
    soundness_risk: int
    status: str
    why: str
    next_probe: str
    patterns: tuple[PatternSpec, ...]


PRECISION_WEIGHT = {
    "high": 1.0,
    "medium": 0.55,
    "low": 0.15,
}


def pat(pattern_id: str, lang: str, regex: str, precision: str = "high") -> PatternSpec:
    if precision not in PRECISION_WEIGHT:
        raise ValueError(f"unknown pattern precision: {precision}")
    return PatternSpec(pattern_id, lang, re.compile(regex), precision)


def probe(probe_id: str, lang: str, regex: str) -> ProbeSpec:
    return ProbeSpec(probe_id, lang, re.compile(regex))


CANDIDATES = [
    Candidate(
        "collection_empty_check",
        "Collection emptiness and non-emptiness",
        "all-language",
        2,
        2,
        "covered-current",
        "Covered by the current strict frontier; retained so future reports can track real-corpus yield and uncovered broad-probe gaps.",
        "No new ordinary loop; monitor real-corpus deltas and keep boundary cases for nonzero thresholds, wrong receivers, and over-broad collection predicates.",
        (
            pat("c_len_zero", "c", r"\b\w*(?:len|length|count|size)\w*\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*\w*(?:len|length|count|size)\w*", "medium"),
            pat("go_len_zero", "go", r"\blen\s*\([^)]{1,80}\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*len\s*\([^)]{1,80}\)", "high"),
            pat("java_named_empty", "java", r"\.\s*isEmpty\s*\(\s*\)", "high"),
            pat("java_size_zero", "java", r"\.\s*size\s*\(\s*\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*[^;\n]{1,80}\.\s*size\s*\(\s*\)", "high"),
            pat("java_assert_size_zero", "java", r"\bassert(?:Equals|That)\s*\(\s*(?:0\s*,\s*[^;\n]{1,100}\.\s*size\s*\(\s*\)|[^;\n]{1,100}\.\s*size\s*\(\s*\)\s*,\s*0)\b", "medium"),
            pat("js_length_zero", "javascript", r"\.\s*length\s*(?:={2,3}|!==|!=|>|>=)\s*0\b|\b0\s*(?:={2,3}|!==|!=|<|<=)\s*[^;\n]{1,80}\.\s*length\b", "high"),
            pat("js_expect_length_zero", "javascript", r"\bexpect\s*\([^;\n]{1,120}\.\s*length\s*\)\s*\.\s*(?:toBe|toEqual|toStrictEqual)\s*\(\s*0\s*\)", "medium"),
            pat("py_len_zero", "python", r"\blen\s*\([^)]{1,80}\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*len\s*\([^)]{1,80}\)", "high"),
            pat("py_assert_len_zero", "python", r"\bassert(?:Equal|Equals)?\s*\(\s*(?:0\s*,\s*len\s*\([^)]{1,80}\)|len\s*\([^)]{1,80}\)\s*,\s*0)", "medium"),
            pat("py_truthy_collection", "python", r"(?m)^\s*(?:if|elif|while)\s+(?:not\s+)?(?:[A-Za-z_][\w.]*s|[A-Za-z_][\w.]*(?:items|values|tokens|children|entries|lines|args|params|headers|rows|cols|names|files|dirs|results|messages|errors|warnings|listeners|options))\s*:", "low"),
            pat("ruby_empty", "ruby", r"\.\s*empty\?\b", "high"),
            pat("ruby_any", "ruby", r"\.\s*any\?\b", "medium"),
            pat("ruby_length_zero", "ruby", r"\.\s*(?:length|size)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*[^;\n]{1,80}\.\s*(?:length|size)\b", "high"),
            pat("rust_named_empty", "rust", r"\.\s*is_empty\s*\(\s*\)", "high"),
            pat("rust_len_zero", "rust", r"\.\s*len\s*\(\s*\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*[^;\n]{1,80}\.\s*len\s*\(\s*\)", "high"),
            pat("rust_assert_len_zero", "rust", r"\bassert(?:_eq|_ne)?!\s*\(\s*(?:0\s*,\s*[^;\n]{1,100}\.\s*len\s*\(\s*\)|[^;\n]{1,100}\.\s*len\s*\(\s*\)\s*,\s*0)\b", "medium"),
            pat("ts_length_zero", "typescript", r"\.\s*length\s*(?:={2,3}|!==|!=|>|>=)\s*0\b|\b0\s*(?:={2,3}|!==|!=|<|<=)\s*[^;\n]{1,80}\.\s*length\b", "high"),
            pat("ts_expect_length_zero", "typescript", r"\bexpect\s*\([^;\n]{1,120}\.\s*length\s*\)\s*\.\s*(?:toBe|toEqual|toStrictEqual)\s*\(\s*0\s*\)", "medium"),
        ),
    ),
    Candidate(
        "string_prefix_suffix",
        "String prefix/suffix predicates",
        "all-language",
        2,
        2,
        "covered-current",
        "Covered by the current strict frontier; retained so real-corpus yield and future boundary gaps stay visible.",
        "No new ordinary loop; monitor regex, contains, and case-folding boundaries as separate axes.",
        (
            pat("go_strings_prefix_suffix", "go", r"\bstrings\.(?:HasPrefix|HasSuffix)\s*\("),
            pat("java_prefix_suffix", "java", r"\.\s*(?:startsWith|endsWith)\s*\("),
            pat("js_prefix_suffix", "javascript", r"\.\s*(?:startsWith|endsWith)\s*\("),
            pat("py_prefix_suffix", "python", r"\.\s*(?:startswith|endswith)\s*\("),
            pat("ruby_prefix_suffix", "ruby", r"\.\s*(?:start_with\?|end_with\?)\s*\("),
            pat("rust_prefix_suffix", "rust", r"\.\s*(?:starts_with|ends_with)\s*\("),
            pat("ts_prefix_suffix", "typescript", r"\.\s*(?:startsWith|endsWith)\s*\("),
        ),
    ),
    Candidate(
        "membership_contains",
        "Membership and contains predicates",
        "multi-language",
        3,
        3,
        "partially-covered",
        "Static literal collection membership, typed dynamic receiver membership including Python `tuple[T, ...]`, Java `Queue<T>`, and Rust `VecDeque<T>` type facts, Python builtin `set`/`tuple`/`frozenset` factories, function-local Go slice / Java `List.of` / Rust `vec!` constructed bindings, Rust std `HashSet::from`/`BTreeSet::from`/`VecDeque::from` constructed bindings, proven Set construction, static JS/TS array `.some(...)` existential, `.every(...)` absence, `.indexOf(...)` membership comparisons, `.findIndex(...)` lambda membership comparisons, and `.filter(...).length` nonempty membership / zero-count absence checks, Java literal collection factories, same-file module/static-final JS/TS/Java collection bindings, Go `slices.Contains` over package-level proven slice bindings, and Rust local immutable literal array/slice bindings are covered; substring contains, map-key membership, mutated or append-expanded bindings, shadowed constructors/types/packages, untyped dynamic sets, and ambiguous receiver contains must stay distinct.",
        "Continue with dynamic collection/set membership only when receiver/element coordinates can be proven by imported or cross-file immutable bindings, construction facts, or type facts beyond the current typed-parameter, Python builtin factory, function-local constructed binding, Python tuple, Java Queue, Rust VecDeque, literal-Set, Java literal-factory, Rust std factory, same-file module-binding, Go imported-package slice-binding, and Rust local literal-binding cases; keep substring, regex, map-key, mutation, shadowing, append-expanded construction, and unproven receiver-overloaded calls as hard boundaries.",
        (
            pat("go_slices_contains", "go", r"\bslices\.Contains\s*\(", "high"),
            pat("go_map_ok", "go", r"\b_,\s*\w+\s*:=\s*\w+\s*\[[^\]]+\]", "high"),
            pat("java_contains_key", "java", r"\.\s*containsKey\s*\(", "high"),
            pat("java_contains_ambiguous", "java", r"\.\s*contains\s*\(", "medium"),
            pat("js_membership_ambiguous", "javascript", r"\.\s*(?:includes|has|indexOf)\s*\(", "medium"),
            pat("py_in_predicate", "python", r"(?m)^\s*(?:if|elif|while|return|assert)\s+[^:\n]{1,140}\b(?:in|not\s+in)\b[^:\n]{0,140}", "medium"),
            pat("ruby_membership", "ruby", r"\.\s*(?:include\?|key\?|has_key\?|member\?)\s*\(", "medium"),
            pat("rust_contains_key", "rust", r"\.\s*contains_key\s*\(", "high"),
            pat("rust_contains_ambiguous", "rust", r"\.\s*contains\s*\(", "medium"),
            pat("ts_membership_ambiguous", "typescript", r"\.\s*(?:includes|has|indexOf)\s*\(", "medium"),
        ),
    ),
    Candidate(
        "null_option_presence",
        "Null, nil, none, and option presence guards",
        "all-language",
        3,
        3,
        "covered-current",
        "Strict null/none/nil/option presence and value-or-fallback defaulting are covered for the currently modeled coordinates; alias/effectful-guard variants need separate proof facts before ordinary detector work.",
        "No ordinary loop; reopen only when pointer/reference alias coordinates or effect-free guard-body facts are modeled.",
        (
            pat("c_null_compare", "c", r"(?:==|!=)\s*NULL\b|\bNULL\s*(?:==|!=)", "high"),
            pat("go_nil_compare", "go", r"(?:==|!=)\s*nil\b|\bnil\s*(?:==|!=)", "high"),
            pat("java_null_compare", "java", r"(?:==|!=)\s*null\b|\bnull\s*(?:==|!=)", "high"),
            pat("js_nullish_compare", "javascript", r"(?:={2,3}|!==|!=)\s*(?:null|undefined)\b", "high"),
            pat("js_nullish_default", "javascript", r"\?\?", "medium"),
            pat("py_none_compare", "python", r"\bis\s+(?:not\s+)?None\b", "high"),
            pat("ruby_nil_predicate", "ruby", r"\.\s*nil\?\b", "high"),
            pat("rust_option_predicate", "rust", r"\.\s*(?:is_some|is_none)\s*\(\s*\)", "high"),
            pat("rust_if_let_some", "rust", r"\bif\s+let\s+Some\s*\(", "medium"),
            pat("ts_nullish_compare", "typescript", r"(?:={2,3}|!==|!=)\s*(?:null|undefined)\b", "high"),
            pat("ts_nullish_default", "typescript", r"\?\?", "medium"),
        ),
    ),
    Candidate(
        "map_default_lookup",
        "Map/dict lookup with default",
        "multi-language",
        4,
        4,
        "partially-covered",
        "Literal Python/Ruby map lookup, JS/TS inline/local/module Map and object defaults, typed Go/Java/Rust maps, typed TypeScript Map fallbacks, typed Python `dict`/`Mapping` fallbacks, Java Map.of/Map.ofEntries literal factories, Java static-final Map.of bindings, Rust std HashMap/BTreeMap literal factories, and Go `map[string]int|string|bool|float64|*T{...}[key]` zero-value default lookups are covered; cross-file imports, richer receiver facts, untyped Python/Ruby/JS receiver defaults, remaining Go zero-value families, absent-key semantics beyond proven zero defaults, and mutation/effects remain open.",
        "Continue with imported or cross-file Map/object defaults only when receiver/key/default coordinates can be proven by import identity, immutable binding, type facts, and whole-file mutation exclusion beyond the current inline/local/module construction and Rust std factory cases.",
        (
            pat("go_map_lookup_ok", "go", r"\b\w+\s*,\s*\w+\s*:=\s*\w+\s*\[[^\]]+\]", "medium"),
            pat("java_get_or_default", "java", r"\.\s*getOrDefault\s*\(", "high"),
            pat("js_map_get_default", "javascript", r"\.\s*get\s*\([^)]{1,100}\)\s*\?\?", "medium"),
            pat("py_get_default", "python", r"\.\s*get\s*\([^,\n()]{1,100},\s*[^)\n]{1,120}\)", "high"),
            pat("ruby_fetch_default", "ruby", r"\.\s*fetch\s*\([^,\n()]{1,100},\s*[^)\n]{1,120}\)|\.\s*fetch\s*\([^)]{1,120}\)\s*(?:do|\{)", "high"),
            pat("rust_get_unwrap_default", "rust", r"\.\s*get\s*\([^)]*\)\s*\.\s*(?:unwrap_or|unwrap_or_else|unwrap_or_default)", "high"),
            pat("ts_map_get_default", "typescript", r"\.\s*get\s*\([^)]{1,100}\)\s*\?\?", "medium"),
        ),
    ),
    Candidate(
        "numeric_minmax_abs",
        "Scalar min/max/abs idioms",
        "all-language",
        2,
        2,
        "covered-current",
        "Scalar min/max/abs expression facts are covered for C, Go, Java, JavaScript/TypeScript, Python, Ruby, Rust, and embedded script surfaces, including Rust numeric `.abs()`, `.min()`, and `.max()` when the receiver has an explicit numeric type fact.",
        "No ordinary detector slice remains open; only revisit if real-repo audit exposes a method-like numeric form that can be proven without accepting custom receiver methods.",
        (
            pat("rust_numeric_method", "rust", r"\.\s*(?:abs|min|max)\s*\(", "high"),
        ),
    ),
    Candidate(
        "property_type_guard",
        "Property type guards",
        "language-family",
        2,
        3,
        "covered-current",
        "Focused probing showed current strict property-type guard positives already converge; retained for real-corpus monitoring rather than ordinary detector work.",
        "No ordinary loop; only reopen if new dynamic-key, aliasing, or shadowing boundaries expose a strict miss.",
        (
            pat("js_typeof_property", "javascript", r"typeof\s+[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*|\[['\"][^'\"]+['\"]\])\s*={2,3}\s*['\"](?:string|number|boolean|function|object|undefined)['\"]"),
            pat("ts_typeof_property", "typescript", r"typeof\s+[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*|\[['\"][^'\"]+['\"]\])\s*={2,3}\s*['\"](?:string|number|boolean|function|object|undefined)['\"]"),
        ),
    ),
    Candidate(
        "own_property_guard",
        "Own-property guard forms",
        "language-family",
        1,
        3,
        "covered-current",
        "Covered by the current loop; retained so future reports show why it should not be picked again.",
        "No new loop; monitor real-corpus yield.",
        (
            pat("js_own_property", "javascript", r"\bObject\.hasOwn\s*\(|Object\.prototype\.hasOwnProperty\.call\s*\(|\.hasOwnProperty\s*\(|['\"][\w$-]+['\"]\s+in\s+\w+"),
            pat("ts_own_property", "typescript", r"\bObject\.hasOwn\s*\(|Object\.prototype\.hasOwnProperty\.call\s*\(|\.hasOwnProperty\s*\(|['\"][\w$-]+['\"]\s+in\s+\w+"),
        ),
    ),
]


PROBES_BY_CANDIDATE = {
    "collection_empty_check": (
        probe("c_collection_emptyish", "c", r"\b\w*(?:len|length|count|size)\w*\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*\w*(?:len|length|count|size)\w*"),
        probe("go_collection_emptyish", "go", r"\blen\s*\([^)]{1,80}\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*len\s*\([^)]{1,80}\)"),
        probe("java_collection_emptyish", "java", r"\.\s*isEmpty\s*\(\s*\)|\.\s*size\s*\(\s*\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*[^;\n]{1,80}\.\s*size\s*\(\s*\)|\bassert(?:Equals|That)\s*\(\s*(?:0\s*,\s*[^;\n]{1,100}\.\s*size\s*\(\s*\)|[^;\n]{1,100}\.\s*size\s*\(\s*\)\s*,\s*0)\b"),
        probe("js_collection_emptyish", "javascript", r"\.\s*length\s*(?:={2,3}|!==|!=|>|>=)\s*0\b|\b0\s*(?:={2,3}|!==|!=|<|<=)\s*[^;\n]{1,80}\.\s*length\b|\bexpect\s*\([^;\n]{1,120}\.\s*length\s*\)\s*\.\s*(?:toBe|toEqual|toStrictEqual)\s*\(\s*0\s*\)"),
        probe("py_collection_emptyish", "python", r"\blen\s*\([^)]{1,80}\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*len\s*\([^)]{1,80}\)|\bassert(?:Equal|Equals)?\s*\([^;\n]{0,160}len\s*\([^)]{1,80}\)[^;\n]*0|(?m:^\s*(?:if|elif|while)\s+(?:not\s+)?(?:[A-Za-z_][\w.]*s|[A-Za-z_][\w.]*(?:items|values|tokens|children|entries|lines|args|params|headers|rows|cols|names|files|dirs|results|messages|errors|warnings|listeners|options))\s*:)"),
        probe("ruby_collection_emptyish", "ruby", r"\.\s*(?:empty\?|any\?|none\?)\b|\.\s*(?:length|size)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*[^;\n]{1,80}\.\s*(?:length|size)\b"),
        probe("rust_collection_emptyish", "rust", r"\.\s*is_empty\s*\(\s*\)|\.\s*len\s*\(\s*\)\s*(?:==|!=|>|>=)\s*0\b|\b0\s*(?:==|!=|<|<=)\s*[^;\n]{1,80}\.\s*len\s*\(\s*\)|\bassert(?:_eq|_ne)?!\s*\(\s*(?:0\s*,\s*[^;\n]{1,100}\.\s*len\s*\(\s*\)|[^;\n]{1,100}\.\s*len\s*\(\s*\)\s*,\s*0)\b"),
        probe("ts_collection_emptyish", "typescript", r"\.\s*length\s*(?:={2,3}|!==|!=|>|>=)\s*0\b|\b0\s*(?:={2,3}|!==|!=|<|<=)\s*[^;\n]{1,80}\.\s*length\b|\bexpect\s*\([^;\n]{1,120}\.\s*length\s*\)\s*\.\s*(?:toBe|toEqual|toStrictEqual)\s*\(\s*0\s*\)"),
    ),
    "string_prefix_suffix": (
        probe("go_prefix_suffix_broad", "go", r"\bstrings\.(?:HasPrefix|HasSuffix)\s*\("),
        probe("java_prefix_suffix_broad", "java", r"\.\s*(?:startsWith|endsWith)\s*\("),
        probe("js_prefix_suffix_broad", "javascript", r"\.\s*(?:startsWith|endsWith)\s*\("),
        probe("py_prefix_suffix_broad", "python", r"\.\s*(?:startswith|endswith)\s*\("),
        probe("ruby_prefix_suffix_broad", "ruby", r"\.\s*(?:start_with\?|end_with\?)\s*\("),
        probe("rust_prefix_suffix_broad", "rust", r"\.\s*(?:starts_with|ends_with)\s*\("),
        probe("ts_prefix_suffix_broad", "typescript", r"\.\s*(?:startsWith|endsWith)\s*\("),
    ),
    "membership_contains": (
        probe("go_membership_broad", "go", r"\bslices\.Contains\s*\(|\b_,\s*\w+\s*:=\s*\w+\s*\[[^\]]+\]"),
        probe("java_membership_broad", "java", r"\.\s*(?:contains|containsKey)\s*\("),
        probe("js_membership_broad", "javascript", r"\.\s*(?:includes|has|indexOf)\s*\("),
        probe("py_membership_broad", "python", r"(?m)^\s*(?:if|elif|while|return|assert)\s+[^:\n]{1,160}\b(?:in|not\s+in)\b[^:\n]{0,160}"),
        probe("ruby_membership_broad", "ruby", r"\.\s*(?:include\?|key\?|has_key\?|member\?)\s*\("),
        probe("rust_membership_broad", "rust", r"\.\s*(?:contains|contains_key)\s*\("),
        probe("ts_membership_broad", "typescript", r"\.\s*(?:includes|has|indexOf)\s*\("),
    ),
    "null_option_presence": (
        probe("c_null_presence_broad", "c", r"(?:==|!=)\s*NULL\b|\bNULL\s*(?:==|!=)"),
        probe("go_nil_presence_broad", "go", r"(?:==|!=)\s*nil\b|\bnil\s*(?:==|!=)"),
        probe("java_null_presence_broad", "java", r"(?:==|!=)\s*null\b|\bnull\s*(?:==|!=)"),
        probe("js_nullish_presence_broad", "javascript", r"(?:={2,3}|!==|!=)\s*(?:null|undefined)\b|\?\?"),
        probe("py_none_presence_broad", "python", r"\bis\s+(?:not\s+)?None\b"),
        probe("ruby_nil_presence_broad", "ruby", r"\.\s*nil\?\b"),
        probe("rust_option_presence_broad", "rust", r"\.\s*(?:is_some|is_none)\s*\(\s*\)|\bif\s+let\s+Some\s*\("),
        probe("ts_nullish_presence_broad", "typescript", r"(?:={2,3}|!==|!=)\s*(?:null|undefined)\b|\?\?"),
    ),
    "map_default_lookup": (
        probe("go_map_lookup_default_broad", "go", r"\b\w+\s*,\s*\w+\s*:=\s*\w+\s*\[[^\]]+\]"),
        probe("java_map_get_default_broad", "java", r"\.\s*getOrDefault\s*\("),
        probe("js_map_get_default_broad", "javascript", r"\.\s*get\s*\([^)]{1,100}\)\s*\?\?"),
        probe("py_map_get_default_broad", "python", r"\.\s*get\s*\([^,\n()]{1,100},\s*[^)\n]{1,120}\)"),
        probe("ruby_map_fetch_default_broad", "ruby", r"\.\s*fetch\s*\([^,\n()]{1,100},\s*[^)\n]{1,120}\)|\.\s*fetch\s*\([^)]{1,120}\)\s*(?:do|\{)"),
        probe("rust_map_get_default_broad", "rust", r"\.\s*get\s*\([^)]*\)\s*\.\s*(?:unwrap_or|unwrap_or_else|unwrap_or_default)"),
        probe("ts_map_get_default_broad", "typescript", r"\.\s*get\s*\([^)]{1,100}\)\s*\?\?"),
    ),
    "numeric_minmax_abs": (
        probe("rust_numeric_broad", "rust", r"\.\s*(?:abs|min|max)\s*\("),
    ),
    "property_type_guard": (
        probe("js_typeof_property_broad", "javascript", r"typeof\s+[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*|\[['\"][^'\"]+['\"]\])\s*={2,3}\s*['\"](?:string|number|boolean|function|object|undefined)['\"]"),
        probe("ts_typeof_property_broad", "typescript", r"typeof\s+[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*|\[['\"][^'\"]+['\"]\])\s*={2,3}\s*['\"](?:string|number|boolean|function|object|undefined)['\"]"),
    ),
    "own_property_guard": (
        probe("js_own_property_broad", "javascript", r"\bObject\.hasOwn\s*\(|Object\.prototype\.hasOwnProperty\.call\s*\(|\.hasOwnProperty\s*\(|['\"][\w$-]+['\"]\s+in\s+\w+"),
        probe("ts_own_property_broad", "typescript", r"\bObject\.hasOwn\s*\(|Object\.prototype\.hasOwnProperty\.call\s*\(|\.hasOwnProperty\s*\(|['\"][\w$-]+['\"]\s+in\s+\w+"),
    ),
}


def load_repos(corpus_path: Path, repos_root: Path) -> list[dict]:
    if corpus_path.exists():
        doc = json.loads(corpus_path.read_text())
        return [
            {
                "id": repo["id"],
                "split": repo.get("split", ""),
                "primary_language": repo.get("primary_language", ""),
                "path": repos_root / repo["id"],
            }
            for repo in doc.get("repositories", [])
            if (repos_root / repo["id"]).is_dir()
        ]
    return [
        {"id": p.name, "split": "", "primary_language": "", "path": p}
        for p in sorted(repos_root.iterdir())
        if p.is_dir()
    ]


def iter_source_files(repo_path: Path, max_bytes: int) -> Iterable[tuple[Path, str]]:
    for path in repo_path.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.name.endswith(".min.js"):
            continue
        lang = LANG_BY_EXT.get(path.suffix)
        if lang is None:
            continue
        try:
            if path.stat().st_size > max_bytes:
                continue
        except OSError:
            continue
        yield path, lang


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def snippet(text: str, start: int, end: int) -> str:
    line_start = text.rfind("\n", 0, start) + 1
    line_end = text.find("\n", end)
    if line_end == -1:
        line_end = len(text)
    return " ".join(text[line_start:line_end].strip().split())[:180]


def is_comment_only_line(text: str, offset: int, lang: str) -> bool:
    line_start = text.rfind("\n", 0, offset) + 1
    line_end = text.find("\n", offset)
    if line_end == -1:
        line_end = len(text)
    stripped = text[line_start:line_end].lstrip()
    if not stripped:
        return True
    if lang in {"python", "ruby"}:
        return stripped.startswith("#")
    if lang in {"c", "go", "java", "javascript", "rust", "typescript"}:
        return stripped.startswith(("//", "/*", "*", "*/"))
    return False


def has_non_iteration_python_membership(segment: str) -> bool:
    """Return true when a Python `in` token looks like membership, not iteration."""
    for match in re.finditer(r"\b(?:not\s+in|in)\b", segment):
        prefix = segment[: match.start()]
        if re.search(r"(?:^|[\s(])(?:async\s+)?for\s+[^:\n]{0,80}\s+$", prefix):
            continue
        return True
    return False


def is_compound_python_len_arithmetic(segment: str) -> bool:
    return bool(
        re.search(r"\blen\s*\([^)]*\)\s*[-+*/%]", segment)
        or re.search(r"[-+*/%]\s*len\s*\(", segment)
    )


def match_filter_reason(candidate_id: str, lang: str, match: re.Match[str]) -> str | None:
    segment = match.group(0)
    if candidate_id == "membership_contains" and lang == "python":
        if not has_non_iteration_python_membership(segment):
            return "python-for-in-iteration"
    if candidate_id == "collection_empty_check" and lang == "python":
        if is_compound_python_len_arithmetic(segment):
            return "compound-length-arithmetic"
    return None


def spans_overlap(a: tuple[int, int], b: tuple[int, int]) -> bool:
    return a[0] < b[1] and b[0] < a[1]


def make_sample(repo: dict, rel: str, lang: str, text: str, start: int, end: int) -> dict:
    return {
        "repo": repo["id"],
        "split": repo.get("split") or "unknown",
        "language": lang,
        "path": rel,
        "line": line_number(text, start),
        "snippet": snippet(text, start, end),
    }


def scope_weight(scope: str) -> float:
    return {
        "all-language": 1.35,
        "multi-language": 1.15,
        "language-family": 0.7,
        "single-language": 0.45,
    }.get(scope, 1.0)


def status_weight(status: str) -> float:
    return {
        "open": 1.0,
        "partially-covered": 0.65,
        "covered-current": 0.08,
    }.get(status, 1.0)


def rank_score(matches: float, repos: int, languages: int, candidate: Candidate) -> float:
    if matches == 0:
        return 0.0
    impact = math.log1p(matches) * math.sqrt(repos) * (1.0 + 0.35 * max(0, languages - 1))
    cost = candidate.implementation_cost + candidate.soundness_risk
    return impact * scope_weight(candidate.scope) * status_weight(candidate.status) / cost


def format_probe_coverage(row: dict) -> str:
    coverage = row["probe_coverage"]
    if coverage is None:
        return "n/a"
    if row["probe_uncovered"] and coverage >= 0.9995:
        return "<100%"
    return f"{coverage:.1%}"


def analyze(corpus_path: Path, repos_root: Path, max_bytes: int, sample_limit: int) -> dict:
    repos = load_repos(corpus_path, repos_root)
    totals = {
        candidate.candidate_id: {
            "candidate": candidate,
            "raw_matches": 0,
            "weighted_matches": 0.0,
            "repos": set(),
            "repo_stats": {},
            "languages": set(),
            "splits": {},
            "samples": [],
            "pattern_stats": {
                spec.pattern_id: {
                    "pattern_id": spec.pattern_id,
                    "language": spec.lang,
                    "precision": spec.precision,
                    "regex": spec.regex.pattern,
                    "raw_matches": 0,
                    "weighted_matches": 0.0,
                    "repos": set(),
                    "samples": [],
                }
                for spec in candidate.patterns
            },
            "probe_matches": 0,
            "probe_filtered": 0,
            "probe_uncovered": 0,
            "probe_repos": set(),
            "gap_samples": [],
            "filter_samples": [],
        }
        for candidate in CANDIDATES
    }
    files_scanned = 0
    bytes_scanned = 0

    for repo in repos:
        repo_path = repo["path"]
        for path, lang in iter_source_files(repo_path, max_bytes):
            try:
                text = path.read_text(errors="ignore")
            except OSError:
                continue
            files_scanned += 1
            bytes_scanned += len(text)
            rel = str(path.relative_to(repo_path))
            for candidate in CANDIDATES:
                specs = [spec for spec in candidate.patterns if spec.lang == lang]
                probes = [spec for spec in PROBES_BY_CANDIDATE.get(candidate.candidate_id, ()) if spec.lang == lang]
                if not specs and not probes:
                    continue
                raw_for_file = 0
                weighted_for_file = 0.0
                extracted_spans = []
                first_matches: list[tuple[re.Match[str], PatternSpec]] = []
                bucket = totals[candidate.candidate_id]
                split = repo.get("split") or "unknown"
                for spec in specs:
                    for match in spec.regex.finditer(text):
                        if is_comment_only_line(text, match.start(), lang):
                            continue
                        if match_filter_reason(candidate.candidate_id, lang, match):
                            continue
                        weight = PRECISION_WEIGHT[spec.precision]
                        raw_for_file += 1
                        weighted_for_file += weight
                        extracted_spans.append((match.start(), match.end()))
                        stat = bucket["pattern_stats"][spec.pattern_id]
                        stat["raw_matches"] += 1
                        stat["weighted_matches"] += weight
                        stat["repos"].add(repo["id"])
                        if len(stat["samples"]) < sample_limit:
                            stat["samples"].append(make_sample(repo, rel, lang, text, match.start(), match.end()))
                        if len(first_matches) < sample_limit:
                            first_matches.append((match, spec))
                if raw_for_file:
                    bucket["raw_matches"] += raw_for_file
                    bucket["weighted_matches"] += weighted_for_file
                    bucket["repos"].add(repo["id"])
                    bucket["languages"].add(lang)
                    bucket["splits"][split] = bucket["splits"].get(split, 0) + raw_for_file
                    repo_stat = bucket["repo_stats"].setdefault(
                        repo["id"],
                        {
                            "repo": repo["id"],
                            "split": split,
                            "primary_language": repo.get("primary_language") or "",
                            "raw_matches": 0,
                            "weighted_matches": 0.0,
                            "languages": set(),
                        },
                    )
                    repo_stat["raw_matches"] += raw_for_file
                    repo_stat["weighted_matches"] += weighted_for_file
                    repo_stat["languages"].add(lang)
                for probe_spec in probes:
                    for match in probe_spec.regex.finditer(text):
                        if is_comment_only_line(text, match.start(), lang):
                            continue
                        filter_reason = match_filter_reason(candidate.candidate_id, lang, match)
                        if filter_reason:
                            bucket["probe_filtered"] += 1
                            if len(bucket["filter_samples"]) < sample_limit:
                                sample = make_sample(repo, rel, lang, text, match.start(), match.end())
                                sample["probe_id"] = probe_spec.probe_id
                                sample["filter_reason"] = filter_reason
                                bucket["filter_samples"].append(sample)
                            continue
                        bucket["probe_matches"] += 1
                        bucket["probe_repos"].add(repo["id"])
                        span = (match.start(), match.end())
                        if any(spans_overlap(span, extracted) for extracted in extracted_spans):
                            continue
                        bucket["probe_uncovered"] += 1
                        if len(bucket["gap_samples"]) < sample_limit:
                            sample = make_sample(repo, rel, lang, text, match.start(), match.end())
                            sample["probe_id"] = probe_spec.probe_id
                            bucket["gap_samples"].append(sample)
                for match, spec in first_matches:
                    if len(bucket["samples"]) >= sample_limit:
                        break
                    sample = make_sample(repo, rel, lang, text, match.start(), match.end())
                    sample["pattern_id"] = spec.pattern_id
                    sample["precision"] = spec.precision
                    bucket["samples"].append(sample)

    rows = []
    for candidate in CANDIDATES:
        bucket = totals[candidate.candidate_id]
        repos_count = len(bucket["repos"])
        languages_count = len(bucket["languages"])
        probe_matches = bucket["probe_matches"]
        probe_filtered = bucket["probe_filtered"]
        probe_uncovered = bucket["probe_uncovered"]
        probe_coverage = None
        if probe_matches:
            probe_coverage = (probe_matches - probe_uncovered) / probe_matches
        pattern_stats = []
        for stat in bucket["pattern_stats"].values():
            pattern_stats.append(
                {
                    "pattern_id": stat["pattern_id"],
                    "language": stat["language"],
                    "precision": stat["precision"],
                    "regex": stat["regex"],
                    "raw_matches": stat["raw_matches"],
                    "weighted_matches": round(stat["weighted_matches"], 2),
                    "repos": len(stat["repos"]),
                    "samples": stat["samples"],
                }
            )
        pattern_stats.sort(key=lambda stat: stat["weighted_matches"], reverse=True)
        repo_stats = []
        for stat in bucket["repo_stats"].values():
            repo_stats.append(
                {
                    "repo": stat["repo"],
                    "split": stat["split"],
                    "primary_language": stat["primary_language"],
                    "raw_matches": stat["raw_matches"],
                    "weighted_matches": round(stat["weighted_matches"], 2),
                    "languages": sorted(stat["languages"]),
                }
            )
        repo_stats.sort(key=lambda stat: stat["weighted_matches"], reverse=True)
        rows.append(
            {
                "candidate_id": candidate.candidate_id,
                "title": candidate.title,
                "scope": candidate.scope,
                "status": candidate.status,
                "matches": bucket["raw_matches"],
                "raw_matches": bucket["raw_matches"],
                "weighted_matches": round(bucket["weighted_matches"], 2),
                "repos": repos_count,
                "languages": sorted(bucket["languages"]),
                "language_count": languages_count,
                "splits": dict(sorted(bucket["splits"].items())),
                "implementation_cost": candidate.implementation_cost,
                "soundness_risk": candidate.soundness_risk,
                "score": rank_score(bucket["weighted_matches"], repos_count, languages_count, candidate),
                "probe_matches": probe_matches,
                "probe_filtered": probe_filtered,
                "probe_uncovered": probe_uncovered,
                "probe_coverage": None if probe_coverage is None else round(probe_coverage, 4),
                "probe_repos": len(bucket["probe_repos"]),
                "why": candidate.why,
                "next_probe": candidate.next_probe,
                "samples": bucket["samples"],
                "gap_samples": bucket["gap_samples"],
                "filter_samples": bucket["filter_samples"],
                "pattern_stats": pattern_stats,
                "top_repos": repo_stats[:20],
            }
        )
    rows.sort(key=lambda row: row["score"], reverse=True)
    return {
        "schema_version": "0.4.0",
        "repos_root": str(repos_root),
        "corpus": str(corpus_path),
        "repo_count": len(repos),
        "files_scanned": files_scanned,
        "bytes_scanned": bytes_scanned,
        "max_bytes_per_file": max_bytes,
        "ranking": rows,
    }


def candidate_signature() -> str:
    payload = {
        "candidates": [
            {
                "candidate_id": candidate.candidate_id,
                "title": candidate.title,
                "scope": candidate.scope,
                "status": candidate.status,
                "why": candidate.why,
                "next_probe": candidate.next_probe,
                "patterns": [
                    {
                        "pattern_id": spec.pattern_id,
                        "lang": spec.lang,
                        "regex": spec.regex.pattern,
                        "precision": spec.precision,
                    }
                    for spec in candidate.patterns
                ],
                "probes": [
                    {
                        "probe_id": spec.probe_id,
                        "lang": spec.lang,
                        "regex": spec.regex.pattern,
                    }
                    for spec in PROBES_BY_CANDIDATE.get(candidate.candidate_id, ())
                ],
            }
            for candidate in CANDIDATES
        ],
        "precision_weight": PRECISION_WEIGHT,
        "schema": "0.4.0",
    }
    text = json.dumps(payload, sort_keys=True)
    return hashlib.sha256(text.encode()).hexdigest()


def corpus_signature(corpus_path: Path, repos_root: Path, max_bytes: int, sample_limit: int) -> dict:
    repos = load_repos(corpus_path, repos_root)
    h = hashlib.sha256()
    files = 0
    for repo in repos:
        h.update(repo["id"].encode())
        h.update((repo.get("split") or "").encode())
        h.update((repo.get("primary_language") or "").encode())
        for path, lang in iter_source_files(repo["path"], max_bytes):
            try:
                stat = path.stat()
            except OSError:
                continue
            rel = str(path.relative_to(repo["path"]))
            h.update(repo["id"].encode())
            h.update(rel.encode())
            h.update(lang.encode())
            h.update(str(stat.st_size).encode())
            h.update(str(stat.st_mtime_ns).encode())
            files += 1
    return {
        "candidate_signature": candidate_signature(),
        "corpus_path": str(corpus_path.resolve()),
        "repos_root": str(repos_root.resolve()),
        "max_bytes": max_bytes,
        "sample_limit": sample_limit,
        "repo_count": len(repos),
        "source_file_count": files,
        "digest": h.hexdigest(),
    }


def markdown_report(result: dict, top: int) -> str:
    lines = [
        "# Type-4 frontier priorities",
        "",
        "This report is generated from the pinned benchmark repos by",
        "`bench/type4/prioritize_frontier.py`. Scores combine real-code frequency,",
        "repo/language spread, estimated implementation cost, soundness risk, scope,",
        "and whether a frontier is already covered.",
        "",
        f"- repos scanned: {result['repo_count']}",
        f"- files scanned: {result['files_scanned']}",
        f"- max bytes per file: {result['max_bytes_per_file']}",
        "- matches: raw syntactic hits",
        "- weighted: raw hits adjusted by pattern precision (`high=1.0`, `medium=0.55`, `low=0.15`)",
        "- probe coverage: broad-probe hits already covered by extraction patterns; gaps feed the next pattern loop",
        "- filtered: broad-probe hits rejected as overreach before coverage is scored",
        "",
        "| rank | candidate | scope | status | score | raw | weighted | repos | languages | probe coverage | gaps | filtered |",
        "|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for idx, row in enumerate(result["ranking"][:top], start=1):
        coverage = format_probe_coverage(row)
        lines.append(
            "| {rank} | `{candidate_id}` | {scope} | {status} | {score:.2f} | {raw_matches} | {weighted_matches:.1f} | {repos} | {language_count} | {coverage} | {probe_uncovered} | {probe_filtered} |".format(
                rank=idx,
                coverage=coverage,
                **row,
            )
        )
    lines.extend(["", "## Recommended Order", ""])
    recommended_rows = [row for row in result["ranking"] if row["status"] != "covered-current"]
    for idx, row in enumerate(recommended_rows[:top], start=1):
        langs = ", ".join(row["languages"])
        lines.extend(
            [
                f"{idx}. `{row['candidate_id']}`",
                f"   - why: {row['why']}",
                f"   - evidence: {row['raw_matches']} raw / {row['weighted_matches']:.1f} weighted matches across {row['repos']} repos and {row['language_count']} languages ({langs})",
                f"   - probe coverage: {format_probe_coverage(row)}; uncovered probe hits: {row['probe_uncovered']}; filtered probe hits: {row['probe_filtered']}",
                f"   - next probe: {row['next_probe']}",
            ]
        )
    lines.extend(["", "## Pattern Diagnostics", ""])
    for row in result["ranking"][:top]:
        lines.append(f"### `{row['candidate_id']}`")
        lines.append("")
        lines.append("| pattern | language | precision | raw | weighted | repos |")
        lines.append("|---|---|---|---:|---:|---:|")
        for stat in row["pattern_stats"][:12]:
            if stat["raw_matches"] == 0:
                continue
            lines.append(
                f"| `{stat['pattern_id']}` | {stat['language']} | {stat['precision']} | {stat['raw_matches']} | {stat['weighted_matches']:.1f} | {stat['repos']} |"
            )
        lines.append("")
    lines.extend(["", "## Gap Samples", ""])
    for row in result["ranking"][:top]:
        lines.append(f"### `{row['candidate_id']}`")
        if not row["gap_samples"]:
            lines.append("- no uncovered broad-probe samples")
            lines.append("")
            continue
        for sample in row["gap_samples"][:5]:
            lines.append(
                f"- `{sample['repo']}/{sample['path']}:{sample['line']}` ({sample['language']}, {sample['probe_id']}): {sample['snippet']}"
            )
        lines.append("")
    lines.extend(["", "## Filtered Probe Samples", ""])
    for row in result["ranking"][:top]:
        lines.append(f"### `{row['candidate_id']}`")
        if not row["filter_samples"]:
            lines.append("- no filtered broad-probe samples")
            lines.append("")
            continue
        for sample in row["filter_samples"][:5]:
            lines.append(
                f"- `{sample['repo']}/{sample['path']}:{sample['line']}` ({sample['language']}, {sample['probe_id']}, {sample['filter_reason']}): {sample['snippet']}"
            )
        lines.append("")
    lines.extend(["", "## Audit Repo Samples", ""])
    for row in result["ranking"][:top]:
        lines.append(f"### `{row['candidate_id']}`")
        if not row["top_repos"]:
            lines.append("- no repo samples")
            lines.append("")
            continue
        for sample in row["top_repos"][:5]:
            langs = ", ".join(sample["languages"])
            lines.append(
                f"- `{sample['repo']}` ({sample['split']}, {sample['primary_language']}; {langs}): {sample['raw_matches']} raw / {sample['weighted_matches']:.1f} weighted"
            )
        lines.append("")
    lines.extend(["", "## Extraction Samples", ""])
    for row in result["ranking"][:top]:
        lines.append(f"### `{row['candidate_id']}`")
        if not row["samples"]:
            lines.append("")
            continue
        for sample in row["samples"][:5]:
            lines.append(
                f"- `{sample['repo']}/{sample['path']}:{sample['line']}` ({sample['language']}, {sample['pattern_id']}): {sample['snippet']}"
            )
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--max-bytes", type=int, default=512_000)
    parser.add_argument("--sample-limit", type=int, default=12)
    parser.add_argument("--top", type=int, default=8)
    parser.add_argument("--cache", type=Path, help="reuse a cached analysis when corpus and pattern inputs are unchanged")
    parser.add_argument("--no-cache-read", action="store_true")
    parser.add_argument("--no-cache-write", action="store_true")
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--markdown-out", type=Path)
    args = parser.parse_args()

    result = None
    cache_key = None
    if args.cache:
        cache_key = corpus_signature(args.corpus, args.repos_root, args.max_bytes, args.sample_limit)
        if not args.no_cache_read and args.cache.exists():
            try:
                cache_doc = json.loads(args.cache.read_text())
            except json.JSONDecodeError:
                cache_doc = {}
            if cache_doc.get("key") == cache_key:
                result = cache_doc.get("result")
    if result is None:
        result = analyze(args.corpus, args.repos_root, args.max_bytes, args.sample_limit)
        if args.cache and not args.no_cache_write:
            args.cache.parent.mkdir(parents=True, exist_ok=True)
            args.cache.write_text(
                json.dumps({"key": cache_key, "result": result}, indent=2, ensure_ascii=False)
                + "\n"
            )
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(markdown_report(result, args.top))
    if not args.json_out and not args.markdown_out:
        print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
