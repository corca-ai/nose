#!/usr/bin/env python3
"""Price narrow semantic-pack candidate rows against the pinned corpus.

The output is an assessor artifact, not an implementation plan. Corpus matches
are queue signals. A candidate becomes implementation work only after the
recorded impact, safety boundary, and hard negatives justify a row-sized target
packet.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CORPUS = ROOT / "bench" / "goldens" / "corpus.json"
DEFAULT_REPOS_ROOT = ROOT / "bench" / "repos"
DEFAULT_JSON_OUT = ROOT / "bench" / "semantic_pack" / "candidate_pricing.v1.json"
DEFAULT_MD_OUT = ROOT / "bench" / "semantic_pack" / "candidate_pricing.md"
DEFAULT_REVIEW_LOG = ROOT / "bench" / "semantic_pack" / "loop_reviews.v1.json"

SCHEMA_VERSION = 1
TOOL_VERSION = "semantic-pack-pricing/1"
MAX_FILE_BYTES = 1_000_000
MAX_SAMPLES_PER_CANDIDATE = 8

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

VERDICTS = {"priced-ready", "priced-but-blocked", "unpriced"}
EVIDENCE_TIERS = {"queue-signal", "manual-pricing"}


@dataclass(frozen=True)
class PatternSpec:
    pattern_id: str
    lang: str
    regex: re.Pattern[str]
    precision: str = "high"
    context: re.Pattern[str] | None = None


@dataclass(frozen=True)
class Candidate:
    iteration: int
    candidate_id: str
    title: str
    proposed_pack_id: str | None
    ecosystem: str
    row_slice: str
    target_lane: str
    patterns: tuple[PatternSpec, ...]
    impact_price: str
    safety_price: str
    current_detector_result: str
    existing_vocabulary: tuple[str, ...]
    required_evidence: tuple[str, ...]
    hard_negatives: tuple[str, ...]
    unsupported_cases: tuple[str, ...]
    product_runtime_plan: str
    rollback_path: str
    initial_verdict: str
    next_action: str
    blocked_by: tuple[str, ...] = ()
    rejection_context: str = ""


def pat(
    pattern_id: str,
    lang: str,
    regex: str,
    precision: str = "high",
    *,
    context: str | None = None,
) -> PatternSpec:
    context_regex = re.compile(context, re.MULTILINE) if context is not None else None
    return PatternSpec(pattern_id, lang, re.compile(regex), precision, context_regex)


CANDIDATES: tuple[Candidate, ...] = (
    Candidate(
        1,
        "java.guava.immutable_collection_factories",
        "Guava immutable collection factories",
        "nose.java.ecosystem.guava.collection_factories",
        "Guava",
        "ImmutableList.of, ImmutableSet.of, ImmutableMap.of, and constrained copyOf factories",
        "builtin-optional candidate",
        (
            pat("guava_immutable_list", "java", r"\bImmutableList\s*\.\s*(?:of|copyOf)\s*\("),
            pat("guava_immutable_set", "java", r"\bImmutableSet\s*\.\s*(?:of|copyOf)\s*\("),
            pat("guava_immutable_map", "java", r"\bImmutableMap\s*\.\s*(?:of|copyOf)\s*\("),
        ),
        "High: common Java corpus ecosystem and direct collection-factory equivalence value.",
        "Medium: can reuse Java collection-factory vocabulary, but exact package and overload proof must be explicit.",
        "Not a builtin pack today; analogous Java stdlib collection factories are covered, while Guava package coordinates are not admitted.",
        ("Java collection factory contracts", "import/static-import proof", "arity/overload proof"),
        (
            "exact Guava package coordinate",
            "import or static-import identity",
            "supported overload",
            "result collection domain",
            "copyOf source-domain/effect proof",
        ),
        ("wrong package with same class name", "shadowed ImmutableList/Set/Map", "unsupported overload", "copyOf over mutable or effectful source"),
        ("builder chains", "toImmutable* collectors", "mutable source copyOf without source-domain proof"),
        "Run semantic query-regression on Java-heavy subset plus focused Guava fixtures; classify drift as metadata or measured recall.",
        "Keep pack builtin-optional; disable only the copyOf row first if post-release risk appears.",
        "priced-ready",
        "Write target packet before implementation.",
    ),
    Candidate(
        2,
        "java.guava.optional_presence",
        "Guava Optional presence predicates",
        "nose.java.ecosystem.guava.optional",
        "Guava",
        "Optional.absent/fromNullable/isPresent/or/default-style presence slices",
        "deferred",
        (
            pat(
                "guava_optional",
                "java",
                r"\bOptional\s*\.\s*(?:absent|fromNullable|of)\s*\(|\.\s*(?:isPresent|orNull)\s*\(",
                context=r"com\.google\.common\.base\.Optional|package\s+com\.google\.common\.base",
            ),
        ),
        "Medium: visible where legacy Guava Optional predates java.util.Optional.",
        "High: nullability and default semantics differ from java.util.Optional and require version/package proof.",
        "Current option/null presence support does not prove Guava Optional identity.",
        ("null/option presence value facts", "Java import proof"),
        ("exact com.google.common.base.Optional", "method identity", "null/default semantics", "version policy"),
        ("java.util.Optional", "shadowed Optional", "or(default) with effectful default", "fromNullable vs of null behavior"),
        ("transform chains", "supplier defaults", "interop with nullable annotations"),
        "Measure only after package/version proof exists; compare Java corpus semantic output and focused hard negatives.",
        "Do not default-enable; keep all Guava Optional rows optional or absent.",
        "priced-but-blocked",
        "Record proof prerequisite.",
        ("Guava Optional version/package proof", "default-demand/effect profile"),
    ),
    Candidate(
        3,
        "javascript.lodash.collection_projection",
        "Lodash collection projection helpers",
        "nose.javascript.ecosystem.lodash.collection_projection",
        "Lodash",
        "map/filter/some/every over arrays with explicit callbacks",
        "deferred",
        (
            pat("lodash_namespace_js", "javascript", r"\b(?:_|lodash)\s*\.\s*(?:map|filter|some|every)\s*\(", context=r"lodash|underscore"),
            pat("lodash_namespace_ts", "typescript", r"\b(?:_|lodash)\s*\.\s*(?:map|filter|some|every)\s*\(", context=r"lodash|underscore"),
        ),
        "High if present: projection/predicate helpers are high-value Type-4 surfaces.",
        "High: iteratee shorthands, object iteration order, lazy chains, and callback effects need hard boundaries.",
        "Current JS array/HOF facts do not admit Lodash selectors as builtin evidence.",
        ("JS array/predicate contracts", "callback demand/effect vocabulary"),
        ("lodash import identity", "array receiver domain", "explicit callback form", "callback effect profile"),
        ("iteratee shorthand", "object receiver", "lazy chain", "custom underscore binding", "effectful callback"),
        ("chain/thru/value", "property-path iteratees", "collection objects with order-sensitive keys"),
        "Price with JS/TS corpus samples and query only after explicit callback subset is isolated.",
        "No exact row; near-only or builtin-optional first if ever admitted.",
        "priced-but-blocked",
        "Record callback/demand proof blocker.",
        ("callback demand/effect proof", "receiver array-domain proof", "lodash import identity"),
    ),
    Candidate(
        4,
        "javascript.lodash.membership_predicates",
        "Lodash membership and inclusion helpers",
        "nose.javascript.ecosystem.lodash.membership",
        "Lodash",
        "includes/has/some membership slices",
        "deferred",
        (
            pat("lodash_membership_js", "javascript", r"\b(?:_|lodash)\s*\.\s*(?:includes|has|some)\s*\(", context=r"lodash|underscore"),
            pat("lodash_membership_ts", "typescript", r"\b(?:_|lodash)\s*\.\s*(?:includes|has|some)\s*\(", context=r"lodash|underscore"),
        ),
        "Medium: membership is common and maps to existing protocol vocabulary when receiver proof exists.",
        "High: strings, objects, arrays, path keys, and SameValueZero behavior diverge.",
        "Current receiver-membership protocol does not include Lodash package semantics.",
        ("receiver-membership protocol", "static index membership hard negatives"),
        ("lodash import identity", "receiver kind", "key/value coordinate", "SameValueZero boundary"),
        ("string substring includes", "object path lookup", "custom lodash binding", "NaN/-0 boundary", "iteratee predicate some"),
        ("deep path helpers", "collection objects", "lazy chains"),
        "Use product query-regression only after a single receiver kind is chosen.",
        "Disable risky row; fall back to near-only evidence if receiver kind is ambiguous.",
        "priced-but-blocked",
        "Split by receiver kind before target packet.",
        ("receiver kind proof", "SameValueZero/domain boundary"),
    ),
    Candidate(
        5,
        "python.numpy.scalar_integer_ufuncs",
        "NumPy scalar integer ufuncs",
        "nose.python.ecosystem.numpy.scalar_integer_ufuncs",
        "NumPy",
        "np.abs/minimum/maximum over proven scalar integer values",
        "deferred",
        (
            pat("numpy_scalar_np", "python", r"\bnp\s*\.\s*(?:abs|absolute|minimum|maximum)\s*\("),
            pat("numpy_scalar_numpy", "python", r"\bnumpy\s*\.\s*(?:abs|absolute|minimum|maximum)\s*\("),
        ),
        "Observed corpus evidence is NumPy ufunc presence only; scalar integer impact remains blocked until value-domain proof exists.",
        "High: dtype, broadcasting, NaN, signed zero, overflow, and ndarray mutation/view semantics are outside current scalar proof.",
        "Current numeric min/max/abs support is scalar language/stdlib, not NumPy package semantics.",
        ("numeric min/max/abs laws", "Python import proof"),
        ("numpy import identity", "scalar integer domain", "dtype/overflow policy", "non-array receiver proof"),
        ("ndarray broadcasting", "float NaN/signed-zero", "object dtype", "in-place out parameter", "shadowed np"),
        ("arrays", "Series/DataFrame", "ufunc out/where arguments"),
        "Run only focused fixtures until scalar proof excludes arrays; then Python corpus query-regression.",
        "Keep all NumPy rows disabled if dtype/domain proof is incomplete.",
        "priced-but-blocked",
        "Record dtype/scalar-domain blocker.",
        ("dtype/domain proof", "array-vs-scalar boundary"),
    ),
    Candidate(
        6,
        "python.numpy_clip_minmax",
        "NumPy clip/min/max laws",
        "nose.python.ecosystem.numpy.clip",
        "NumPy",
        "np.clip and min(max(x, lo), hi) only under proven scalar integer bounds",
        "deferred",
        (
            pat("numpy_clip_np", "python", r"\bnp\s*\.\s*clip\s*\("),
            pat("numpy_clip_numpy", "python", r"\bnumpy\s*\.\s*clip\s*\("),
        ),
        "Medium-high if scalar instances exist; aligns with existing clamp proof shape.",
        "High: array broadcasting, dtype, NaN, and bound ordering can invalidate exact equivalence.",
        "Current clamp law is value-graph scalar/integer, not NumPy ufunc semantics.",
        ("integer clamp law", "literal/proven bound facts"),
        ("numpy import identity", "scalar integer domain", "lo <= hi proof", "no array broadcasting"),
        ("unproven bound order", "float/NaN", "array input", "broadcasted bounds", "out parameter"),
        ("ndarray clip", "pandas clip", "dtype-dependent overflow"),
        "Require focused oracle/hard-negative fixtures before any corpus query-regression.",
        "Do not ship exact row until array cases remain explicitly unsupported.",
        "priced-but-blocked",
        "Record scalar clamp proof blocker.",
        ("scalar integer dtype proof", "NumPy broadcast boundary"),
    ),
    Candidate(
        7,
        "python.pandas_series_null_predicates",
        "pandas Series null predicates",
        "nose.python.ecosystem.pandas.series_null_predicates",
        "pandas",
        "Series.isna/notna/isnull/notnull predicates",
        "deferred",
        (
            pat("pandas_series_nulls", "python", r"\.\s*(?:isna|notna|isnull|notnull)\s*\(\s*\)"),
        ),
        "Medium in pandas-heavy repos, but current pinned corpus may be sparse.",
        "Very high: index alignment, NA value model, dtype, mask shape, and view/copy behavior are unmodeled.",
        "Current null presence facts are scalar/control-flow, not vectorized pandas masks.",
        ("null presence vocabulary",),
        ("pandas receiver proof", "Series domain", "mask/index shape", "NA semantics"),
        ("DataFrame receiver", "object dtype", "index alignment", "view/copy mutation", "shadowed pandas"),
        ("DataFrame masks", "groupby/window contexts", "chained assignment"),
        "Do not run product query-regression until vector mask semantics are scoped.",
        "No exact support; likely near-only evidence before exact rows.",
        "priced-but-blocked",
        "Record vector-mask substrate blocker.",
        ("vector mask value model", "pandas receiver/domain proof"),
    ),
    Candidate(
        8,
        "python.pandas_fillna_defaults",
        "pandas fillna/defaulting",
        "nose.python.ecosystem.pandas.fillna_defaults",
        "pandas",
        "Series/DataFrame fillna and defaulting slices",
        "deferred",
        (
            pat("pandas_fillna", "python", r"\.\s*fillna\s*\("),
        ),
        "Medium where pandas appears, but refactoring value needs manual confirmation.",
        "Very high: index alignment, inplace mutation, dtype coercion, NA semantics, and method options change behavior.",
        "Current map/default and nullish-default facts are scalar/map, not pandas vector operations.",
        ("map/default vocabulary", "nullish default boundary lessons"),
        ("pandas receiver proof", "no inplace mutation", "dtype/NA policy", "axis/method arguments absent"),
        ("inplace=True", "method/limit/axis arguments", "DataFrame alignment", "nullable dtypes", "view/copy boundary"),
        ("DataFrame-wide fill", "groupby fill", "interpolation-like forms"),
        "Keep as pricing-only until pandas value substrate exists.",
        "No exact row; reject or route to near-only if evidence remains judgment-deep.",
        "priced-but-blocked",
        "Record pandas value-substrate blocker.",
        ("pandas value model", "mutation/view-copy proof"),
    ),
    Candidate(
        9,
        "javascript.rxjs_observable_identity_adapters",
        "RxJS Observable identity adapters",
        "nose.javascript.ecosystem.rxjs.observable_identity_adapters",
        "RxJS",
        "identity-like pipe/map/filter slices",
        "deferred",
        (
            pat(
                "rxjs_identity_map_js",
                "javascript",
                r"\.\s*pipe\s*\(\s*map\s*\(\s*\(?\s*([A-Za-z_$][\w$]*)\s*\)?\s*=>\s*\1\s*\)",
                context=r"rxjs|Observable|@trpc/server/observable",
            ),
            pat(
                "rxjs_identity_map_ts",
                "typescript",
                r"\.\s*pipe\s*\(\s*map\s*\(\s*\(?\s*([A-Za-z_$][\w$]*)\s*\)?\s*=>\s*\1\s*\)",
                context=r"rxjs|Observable|@trpc/server/observable",
            ),
            pat(
                "rxjs_filter_boolean_ts",
                "typescript",
                r"\.\s*pipe\s*\(\s*filter\s*\(\s*Boolean\s*\)",
                context=r"rxjs|Observable|@trpc/server/observable",
            ),
        ),
        "Medium-high in reactive TypeScript repos.",
        "High: scheduler, subscription timing, hot/cold streams, completion, error, and cancellation are semantic boundaries.",
        "Current promise/async facts do not model Observable demand/effect.",
        ("demand/effect vocabulary", "JS/TS import proof"),
        ("rxjs import identity", "Observable receiver proof", "operator identity", "scheduler/error/completion profile"),
        ("hot vs cold stream", "scheduler argument", "side-effecting operator", "error/completion changes", "custom Observable"),
        ("mergeMap/switchMap", "share/retry/tap", "subscription side effects"),
        "Likely near-only before exact; product query-regression only after demand/effect contract is narrow.",
        "Keep pack builtin-optional and exact rows disabled by default.",
        "priced-but-blocked",
        "Record Observable demand/effect blocker.",
        ("Observable demand/effect substrate", "scheduler boundary"),
    ),
    Candidate(
        10,
        "javascript.rxjs_projection_operators",
        "RxJS projection operators",
        "nose.javascript.ecosystem.rxjs.projection_operators",
        "RxJS",
        "map/filter/take projection and predicate operators inside pipe",
        "deferred",
        (
            pat(
                "rxjs_pipe_operator_js",
                "javascript",
                r"\.\s*pipe\s*\([\s\S]{0,240}\b(?:map|filter|take)\s*\(",
                context=r"from\s+['\"]rxjs|from\s+['\"]rxjs/operators|Observable|@trpc/server/observable",
            ),
            pat(
                "rxjs_pipe_operator_ts",
                "typescript",
                r"\.\s*pipe\s*\([\s\S]{0,240}\b(?:map|filter|take)\s*\(",
                context=r"from\s+['\"]rxjs|from\s+['\"]rxjs/operators|Observable|@trpc/server/observable",
            ),
        ),
        "Medium where RxJS/trpc-style reactive code is present.",
        "High: callback effects and stream lifecycle make array-HOF analogies unsafe.",
        "No current exact Observable operator contract.",
        ("callback demand/effect vocabulary",),
        ("operator import identity", "Observable receiver proof", "callback effect profile", "lifecycle preservation"),
        ("tap side effects", "switchMap/mergeMap flattening", "throwError/catchError", "scheduler-dependent timing"),
        ("higher-order streams", "multicast/share", "custom pipe implementation"),
        "Start with target packet only if two real misses share one lifecycle invariant.",
        "Disable exact row; route to blocked proof prerequisite.",
        "priced-but-blocked",
        "Record lifecycle/callback blocker.",
        ("stream lifecycle proof", "callback effect proof"),
    ),
    Candidate(
        11,
        "rust.tokio_future_identity_adapters",
        "Tokio task/runtime boundary signal",
        "nose.rust.ecosystem.tokio.future_identity_adapters",
        "Tokio",
        "tokio::spawn/select/join/test boundaries as non-equivalence corpus signal",
        "deferred",
        (
            pat("tokio_spawn", "rust", r"\btokio::(?:spawn|select!|join!)\b|#\s*\[\s*tokio::", context=r"tokio"),
        ),
        "High prevalence in async Rust repos, but this is runtime-boundary prevalence rather than row-sized semantic-equivalence impact.",
        "High: scheduler, cancellation, pinning, wake order, and side effects are unmodeled.",
        "Current async/future facts do not treat Tokio task/runtime boundaries as exact semantic evidence.",
        ("Rust iterator identity-adapter pattern", "demand/effect vocabulary"),
        ("tokio import identity", "Future receiver proof", "no cancellation-visible difference", "effect order"),
        ("spawn boundary", "select race", "timeout/cancellation", "side-effecting future", "pin/projection boundary"),
        ("task spawning", "channels", "timers", "runtime handles"),
        "Do not product-run until this is reframed as one concrete adapter row with cancellation proof.",
        "No row to disable; reject the broad runtime-boundary candidate.",
        "unpriced",
        "Reject this broad Tokio runtime-boundary signal; re-open only after a concrete adapter row is isolated.",
        ("Future demand/effect substrate", "cancellation semantics"),
        rejection_context="The matcher prices Tokio spawn/test/runtime boundaries, not identity adapter equivalence.",
    ),
    Candidate(
        12,
        "rust.tokio_stream_projection",
        "Tokio stream projection adapters",
        "nose.rust.ecosystem.tokio.stream_projection",
        "Tokio",
        "StreamExt map/filter/then identity or projection slices",
        "deferred",
        (
            pat(
                "tokio_stream_ext",
                "rust",
                r"\.\s*(?:then|filter_map)\s*\(",
                context=r"StreamExt|tokio_stream|futures::(?:Stream|stream)",
            ),
        ),
        "Medium if stream-heavy repos expose repeated projection idioms.",
        "High: poll order, pending, cancellation, and effectful closures differ from iterator semantics.",
        "Current Rust iterator adapters are exact only for iterator identity, not async streams.",
        ("Rust iterator adapter contracts", "callback effect vocabulary"),
        ("StreamExt import identity", "stream receiver proof", "poll/cancellation profile", "closure effect profile"),
        ("effectful closure", "then async boundary", "filter_map absence semantics", "custom map method"),
        ("channels", "timers", "backpressure", "multicast streams"),
        "Use focused fixtures first; no broad corpus product run until stream substrate exists.",
        "Route to proof prerequisite; no default pack row.",
        "unpriced",
        "Reject current signal until receiver-proximate StreamExt calls can be isolated.",
        ("stream poll semantics", "closure effect proof"),
        rejection_context="Current corpus signal is too easy to pollute with non-stream Option/Result/Iterator calls.",
    ),
    Candidate(
        13,
        "ruby.rails_active_support_presence",
        "Rails ActiveSupport presence helpers",
        "nose.ruby.ecosystem.rails.active_support_presence",
        "Rails ActiveSupport",
        "present?/blank?/presence slices",
        "deferred",
        (
            pat("rails_presence", "ruby", r"\.\s*(?:present\?|blank\?|presence)\b"),
        ),
        "Medium in Rails repos; useful only when receiver classes are constrained.",
        "High: monkey patching, nil/empty/string whitespace behavior, and receiver class differ by ActiveSupport version.",
        "Current Ruby nil/empty facts do not include ActiveSupport monkey-patched semantics.",
        ("Ruby nil/empty predicates", "receiver-domain proof"),
        ("ActiveSupport loaded", "receiver class/domain", "version policy", "blank whitespace semantics"),
        ("custom present?/blank?", "non-ActiveSupport environment", "String whitespace", "relation/query receivers"),
        ("Rails relation presence", "lazy DB queries", "monkey-patched custom classes"),
        "Measure after a specific receiver class and version policy are scoped.",
        "Keep external/optional; disable row on monkey-patch ambiguity.",
        "priced-but-blocked",
        "Record monkey-patch/version blocker.",
        ("ActiveSupport version proof", "receiver class proof"),
    ),
    Candidate(
        14,
        "ruby.rails_collection_helpers",
        "Rails collection helper slices",
        "nose.ruby.ecosystem.rails.collection_helpers",
        "Rails ActiveSupport",
        "many?/exclude?/including?/excluding? collection helper rows",
        "deferred",
        (
            pat("rails_collection_helpers", "ruby", r"\.\s*(?:many\?|exclude\?|including|excluding)\b"),
        ),
        "Low-medium unless repeated in Rails-heavy held-out repos.",
        "High: monkey-patched methods and ActiveRecord relation laziness make exact support risky.",
        "Current Ruby collection membership/factory facts do not model ActiveSupport helpers.",
        ("Ruby collection membership", "receiver-domain proof"),
        ("ActiveSupport loaded", "receiver collection class", "non-lazy receiver", "method semantics"),
        ("ActiveRecord::Relation", "custom helper method", "lazy query", "mutating helper behavior"),
        ("database-backed relations", "scope chains", "monkey patches"),
        "Treat as pricing-only until real repeated miss evidence appears.",
        "Reject exact row if sample value is judgment-deep or DB-backed.",
        "unpriced",
        "Record rejection unless corpus shows repeated non-DB collection misses.",
        rejection_context="Likely broad Rails semantics without a narrow non-lazy row boundary.",
    ),
    Candidate(
        15,
        "python.pathlib_path_predicates",
        "pathlib Path predicates",
        "nose.python.stdlib.pathlib.predicates",
        "Python stdlib",
        "Path.exists/is_file/is_dir style predicates",
        "deferred",
        (
            pat(
                "pathlib_predicates",
                "python",
                r"\.\s*(?:exists|is_file|is_dir|is_absolute)\s*\(\s*\)",
                context=r"from pathlib import Path|import pathlib|\bPath\s*\(",
            ),
        ),
        "Medium prevalence in Python repos, but many uses are filesystem effects rather than refactoring equivalence.",
        "High: filesystem state and symlink/error behavior are external effects.",
        "Current exact semantic channel is not allowed to depend on filesystem state.",
        ("effect vocabulary", "Python import proof"),
        ("pathlib import identity", "pure path-only predicate subset", "filesystem-effect exclusion"),
        ("exists/is_file/is_dir hit filesystem", "symlink behavior", "permissions/errors", "shadowed Path"),
        ("runtime filesystem predicates", "glob/iterdir", "resolve/stat"),
        "No product query-regression until a pure path-syntax row is separated.",
        "Keep unpriced; exact support likely inappropriate for stateful predicates.",
        "unpriced",
        "Reject stateful predicate rows; maybe split pure path normalization later.",
        rejection_context="Observed Path usage does not imply a pure semantic row.",
    ),
    Candidate(
        16,
        "python.itertools_chain_factories",
        "Python itertools chain factories",
        "nose.python.stdlib.itertools.chain",
        "Python stdlib",
        "itertools.chain and chain.from_iterable identity/flattening slices",
        "deferred",
        (
            pat("itertools_chain", "python", r"\bitertools\s*\.\s*chain\s*(?:\.from_iterable)?\s*\("),
            pat(
                "imported_chain",
                "python",
                r"(?<!\.)\bchain\s*(?:\.from_iterable)?\s*\(",
                context=r"from itertools import .*chain",
            ),
        ),
        "Medium in utility-heavy Python repos; may recover collection flattening idioms.",
        "Medium-high: iterator laziness, one-shot iterators, and consumption side effects are demand-sensitive.",
        "Current collection factories and iterator identity adapters do not model Python itertools laziness.",
        ("demand/effect semantics", "Python import proof", "iterator protocol vocabulary"),
        ("itertools import identity", "iterator demand profile", "no duplicated consumption", "effect-free source iteration"),
        ("generator side effects", "one-shot iterator reuse", "shadowed chain", "mixed strings/bytes flattening"),
        ("lazy infinite iterators", "tee/groupby", "custom chain"),
        "Use focused fixtures and HoF/demand budget before corpus product run.",
        "Disable exact row if one-shot iterator demand cannot be proven.",
        "priced-but-blocked",
        "Record laziness/demand blocker.",
        ("iterator demand proof", "source iteration effect proof"),
    ),
    Candidate(
        17,
        "javascript.node_path_normalization",
        "Node path normalization helpers",
        "nose.javascript.ecosystem.node.path_normalization",
        "Node.js stdlib",
        "path.join/resolve/normalize slices",
        "deferred",
        (
            pat("node_path_js", "javascript", r"\bpath\s*\.\s*(?:join|resolve|normalize)\s*\("),
            pat("node_path_ts", "typescript", r"\bpath\s*\.\s*(?:join|resolve|normalize)\s*\("),
        ),
        "Medium in tooling repos, but semantic equality is platform-sensitive.",
        "High: OS path separator, cwd, drive letters, symlinks, and absolute path rules affect behavior.",
        "Current JS semantic packs do not model Node platform path semantics.",
        ("JS import proof",),
        ("node:path import identity", "platform policy", "argument literal/domain proof", "cwd independence for resolve"),
        ("Windows vs POSIX", "absolute path reset", "cwd-dependent resolve", "URL/fileURL conversion", "shadowed path"),
        ("filesystem resolution", "symlinks", "process cwd"),
        "Do not product-run until a platform-independent literal-only slice exists.",
        "Keep unpriced unless pure literal normalization proves useful.",
        "unpriced",
        "Reject broad Node path row; split literal POSIX-only candidate if evidence appears.",
        rejection_context="Platform/cwd sensitivity dominates current value.",
    ),
    Candidate(
        18,
        "go.stdlib.maps_helpers",
        "Go stdlib maps helpers",
        "nose.go.stdlib.maps_helpers",
        "Go stdlib",
        "maps.Clone/Keys/Values/Equal slices",
        "deferred",
        (
            pat("go_maps_helpers", "go", r"\bmaps\s*\.\s*(?:Clone|Keys|Values|Equal)\s*\("),
        ),
        "Medium: Go map helpers can map to existing map/key-view protocols when present.",
        "Medium-high: Go version policy, map iteration order, comparability, and mutation boundaries matter.",
        "Current Go namespace calls cover selected fmt/strings/slices, not maps helpers.",
        ("Go namespace import proof", "map key-view protocol", "map factory/access vocabulary"),
        ("maps import identity", "Go version policy", "map receiver proof", "order-insensitive result handling"),
        ("custom maps package", "non-comparable values for Equal", "mutation during iteration", "order-sensitive Keys/Values consumer"),
        ("iterator order dependent code", "generic type constraints beyond proof"),
        "Run focused Go fixtures first, then query-regression on Go subset.",
        "Disable Keys/Values rows if order-insensitive use cannot be proven.",
        "priced-but-blocked",
        "Record Go version/order blocker.",
        ("Go version proof", "map iteration order boundary"),
    ),
    Candidate(
        19,
        "java.apache_commons_lang_string_utils",
        "Apache Commons Lang StringUtils predicates",
        "nose.java.ecosystem.commons_lang.string_utils",
        "Apache Commons Lang",
        "StringUtils.isBlank/isEmpty/defaultString/equals style rows",
        "builtin-optional candidate",
        (
            pat(
                "commons_string_utils",
                "java",
                r"\bStringUtils\s*\.\s*(?:isBlank|isEmpty|isNotBlank|isNotEmpty|defaultString|equals)\s*\(",
                context=r"org\.apache\.commons\.lang3\.StringUtils|package\s+org\.apache\.commons\.lang3",
            ),
        ),
        "Medium potential, but current pinned corpus only shows own-source dev occurrences; external consumer breadth is missing.",
        "Medium: package/import identity and null/whitespace semantics are precise but need hard negatives.",
        "Current Java string/null facts do not admit Apache Commons selectors.",
        ("Java import/static-import proof", "string prefix/null/default vocabulary"),
        ("org.apache.commons.lang3.StringUtils identity", "method semantics", "arity", "null/whitespace domain"),
        ("project-local StringUtils", "Junit/platform StringUtils", "wrong commons package/version", "locale/case-sensitive helpers"),
        ("strip/accent/case helpers", "CharSequence subclass edge cases"),
        "Collect external consumer or held-out usage, then run Java subset query-regression and focused StringUtils hard negatives.",
        "Keep absent until external usage proves this is more than own-source library internals.",
        "priced-but-blocked",
        "Require external consumer or held-out corpus evidence before target packet.",
        ("external consumer corpus evidence", "candidate-specific current miss/query evidence"),
    ),
    Candidate(
        20,
        "rust.itertools_collect_vec",
        "Rust itertools collect_vec helpers",
        "nose.rust.ecosystem.itertools.collect_vec",
        "Rust itertools",
        "Itertools::collect_vec and selected identity adapters",
        "deferred",
        (
            pat("rust_collect_vec", "rust", r"\.\s*collect_vec\s*\(\s*\)", context=r"itertools::Itertools|collect_vec"),
        ),
        "Low-medium: actual collect_vec calls appear in a few repos; imports alone are not counted as row occurrences.",
        "Medium-high: trait import, adapter chain demand, allocation, and custom trait ambiguity need proof.",
        "Current Rust std iterator identity adapters do not include external itertools trait methods.",
        ("Rust iterator identity adapters", "trait/import proof"),
        ("itertools trait import identity", "iterator receiver proof", "adapter semantics", "allocation-insensitive exact value"),
        ("custom collect_vec trait", "effectful iterator", "consumption order", "non-Vec collection semantics"),
        ("multi_cartesian_product", "grouping/chunking", "peeking_take_while"),
        "Use Rust focused fixtures, then Rust subset query-regression if corpus signal prices.",
        "Keep builtin-optional or blocked until trait identity is dependency-backed.",
        "priced-but-blocked",
        "Record trait/import proof blocker.",
        ("Rust trait identity proof", "iterator consumption effect proof"),
    ),
)


def load_corpus(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def corpus_digest(corpus: dict) -> str:
    entries = [
        {
            "id": repo["id"],
            "split": repo["split"],
            "primary_language": repo["primary_language"],
            "commit": repo["commit"],
        }
        for repo in corpus["repositories"]
    ]
    data = json.dumps(entries, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(data).hexdigest()[:16]


def candidate_signature(candidates: Iterable[Candidate]) -> str:
    payload = [
        {
            "candidate_id": c.candidate_id,
            "title": c.title,
            "proposed_pack_id": c.proposed_pack_id,
            "ecosystem": c.ecosystem,
            "row_slice": c.row_slice,
            "target_lane": c.target_lane,
            "patterns": [
                (
                    p.pattern_id,
                    p.lang,
                    p.regex.pattern,
                    p.precision,
                    p.context.pattern if p.context is not None else None,
                )
                for p in c.patterns
            ],
            "impact_price": c.impact_price,
            "safety_price": c.safety_price,
            "current_detector_result": c.current_detector_result,
            "existing_vocabulary": c.existing_vocabulary,
            "required_evidence": c.required_evidence,
            "hard_negatives": c.hard_negatives,
            "unsupported_cases": c.unsupported_cases,
            "product_runtime_plan": c.product_runtime_plan,
            "rollback_path": c.rollback_path,
            "initial_verdict": c.initial_verdict,
            "next_action": c.next_action,
            "blocked_by": c.blocked_by,
            "rejection_context": c.rejection_context,
        }
        for c in candidates
    ]
    data = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(data).hexdigest()[:16]


def report_path(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def iter_source_files(repo_root: Path) -> Iterable[tuple[Path, str]]:
    for path in sorted(repo_root.rglob("*")):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        lang = LANG_BY_EXT.get(path.suffix.lower())
        if lang is None:
            continue
        try:
            if path.stat().st_size > MAX_FILE_BYTES:
                continue
        except OSError:
            continue
        yield path, lang


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def scan_repo(candidate: Candidate, repo: dict, repo_root: Path) -> dict:
    raw_occurrences = 0
    weighted_occurrences = 0.0
    pattern_counts = {pattern.pattern_id: 0 for pattern in candidate.patterns}
    samples = []
    lang_patterns: dict[str, list[PatternSpec]] = {}
    for pattern in candidate.patterns:
        lang_patterns.setdefault(pattern.lang, []).append(pattern)

    if not repo_root.exists():
        return {
            "repo_id": repo["id"],
            "split": repo["split"],
            "primary_language": repo["primary_language"],
            "raw_occurrences": 0,
            "weighted_occurrences": 0.0,
            "pattern_counts": pattern_counts,
            "samples": [],
            "present": False,
            "missing_repo_checkout": True,
        }

    for path, lang in iter_source_files(repo_root):
        patterns = lang_patterns.get(lang)
        if not patterns:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        for pattern in patterns:
            if pattern.context is not None and pattern.context.search(text) is None:
                continue
            matches = list(pattern.regex.finditer(text))
            if not matches:
                continue
            count = len(matches)
            raw_occurrences += count
            pattern_counts[pattern.pattern_id] += count
            weighted_occurrences += count if pattern.precision == "high" else count * 0.5
            for match in matches:
                if len(samples) >= MAX_SAMPLES_PER_CANDIDATE:
                    break
                start = max(0, match.start() - 80)
                end = min(len(text), match.end() + 80)
                samples.append(
                    {
                        "repo": repo["id"],
                        "path": str(path.relative_to(repo_root)),
                        "line": line_number(text, match.start()),
                        "pattern_id": pattern.pattern_id,
                        "snippet": " ".join(text[start:end].strip().split()),
                    }
                )
    return {
        "repo_id": repo["id"],
        "split": repo["split"],
        "primary_language": repo["primary_language"],
        "raw_occurrences": raw_occurrences,
        "weighted_occurrences": round(weighted_occurrences, 2),
        "pattern_counts": pattern_counts,
        "samples": samples,
        "present": raw_occurrences > 0,
        "missing_repo_checkout": False,
    }


def verdict_for(candidate: Candidate, repo_presence: int) -> tuple[str, str]:
    if candidate.initial_verdict not in VERDICTS:
        raise ValueError(f"unknown verdict for {candidate.candidate_id}: {candidate.initial_verdict}")
    if repo_presence == 0:
        return (
            "unpriced",
            "No corpus presence in the available pinned checkouts; retain only as seed-list context.",
        )
    if candidate.initial_verdict == "priced-ready":
        return candidate.initial_verdict, "Corpus signal plus existing vocabulary justify a target packet before implementation."
    if candidate.initial_verdict == "priced-but-blocked":
        return candidate.initial_verdict, "Impact exists, but exact influence is blocked by the recorded proof/evidence prerequisite."
    return candidate.initial_verdict, candidate.rejection_context or "Corpus signal does not yet justify a sound row boundary."


def next_action_for(candidate: Candidate, verdict: str, repo_presence: int) -> str:
    if verdict == "unpriced" and repo_presence == 0:
        return "Retain only as seed-list context until corpus evidence appears."
    return candidate.next_action


def target_packet_for(candidate: Candidate, verdict: str) -> dict | None:
    if verdict != "priced-ready":
        return None
    return {
        "packet_id": f"semantic-pack-{candidate.candidate_id.replace('.', '-')}",
        "pack_id": candidate.proposed_pack_id,
        "semantic_claim": candidate.row_slice,
        "required_evidence": list(candidate.required_evidence),
        "unsupported_cases": list(candidate.unsupported_cases),
        "hard_negative_siblings": list(candidate.hard_negatives),
        "product_output_measurement": candidate.product_runtime_plan,
        "runtime_measurement": "Before implementation, time the candidate query-regression subset before/after the row and keep the row disabled if runtime drift is unexplained.",
        "rollback_path": candidate.rollback_path,
        "implementation_boundary": "Do not implement a broad ecosystem pack; implement only this row slice.",
    }


def location_covers_sample(location: dict, sample: dict, repo_root: Path) -> bool:
    file_name = location.get("file")
    if not isinstance(file_name, str):
        return False
    sample_path = sample["path"]
    if not (
        file_name.endswith(sample_path)
        or file_name.endswith(f"{repo_root.name}/{sample_path}")
        or file_name.endswith(f"/{repo_root.name}/{sample_path}")
    ):
        return False
    start = location.get("start")
    end = location.get("end")
    line = sample.get("line")
    if isinstance(start, int) and isinstance(end, int) and isinstance(line, int):
        return start <= line <= end
    return True


def candidate_sample_query_hits(repo_root: Path, families: list, samples: list[dict]) -> dict:
    checked = []
    all_family_ids = set()
    for sample in samples[:3]:
        family_ids = []
        for family in families:
            if not isinstance(family, dict):
                continue
            locations = family.get("locations", [])
            if not isinstance(locations, list):
                continue
            if any(
                isinstance(location, dict) and location_covers_sample(location, sample, repo_root)
                for location in locations
            ):
                family_id = family.get("id")
                if isinstance(family_id, str):
                    family_ids.append(family_id)
                    all_family_ids.add(family_id)
        checked.append(
            {
                "path": sample["path"],
                "line": sample["line"],
                "pattern_id": sample["pattern_id"],
                "families_covering_line": len(family_ids),
                "family_ids": sorted(family_ids)[:5],
            }
        )
    return {
        "samples_checked": len(checked),
        "samples_with_current_family_hit": sum(1 for sample in checked if sample["families_covering_line"] > 0),
        "families_covering_sample_lines": len(all_family_ids),
        "sample_hits": checked,
        "note": "Candidate-specific overlay: counts current semantic query families whose location spans include sampled candidate lines.",
    }


def render_product_query(base_result: dict, repo_root: Path, samples: list[dict]) -> dict:
    result = {key: value for key, value in base_result.items() if key != "_families"}
    families = base_result.get("_families", [])
    if base_result.get("status") == "ok" and isinstance(families, list):
        result["candidate_sample_query_hits"] = candidate_sample_query_hits(repo_root, families, samples)
    return result


def run_product_query(nose: Path, repo_root: Path, samples: list[dict], cache: dict[str, dict]) -> dict:
    cache_key = str(repo_root)
    if cache_key in cache:
        return render_product_query(cache[cache_key], repo_root, samples)
    nose = nose.resolve()
    display_command = "nose query . all top=0 --mode semantic --format json"
    command = [
        str(nose),
        "query",
        ".",
        "all",
        "top=0",
        "--mode",
        "semantic",
        "--format",
        "json",
    ]
    try:
        completed = subprocess.run(
            command,
            cwd=repo_root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=120,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as err:
        result = {
            "repo": repo_root.name,
            "status": "error",
            "command": display_command,
            "error": str(err),
        }
        cache[cache_key] = result
        return render_product_query(result, repo_root, samples)
    if completed.returncode != 0:
        result = {
            "repo": repo_root.name,
            "status": "error",
            "command": display_command,
            "stderr": completed.stderr[-2000:],
        }
        cache[cache_key] = result
        return render_product_query(result, repo_root, samples)
    try:
        parsed = json.loads(completed.stdout)
    except json.JSONDecodeError as err:
        result = {
            "repo": repo_root.name,
            "status": "error",
            "command": display_command,
            "error": f"invalid JSON: {err}",
        }
        cache[cache_key] = result
        return render_product_query(result, repo_root, samples)
    families = parsed.get("families", [])
    semantic_packs = parsed.get("semantic_packs", [])
    semantic_pack_ids = []
    if isinstance(semantic_packs, list):
        semantic_pack_ids = sorted(
            pack["id"]
            for pack in semantic_packs
            if isinstance(pack, dict) and isinstance(pack.get("id"), str)
        )
    result = {
        "repo": repo_root.name,
        "status": "ok",
        "command": display_command,
        "total_families": parsed.get("total_families", len(families)),
        "shown_families": parsed.get("shown_families", len(families)),
        "tool": parsed.get("tool"),
        "semantic_packs": len(semantic_packs) if isinstance(semantic_packs, list) else None,
        "semantic_pack_ids": semantic_pack_ids,
        "_families": families if isinstance(families, list) else [],
        "note": "Product query summary plus candidate sample-line overlap; query-regression remains required before implementation.",
    }
    cache[cache_key] = result
    return render_product_query(result, repo_root, samples)


def sample_product_queries(
    nose: Path | None,
    repos_root: Path,
    present_results: list[dict],
    sample_limit: int,
    cache: dict[str, dict],
) -> list[dict]:
    if nose is None or sample_limit <= 0:
        return []
    queries = []
    for result in present_results[:sample_limit]:
        repo_root = repos_root / result["repo_id"]
        if repo_root.exists():
            queries.append(run_product_query(nose, repo_root, result["samples"], cache))
    return queries


def price_candidates(
    corpus: dict,
    repos_root: Path,
    *,
    corpus_path: Path = DEFAULT_CORPUS,
    nose: Path | None = None,
    query_sample_repos: int = 0,
) -> dict:
    repos = corpus["repositories"]
    records = []
    query_cache: dict[str, dict] = {}
    for candidate in CANDIDATES:
        repo_results = [
            scan_repo(candidate, repo, repos_root / repo["id"])
            for repo in repos
        ]
        present = [result for result in repo_results if result["present"]]
        dev_present = sorted(result["repo_id"] for result in present if result["split"] == "dev")
        heldout_present = sorted(result["repo_id"] for result in present if result["split"] == "heldout")
        primary_languages = sorted({result["primary_language"] for result in present})
        raw_occurrences = sum(result["raw_occurrences"] for result in present)
        weighted_occurrences = round(sum(result["weighted_occurrences"] for result in present), 2)
        pattern_counts: dict[str, int] = {}
        for result in repo_results:
            for pattern_id, count in result["pattern_counts"].items():
                pattern_counts[pattern_id] = pattern_counts.get(pattern_id, 0) + count
        samples = []
        for result in present:
            for sample in result["samples"]:
                if len(samples) < MAX_SAMPLES_PER_CANDIDATE:
                    samples.append(sample)
        verdict, verdict_reason = verdict_for(candidate, len(present))
        blocked_by = list(candidate.blocked_by if verdict == "priced-but-blocked" else ())
        rejection_context = ""
        if verdict == "unpriced":
            rejection_context = verdict_reason
        product_queries = sample_product_queries(
            nose,
            repos_root,
            present,
            query_sample_repos,
            query_cache,
        )
        query_method = (
            "sample product query overlay plus existing semantic-pack inventory knowledge"
            if product_queries
            else "static pricing overlay plus existing semantic-pack inventory knowledge"
        )
        candidate_pack_observed = any(
            candidate.proposed_pack_id is not None
            and query.get("status") == "ok"
            and candidate.proposed_pack_id in query.get("semantic_pack_ids", [])
            for query in product_queries
        )
        records.append(
            {
                "iteration": candidate.iteration,
                "candidate_id": candidate.candidate_id,
                "title": candidate.title,
                "proposed_pack_id": candidate.proposed_pack_id,
                "ecosystem": candidate.ecosystem,
                "row_slice": candidate.row_slice,
                "target_lane": candidate.target_lane,
                "evidence_tier": "manual-pricing" if verdict == "priced-ready" else "queue-signal",
                "corpus_signal": {
                    "repo_breadth": len(present),
                    "dev_repo_breadth": len(dev_present),
                    "heldout_repo_breadth": len(heldout_present),
                    "primary_language_breadth": len(primary_languages),
                    "raw_occurrences": raw_occurrences,
                    "weighted_occurrences": weighted_occurrences,
                    "present_repos": sorted(result["repo_id"] for result in present),
                    "dev_repos": dev_present,
                    "heldout_repos": heldout_present,
                    "primary_languages": primary_languages,
                    "pattern_counts": dict(sorted(pattern_counts.items())),
                    "samples": samples,
                },
                "current_detector_query_result": {
                    "method": query_method,
                    "summary": candidate.current_detector_result,
                    "product_query_command": "nose query <repo> all top=0 --mode semantic --format json",
                    "sample_product_queries": product_queries,
                    "candidate_pack_observed_in_sample_queries": candidate_pack_observed,
                    "note": "Sample product queries record current semantic-pack inventory and candidate sample-line overlap; full query-regression remains required before implementation.",
                },
                "impact_price": candidate.impact_price,
                "safety_price": candidate.safety_price,
                "existing_vocabulary": list(candidate.existing_vocabulary),
                "required_evidence": list(candidate.required_evidence),
                "hard_negative_siblings": list(candidate.hard_negatives),
                "unsupported_cases": list(candidate.unsupported_cases),
                "verdict": verdict,
                "verdict_reason": verdict_reason,
                "blocked_by": blocked_by,
                "rejection_context": rejection_context,
                "next_action": next_action_for(candidate, verdict, len(present)),
                "target_packet": target_packet_for(candidate, verdict),
            }
        )

    totals = {
        "iterations": len(records),
        "priced_ready": sum(1 for record in records if record["verdict"] == "priced-ready"),
        "priced_but_blocked": sum(1 for record in records if record["verdict"] == "priced-but-blocked"),
        "unpriced": sum(1 for record in records if record["verdict"] == "unpriced"),
        "repos_available": sum(1 for repo in repos if (repos_root / repo["id"]).exists()),
        "repos_total": len(repos),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "tool_version": TOOL_VERSION,
        "corpus": {
            "path": report_path(corpus_path),
            "digest": corpus_digest(corpus),
            "repos_root": report_path(repos_root),
        },
        "candidate_signature": candidate_signature(CANDIDATES),
        "policy": {
            "corpus_signal": "queue-signal-not-proof",
            "unit_of_work": "narrow-row-slice",
            "required_iterations": 20,
            "verdicts": sorted(VERDICTS),
            "promotion_rule": "Only priced-ready rows may become target packets before implementation.",
            "product_query_overlay": "sample summaries are context only; query-regression remains required before implementation",
        },
        "totals": totals,
        "iterations": records,
    }


def render_markdown(report: dict) -> str:
    lines = [
        "# Semantic-pack candidate pricing",
        "",
        "Generated by `bench/semantic_pack/pricing.py`. Corpus matches are queue signals, not proof.",
        "",
        "## Summary",
        "",
        f"- Iterations: {report['totals']['iterations']}",
        f"- Priced ready: {report['totals']['priced_ready']}",
        f"- Priced but blocked: {report['totals']['priced_but_blocked']}",
        f"- Unpriced: {report['totals']['unpriced']}",
        f"- Repos available: {report['totals']['repos_available']} / {report['totals']['repos_total']}",
        f"- Corpus digest: `{report['corpus']['digest']}`",
        f"- Candidate signature: `{report['candidate_signature']}`",
        "",
        "## Iterations",
        "",
        "| # | candidate | verdict | repos | heldout | next action |",
        "|---:|---|---|---:|---:|---|",
    ]
    for record in report["iterations"]:
        lines.append(
            "| {iteration} | `{candidate_id}` | {verdict} | {repos} | {heldout} | {next_action} |".format(
                iteration=record["iteration"],
                candidate_id=record["candidate_id"],
                verdict=record["verdict"],
                repos=record["corpus_signal"]["repo_breadth"],
                heldout=record["corpus_signal"]["heldout_repo_breadth"],
                next_action=record["next_action"],
            )
        )
    lines.extend(["", "## Details", ""])
    for record in report["iterations"]:
        lines.extend(
            [
                f"### {record['iteration']}. `{record['candidate_id']}`",
                "",
                f"- Verdict: `{record['verdict']}` — {record['verdict_reason']}",
                f"- Proposed pack: `{record['proposed_pack_id']}`",
                f"- Row slice: {record['row_slice']}",
                f"- Corpus signal: {record['corpus_signal']['repo_breadth']} repos, "
                f"{record['corpus_signal']['heldout_repo_breadth']} held-out repos, "
                f"{record['corpus_signal']['raw_occurrences']} raw occurrences",
                f"- Current detector/query result: {record['current_detector_query_result']['summary']}",
                f"- Product query sample: {record['current_detector_query_result']['method']}; "
                f"candidate pack observed: "
                f"{record['current_detector_query_result']['candidate_pack_observed_in_sample_queries']}",
                f"- Impact: {record['impact_price']}",
                f"- Safety: {record['safety_price']}",
                f"- Hard negatives: {', '.join(record['hard_negative_siblings'])}",
            ]
        )
        if record["blocked_by"]:
            lines.append(f"- Blocked by: {', '.join(record['blocked_by'])}")
        if record["rejection_context"]:
            lines.append(f"- Rejection context: {record['rejection_context']}")
        samples = record["corpus_signal"]["samples"][:3]
        if samples:
            lines.append("- Samples:")
            for sample in samples:
                lines.append(
                    f"  - `{sample['repo']}:{sample['path']}:{sample['line']}` "
                    f"({sample['pattern_id']}) — {sample['snippet']}"
                )
        lines.append("")
    return "\n".join(lines).rstrip()


def validate_report(report: dict) -> None:
    if report["totals"]["iterations"] != 20:
        raise SystemExit("pricing report must contain exactly 20 iterations")
    seen = set()
    for record in report["iterations"]:
        candidate_id = record["candidate_id"]
        if candidate_id in seen:
            raise SystemExit(f"duplicate candidate id: {candidate_id}")
        seen.add(candidate_id)
        verdict = record["verdict"]
        if verdict not in VERDICTS:
            raise SystemExit(f"{candidate_id}: invalid verdict {verdict}")
        required = [
            "candidate_id",
            "proposed_pack_id",
            "corpus_signal",
            "current_detector_query_result",
            "impact_price",
            "safety_price",
            "hard_negative_siblings",
            "verdict",
            "next_action",
        ]
        for key in required:
            if key not in record:
                raise SystemExit(f"{candidate_id}: missing {key}")
        if verdict == "priced-ready" and record["target_packet"] is None:
            raise SystemExit(f"{candidate_id}: priced-ready candidate needs target packet")
        if verdict == "priced-but-blocked" and not record["blocked_by"]:
            raise SystemExit(f"{candidate_id}: priced-but-blocked candidate needs blocker")
        if verdict == "unpriced" and not record["rejection_context"]:
            raise SystemExit(f"{candidate_id}: unpriced candidate needs rejection context")


def validate_loop_reviews(report: dict, reviews: dict) -> None:
    if reviews.get("schema_version") != 1:
        raise SystemExit("loop review record must use schema_version 1")
    review_iterations = reviews.get("iterations")
    if not isinstance(review_iterations, list):
        raise SystemExit("loop review record needs iterations list")
    if len(review_iterations) != report["totals"]["iterations"]:
        raise SystemExit("loop review record must cover every pricing iteration")
    for pricing_record, review_record in zip(report["iterations"], review_iterations):
        candidate_id = pricing_record["candidate_id"]
        if review_record.get("iteration") != pricing_record["iteration"]:
            raise SystemExit(f"{candidate_id}: review iteration mismatch")
        if review_record.get("candidate_id") != candidate_id:
            raise SystemExit(f"{candidate_id}: review candidate mismatch")
        review_entries = review_record.get("reviews")
        if not isinstance(review_entries, list) or len(review_entries) != 2:
            raise SystemExit(f"{candidate_id}: expected exactly two independent review entries")
        reviewers = set()
        for entry in review_entries:
            reviewer = entry.get("reviewer")
            if not isinstance(reviewer, str) or not reviewer:
                raise SystemExit(f"{candidate_id}: review entry missing reviewer")
            if reviewer in reviewers:
                raise SystemExit(f"{candidate_id}: duplicate reviewer entry {reviewer}")
            reviewers.add(reviewer)
            for key in ["agent_id", "verdict", "challenged_categories", "findings", "resolution"]:
                if key not in entry:
                    raise SystemExit(f"{candidate_id}: review entry missing {key}")


def check_artifacts(corpus_path: Path, json_out: Path, markdown_out: Path, review_log: Path) -> None:
    report = json.loads(json_out.read_text(encoding="utf-8"))
    validate_report(report)
    if report.get("candidate_signature") != candidate_signature(CANDIDATES):
        raise SystemExit("candidate pricing JSON is stale relative to pricing.py candidates")
    expected_markdown = render_markdown(report) + "\n"
    actual_markdown = markdown_out.read_text(encoding="utf-8")
    if actual_markdown != expected_markdown:
        raise SystemExit("candidate pricing Markdown is stale relative to JSON/generator")
    reviews = json.loads(review_log.read_text(encoding="utf-8"))
    validate_loop_reviews(report, reviews)
    corpus = load_corpus(corpus_path)
    if report.get("corpus", {}).get("digest") != corpus_digest(corpus):
        raise SystemExit("candidate pricing JSON is stale relative to corpus digest")
    print("semantic-pack pricing artifact check passed")


def run_selftest() -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        corpus = {
            "repositories": [
                {
                    "id": "sample-java",
                    "split": "dev",
                    "primary_language": "Java",
                    "commit": "abc",
                },
                {
                    "id": "sample-js",
                    "split": "heldout",
                    "primary_language": "TypeScript",
                    "commit": "def",
                },
            ]
        }
        java_dir = root / "sample-java" / "src"
        js_dir = root / "sample-js" / "src"
        java_dir.mkdir(parents=True)
        js_dir.mkdir(parents=True)
        (java_dir / "Example.java").write_text(
            "import com.google.common.collect.ImmutableList;\n"
            "class Example { Object x = ImmutableList.of(\"a\", \"b\"); }\n",
            encoding="utf-8",
        )
        (js_dir / "obs.ts").write_text(
            "import { Observable } from '@trpc/server/observable';\n"
            "const value = source.pipe(map(x => x));\n",
            encoding="utf-8",
        )
        report = price_candidates(corpus, root)
        validate_report(report)
        guava = next(r for r in report["iterations"] if r["candidate_id"] == "java.guava.immutable_collection_factories")
        rxjs = next(r for r in report["iterations"] if r["candidate_id"] == "javascript.rxjs_observable_identity_adapters")
        assert guava["corpus_signal"]["repo_breadth"] == 1
        assert guava["verdict"] == "priced-ready"
        assert rxjs["corpus_signal"]["repo_breadth"] == 1
        assert rxjs["verdict"] == "priced-but-blocked"
    print("semantic-pack pricing self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--repos-root", type=Path, default=DEFAULT_REPOS_ROOT)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON_OUT)
    parser.add_argument("--markdown-out", type=Path, default=DEFAULT_MD_OUT)
    parser.add_argument("--review-log", type=Path, default=DEFAULT_REVIEW_LOG)
    parser.add_argument(
        "--nose",
        type=Path,
        default=None,
        help="optional nose binary used to sample current semantic query output",
    )
    parser.add_argument(
        "--query-sample-repos",
        type=int,
        default=0,
        help="number of signal-bearing repos per candidate to query with --nose",
    )
    parser.add_argument("--selftest", action="store_true")
    parser.add_argument("--check-artifacts", action="store_true")
    args = parser.parse_args()

    if args.selftest:
        run_selftest()
        return 0

    if args.check_artifacts:
        check_artifacts(args.corpus, args.json_out, args.markdown_out, args.review_log)
        return 0

    if args.query_sample_repos < 0:
        raise SystemExit("--query-sample-repos must be non-negative")
    if args.query_sample_repos and args.nose is None:
        raise SystemExit("--query-sample-repos requires --nose")

    corpus = load_corpus(args.corpus)
    report = price_candidates(
        corpus,
        args.repos_root,
        corpus_path=args.corpus,
        nose=args.nose,
        query_sample_repos=args.query_sample_repos,
    )
    validate_report(report)
    args.json_out.parent.mkdir(parents=True, exist_ok=True)
    args.json_out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown_out.write_text(render_markdown(report) + "\n", encoding="utf-8")
    print(f"wrote {args.json_out}")
    print(f"wrote {args.markdown_out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
